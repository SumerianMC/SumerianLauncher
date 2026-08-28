/// Friends list with server-status pinging.
///
/// Each friend entry stores a display name and the address of their usual
/// Minecraft server.  At display time the launcher TCP-pings the server and
/// shows latency (or "offline").
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Friend {
    pub name: String,
    /// Optional server address (host or host:port) to check.
    pub server: Option<String>,
    #[serde(default)]
    pub notes: String,
}

impl Friend {
    /// Parse host + port from the server field.  Defaults to port 25565.
    pub fn host_port(&self) -> Option<(String, u16)> {
        let addr = self.server.as_deref()?.trim();
        if addr.is_empty() { return None; }
        if addr.contains(':') {
            let mut parts = addr.rsplitn(2, ':');
            let port: u16 = parts.next()?.parse().ok()?;
            let host = parts.next()?.to_string();
            Some((host, port))
        } else {
            Some((addr.to_string(), 25565))
        }
    }

    /// TCP-ping the friend's server.  Returns round-trip ms or `None` if
    /// the server address is unset or unreachable.
    pub fn ping(&self) -> Option<u64> {
        let (host, port) = self.host_port()?;
        let addr = format!("{}:{}", host, port);
        let start = Instant::now();
        match TcpStream::connect_timeout(&addr.parse().ok()?, Duration::from_secs(3)) {
            Ok(_) => Some(start.elapsed().as_millis() as u64),
            Err(_) => None,
        }
    }
}

pub struct FriendList {
    path: PathBuf,
}

impl FriendList {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("friends.json") }
    }

    pub async fn load(&self) -> Result<Vec<Friend>> {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_)  => Ok(Vec::new()),
        }
    }

    pub async fn save(&self, friends: &[Friend]) -> Result<()> {
        if let Some(p) = self.path.parent() { fs::create_dir_all(p).await?; }
        fs::write(&self.path, serde_json::to_string_pretty(friends)?).await?;
        Ok(())
    }

    pub async fn add(&self, friend: Friend) -> Result<()> {
        let mut list = self.load().await?;
        list.push(friend);
        self.save(&list).await
    }

    pub async fn remove(&self, index: usize) -> Result<()> {
        let mut list = self.load().await?;
        if index >= list.len() { anyhow::bail!("Index out of range"); }
        list.remove(index);
        self.save(&list).await
    }

    pub async fn update(&self, index: usize, friend: Friend) -> Result<()> {
        let mut list = self.load().await?;
        if index >= list.len() { anyhow::bail!("Index out of range"); }
        list[index] = friend;
        self.save(&list).await
    }
}
