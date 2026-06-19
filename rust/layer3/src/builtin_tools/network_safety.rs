//! SSRF protection for HTTP-based tools.
//!
//! Validates URLs before sending requests to prevent:
//! - Cloud metadata endpoint access (169.254.169.254, etc.)
//! - Internal service probing (localhost, 10.x, 192.168.x)
//! - Link-local / loopback attacks
//! - Dangerous port scanning (22, 25, 6379, etc.)
//! - Non-HTTP schemes (file://, gopher://)

use crate::builtin_tools::limits::FileOpsLimits;
use crate::types::Layer3Result;
use async_trait::async_trait;
use std::net::IpAddr;
use std::sync::Arc;
use url::Url;

/// URL safety validator. Implementations check scheme/host/IP/port policy.
#[async_trait]
pub trait UrlValidator: Send + Sync {
    /// Validate URL. Returns Err with descriptive reason if rejected.
    async fn validate(&self, url: &Url) -> Layer3Result<()>;
}

/// Default validator combining scheme/IP/port checks per `FileOpsLimits`.
pub struct DefaultUrlValidator {
    limits: Arc<FileOpsLimits>,
}

impl DefaultUrlValidator {
    pub fn new(limits: Arc<FileOpsLimits>) -> Self {
        Self { limits }
    }
}

#[async_trait]
impl UrlValidator for DefaultUrlValidator {
    async fn validate(&self, url: &Url) -> Layer3Result<()> {
        // 1. Scheme whitelist
        match url.scheme() {
            "http" | "https" => {}
            other => {
                return Err(anyhow::anyhow!(
                    "URL rejected: scheme '{}' not allowed (only http/https)",
                    other
                ))
            }
        }

        // 2. Host must exist
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("URL rejected: missing host"))?;

        // 3. Resolve host (accept IP literal or domain)
        let ips = resolve_host(host).await?;

        // 4. IP policy checks
        for ip in &ips {
            if self.limits.block_loopback && ip.is_loopback() {
                return Err(anyhow::anyhow!("URL rejected: loopback address blocked"));
            }
            if self.limits.block_private_ips && is_private_ip(ip) {
                return Err(anyhow::anyhow!(
                    "URL rejected: private IP range blocked (RFC 1918)"
                ));
            }
            if self.limits.block_link_local && is_link_local(ip) {
                return Err(anyhow::anyhow!(
                    "URL rejected: link-local address blocked (cloud metadata?)"
                ));
            }
            if self.limits.block_metadata_endpoints && is_metadata_endpoint(ip) {
                return Err(anyhow::anyhow!(
                    "URL rejected: cloud metadata endpoint blocked"
                ));
            }
        }

        // 5. Port blacklist (non-HTTP services)
        if let Some(port) = url.port() {
            const FORBIDDEN_PORTS: &[u16] = &[
                22, 23, 25, 110, 143, 389, 6379,
                11211, // SSH, Telnet, SMTP, POP3, IMAP, LDAP, Redis, Memcached
            ];
            if FORBIDDEN_PORTS.contains(&port) {
                return Err(anyhow::anyhow!(
                    "URL rejected: port {} blocked (non-HTTP service)",
                    port
                ));
            }
        }

        Ok(())
    }
}

/// Resolve hostname to IPs. IP literals returned as-is.
async fn resolve_host(host: &str) -> Layer3Result<Vec<IpAddr>> {
    // If host is already an IP literal, return it
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }

    // Else DNS lookup
    let addrs = tokio::net::lookup_host(format!("{}:80", host))
        .await
        .map_err(|e| anyhow::anyhow!("DNS lookup failed for '{}': {}", host, e))?;
    let ips: Vec<IpAddr> = addrs.map(|sa| sa.ip()).collect();
    if ips.is_empty() {
        return Err(anyhow::anyhow!("DNS returned no records for '{}'", host));
    }
    Ok(ips)
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private(),
        IpAddr::V6(_) => false, // IPv6 ULA checking requires manual range logic
    }
}

fn is_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        // IPv6 link-local fe80::/10
        IpAddr::V6(v6) => {
            let segs = v6.segments();
            (segs[0] & 0xffc0) == 0xfe80
        }
    }
}

fn is_metadata_endpoint(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // AWS / GCP
            *v4 == std::net::Ipv4Addr::new(169, 254, 169, 254)
            // Alibaba Cloud
                || *v4 == std::net::Ipv4Addr::new(100, 100, 100, 200)
            // Tencent Cloud
                || *v4 == std::net::Ipv4Addr::new(169, 254, 169, 253)
        }
        IpAddr::V6(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_validator() -> DefaultUrlValidator {
        DefaultUrlValidator::new(FileOpsLimits::default().into_arc())
    }

    #[test]
    fn test_metadata_endpoint_detection() {
        let aws_meta: IpAddr = "169.254.169.254".parse().unwrap();
        assert!(is_metadata_endpoint(&aws_meta));
        let alibaba: IpAddr = "100.100.100.200".parse().unwrap();
        assert!(is_metadata_endpoint(&alibaba));
        let external: IpAddr = "8.8.8.8".parse().unwrap();
        assert!(!is_metadata_endpoint(&external));
    }

    #[test]
    fn test_private_ip_detection() {
        let home: IpAddr = "192.168.1.1".parse().unwrap();
        assert!(is_private_ip(&home));
        let ten: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(is_private_ip(&ten));
        let public: IpAddr = "8.8.8.8".parse().unwrap();
        assert!(!is_private_ip(&public));
    }

    #[tokio::test]
    async fn test_reject_file_scheme() {
        let v = test_validator();
        let url = Url::parse("file:///etc/passwd").unwrap();
        let result = v.validate(&url).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("scheme"));
    }

    #[tokio::test]
    async fn test_reject_loopback_ip_literal() {
        let v = test_validator();
        let url = Url::parse("http://127.0.0.1/").unwrap();
        let result = v.validate(&url).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("loopback"));
    }

    #[tokio::test]
    async fn test_reject_metadata_endpoint() {
        let v = test_validator();
        let url = Url::parse("http://169.254.169.254/latest/meta-data/").unwrap();
        let result = v.validate(&url).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("metadata") || err.contains("link-local"),
            "got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_reject_redis_port() {
        let v = test_validator();
        let url = Url::parse("http://example.com:6379/").unwrap();
        let result = v.validate(&url).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("port 6379"));
    }
}
