mod handlers;
mod rate_limiter;

use actix_web::{web, App, HttpServer};
use rate_limiter::RateLimiter;
use std::env;
use tracing:: info;

#[tokio::main]
async fn main() -> std::io:: Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing:: Level::INFO.into()),
        )
        .init();

    let redis_url = env:: var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port:  u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    let rate_limiter = RateLimiter:: new(&redis_url)
        .await
        .expect("Failed to connect to Redis");

    info!("Starting server at http://{}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(rate_limiter.clone()))
            .route("/check", web::post().to(handlers::check_rate_limit))
            .route("/health", web::get().to(handlers::health))
    })
    .bind((host. as_str(), port))?
    .run()
    .await
}