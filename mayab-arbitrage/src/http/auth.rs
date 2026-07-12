use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

#[derive(Clone, Debug)]
pub struct AdminAuthState {
    pub token: Option<String>,
    pub env: crate::config::Environment,
}

impl AdminAuthState {
    pub fn new(token: Option<String>) -> Result<Self, AuthConfigError> {
        let env = crate::config::Environment::from_env();
        let token = token.filter(|t| !t.trim().is_empty());

        if env.requires_admin_token() {
            let token = token.ok_or(AuthConfigError::MissingAdminToken)?;
            if token.len() < env.min_token_length() {
                return Err(AuthConfigError::TokenTooShort {
                    min: env.min_token_length(),
                    got: token.len(),
                });
            }
            Ok(Self {
                token: Some(token),
                env,
            })
        } else {
            Ok(Self { token, env })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthConfigError {
    #[error("ADMIN_TOKEN requerido en produccion")]
    MissingAdminToken,
    #[error("ADMIN_TOKEN muy corto: minimo {min} caracteres, recibido {got}")]
    TokenTooShort { min: usize, got: usize },
}

impl IntoResponse for AuthConfigError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::MissingAdminToken => (StatusCode::INTERNAL_SERVER_ERROR, "config_error"),
            Self::TokenTooShort { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "config_error"),
        };
        (
            status,
            axum::Json(json!({
                "ok": false,
                "error": {
                    "code": code,
                    "message": self.to_string()
                }
            })),
        )
            .into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("token de admin requerido")]
    MissingToken,
    #[error("token invalido")]
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::MissingToken => (StatusCode::UNAUTHORIZED, "token_admin_requerido"),
            Self::InvalidToken => (StatusCode::UNAUTHORIZED, "token_invalido"),
        };
        (
            status,
            axum::Json(json!({
                "ok": false,
                "error": {
                    "code": code,
                    "message": self.to_string()
                }
            })),
        )
            .into_response()
    }
}

pub async fn auth_middleware(
    Extension(state): Extension<Arc<AdminAuthState>>,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    if !state.has_token() {
        return Ok(next.run(request).await);
    }

    let headers = request.headers();
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let header_token = headers.get("x-admin-token").and_then(|v| v.to_str().ok());

    let expected = state.token.as_ref().unwrap();
    let provided = bearer.or(header_token);

    if provided == Some(expected.as_str()) {
        Ok(next.run(request).await)
    } else {
        warn!(path = %request.uri().path(), "auth failed: invalid token");
        Err(AuthError::InvalidToken)
    }
}

impl AdminAuthState {
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub fn require_auth(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        if !self.has_token() {
            return Ok(());
        }

        let bearer = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        let header_token = headers.get("x-admin-token").and_then(|v| v.to_str().ok());

        let expected = self.token.as_ref().unwrap();
        let provided = bearer.or(header_token);

        if provided == Some(expected.as_str()) {
            Ok(())
        } else {
            Err(AuthError::InvalidToken)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_admin_auth_state_production_requires_token() {
        std::env::set_var("MAYAB_ENV", "production");
        let result = AdminAuthState::new(None);
        assert!(matches!(result, Err(AuthConfigError::MissingAdminToken)));

        let result = AdminAuthState::new(Some("short".to_string()));
        assert!(matches!(result, Err(AuthConfigError::TokenTooShort { .. })));

        let result = AdminAuthState::new(Some("a".repeat(32)));
        assert!(result.is_ok());
    }

    #[test]
    fn test_admin_auth_state_dev_allows_no_token() {
        std::env::set_var("MAYAB_ENV", "development");
        let result = AdminAuthState::new(None);
        assert!(result.is_ok());
        assert!(!result.unwrap().has_token());
    }

    #[test]
    fn test_require_auth() {
        let state = Arc::new(
            AdminAuthState::new(Some("test-token-32-chars-long-enough".to_string())).unwrap(),
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer test-token-32-chars-long-enough"),
        );
        assert!(state.require_auth(&headers).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-admin-token",
            HeaderValue::from_static("test-token-32-chars-long-enough"),
        );
        assert!(state.require_auth(&headers).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer wrong"));
        assert!(state.require_auth(&headers).is_err());

        let state_no_token = Arc::new(AdminAuthState::new(None).unwrap());
        assert!(state_no_token.require_auth(&HeaderMap::new()).is_ok());
    }
}
