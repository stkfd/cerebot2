use std::collections::BTreeMap;
use std::iter::FromIterator;

use diesel::expression::sql_literal::sql;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Text;
use diesel::sql_types::{Array, Int4};
use diesel_derive_enum::DbEnum;
use fnv::FnvHashSet;
use serde::{Deserialize, Serialize};
use tokio::task;

use once_cell::sync::Lazy;

use crate::schema::{implied_permissions, permissions, user_permissions};
use crate::state::{BotContext, BotStateError, DbContext};
use crate::Result;

#[derive(DbEnum, Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionState {
    Allow,
    Deny,
}

/// Represents a permission for any feature in the bot, contains a unique name, user-facing description
/// and default state
#[derive(Queryable, Debug)]
pub struct Permission {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub default_state: PermissionState,
}

/// Permission information loaded from the database. Provides methods to resolve permission
/// requirements for commands
#[derive(Debug)]
pub struct PermissionStore {
    permissions: BTreeMap<String, Permission>,
    leaves: BTreeMap<i32, PermissionNode>,
}

/// Contains a permission ID and all other permissions that imply this permission is present. Internal.
#[derive(QueryableByName, Debug)]
struct PermissionNode {
    #[sql_type = "Int4"]
    permission_id: i32,
    #[sql_type = "Array<Int4>"]
    implied_by: Vec<i32>,
}

impl PermissionStore {
    /// Loads all permissions from the database and saves them in a sort of tree structure in memory
    /// which can be used to resolve the requirements of individual commands
    pub async fn load(ctx: &DbContext) -> Result<Self> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &ctx.db_pool.get()?;
            Ok(PermissionStore {
                permissions: permissions::table
                    .load::<Permission>(pg)?
                    .into_iter()
                    .map(|p| (p.name.clone(), p))
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
        })
        .await?
    }

    /// use the permission store to create a `PermissionRequirement` that can be used to check whether
    /// a user has the needed permissions to fulfill it. This resolves a set of permission IDs, taking
    /// into account which permissions are implied by other permissions
    pub fn get_requirement(
        &self,
        permission_ids: impl IntoIterator<Item = i32>,
    ) -> Result<PermissionRequirement> {
        let mut requirements_vec: Vec<Vec<i32>> = vec![];
        for id in permission_ids.into_iter() {
            let mut v = vec![id];
            if let Some(node) = self.leaves.get(&id) {
                v.extend(&node.implied_by)
            }
            requirements_vec.push(v);
        }

        Ok(PermissionRequirement {
            required: requirements_vec,
        })
    }

    pub fn get_permissions<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<Vec<&Permission>> {
        names
            .into_iter()
            .map(|name| {
                self.permissions
                    .get(name)
                    .ok_or_else(|| BotStateError::PermissionNotFound(name.to_string()).into())
            })
            .collect::<Result<Vec<_>>>()
    }

    pub fn get_permission(&self, name: &str) -> Result<&Permission> {
        self.permissions
            .get(name)
            .ok_or_else(|| BotStateError::PermissionNotFound(name.to_string()).into())
    }
}

/// Fully flattened permission requirement with a method to check whether a set of permissions is
/// sufficient to satisfy it
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PermissionRequirement {
    required: Vec<Vec<i32>>,
}

impl PermissionRequirement {
    /// Check whether the given set of permissions (by IDs) is sufficient to satisfy this permission
    /// requirement
    pub fn check(&self, available_permissions: &[i32]) -> bool {
        let result = self.required.iter().all(|any_required| {
            any_required
                .iter()
                .any(|id| available_permissions.contains(id))
        });
        if !result {
            debug!(
                "Permission check failed! Required: {:?} Actual: {:?}",
                self.required, available_permissions
            );
        }
        result
    }
}

#[derive(Insertable)]
#[table_name = "permissions"]
pub struct NewPermissionAttributes<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub default_state: PermissionState,
}

pub struct AddPermission<'a> {
    pub attributes: NewPermissionAttributes<'a>,
    pub implied_by: Vec<&'a str>,
}

#[derive(Queryable, Insertable)]
pub struct UserPermission {
    pub permission_id: i32,
    pub user_id: i32,
    pub user_permission_state: PermissionState,
}

impl UserPermission {
    pub async fn get_by_user_id(ctx: &DbContext, user_id: i32) -> Result<Vec<i32>> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            permissions::table
                .select(permissions::id)
                .filter(
                    sql::<PermissionStateMapping>("coalesce(user_permission_state, default_state)")
                        .eq(PermissionState::Allow),
                )
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
    ) -> Result<PermissionState> {
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

    pub async fn get_named_multi(
        ctx: &DbContext,
        user_id: i32,
        permissions: &[&str],
    ) -> Result<Vec<(String, PermissionState)>> {
        let ctx = ctx.clone();
        let permissions = permissions
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>();

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

/// A set of default permissions that should always be available to all commands
static DEFAULT_PERMISSIONS: Lazy<Vec<AddPermission<'static>>> = Lazy::new(|| {
    vec![AddPermission {
        attributes: NewPermissionAttributes {
            name: "root",
            description: Some("Super admin override"),
            default_state: PermissionState::Deny,
        },
        implied_by: vec![],
    }]
});

fn create_permissions_blocking(
    ctx: &BotContext,
    new_permissions: &[AddPermission<'_>],
) -> Result<()> {
    let pg = &*ctx.db_context.db_pool.get()?;
    pg.transaction(|| {
        let existing = FnvHashSet::from_iter(
            permissions::table
                .select(permissions::name)
                .get_results::<String>(pg)?
                .into_iter(),
        );

        for permission in new_permissions {
            if existing.contains(&permission.attributes.name as &str) {
                continue;
            }
            info!("Adding new permission {}", &permission.attributes.name);
            let inserted = diesel::insert_into(permissions::table)
                .values(&permission.attributes)
                .get_result::<Permission>(pg)?;

            for implied_by in &permission.implied_by {
                let implied_by_permission = permissions::table
                    .filter(permissions::name.eq(implied_by))
                    .first::<Permission>(pg)?;
                diesel::insert_into(implied_permissions::table)
                    .values((
                        implied_permissions::implied_by_id.eq(implied_by_permission.id),
                        implied_permissions::permission_id.eq(inserted.id),
                    ))
                    .execute(pg)?;
            }
        }

        Ok(())
    })
}

pub async fn create_permissions(
    ctx: &BotContext,
    permissions: Vec<AddPermission<'static>>,
) -> Result<()> {
    let ctx = ctx.clone();
    task::spawn_blocking(move || create_permissions_blocking(&ctx, &permissions)).await?
}

/// Create the global default permissions
pub async fn create_default_permissions(ctx: &BotContext) -> Result<()> {
    let ctx = ctx.clone();
    task::spawn_blocking(move || create_permissions_blocking(&ctx, &*DEFAULT_PERMISSIONS)).await?
}
