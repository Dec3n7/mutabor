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

    /// The promise in the README and docs/DEVELOPMENT.md, as a test: the app
    /// talks only to these hosts. A new address has to be added here on purpose.
    const APP_HOSTS: [&str; 2] = ["api.github.com", "github.com"];
    /// ffmpeg.martin-riedl.de is only used at build time, by scripts/fetch-tools.mjs.
    const DOWNLOAD_HOSTS: [&str; 4] = ["github.com", "casper.mupdf.com", "huggingface.co", "ffmpeg.martin-riedl.de"];

    fn hosts_in(text: &str) -> Vec<String> {
        text.match_indices("://")
            .filter(|(at, _)| text[..*at].ends_with("http") || text[..*at].ends_with("https"))
            .map(|(at, _)| text[at + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '-').collect())
            .filter(|host: &String| !host.is_empty())
            .collect()
    }

    fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "ts") {
                out.push(path);
            }
        }
    }

    #[test]
    fn code_talks_only_to_known_hosts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        sources(&root.join("src"), &mut files);
        sources(&root.join("../src"), &mut files);
        assert!(files.len() > 10);
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            // Tests may use local servers.
            let code = text.split("#[cfg(test)]").next().unwrap();
            for line in code.lines().filter(|l| !l.trim_start().starts_with("//") && !l.trim_start().starts_with('*')) {
                for host in hosts_in(line) {
                    // XML namespace, a name and not an address.
                    let known = APP_HOSTS.contains(&host.as_str()) || host == "www.w3.org";
                    assert!(known, "{}: unknown host {host}", file.display());
                }
            }
        }
    }

    /// Every url in the manifest, each with its SHA-256 beside it.
    fn check_downloads(value: &serde_json::Value, count: &mut usize) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(url) = map.get("url").and_then(|u| u.as_str()) {
                    assert!(url.starts_with("https://"), "{url}");
                    let hosts = hosts_in(url);
                    assert!(DOWNLOAD_HOSTS.contains(&hosts[0].as_str()), "unknown host in {url}");
                    let hash = map.get("sha256").and_then(|h| h.as_str()).unwrap_or_default();
                    assert!(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()), "no hash for {url}");
                    *count += 1;
                }
                map.values().for_each(|v| check_downloads(v, count));
            }
            serde_json::Value::Array(list) => list.iter().for_each(|v| check_downloads(v, count)),
            _ => {}
        }
    }

    #[test]
    fn every_download_has_a_known_host_and_a_hash() {
        let text = include_str!("../../tools/manifest.json");
        let manifest = serde_json::from_str(text).unwrap();
        let mut count = 0;
        check_downloads(&manifest, &mut count);
        assert_eq!(count, text.matches("\"url\"").count());
    }

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
