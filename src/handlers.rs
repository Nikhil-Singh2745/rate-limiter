use crate::rate_limiter::RateLimiter;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use tracing:: info;

#[derive(Deserialize)]
pub struct CheckRequest {
    limit: i64,
    #[serde(default)]
    burst: Option<i64>,
}

#[derive(Serialize)]
pub struct CheckResponse {
    allowed: bool,
    remaining:  i64,
    retry_after_ms: i64,
}

pub async fn check_rate_limit(
    req: HttpRequest,
    body: web::Json<CheckRequest>,
    limiter: web::Data<RateLimiter>,
) -> HttpResponse {
    let client_id = extract_client_id(&req);
    let burst = body.burst. unwrap_or(body.limit);

    info!(client_id = %client_id, limit = body.limit, burst = burst, "Rate limit check");

    match limiter.check(&client_id, body.limit, burst).await {
        Ok(result) => {
            let status = if result.allowed { 200 } else { 429 };
            HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap()).json(
                CheckResponse {
                    allowed: result.allowed,
                    remaining: result. remaining,
                    retry_after_ms: result.retry_after_ms,
                },
            )
        }
        Err(e) => {
            tracing::error!("Redis error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error"
            }))
        }
    }
}

pub async fn health(limiter: web::Data<RateLimiter>) -> HttpResponse {
    match limiter.ping().await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok"})),
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({"status": "unhealthy"})),
    }
}

fn extract_client_id(req: &HttpRequest) -> String {
    if let Some(api_key) = req.headers().get("X-API-Key") {
        if let Ok(key) = api_key. to_str() {
            return key. to_string();
        }
    }

    req.connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string()
}