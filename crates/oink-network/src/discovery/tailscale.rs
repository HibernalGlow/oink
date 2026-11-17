//! Tailscale discovery: uses `tailscale status --json` to detect online peers on the tailnet,
//! and queries their Oink discovery port directly via unicast UDP.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const MAX_JSON_OUTPUT_BYTES: usize = 2 * 1024 * 1024; // 2 MB bound
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, thiserror::Error)]
pub enum TailscaleError {
    #[error("Tailscale CLI binary not found")]
    BinaryNotFound,
    #[error("Tailscale command execution failed: {0}")]
    Execution(String),
    #[error("Tailscale command timed out after {:?}", COMMAND_TIMEOUT)]
    Timeout,
    #[error("Tailscale JSON output too large ({0} bytes)")]
    OutputTooLarge(usize),
    #[error("Tailscale JSON decode error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Tailscale CGNAT IPv4 range is 100.64.0.0/10 (RFC 6598).
pub fn is_tailscale(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (octets[1] & 0b1100_0000) == 0b0100_0000
}

#[derive(Debug, Deserialize)]
struct StatusPayload {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<PeerPayload>,
    #[serde(rename = "Peer")]
    peer: Option<HashMap<String, PeerPayload>>,
}

#[derive(Debug, Deserialize)]
struct PeerPayload {
    #[serde(rename = "HostName")]
    _host_name: Option<String>,
    #[serde(rename = "OS")]
    _os: Option<String>,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
    #[serde(rename = "Online")]
    online: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct TailscaleDiscovery {
    binary: Option<PathBuf>,
}

impl Default for TailscaleDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl TailscaleDiscovery {
    pub fn new() -> Self {
        Self {
            binary: locate_binary(),
        }
    }

    pub fn with_binary(path: PathBuf) -> Self {
        Self { binary: Some(path) }
    }

    pub fn binary_path(&self) -> Option<&Path> {
        self.binary.as_deref()
    }

    /// Safely executes `tailscale status --json` with timeout and bounded buffer.
    pub async fn query_status(&self) -> Result<Vec<Ipv4Addr>, TailscaleError> {
        let binary = self.binary.as_ref().ok_or(TailscaleError::BinaryNotFound)?;

        let mut cmd = tokio::process::Command::new(binary);
        cmd.args(["status", "--json"]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn()?;
        let output = match tokio::time::timeout(COMMAND_TIMEOUT, child.wait_with_output()).await {
            Ok(res) => res?,
            Err(_) => return Err(TailscaleError::Timeout),
        };

        if !output.status.success() {
            return Err(TailscaleError::Execution(format!(
                "exit code: {:?}",
                output.status.code()
            )));
        }

        if output.stdout.len() > MAX_JSON_OUTPUT_BYTES {
            return Err(TailscaleError::OutputTooLarge(output.stdout.len()));
        }

        Self::parse_status_json(&output.stdout)
    }

    /// Parses `tailscale status --json` output bytes, extracting online peer IPv4 addresses
    /// while strictly excluding the local self node and non-CGNAT / IPv6 addresses.
    pub fn parse_status_json(bytes: &[u8]) -> Result<Vec<Ipv4Addr>, TailscaleError> {
        if bytes.len() > MAX_JSON_OUTPUT_BYTES {
            return Err(TailscaleError::OutputTooLarge(bytes.len()));
        }

        let status: StatusPayload = serde_json::from_slice(bytes)?;

        // If backend is not running or stopped, return empty list
        if let Some(state) = &status.backend_state {
            if state != "Running" {
                return Ok(Vec::new());
            }
        }

        // Collect self IPs to prevent self-loop queries
        let mut self_ips = HashSet::new();
        if let Some(self_node) = status.self_node {
            if let Some(ips) = self_node.tailscale_ips {
                for ip_str in ips {
                    if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                        self_ips.insert(ip);
                    }
                }
            }
        }

        let mut out = Vec::new();
        if let Some(peers) = status.peer {
            for (_key, peer) in peers {
                if peer.online != Some(true) {
                    continue;
                }
                if let Some(ips) = peer.tailscale_ips {
                    for ip_str in ips {
                        if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                            if is_tailscale(ip) && !self_ips.contains(&ip) {
                                out.push(ip);
                            }
                        }
                    }
                }
            }
        }

        out.sort();
        out.dedup();
        Ok(out)
    }
}

pub fn locate_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TAILSCALE_PATH") {
        let pb = PathBuf::from(path);
        if pb.is_file() {
            return Some(pb);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
            "/opt/homebrew/bin/tailscale",
            "/usr/local/bin/tailscale",
        ];
        for c in candidates {
            let pb = PathBuf::from(c);
            if pb.is_file() {
                return Some(pb);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(prog_files) = std::env::var_os("ProgramFiles") {
            let pb = PathBuf::from(prog_files).join("Tailscale").join("tailscale.exe");
            if pb.is_file() {
                return Some(pb);
            }
        }
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let pb = PathBuf::from(local_app_data).join("Tailscale").join("tailscale.exe");
            if pb.is_file() {
                return Some(pb);
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let candidates = ["/usr/bin/tailscale", "/usr/local/bin/tailscale"];
        for c in candidates {
            let pb = PathBuf::from(c);
            if pb.is_file() {
                return Some(pb);
            }
        }
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = if cfg!(windows) {
                dir.join("tailscale.exe")
            } else {
                dir.join("tailscale")
            };
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOCK_STATUS_JSON: &str = r#"{
        "BackendState": "Running",
        "Self": {
            "HostName": "my-laptop",
            "OS": "macos",
            "TailscaleIPs": ["100.86.164.127", "fd7a:115c:a1e0::1"],
            "Online": true
        },
        "Peer": {
            "nodekey:1": {
                "HostName": "desktop-pc",
                "OS": "windows",
                "TailscaleIPs": ["100.122.176.77", "fd7a:115c:a1e0::2"],
                "Online": true
            },
            "nodekey:2": {
                "HostName": "server",
                "OS": "linux",
                "TailscaleIPs": ["100.90.10.20", "fd7a:115c:a1e0::3"],
                "Online": false
            },
            "nodekey:3": {
                "HostName": "workstation",
                "OS": "linux",
                "TailscaleIPs": ["100.64.1.2"],
                "Online": true
            },
            "nodekey:4": {
                "HostName": "self-reflection",
                "OS": "macos",
                "TailscaleIPs": ["100.86.164.127"],
                "Online": true
            },
            "nodekey:5": {
                "HostName": "foreign-ip",
                "OS": "linux",
                "TailscaleIPs": ["192.168.1.50"],
                "Online": true
            }
        }
    }"#;

    #[test]
    fn parse_valid_tailscale_status() {
        let peers = TailscaleDiscovery::parse_status_json(MOCK_STATUS_JSON.as_bytes()).unwrap();
        // nodekey:1 (100.122.176.77, Online: true) should be included
        // nodekey:2 (Online: false) should be excluded
        // nodekey:3 (100.64.1.2, Online: true) should be included
        // nodekey:4 (matches Self IP 100.86.164.127) should be excluded
        // nodekey:5 (192.168.1.50, not CGNAT 100.64.0.0/10) should be excluded
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0], "100.64.1.2".parse::<Ipv4Addr>().unwrap());
        assert_eq!(peers[1], "100.122.176.77".parse::<Ipv4Addr>().unwrap());
    }

    #[test]
    fn stopped_backend_returns_empty() {
        let json =
            r#"{ "BackendState": "Stopped", "Peer": { "k": { "Online": true, "TailscaleIPs": ["100.64.1.1"] } } }"#;
        let peers = TailscaleDiscovery::parse_status_json(json.as_bytes()).unwrap();
        assert!(peers.is_empty());
    }

    #[test]
    fn malformed_json_returns_error() {
        assert!(TailscaleDiscovery::parse_status_json(b"not json").is_err());
        assert!(TailscaleDiscovery::parse_status_json(b"").is_err());
    }

    #[test]
    fn oversized_json_returns_error() {
        let oversized = vec![b' '; MAX_JSON_OUTPUT_BYTES + 1];
        assert!(matches!(
            TailscaleDiscovery::parse_status_json(&oversized),
            Err(TailscaleError::OutputTooLarge(_))
        ));
    }

    #[test]
    fn cgnat_tailscale_range_check() {
        assert!(is_tailscale("100.64.0.1".parse().unwrap()));
        assert!(is_tailscale("100.127.255.254".parse().unwrap()));
        assert!(!is_tailscale("100.63.255.255".parse().unwrap()));
        assert!(!is_tailscale("100.128.0.0".parse().unwrap()));
        assert!(!is_tailscale("192.168.1.1".parse().unwrap()));
        assert!(!is_tailscale("10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn missing_binary_error() {
        let ts = TailscaleDiscovery { binary: None };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            assert!(matches!(ts.query_status().await, Err(TailscaleError::BinaryNotFound)));
        });
    }
}
