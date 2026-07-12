use axum::{
    extract::{Request, State},
    http::{self, header::ORIGIN, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::{collections::HashSet, sync::Arc};
use tracing::warn;

#[derive(Clone, Debug)]
pub struct OriginPolicy {
    allowed: Arc<HashSet<String>>,
}

impl Default for OriginPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl OriginPolicy {
    pub fn new() -> Self {
        let env = crate::config::Environment::from_env();
        let mut allowed = HashSet::new();

        if let Ok(origins) = std::env::var("ALLOWED_ORIGINS") {
            for origin in origins.split(',') {
                let normalized = normalize_origin(origin.trim());
                if !normalized.is_empty() {
                    allowed.insert(normalized);
                }
            }
        }

        if env == crate::config::Environment::Development && allowed.is_empty() {
            allowed.insert("http://localhost:8080".to_string());
            allowed.insert("http://127.0.0.1:8080".to_string());
        }

        Self {
            allowed: Arc::new(allowed),
        }
    }

    pub fn is_allowed(&self, origin: Option<&str>) -> bool {
        match origin {
            Some(origin) => {
                let normalized = normalize_origin(origin);
                self.allowed.contains(&normalized)
            }
            None => true,
        }
    }

    pub fn allowed_origins(&self) -> Vec<String> {
        self.allowed.iter().cloned().collect()
    }
}

fn normalize_origin(origin: &str) -> String {
    let normalized = origin.trim().trim_end_matches('/').to_ascii_lowercase();
    if normalized.starts_with("https://") || normalized.starts_with("http://") {
        normalized
    } else {
        String::new()
    }
}

pub async fn origin_middleware(
    State(policy): State<OriginPolicy>,
    request: Request,
    next: Next,
) -> Response {
    let browser_sensitive = request.uri().path() == "/tiempo-real"
        || !matches!(
            *request.method(),
            http::Method::GET | http::Method::HEAD | http::Method::OPTIONS
        );
    if !browser_sensitive {
        return next.run(request).await;
    }
    let origin = request.headers().get(ORIGIN).and_then(|v| v.to_str().ok());

    if !policy.is_allowed(origin) {
        warn!(origin = ?origin, allowed = ?policy.allowed_origins(), "origin rejected");
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "ok": false,
                "error": {
                    "code": "origin_not_allowed",
                    "message": "origen no permitido"
                }
            })),
        )
            .into_response();
    }

    next.run(request).await
}

pub fn cors_layer(policy: &OriginPolicy) -> tower_http::cors::CorsLayer {
    use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, ExposeHeaders};

    let origins: Vec<_> = policy.allowed_origins();

    tower_http::cors::CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect::<Vec<_>>(),
        ))
        .allow_methods(AllowMethods::list([
            http::Method::GET,
            http::Method::POST,
            http::Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::list([
            http::header::CONTENT_TYPE,
            http::header::AUTHORIZATION,
            http::header::HeaderName::from_static("x-admin-token"),
        ]))
        .expose_headers(ExposeHeaders::list([http::header::CONTENT_DISPOSITION]))
        .allow_credentials(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, middleware, routing::post, Router};
    use tower::ServiceExt;

    #[test]
    fn test_normalize_origin() {
        assert_eq!(
            normalize_origin("https://example.com"),
            "https://example.com"
        );
        assert_eq!(
            normalize_origin("HTTPS://EXAMPLE.COM"),
            "https://example.com"
        );
        assert_eq!(
            normalize_origin("http://localhost:8080"),
            "http://localhost:8080"
        );
        assert_eq!(
            normalize_origin("http://127.0.0.1:3000"),
            "http://127.0.0.1:3000"
        );
        assert_eq!(normalize_origin("example.com"), "");
    }

    #[test]
    fn test_origin_policy_dev_defaults() {
        let policy = OriginPolicy {
            allowed: Arc::new(HashSet::from([
                "http://localhost:8080".to_string(),
                "http://127.0.0.1:8080".to_string(),
            ])),
        };
        assert!(policy.is_allowed(Some("http://localhost:8080")));
        assert!(policy.is_allowed(Some("http://127.0.0.1:8080")));
        assert!(!policy.is_allowed(Some("https://evil.com")));
    }

    #[test]
    fn test_origin_policy_exact_match() {
        let policy = OriginPolicy {
            allowed: Arc::new(HashSet::from(["https://app.example.com".to_string()])),
        };
        assert!(policy.is_allowed(Some("https://app.example.com")));
        assert!(!policy.is_allowed(Some("https://sub.app.example.com")));
        assert!(!policy.is_allowed(Some("https://evil.com")));
    }

    #[tokio::test]
    async fn browser_mutation_accepts_exact_origin_and_rejects_another() {
        let policy = OriginPolicy {
            allowed: Arc::new(HashSet::from(["https://app.example.com".to_string()])),
        };
        let app = Router::new()
            .route("/mutate", post(|| async { StatusCode::NO_CONTENT }))
            .layer(middleware::from_fn_with_state(policy, origin_middleware));

        let allowed = app
            .clone()
            .oneshot(
                Request::post("/mutate")
                    .header(ORIGIN, "https://app.example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::NO_CONTENT);

        let rejected = app
            .oneshot(
                Request::post("/mutate")
                    .header(ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    }
}
