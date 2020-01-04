use actix_web::{get, web, HttpResponse};
use validator::Validate;

use persistence::commands::attributes::CommandAttributes;
use persistence::DbContext;

use crate::error::{ApiError, UserError};
use crate::models::requests::pagination::PaginationParams;
use crate::models::responses::command::{ApiCommand, ApiDetailedCommand};
use crate::models::responses::list::ListResponse;
use crate::ApiResult;

#[get("/commands")]
pub async fn index(
    pagination: web::Query<PaginationParams>,
    ctx: web::Data<DbContext>,
) -> ApiResult<HttpResponse> {
    pagination.validate().map_err(UserError::Validation)?;
    let (total, attributes) =
        CommandAttributes::list_with_aliases(&ctx.db_pool, pagination.as_offset())
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

    let response = ListResponse::new(
        attributes.into_iter().map(ApiCommand::from).collect(),
        total,
        pagination.page,
        pagination.per_page,
    );

    Ok(HttpResponse::Ok().json(response))
}

#[get("/commands/{id}")]
pub async fn get(command_id: web::Path<i32>, ctx: web::Data<DbContext>) -> ApiResult<HttpResponse> {
    let command = CommandAttributes::get_detailed(&ctx.db_pool, *command_id).await?;
    let response = ApiDetailedCommand::from(command);
    Ok(HttpResponse::Ok().json(response))
}
