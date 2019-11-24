use std::borrow::Cow;
use std::iter::FromIterator;

use diesel::prelude::*;
use diesel::sql_types::{Integer, Text};
use diesel_derive_enum::DbEnum;
use fnv::FnvHashSet;
use r2d2_redis::redis;
use serde::{Deserialize, Serialize};
use tokio_executor::blocking;
use tokio_executor::blocking::Blocking;

use crate::cerebot::DbContext;
use crate::error::Error;
use crate::schema::{permissions, user_permissions};
use diesel::deserialize::FromSql;
use diesel::deserialize::QueryableByName;
use diesel::expression::sql_literal::sql;
use diesel::pg::Pg;
use diesel::query_builder::AsQuery;
use diesel::row::NamedRow;

#[derive(DbEnum, Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionState {
    Allow,
    Deny,
}

impl QueryableByName<Pg> for PermissionState {
    fn build<R: NamedRow<Pg>>(row: &R) -> diesel::deserialize::Result<Self> {
        row.get("user_permission_state")
    }
}

#[derive(Queryable)]
pub struct Permission {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub default_state: PermissionState,
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

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct PermissionAndState {
    pub name: String,
    pub permission_state: PermissionState,
}

impl QueryableByName<Pg> for PermissionAndState {
    fn build<R: NamedRow<Pg>>(row: &R) -> diesel::deserialize::Result<Self> {
        Ok(PermissionAndState {
            name: row.get::<Text, _>("name")?,
            permission_state: row.get::<PermissionStateMapping, _>("permission_state")?,
        })
    }
}

pub fn get_permission_state(
    ctx: &DbContext,
    user_id: i32,
    permission: &str,
) -> Blocking<Result<PermissionState, Error>> {
    let ctx = ctx.clone();
    let permission = permission.to_string();

    blocking::run(move || {
        permissions::table
            .select(sql::<PermissionStateMapping>(
                "coalesce(user_permission_state, default_state)",
            ))
            .filter(permissions::name.eq(permission))
            .left_outer_join(user_permissions::table)
            .first::<PermissionState>(&*ctx.db_pool.get()?)
            .map_err(Into::into)
    })
}

pub fn get_permission_states(
    ctx: &DbContext,
    user_id: i32,
    permissions: &[&str],
) -> Blocking<Result<Vec<(String, PermissionState)>, Error>> {
    let ctx = ctx.clone();

    let permissions: Vec<String> = permissions.iter().map(|&s| s.to_owned()).collect();
    blocking::run(move || {
        permissions::table
            .select(sql::<(Text, PermissionStateMapping)>(
                "permission.name, coalesce(user_permission_state, default_state)",
            ))
            .filter(permissions::name.eq_any(permissions))
            .left_outer_join(user_permissions::table)
            .load::<(String, PermissionState)>(&*ctx.db_pool.get()?)
            .map_err(Into::into)
    })
}

static DEFAULT_PERMISSIONS: &'static [NewPermission<'static>] = &[NewPermission {
    name: "root",
    description: Some("Super admin override"),
    default_state: PermissionState::Deny,
}];

pub fn create_permissions(ctx: &DbContext) -> Blocking<Result<(), Error>> {
    let pg_pool = ctx.db_pool.clone();
    blocking::run(move || {
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
}
