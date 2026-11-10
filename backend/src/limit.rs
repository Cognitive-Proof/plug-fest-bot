//! A per-client rate limit for the API and the MCP server: at most `RATE_LIMIT_PER_MINUTE`
//! requests (default 60; 0 turns it off) from one address each minute. Each instance counts on
//! its own, so the service as a whole allows that times its instance count.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

const DEFAULT_PER_MINUTE: u32 = 60;
const WINDOW: Duration = Duration::from_secs(60);
/// Forget finished windows once this many clients are tracked.
const PRUNE_AT: usize = 10_000;

pub struct RateLimit {
    per_minute: u32,
    seen: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl RateLimit {
    pub fn from_env() -> Option<Arc<Self>> {
        let per_minute = match std::env::var("RATE_LIMIT_PER_MINUTE") {
            Ok(v) => v.trim().parse().unwrap_or(DEFAULT_PER_MINUTE),
            Err(_) => DEFAULT_PER_MINUTE,
        };
        if per_minute == 0 {
            println!("rate limit off");
            return None;
        }
        println!("rate limit: {per_minute} requests a minute per client");
        Some(Arc::new(Self { per_minute, seen: Mutex::default() }))
    }

    /// `Err` holds the seconds until `ip` may try again.
    fn allow(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let mut seen = self.seen.lock().unwrap();
        if seen.len() >= PRUNE_AT {
            seen.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        }
        let (start, count) = seen.entry(ip).or_insert((now, 0));
        if now.duration_since(*start) >= WINDOW {
            (*start, *count) = (now, 0);
        }
        if *count >= self.per_minute {
            return Err((WINDOW - now.duration_since(*start)).as_secs().max(1));
        }
        *count += 1;
        Ok(())
    }
}

/// The client's address: the last `X-Forwarded-For` entry, which Cloud Run's front end appends
/// (earlier entries come from the client and can't be trusted), else the connection's peer.
fn client(req: &Request) -> IpAddr {
    let forwarded = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .and_then(|ip| ip.trim().parse().ok());
    forwarded
        .or_else(|| req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip()))
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
}

pub async fn check(State(limit): State<Arc<RateLimit>>, req: Request, next: Next) -> Response {
    match limit.allow(client(&req), Instant::now()) {
        Ok(()) => next.run(req).await,
        Err(secs) => {
            let mut res = (StatusCode::TOO_MANY_REQUESTS, format!("too many requests; try again in {secs} s")).into_response();
            res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(secs));
            res
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_each_client_per_window() {
        let limit = RateLimit { per_minute: 2, seen: Mutex::default() };
        let (a, b) = ("1.2.3.4".parse().unwrap(), "5.6.7.8".parse().unwrap());
        let t = Instant::now();
        assert!(limit.allow(a, t).is_ok());
        assert!(limit.allow(a, t).is_ok());
        assert_eq!(limit.allow(a, t + Duration::from_secs(20)), Err(40));
        assert!(limit.allow(b, t).is_ok());
        assert!(limit.allow(a, t + WINDOW).is_ok());
    }
}
