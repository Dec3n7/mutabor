//! The HTTP client for engine downloads and the update check. It goes
//! through the same proxy as the browser: HTTPS_PROXY / ALL_PROXY when set,
//! else the system proxy (Windows "Internet Settings"), else directly.

use std::sync::OnceLock;

pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let proxy = proxy_url().and_then(|url| ureq::Proxy::new(&url).ok());
        ureq::Agent::config_builder().proxy(proxy).build().into()
    })
}

fn proxy_url() -> Option<String> {
    for var in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    system_proxy()
}

/// "host:port", or per-protocol "http=h:p;https=h:p;socks=h:p" as Windows
/// stores it; the HTTPS entry wins, then SOCKS, then plain HTTP.
fn parse_windows_proxy(server: &str) -> Option<String> {
    let server = server.trim();
    if server.is_empty() {
        return None;
    }
    if !server.contains('=') {
        return Some(if server.contains("://") { server.to_string() } else { format!("http://{server}") });
    }
    let entry = |name: &str| {
        server.split(';').find_map(|part| {
            let (k, v) = part.split_once('=')?;
            (k.trim().eq_ignore_ascii_case(name) && !v.trim().is_empty()).then(|| v.trim().to_string())
        })
    };
    entry("https")
        .map(|v| format!("http://{v}"))
        .or_else(|| entry("socks").map(|v| format!("socks5://{v}")))
        .or_else(|| entry("http").map(|v| format!("http://{v}")))
}

#[cfg(windows)]
fn system_proxy() -> Option<String> {
    use std::os::windows::process::CommandExt;
    // reg.exe ships with Windows; avoids a registry crate for two values.
    let out = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings"])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let value = |name: &str| {
        text.lines().find_map(|l| {
            let mut parts = l.split_whitespace();
            (parts.next()? == name).then(|| parts.skip(1).collect::<Vec<_>>().join(" "))
        })
    };
    let enabled = value("ProxyEnable").is_some_and(|v| v == "0x1");
    if !enabled {
        return None;
    }
    parse_windows_proxy(&value("ProxyServer")?)
}

#[cfg(not(windows))]
fn system_proxy() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_proxy_formats() {
        assert_eq!(parse_windows_proxy("127.0.0.1:2080").as_deref(), Some("http://127.0.0.1:2080"));
        assert_eq!(
            parse_windows_proxy("http=10.0.0.1:3128;https=10.0.0.1:3129").as_deref(),
            Some("http://10.0.0.1:3129")
        );
        assert_eq!(parse_windows_proxy("socks=127.0.0.1:1080").as_deref(), Some("socks5://127.0.0.1:1080"));
        assert_eq!(parse_windows_proxy(""), None);
    }
}
