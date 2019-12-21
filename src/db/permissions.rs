use std::iter::FromIterator;

use diesel::expression::sql_literal::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel_derive_enum::DbEnum;
use fnv::FnvHashSet;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::schema::{implied_permissions, permissions, user_permissions};
use crate::state::{BotContext, DbContext};
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
            .map(|&a| a.to_string())
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
