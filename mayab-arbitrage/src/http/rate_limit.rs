use axum::{
    extract::{Request, State},
    http::{header::HeaderName, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub requests_per_window: u32,
    pub window: Duration,
    pub burst: Option<u32>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_window: 100,
            window: Duration::from_secs(60),
            burst: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RateLimitPolicy {
    pub default: RateLimitConfig,
    pub auth: RateLimitConfig,
    pub mutation: RateLimitConfig,
    pub websocket: RateLimitConfig,
    pub max_body_bytes: usize,
    pub read_timeout_secs: u64,
    pub admin_handler_timeout_secs: u64,
    pub max_concurrent_expensive_ops: usize,
}

impl RateLimitPolicy {
    pub fn from_env() -> Self {
        Self {
            default: RateLimitConfig {
                requests_per_window: env_u32("RATE_LIMIT_DEFAULT_RPM", 120),
                window: Duration::from_secs(env_u64("RATE_LIMIT_DEFAULT_WINDOW_SEC", 60)),
                burst: env_optional_u32("RATE_LIMIT_DEFAULT_BURST"),
            },
            auth: RateLimitConfig {
                requests_per_window: env_u32("RATE_LIMIT_AUTH_RPM", 10),
                window: Duration::from_secs(env_u64("RATE_LIMIT_AUTH_WINDOW_SEC", 60)),
                burst: env_optional_u32("RATE_LIMIT_AUTH_BURST"),
            },
            mutation: RateLimitConfig {
                requests_per_window: env_u32("RATE_LIMIT_MUTATION_RPM", 30),
                window: Duration::from_secs(env_u64("RATE_LIMIT_MUTATION_WINDOW_SEC", 60)),
                burst: env_optional_u32("RATE_LIMIT_MUTATION_BURST"),
            },
            websocket: RateLimitConfig {
                requests_per_window: env_u32("RATE_LIMIT_WS_RPM", 5),
                window: Duration::from_secs(env_u64("RATE_LIMIT_WS_WINDOW_SEC", 60)),
                burst: env_optional_u32("RATE_LIMIT_WS_BURST"),
            },
            max_body_bytes: env_usize("RATE_LIMIT_MAX_BODY_BYTES", 1_048_576),
            read_timeout_secs: env_u64("RATE_LIMIT_READ_TIMEOUT_SEC", 30),
            admin_handler_timeout_secs: env_u64("RATE_LIMIT_ADMIN_TIMEOUT_SEC", 60),
            max_concurrent_expensive_ops: env_usize("RATE_LIMIT_MAX_CONCURRENT_EXPENSIVE", 10),
        }
    }
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_optional_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok().and_then(|v| v.parse().ok())
}

#[derive(Clone)]
struct Bucket {
    requests: u32,
    window_start: Instant,
    burst_used: u32,
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, Bucket>>>,
    policy: RateLimitPolicy,
}

impl RateLimiter {
    pub fn new(policy: RateLimitPolicy) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            policy,
        }
    }

    fn key_for(&self, ip: IpAddr, route_class: RouteClass) -> String {
        format!("{}:{}", ip, route_class.as_str())
    }

    pub async fn check(&self, ip: IpAddr, route_class: RouteClass) -> Result<(), RateLimitError> {
        let mut buckets = self.buckets.write().await;
        let key = self.key_for(ip, route_class);
        let config = self.config_for(route_class);
        let now = Instant::now();

        let bucket = buckets.entry(key).or_insert(Bucket {
            requests: 0,
            window_start: now,
            burst_used: 0,
        });

        if now.duration_since(bucket.window_start) >= config.window {
            bucket.requests = 0;
            bucket.window_start = now;
            bucket.burst_used = 0;
        }

        let limit = config.burst.unwrap_or(config.requests_per_window);
        if bucket.requests >= limit {
            warn!(
                ip = %ip,
                route_class = %route_class.as_str(),
                limit = limit,
                "rate limit exceeded"
            );
            return Err(RateLimitError::TooManyRequests);
        }

        bucket.requests += 1;
        if let Some(burst) = config.burst {
            bucket.burst_used += 1;
            if bucket.burst_used > burst {
                warn!(
                    ip = %ip,
                    route_class = %route_class.as_str(),
                    burst = burst,
                    "burst limit exceeded"
                );
                return Err(RateLimitError::TooManyRequests);
            }
        }

        Ok(())
    }

    fn config_for(&self, route_class: RouteClass) -> &RateLimitConfig {
        match route_class {
            RouteClass::Default => &self.policy.default,
            RouteClass::Auth => &self.policy.auth,
            RouteClass::Mutation => &self.policy.mutation,
            RouteClass::WebSocket => &self.policy.websocket,
        }
    }

    pub fn max_body_bytes(&self) -> usize {
        self.policy.max_body_bytes
    }

    pub fn read_timeout(&self) -> Duration {
        Duration::from_secs(self.policy.read_timeout_secs)
    }

    pub fn admin_handler_timeout(&self) -> Duration {
        Duration::from_secs(self.policy.admin_handler_timeout_secs)
    }

    pub fn max_concurrent_expensive_ops(&self) -> usize {
        self.policy.max_concurrent_expensive_ops
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RouteClass {
    Default,
    Auth,
    Mutation,
    WebSocket,
}

impl RouteClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Auth => "auth",
            Self::Mutation => "mutation",
            Self::WebSocket => "websocket",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("too many requests")]
    TooManyRequests,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            Self::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": "rate_limited",
                        "message": "demasiadas peticiones; intente mas tarde"
                    }
                })),
            )
                .into_response(),
        }
    }
}

pub async fn rate_limit_middleware(
    Extension(limiter): Extension<Arc<RateLimiter>>,
    request: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    let ip = extract_client_ip(&request);
    let route_class = classify_route(&request);

    limiter.check(ip, route_class).await?;

    Ok(next.run(request).await)
}

fn extract_client_ip(request: &Request) -> IpAddr {
    let headers = request.headers();

    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(first) = s.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            if let Ok(ip) = s.parse::<IpAddr>() {
                return ip;
            }
        }
    }

    request
        .extensions()
        .get::<axum::extract::ConnectInfo<IpAddr>>()
        .map(|ci| ci.0)
        .unwrap_or_else(|| "127.0.0.1".parse().unwrap())
}

fn classify_route(request: &Request) -> RouteClass {
    let path = request.uri().path();
    let method = request.method();

    if path == "/tiempo-real" && method.as_str() == "GET" {
        return RouteClass::WebSocket;
    }

    if path.starts_with("/api/") {
        if method.as_str() == "POST" {
            if path == "/api/mcp/call" {
                return RouteClass::Mutation;
            }
            if path.starts_with("/api/admin/")
                || path == "/api/config"
                || path == "/api/demo"
                || path == "/api/demo/caos"
                || path == "/api/demo/final"
                || path == "/api/demo/reset"
                || path == "/api/demo/capturar/iniciar"
                || path == "/api/demo/capturar/detener"
                || path == "/api/demo/capturar/replay"
                || path == "/api/ga/evolucionar"
                || path == "/api/ga/config"
                || path == "/api/exchanges"
                || path == "/api/rebalance/rules"
                || path == "/api/adverso"
            {
                return RouteClass::Mutation;
            }
            if path == "/api/discord/interactions" {
                return RouteClass::Auth;
            }
        }
    }

    RouteClass::Default
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Method, Uri};

    #[test]
    fn test_rate_limiter_basic() {
        let policy = RateLimitPolicy {
            default: RateLimitConfig {
                requests_per_window: 5,
                window: Duration::from_secs(60),
                burst: None,
            },
            ..Default::default()
        };
        let limiter = RateLimiter::new(policy);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        for _ in 0..5 {
            assert!(block_on(limiter.check(ip, RouteClass::Default)).is_ok());
        }
        assert!(block_on(limiter.check(ip, RouteClass::Default)).is_err());
    }

    #[test]
    fn test_route_classification() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/estado")
            .body(())
            .unwrap();
        assert_eq!(classify_route(&req), RouteClass::Default);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/demo")
            .body(())
            .unwrap();
        assert_eq!(classify_route(&req), RouteClass::Mutation);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/tiempo-real")
            .body(())
            .unwrap();
        assert_eq!(classify_route(&req), RouteClass::WebSocket);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/discord/interactions")
            .body(())
            .unwrap();
        assert_eq!(classify_route(&req), RouteClass::Auth);
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }
}
