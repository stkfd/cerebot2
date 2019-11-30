use std::collections::BTreeMap;
use std::iter::FromIterator;

use diesel::expression::sql_literal::sql;
use diesel::sql_types::{Array, Int4};
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel_derive_enum::DbEnum;
use fnv::FnvHashSet;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::error::Error;
use crate::schema::{permissions, user_permissions};
use crate::state::DbContext;
use diesel::sql_query;

#[derive(DbEnum, Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionState {
    Allow,
    Deny,
}

#[derive(Queryable, Debug)]
/// Represents a permission for any feature in the bot, contains a unique name, user-facing description
/// and default state
pub struct Permission {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub default_state: PermissionState,
}

#[derive(Debug)]
pub struct PermissionStore {
    permissions: BTreeMap<i32, Permission>,
    leaves: BTreeMap<i32, PermissionNode>,
}

#[derive(QueryableByName, Debug)]
pub struct PermissionNode {
    #[sql_type = "Int4"]
    permission_id: i32,
    #[sql_type = "Array<Int4>"]
    implied_by: Vec<i32>,
}

impl PermissionStore {
    /// Loads all permissions from the database and saves them in a sort of tree structure in memory
    /// which can be used to resolve the requirements of individual commands
    pub async fn load(ctx: &DbContext) -> Result<Self, Error> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &ctx.db_pool.get()?;
            Ok(PermissionStore {
                permissions: permissions::table
                    .load::<Permission>(pg)?
                    .into_iter()
                    .map(|p| (p.id, p))
                    .collect(),
                leaves: sql_query(
                        "select permission_id, array_agg(implied_by_id) as implied_by \
                         from implied_permissions \
                         group by permission_id;",
                    )
                    .load::<PermissionNode>(pg)?
                    .into_iter()
                    .map(|p| (p.permission_id, p))
                    .collect(),
            })
        }).await?
    }

    /// use the permission store to create a `PermissionRequirement` that can be used to check whether
    /// a user has the needed permissions to fulfill it. This resolves a set of permission IDs, taking
    /// into account which permissions are implied by other permissions
    pub fn get_requirement(&self, permission_ids: impl IntoIterator<Item = i32>) -> Result<PermissionRequirement, Error> {
        let mut requirements_vec: Vec<Vec<i32>> = vec![];
        for id in permission_ids.into_iter() {
            let mut v = vec![id];
            self.leaves.get(&id).as_ref().map(|node| v.extend(&node.implied_by));
            requirements_vec.push(v);
        }

        Ok(PermissionRequirement {
            required: requirements_vec
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct PermissionRequirement {
    required: Vec<Vec<i32>>,
}

impl PermissionRequirement {
    pub fn check(&self, available_permissions: &[i32]) -> bool {
        self.required.iter().all(|any_required| {
            any_required.iter().any(|id| available_permissions.contains(id))
        })
    }
}


#[derive(Insertable, AsChangeset)]
#[table_name = "permissions"]
pub struct NewPermission<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub default_state: PermissionState,
}

#[derive(Queryable, Insertable)]
pub struct UserPermission {
    pub permission_id: i32,
    pub user_id: i32,
    pub user_permission_state: PermissionState,
}

impl UserPermission {
    pub async fn get_by_user_id(ctx: &DbContext, user_id: i32) -> Result<Vec<i32>, Error> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            permissions::table
                .select(permissions::id)
                .filter(sql::<PermissionStateMapping>(
                    "coalesce(user_permission_state, default_state)",
                ).eq(PermissionState::Allow))
                .filter(user_permissions::user_id.eq(user_id))
                .left_outer_join(user_permissions::table)
                .load::<i32>(&*ctx.db_pool.get()?)
                .map_err(Into::into)
        })
        .await?
    }

    pub async fn get_named(
        ctx: &DbContext,
        user_id: i32,
        permission: &str,
    ) -> Result<PermissionState, Error> {
        let ctx = ctx.clone();
        let permission = permission.to_string();

        task::spawn_blocking(move || {
            permissions::table
                .select(sql::<PermissionStateMapping>(
                    "coalesce(user_permission_state, default_state)",
                ))
                .filter(permissions::name.eq(permission))
                .filter(user_permissions::user_id.eq(user_id))
                .left_outer_join(user_permissions::table)
                .first::<PermissionState>(&*ctx.db_pool.get()?)
                .map_err(Into::into)
        })
        .await?
    }

    /// Get the permission states
    pub async fn get_named_multi(
        ctx: &DbContext,
        user_id: i32,
        permissions: &[&str],
    ) -> Result<Vec<(String, PermissionState)>, Error> {
        let ctx = ctx.clone();

        let permissions: Vec<String> = permissions.iter().map(|&s| s.to_owned()).collect();
        task::spawn_blocking(move || {
            permissions::table
                .select(sql::<(Text, PermissionStateMapping)>(
                    "permission.name, coalesce(user_permission_state, default_state)",
                ))
                .filter(permissions::name.eq_any(permissions))
                .filter(user_permissions::user_id.eq(user_id))
                .left_outer_join(user_permissions::table)
                .load::<(String, PermissionState)>(&*ctx.db_pool.get()?)
                .map_err(Into::into)
        })
        .await?
    }
}

static DEFAULT_PERMISSIONS: &'static [NewPermission<'static>] = &[NewPermission {
    name: "root",
    description: Some("Super admin override"),
    default_state: PermissionState::Deny,
}];

pub async fn create_permissions(ctx: &DbContext) -> Result<(), Error> {
    let pg_pool = ctx.db_pool.clone();
    task::spawn_blocking(move || {
        let pg = &pg_pool.get()?;
        let existing = FnvHashSet::from_iter(
            permissions::table
                .select(permissions::name)
                .filter(permissions::name.eq_any(DEFAULT_PERMISSIONS.iter().map(|p| p.name)))
                .get_results::<String>(pg)?
                .into_iter(),
        );

        DEFAULT_PERMISSIONS
            .iter()
            .filter(|perm| !existing.contains(perm.name))
            .map(|permission| {
                diesel::insert_into(permissions::table)
                    .values(permission)
                    .execute(pg)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    })
    .await?
}
