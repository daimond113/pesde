use crate::auth::{get_token_from_req, AuthImpl, UserId};
use actix_web::{dev::ServiceRequest, Error as ActixError};
use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug)]
pub struct GitHubAuth {
    pub reqwest_client: reqwest::Client,
}

impl AuthImpl for GitHubAuth {
    async fn for_write_request(&self, req: &ServiceRequest) -> Result<Option<UserId>, ActixError> {
        let token = match get_token_from_req(req, true) {
            Some(token) => token,
            None => return Ok(None),
        };

        let response = match self
            .reqwest_client
            .get("https://api.github.com/user")
            .header(reqwest::header::AUTHORIZATION, token)
            .send()
            .await
            .and_then(|res| res.error_for_status())
        {
            Ok(response) => response,
            Err(e) => {
                log::error!("failed to get user: {e}");
                return Ok(None);
            }
        };

        let user_id = match response.json::<UserResponse>().await {
            Ok(user) => user.id,
            Err(e) => {
                log::error!("failed to get user: {e}");
                return Ok(None);
            }
        };

        Ok(Some(UserId(user_id)))
    }
}

impl Display for GitHubAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GitHub")
    }
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    id: u64,
}
