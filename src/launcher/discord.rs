use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

const APP_ID: &str = "1531454673561850027";
pub struct DiscordPresence {
    client: DiscordIpcClient,
    connected: bool,
}

impl DiscordPresence {
    pub fn new() -> Self {
        Self {
            client: DiscordIpcClient::new(APP_ID),
            connected: false,
        }
    }

    /// Connect to Discord IPC. Silently fails if Discord is not running.
    pub fn connect(&mut self) {
        self.connected = self.client.connect().is_ok();
    }

    /// Set rich presence. Call after game launches.
    pub fn set_playing(&mut self, version: &str, username: &str) {
        if !self.connected {
            return;
        }
        let state = format!("Playing as {}", username);
        let details = format!("Minecraft {}", version);
        let payload = activity::Activity::new()
            .state(&state)
            .details(&details)
            .assets(
                activity::Assets::new()
                    .large_image("minecraft_logo")
                    .large_text("Sumerian Client"),
            )
            .timestamps(activity::Timestamps::new().start(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            ));
        let _ = self.client.set_activity(payload);
    }

    /// Clear presence and disconnect. Call after game exits.
    pub fn clear(&mut self) {
        if !self.connected {
            return;
        }
        let _ = self.client.clear_activity();
        let _ = self.client.close();
        self.connected = false;
    }
}
