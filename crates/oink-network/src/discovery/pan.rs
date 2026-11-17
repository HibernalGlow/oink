//! Bluetooth PAN (Personal Area Network) and link-local interface discovery helpers.
//!
//! Bluetooth PAN connects two devices via BNEP (Bluetooth Network Encapsulation Protocol).
//! The OS presents it as a standard network interface with an assigned IPv4 address (either
//! via DHCP or APIPA link-local 169.254.0.0/16 according to RFC 3927).

use std::net::Ipv4Addr;

/// RFC 3927 APIPA link-local IPv4 range (169.254.0.0/16).
/// Commonly used for direct cable connections and ad-hoc Bluetooth PAN.
pub fn is_link_local(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 169 && octets[1] == 254
}

/// Identifies whether an IPv4 address belongs to a Bluetooth PAN or direct ad-hoc link:
/// - RFC 3927 Link-local range: `169.254.0.0/16`
/// - Apple macOS/iOS Personal Hotspot over Bluetooth: `172.20.10.0/28`
/// - Windows PAN ICS default subnet: `192.168.44.0/24`
pub fn is_bluetooth_pan(ip: Ipv4Addr) -> bool {
    if is_link_local(ip) {
        return true;
    }
    let octets = ip.octets();
    // 172.20.10.0/28
    if octets[0] == 172 && octets[1] == 20 && octets[2] == 10 && (octets[3] & 0xF0) == 0 {
        return true;
    }
    // 192.168.44.0/24
    if octets[0] == 192 && octets[1] == 168 && octets[2] == 44 {
        return true;
    }
    false
}

/// Checks if an interface name suggests Bluetooth PAN or BNEP adapter.
pub fn is_pan_interface_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("bluetooth")
        || lower.contains("bthpan")
        || lower.contains("bnep")
        || lower.contains("pan0")
        || lower.contains("btpan")
}

/// Calculates the exact subnet broadcast address for PAN interfaces.
/// For RFC 3927 link-local addresses, RFC requires 169.254.255.255 regardless of host netmask.
pub fn pan_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Option<Ipv4Addr> {
    if is_link_local(ip) {
        return Some(Ipv4Addr::new(169, 254, 255, 255));
    }
    let ip_u32 = u32::from(ip);
    let mask_u32 = u32::from(netmask);
    if mask_u32 != 0 && mask_u32 != u32::MAX {
        Some(Ipv4Addr::from(ip_u32 | !mask_u32))
    } else {
        None
    }
}

/// Enumerates all currently available PAN or link-local IPv4 interfaces.
pub fn pan_interfaces() -> Vec<(String, Ipv4Addr, Option<Ipv4Addr>)> {
    let Ok(list) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for iface in list {
        if iface.is_loopback() {
            continue;
        }
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            if is_pan_interface_name(&iface.name) || is_bluetooth_pan(v4.ip) {
                let bcast = pan_broadcast(v4.ip, v4.netmask).or(v4.broadcast);
                out.push((iface.name, v4.ip, bcast));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out.dedup_by(|a, b| a.1 == b.1);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_local_detection() {
        assert!(is_link_local("169.254.0.1".parse().unwrap()));
        assert!(is_link_local("169.254.255.254".parse().unwrap()));
        assert!(!is_link_local("169.253.1.1".parse().unwrap()));
        assert!(!is_link_local("169.255.1.1".parse().unwrap()));
        assert!(!is_link_local("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_bluetooth_pan_ip_detection() {
        // Link-local
        assert!(is_bluetooth_pan("169.254.12.34".parse().unwrap()));
        // Apple Personal Hotspot (172.20.10.0/28)
        assert!(is_bluetooth_pan("172.20.10.1".parse().unwrap()));
        assert!(is_bluetooth_pan("172.20.10.14".parse().unwrap()));
        assert!(!is_bluetooth_pan("172.20.10.16".parse().unwrap())); // Outside /28
        assert!(!is_bluetooth_pan("172.20.11.1".parse().unwrap()));
        // Windows PAN ICS (192.168.44.0/24)
        assert!(is_bluetooth_pan("192.168.44.1".parse().unwrap()));
        assert!(is_bluetooth_pan("192.168.44.100".parse().unwrap()));
        assert!(!is_bluetooth_pan("192.168.45.1".parse().unwrap()));
        // Standard LAN / Tailscale
        assert!(!is_bluetooth_pan("192.168.1.100".parse().unwrap()));
        assert!(!is_bluetooth_pan("10.0.0.1".parse().unwrap()));
        assert!(!is_bluetooth_pan("100.64.0.1".parse().unwrap()));
    }

    #[test]
    fn test_pan_interface_names() {
        assert!(is_pan_interface_name("bnep0"));
        assert!(is_pan_interface_name("pan0"));
        assert!(is_pan_interface_name("Bluetooth Network Connection"));
        assert!(is_pan_interface_name("BTHPAN"));
        assert!(is_pan_interface_name("en9-bluetooth"));
        assert!(!is_pan_interface_name("en0"));
        assert!(!is_pan_interface_name("eth0"));
        assert!(!is_pan_interface_name("wlan0"));
        assert!(!is_pan_interface_name("utun3"));
    }

    #[test]
    fn test_pan_broadcast_calculation() {
        // Link-local always produces 169.254.255.255
        let bcast = pan_broadcast("169.254.10.20".parse().unwrap(), "255.255.255.255".parse().unwrap());
        assert_eq!(bcast, Some("169.254.255.255".parse().unwrap()));

        // Standard subnet calculation
        let bcast2 = pan_broadcast("192.168.44.10".parse().unwrap(), "255.255.255.0".parse().unwrap());
        assert_eq!(bcast2, Some("192.168.44.255".parse().unwrap()));

        // Mask 0 or 255.255.255.255 for non-link-local returns None
        let bcast3 = pan_broadcast("10.0.0.1".parse().unwrap(), "255.255.255.255".parse().unwrap());
        assert_eq!(bcast3, None);
    }
}
