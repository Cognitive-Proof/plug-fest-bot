//! Fetches a remote C2PA manifest (one a file links to, e.g. from its XMP `dcterms:provenance`),
//! or a whole file for the MCP `classify_url` tool, with limits, because the URL comes from an
//! untrusted upload or caller: HTTPS only, every address the
//! host resolves to must be public (no loopback, private, link-local or metadata addresses), the
//! connection is pinned to the checked address so DNS can't change it afterwards, no redirects,
//! a size cap and a timeout.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use anyhow::{bail, Context as _, Result};
use reqwest::{redirect, Url};

/// How much to fetch, how long to wait, and what to ask for.
pub struct Limits {
    max_bytes: usize,
    timeout: Duration,
    accept: &'static str,
    what: &'static str,
}

/// A remote manifest: 5 MB and 10 seconds at most.
pub const MANIFEST: Limits =
    Limits { max_bytes: 5 * 1024 * 1024, timeout: Duration::from_secs(10), accept: "application/c2pa", what: "manifest" };

/// A file to classify: the upload size limit and 60 seconds at most.
pub const ASSET: Limits =
    Limits { max_bytes: crate::MAX_UPLOAD, timeout: Duration::from_secs(60), accept: "*/*", what: "file" };

/// The body, and the `Content-Type` the server gave it.
pub struct Fetched {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

pub async fn fetch(url: &str, limits: &Limits) -> Result<Fetched> {
    let Limits { max_bytes, timeout, accept, what } = *limits;
    let too_big = || anyhow::anyhow!("{what} is larger than {} MB", max_bytes / 1024 / 1024);
    let url = Url::parse(url).context("not a valid URL")?;
    if url.scheme() != "https" {
        bail!("only https URLs are fetched");
    }
    let host = url.host_str().context("URL has no host")?.to_string();
    let port = url.port_or_known_default().unwrap_or(443);

    let addrs: Vec<SocketAddr> = tokio::time::timeout(timeout, tokio::net::lookup_host((host.as_str(), port)))
        .await
        .context("DNS lookup timed out")?
        .context("DNS lookup failed")?
        .collect();
    let Some(&addr) = addrs.first() else { bail!("{host} has no addresses") };
    if let Some(bad) = addrs.iter().find(|a| !is_public(a.ip())) {
        bail!("{host} resolves to a non-public address ({})", bad.ip());
    }

    let client = reqwest::Client::builder()
        .redirect(redirect::Policy::none())
        .timeout(timeout)
        .resolve(&host, addr)
        .build()?;
    let mut res = client.get(url).header(reqwest::header::ACCEPT, accept).send().await?;
    if !res.status().is_success() {
        bail!("server answered {}", res.status());
    }
    if res.content_length().is_some_and(|n| n as usize > max_bytes) {
        return Err(too_big());
    }
    let content_type = res.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_string);
    let mut bytes = Vec::new();
    while let Some(chunk) = res.chunk().await? {
        if bytes.len() + chunk.len() > max_bytes {
            return Err(too_big());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Fetched { bytes, content_type })
}

/// Whether `ip` is a globally routable address, so fetching from it can't reach this server's
/// own network or the cloud metadata service.
fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_public_v4(v4),
            None => is_public_v6(v6),
        },
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local() // includes 169.254.169.254, the metadata server
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // shared address space
        || (a == 192 && b == 0 && ip.octets()[2] == 0) // IETF protocol assignments
        || (a == 198 && (18..20).contains(&b)) // benchmarking
        || a >= 240) // reserved
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00 // unique local
        || (first & 0xffc0) == 0xfe80 // link-local
        || (first == 0x2001 && ip.segments()[1] == 0x0db8) // documentation
        || ip.to_ipv4().is_some()) // IPv4-compatible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_addresses() {
        for ok in ["8.8.8.8", "23.45.67.89", "2606:4700::1111"] {
            assert!(is_public(ok.parse().unwrap()), "{ok}");
        }
        for bad in [
            "127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.1.1", "169.254.169.254", "0.0.0.0",
            "100.64.0.1", "198.18.0.1", "224.0.0.1", "255.255.255.255", "::1", "fe80::1",
            "fd00::1", "::ffff:127.0.0.1", "::ffff:169.254.169.254",
        ] {
            assert!(!is_public(bad.parse().unwrap()), "{bad}");
        }
    }

    #[tokio::test]
    async fn refuses_unsafe_urls() {
        for (url, why) in [
            ("http://example.com/m.c2pa", "only https"),
            ("https://127.0.0.1/m.c2pa", "non-public"),
            ("https://169.254.169.254/computeMetadata/v1/", "non-public"),
            ("https://localhost/m.c2pa", "non-public"),
            ("not a url", "not a valid URL"),
        ] {
            let err = fetch(url, &MANIFEST).await.err().unwrap().to_string();
            assert!(err.contains(why), "{url}: {err}");
        }
    }
}
