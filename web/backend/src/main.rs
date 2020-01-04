use actix_web::{http, middleware, App, HttpServer};

use persistence::DbContext;

use crate::config::Config;
use crate::error::ApiError;
use actix_cors::Cors;

mod config;
mod error;
mod models;
mod services;

type ApiResult<T> = std::result::Result<T, ApiError>;

#[actix_rt::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    env_logger::init();

    let config = Config::init();
    let db_context = DbContext::create(&config.database_url, &config.redis_url).await?;

    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::new()
                    .allowed_methods(vec!["GET", "POST", "PUT"])
                    .allowed_headers(vec![
                        http::header::AUTHORIZATION,
                        http::header::ACCEPT,
                        http::header::CONTENT_TYPE,
                    ])
                    .finish(),
            )
            .wrap(middleware::Logger::default())
            .data(db_context.clone())
            .configure(services::web_config)
    })
    .bind("127.0.0.1:3001")?
    .run()
    .await?;
    Ok(())
}
