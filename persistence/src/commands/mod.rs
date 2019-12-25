pub mod alias;
pub mod attributes;
pub mod channel_config;
pub mod permission;
pub mod templates;

pub mod util {
    use diesel::dsl::*;
    use diesel::{ExpressionMethods, QueryDsl};
    use tokio_diesel::AsyncRunQueryDsl;

    use crate::commands::attributes::{CommandAttributes, InsertCommandAttributes};
    use crate::schema::{command_aliases, command_attributes, command_permissions};
    use crate::DbContext;
    use crate::Result;

    pub async fn initialize_command(
        ctx: &DbContext,
        data: InsertCommandAttributes<'static>,
        required_permission_ids: Vec<i32>,
        aliases: Vec<impl AsRef<str> + Send + Sync + 'static>,
    ) -> Result<()> {
        let handler_name = data.handler_name.clone();
        let command_exists: bool = select(exists(
            command_attributes::table.filter(command_attributes::handler_name.eq(handler_name)),
        ))
        .get_result_async(&ctx.db_pool)
        .await?;
        if !command_exists {
            info!(
                "Setting up new command \"{}\", handler name: {}",
                aliases.get(0).map(|a| a.as_ref()).unwrap_or_else(|| ""),
                &data.handler_name
            );

            // insert attributes and aliases
            let attributes = CommandAttributes::insert(&ctx.db_pool, data).await?;

            diesel::insert_into(command_aliases::table)
                .values(
                    aliases
                        .iter()
                        .map(|alias| {
                            (
                                command_aliases::command_id.eq(attributes.id),
                                command_aliases::name.eq(alias.as_ref().to_string()),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .execute_async(&ctx.db_pool)
                .await?;

            // insert default permissions
            let required_permission_values: Vec<_> = required_permission_ids
                .into_iter()
                .map(|permission_id| {
                    (
                        command_permissions::permission_id.eq(permission_id),
                        command_permissions::command_id.eq(attributes.id),
                    )
                })
                .collect();
            diesel::insert_into(command_permissions::table)
                .values(required_permission_values)
                .execute_async(&ctx.db_pool)
                .await?;
        }
        Ok(())
    }
}
