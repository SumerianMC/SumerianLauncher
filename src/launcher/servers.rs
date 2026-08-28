use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub notes: String,
}

impl ServerEntry {
    pub fn host_port(&self) -> (String, u16) {
        let port = if self.port == 0 { 25565 } else { self.port };
        if self.address.contains(':') {
            let mut parts = self.address.rsplitn(2, ':');
            let p: u16 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(port);
            let h = parts.next().unwrap_or(&self.address).to_string();
            (h, p)
        } else {
            (self.address.clone(), port)
        }
    }
}

pub struct ServerBrowser {
    path: PathBuf,
}

impl ServerBrowser {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("servers.json") }
    }

    pub async fn load(&self) -> Result<Vec<ServerEntry>> {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    pub async fn save(&self, servers: &[ServerEntry]) -> Result<()> {
        if let Some(p) = self.path.parent() { fs::create_dir_all(p).await?; }
        fs::write(&self.path, serde_json::to_string_pretty(servers)?).await?;
        Ok(())
    }

    pub async fn add(&self, entry: ServerEntry) -> Result<()> {
        let mut servers = self.load().await?;
        servers.push(entry);
        self.save(&servers).await
    }

    pub async fn remove(&self, index: usize) -> Result<()> {
        let mut servers = self.load().await?;
        if index >= servers.len() { anyhow::bail!("Index out of range"); }
        servers.remove(index);
        self.save(&servers).await
    }

    /// TCP ping — returns round-trip ms or None on failure.
    pub fn ping(entry: &ServerEntry) -> Option<u64> {
        let (host, port) = entry.host_port();
        let addr = format!("{}:{}", host, port);
        let start = Instant::now();
        match TcpStream::connect_timeout(&addr.parse().ok()?, Duration::from_secs(3)) {
            Ok(_) => Some(start.elapsed().as_millis() as u64),
            Err(_) => None,
        }
    }
}
