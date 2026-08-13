/// Port-forwarding helper.
///
/// Detects the local machine IP, reads the server port from
/// `<game_dir>/server.properties` (defaulting to 25565), then prints
/// ready-to-run commands for ngrok and playit.gg so the user can share
/// their LAN world over the internet.
use std::net::UdpSocket;
use std::path::Path;

pub struct PortInfo {
    pub local_ip: String,
    pub port: u16,
}

/// Detect the outbound local IP by making a UDP "connection" to 8.8.8.8.
pub fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| { s.connect("8.8.8.8:80")?; s.local_addr() })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Read `server-port` from `server.properties` in `game_dir`.
/// Returns 25565 if the file is absent or the key is not found.
pub fn read_server_port(game_dir: &Path) -> u16 {
    let props_path = game_dir.join("server.properties");
    let content = match std::fs::read_to_string(&props_path) {
        Ok(c) => c,
        Err(_) => return 25565,
    };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') { continue; }
        if let Some(rest) = line.strip_prefix("server-port=") {
            if let Ok(p) = rest.trim().parse::<u16>() {
                return p;
            }
        }
    }
    25565
}

/// Build the `PortInfo` for the given game directory.
pub fn detect(game_dir: &Path) -> PortInfo {
    PortInfo {
        local_ip: local_ip(),
        port: read_server_port(game_dir),
    }
}

impl PortInfo {
    /// Human-readable instructions for ngrok.
    pub fn ngrok_instructions(&self) -> String {
        format!(
            "ngrok tcp {port}\n\
             \n\
             Share the address shown by ngrok (e.g. 0.tcp.ngrok.io:NNNNN) with your friends.\n\
             Download ngrok: https://ngrok.com/download",
            port = self.port
        )
    }

    /// Human-readable instructions for playit.gg.
    pub fn playit_instructions(&self) -> String {
        format!(
            "1. Download the playit agent: https://playit.gg/download\n\
             2. Run: playit\n\
             3. Follow the setup to create a Minecraft TCP tunnel on port {port}.\n\
             4. Share the assigned address (e.g. your-name.at.ply.gg) with friends.",
            port = self.port
        )
    }

    /// LAN address friends on the same network can use directly.
    pub fn lan_address(&self) -> String {
        format!("{}:{}", self.local_ip, self.port)
    }
}
