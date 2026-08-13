/// Minecraft Realms browser.
///
/// Uses the official Realms API (same endpoint the vanilla launcher uses).
/// Requires a valid Microsoft/Minecraft access token.
use anyhow::{Context, Result};
use serde::Deserialize;

const REALMS_LIST: &str = "https://pc.realms.minecraft.net/worlds";
const REALMS_ADDRESS: &str = "https://pc.realms.minecraft.net/worlds/{id}/join";

#[derive(Debug, Deserialize)]
pub struct RealmWorld {
    pub id: u64,
    pub name: String,
    pub owner: String,
    #[serde(rename = "ownerUUID")]
    pub owner_uuid: String,
    pub state: String,        // "OPEN", "CLOSED", "UNINITIALIZED"
    #[serde(default)]
    pub motd: Option<String>,
    #[serde(rename = "maxPlayers", default)]
    pub max_players: u32,
}

#[derive(Debug, Deserialize)]
struct RealmsList {
    servers: Vec<RealmWorld>,
}

#[derive(Debug, Deserialize)]
struct RealmAddress {
    address: String,
}

/// Fetch all Realms the player has access to.
pub async fn list_realms(
    client: &reqwest::Client,
    access_token: &str,
    uuid: &str,
    username: &str,
    game_version: &str,
) -> Result<Vec<RealmWorld>> {
    let resp = client
        .get(REALMS_LIST)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "SumerianClient")
        .header("x-version", game_version)
        .header("x-player-uuid", uuid)
        .header("x-player-name", username)
        .send()
        .await
        .context("Realms API request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        anyhow::bail!("Realms API returned {status}. Make sure you have an active Realms subscription.");
    }

    let list = resp
        .json::<RealmsList>()
        .await
        .context("Failed to parse Realms response")?;

    Ok(list.servers)
}

/// Get the server address string for joining a specific Realm.
pub async fn get_address(
    client: &reqwest::Client,
    access_token: &str,
    realm_id: u64,
    game_version: &str,
) -> Result<String> {
    let url = REALMS_ADDRESS.replace("{id}", &realm_id.to_string());
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("x-version", game_version)
        .send()
        .await
        .context("Realms address request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Could not fetch Realm address (status {})", resp.status());
    }

    let addr: RealmAddress = resp.json().await.context("Failed to parse address")?;
    Ok(addr.address)
}
