use crate::AppState;
use actix_governor::{KeyExtractor, SimpleKeyExtractionError};
use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::Error as ActixError,
    http::header::AUTHORIZATION,
    middleware::Next,
    web, HttpMessage, HttpResponse,
};
use serde::Deserialize;

#[derive(Debug, Copy, Clone, Hash, PartialOrd, PartialEq, Eq, Ord)]
pub struct UserId(pub u64);

#[derive(Debug, Deserialize)]
struct UserResponse {
    id: u64,
}

pub async fn authentication(
    app_state: web::Data<AppState>,
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, ActixError> {
    let token = match req
        .headers()
        .get(AUTHORIZATION)
        .map(|token| token.to_str().unwrap())
    {
        Some(token) => token,
        None => {
            return Ok(req
                .into_response(HttpResponse::Unauthorized().finish())
                .map_into_right_body())
        }
    };

    let token = if token.to_lowercase().starts_with("bearer ") {
        token.to_string()
    } else {
        format!("Bearer {token}")
    };

    let response = match app_state
        .reqwest_client
        .get("https://api.github.com/user")
        .header(reqwest::header::AUTHORIZATION, token)
        .send()
        .await
        .and_then(|res| res.error_for_status())
    {
        Ok(response) => response,
        Err(e) if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) => {
            return Ok(req
                .into_response(HttpResponse::Unauthorized().finish())
                .map_into_right_body())
        }
        Err(e) => {
            log::error!("failed to get user: {e}");
            return Ok(req
                .into_response(HttpResponse::InternalServerError().finish())
                .map_into_right_body());
        }
    };

    let user_id = match response.json::<UserResponse>().await {
        Ok(user) => user.id,
        Err(_) => {
            return Ok(req
                .into_response(HttpResponse::Unauthorized().finish())
                .map_into_right_body())
        }
    };

    req.extensions_mut().insert(UserId(user_id));

    let res = next.call(req).await?;
    Ok(res.map_into_left_body())
}

#[derive(Debug, Clone)]
pub struct UserIdExtractor;

impl KeyExtractor for UserIdExtractor {
    type Key = UserId;
    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        match req.extensions().get::<UserId>() {
            Some(user_id) => Ok(*user_id),
            None => Err(SimpleKeyExtractionError::new("UserId not found")),
        }
    }
}
