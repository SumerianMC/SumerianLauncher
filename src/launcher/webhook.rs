/// Webhook notification sender.
///
/// Sends a simple JSON POST to a user-configured URL on game start and stop.
/// The payload is intentionally minimal so it works with Discord webhooks,
/// Slack incoming webhooks, ntfy.sh, or any generic HTTP endpoint.

#[derive(Debug, Clone, PartialEq)]
pub enum WebhookEvent {
    GameStarted,
    GameStopped,
}

/// Fire a webhook for `event`.  Silently swallows errors so a misconfigured
/// webhook never blocks the launch flow.
pub async fn fire(
    client: &reqwest::Client,
    url: &str,
    event: WebhookEvent,
    version: &str,
    username: &str,
    duration_secs: Option<u64>,
) {
    if url.trim().is_empty() { return; }

    // Build a payload that is compatible with Discord and generic webhooks.
    let (content, description) = match event {
        WebhookEvent::GameStarted => (
            format!("🎮 **{}** started playing Minecraft {} ", username, version),
            format!("{} launched Minecraft {}", username, version),
        ),
        WebhookEvent::GameStopped => {
            let dur = duration_secs
                .map(|s| {
                    let h = s / 3600;
                    let m = (s % 3600) / 60;
                    if h > 0 { format!("{}h {}m", h, m) } else { format!("{}m", m) }
                })
                .unwrap_or_else(|| "unknown".into());
            (
                format!("🛑 **{}** stopped playing Minecraft {} ({})", username, version, dur),
                format!("{} stopped Minecraft {} after {}", username, version, dur),
            )
        }
    };

    // Try Discord-style embed payload first; fall back to a plain { "content" } body.
    let discord_body = serde_json::json!({
        "content": content,
        "embeds": [{
            "description": description,
            "color": match event {
                WebhookEvent::GameStarted => 0x57F287u32, // green
                WebhookEvent::GameStopped => 0xED4245u32, // red
            }
        }]
    });

    let result = client
        .post(url)
        .json(&discord_body)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await;

    // If Discord-style fails (non-2xx), try a plain `text` payload for
    // generic endpoints like ntfy.sh.
    if result.map(|r| !r.status().is_success()).unwrap_or(true) {
        let plain_body = serde_json::json!({ "text": content });
        let _ = client
            .post(url)
            .json(&plain_body)
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await;
    }
}
