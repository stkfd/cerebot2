use std::iter::FromIterator;

use diesel::expression::sql_literal::sql;
use diesel::sql_types::Text;
use diesel_derive_enum::DbEnum;
use fnv::FnvHashSet;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tokio_diesel::{AsyncConnection, AsyncRunQueryDsl};

use crate::schema::{command_permissions, implied_permissions, permissions, user_permissions};
use crate::Result;
use crate::{DbContext, DbPool};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use std::borrow::Cow;

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

#[derive(Insertable, Clone, Debug)]
#[table_name = "permissions"]
pub struct NewPermissionAttributes<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub default_state: PermissionState,
}

#[derive(Clone, Debug)]
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

impl Permission {
    pub async fn get_by_command_id(pool: &DbPool, command_id: i32) -> Result<Vec<i32>> {
        permissions::table
            .select(permissions::id)
            .filter(command_permissions::command_id.eq(command_id))
            .left_outer_join(command_permissions::table)
            .load_async::<i32>(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn all(pool: &DbPool) -> Result<Vec<Permission>> {
        permissions::table
            .load_async::<Permission>(pool)
            .await
            .map_err(Into::into)
    }
}

impl UserPermission {
    pub async fn get_by_user_id(ctx: &DbContext, user_id: i32) -> Result<Vec<i32>> {
        permissions::table
            .select(permissions::id)
            .filter(
                sql::<PermissionStateMapping>("coalesce(user_permission_state, default_state)")
                    .eq(PermissionState::Allow),
            )
            .filter(user_permissions::user_id.eq(user_id))
            .left_outer_join(user_permissions::table)
            .load_async::<i32>(&ctx.db_pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_named(
        ctx: &DbContext,
        user_id: i32,
        permission: &str,
    ) -> Result<PermissionState> {
        let permission = permission.to_string();
        permissions::table
            .select(sql::<PermissionStateMapping>(
                "coalesce(user_permission_state, default_state)",
            ))
            .filter(permissions::name.eq(permission))
            .filter(user_permissions::user_id.eq(user_id))
            .left_outer_join(user_permissions::table)
            .first_async::<PermissionState>(&ctx.db_pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_named_multi(
        ctx: &DbContext,
        user_id: i32,
        permissions: &[&str],
    ) -> Result<Vec<(String, PermissionState)>> {
        let permissions = permissions
            .iter()
            .map(|&a| a.to_string())
            .collect::<Vec<_>>();

        permissions::table
            .select(sql::<(Text, PermissionStateMapping)>(
                "permission.name, coalesce(user_permission_state, default_state)",
            ))
            .filter(permissions::name.eq_any(permissions))
            .filter(user_permissions::user_id.eq(user_id))
            .left_outer_join(user_permissions::table)
            .load_async::<(String, PermissionState)>(&ctx.db_pool)
            .await
            .map_err(Into::into)
    }
}

/// A set of default permissions that should always be available to all commands
static DEFAULT_PERMISSIONS: OnceCell<Vec<AddPermission<'static>>> = OnceCell::new();

pub async fn create_permissions(
    pg: &DbPool,
    new_permissions: Cow<'static, Vec<AddPermission<'_>>>,
) -> Result<usize> {
    pg.transaction(move |pg| {
        let mut added = 0;
        let existing = FnvHashSet::from_iter(
            permissions::table
                .select(permissions::name)
                .get_results::<String>(pg)?
                .into_iter(),
        );

        for permission in new_permissions.as_ref() {
            if existing.contains(&permission.attributes.name as &str) {
                continue;
            }
            info!("Adding new permission {}", &permission.attributes.name);
            let inserted = diesel::insert_into(permissions::table)
                .values(&permission.attributes)
                .get_result::<Permission>(pg)?;
            added += 1;

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

        Ok(added)
    })
    .await
    .map_err(Into::into)
}

/// Create the global default permissions
pub async fn create_default_permissions(ctx: &DbContext) -> Result<usize> {
    let permissions: &'static _ = DEFAULT_PERMISSIONS.get_or_init(|| {
        vec![AddPermission {
            attributes: NewPermissionAttributes {
                name: "root",
                description: Some("Super admin override"),
                default_state: PermissionState::Deny,
            },
            implied_by: vec![],
        }]
    });
    create_permissions(&ctx.db_pool, Cow::Borrowed(permissions)).await
}
