/// LAN world scanner.
///
/// Minecraft advertises LAN worlds by broadcasting a UTF-8 string on the
/// LAN multicast group 224.0.2.60 port 4445, formatted as:
///   [MOTD]<motd>[/MOTD][AD]<port>[/AD]
///
/// We listen for a bounded amount of time and return all unique worlds found.
use anyhow::Result;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};
use std::collections::HashMap;

const MC_LAN_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 2, 60);
const MC_LAN_PORT: u16 = 4445;
/// How long to scan before returning results.
const SCAN_SECS: u64 = 4;

#[derive(Debug, Clone)]
pub struct LanWorld {
    pub motd: String,
    pub port: u16,
    pub address: String,
}

/// Scan for LAN worlds for `SCAN_SECS` seconds and return unique results.
pub fn scan() -> Result<Vec<LanWorld>> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MC_LAN_PORT))?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;

    // Join the multicast group
    socket.join_multicast_v4(&MC_LAN_GROUP, &Ipv4Addr::UNSPECIFIED)?;

    let deadline = Instant::now() + Duration::from_secs(SCAN_SECS);
    let mut buf = [0u8; 1024];
    let mut seen: HashMap<u16, LanWorld> = HashMap::new();

    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                let msg = std::str::from_utf8(&buf[..len]).unwrap_or("").to_string();
                if let Some(world) = parse_lan_broadcast(&msg, &src.ip().to_string()) {
                    seen.insert(world.port, world);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                       || e.kind() == std::io::ErrorKind::TimedOut => {
                // No packet within timeout window, keep looping.
            }
            Err(e) => return Err(e.into()),
        }
    }

    let _ = socket.leave_multicast_v4(&MC_LAN_GROUP, &Ipv4Addr::UNSPECIFIED);
    Ok(seen.into_values().collect())
}

fn parse_lan_broadcast(msg: &str, src_ip: &str) -> Option<LanWorld> {
    let motd = extract_between(msg, "[MOTD]", "[/MOTD]")?;
    let port_str = extract_between(msg, "[AD]", "[/AD]")?;
    let port: u16 = port_str.trim().parse().ok()?;
    Some(LanWorld {
        motd: motd.to_string(),
        port,
        address: format!("{}:{}", src_ip, port),
    })
}

fn extract_between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let end = s[start..].find(close)? + start;
    Some(&s[start..end])
}
