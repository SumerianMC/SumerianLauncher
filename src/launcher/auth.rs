use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

// Microsoft OAuth2 endpoints
const MS_DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MS_TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
// Xbox Live
const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
// Minecraft Services
const MC_AUTH_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

// Public client ID for device code flow (Minecraft launcher client ID)
const CLIENT_ID: &str = "00000000402b5328";
const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
const MS_REFRESH_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

// ely.by auth endpoints
const ELY_AUTH_URL: &str = "https://authserver.ely.by/auth/authenticate";
const ELY_REFRESH_URL: &str = "https://authserver.ely.by/auth/refresh";
const ELY_CLIENT_TOKEN: &str = "sumerian-client";
// ely.by OAuth2 (for skin API)
const ELY_OAUTH_CLIENT_ID: &str = "sumerian";
const ELY_OAUTH_CLIENT_SECRET: &str = "YC6h_IxQ46vgasj29BuG_7-w4qdI2gZ8MHcatgRM9ymzTOyuGRxJRVThdtAPCuX";
const ELY_OAUTH_SCOPE: &str = "account_info minecraft_server_session offline_access";
const ELY_OAUTH_DEVICE_URL: &str = "https://account.ely.by/oauth2/v1/authorization/device";
const ELY_OAUTH_TOKEN_URL: &str = "https://account.ely.by/api/oauth2/v1/token";

/// Offline UUID namespace — matches what vanilla offline mode uses:
/// UUID v3 of "OfflinePlayer:<username>" in the DNS namespace.
const OFFLINE_NS: uuid::Uuid = uuid::Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1,
    0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

fn offline_uuid(username: &str) -> String {
    let input = format!("OfflinePlayer:{}", username);
    let id = uuid::Uuid::new_v3(&OFFLINE_NS, input.as_bytes());
    id.as_hyphenated().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    Microsoft,
    Local,
    ElyBy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    #[serde(default = "default_auth_type")]
    pub auth_type: AuthType,
    /// ely.by OAuth2 access token for skin API (separate from Yggdrasil game token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_token: Option<String>,
    /// ely.by OAuth2 refresh token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_refresh_token: Option<String>,
}

fn default_auth_type() -> AuthType {
    AuthType::Microsoft
}

impl AuthSession {
    /// For offline sessions Minecraft expects the token to be "-" or any non-empty string.
    pub fn effective_token(&self) -> &str {
        if self.auth_type == AuthType::Local {
            "-"
        } else {
            &self.access_token
        }
    }
}

#[derive(Debug, Deserialize)]
struct ElyAuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: ElyProfile,
}

#[derive(Debug, Deserialize)]
struct ElyProfile {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ElyRefreshResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: ElyProfile,
}

#[derive(Debug, Deserialize)]
struct ElyOAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    error: Option<String>,
}

/// A saved local (offline) profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProfile {
    pub username: String,
    pub uuid: String,
}

impl LocalProfile {
    pub fn new(username: String) -> Self {
        let uuid = offline_uuid(&username);
        Self { username, uuid }
    }

    pub fn new_random(username: String) -> Self {
        let uuid = uuid::Uuid::new_v4().as_hyphenated().to_string();
        Self { username, uuid }
    }

    pub fn to_session(&self) -> AuthSession {
        AuthSession {
            username: self.username.clone(),
            uuid: self.uuid.clone(),
            access_token: "-".into(),
            refresh_token: None,
            auth_type: AuthType::Local,
            oauth_token: None,
            oauth_refresh_token: None,
        }
    }
}

/// Manages the list of local profiles stored in `profiles.json`.
pub struct ProfileManager {
    path: PathBuf,
}

impl ProfileManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("profiles.json"),
        }
    }

    pub async fn load_all(&self) -> Result<Vec<LocalProfile>> {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    pub async fn save_all(&self, profiles: &[LocalProfile]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(profiles)?).await?;
        Ok(())
    }

    pub async fn add(&self, username: &str) -> Result<LocalProfile> {
        let mut profiles = self.load_all().await?;
        if profiles.iter().any(|p| p.username.eq_ignore_ascii_case(username)) {
            bail!("A local profile named '{}' already exists.", username);
        }
        let profile = LocalProfile::new(username.to_string());
        profiles.push(profile.clone());
        self.save_all(&profiles).await?;
        Ok(profile)
    }

    pub async fn add_profile(&self, profile: LocalProfile) -> Result<LocalProfile> {
        let mut profiles = self.load_all().await?;
        if profiles.iter().any(|p| p.username.eq_ignore_ascii_case(&profile.username)) {
            bail!("A local profile named '{}' already exists.", profile.username);
        }
        profiles.push(profile.clone());
        self.save_all(&profiles).await?;
        Ok(profile)
    }

    pub async fn remove(&self, username: &str) -> Result<()> {
        let mut profiles = self.load_all().await?;
        let before = profiles.len();
        profiles.retain(|p| !p.username.eq_ignore_ascii_case(username));
        if profiles.len() == before {
            bail!("No local profile named '{}' found.", username);
        }
        self.save_all(&profiles).await?;
        Ok(())
    }

    pub async fn rename(&self, old: &str, new_name: &str) -> Result<()> {
        let mut profiles = self.load_all().await?;
        if profiles.iter().any(|p| p.username.eq_ignore_ascii_case(new_name)) {
            bail!("A profile named '{}' already exists.", new_name);
        }
        let p = profiles
            .iter_mut()
            .find(|p| p.username.eq_ignore_ascii_case(old))
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found.", old))?;
        p.username = new_name.to_string();
        p.uuid = offline_uuid(new_name);
        self.save_all(&profiles).await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MsTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblClaims,
}

#[derive(Debug, Deserialize)]
struct XblClaims {
    xui: Vec<XblXui>,
}

#[derive(Debug, Deserialize)]
struct XblXui {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct McAuthResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

pub struct Authenticator {
    client: Client,
    sessions_dir: std::path::PathBuf,
}

impl Authenticator {
    pub fn new(client: Client, data_dir: &Path) -> Self {
        Self {
            client,
            sessions_dir: data_dir.join("accounts"),
        }
    }

    fn session_path(&self, uuid: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", uuid))
    }

    pub async fn load_all_sessions(&self) -> Vec<AuthSession> {
        let mut sessions = Vec::new();
        let Ok(mut dir) = tokio::fs::read_dir(&self.sessions_dir).await else {
            return sessions;
        };
        while let Ok(Some(entry)) = dir.next_entry().await {
            if let Ok(raw) = tokio::fs::read_to_string(entry.path()).await {
                if let Ok(s) = serde_json::from_str::<AuthSession>(&raw) {
                    sessions.push(s);
                }
            }
        }
        sessions
    }

    /// Load a single saved session by UUID, or the first available one.
    pub async fn load_session(&self) -> Option<AuthSession> {
        self.load_all_sessions().await.into_iter().next()
    }

    pub async fn try_refresh(&self, session: AuthSession) -> AuthSession {
        match session.auth_type {
            AuthType::Microsoft => {
                let Some(ref rt) = session.refresh_token else { return session; };
                match self.refresh_ms_token(rt).await {
                    Ok(new_session) => { let _ = self.save_session(&new_session).await; new_session }
                    Err(_) => session,
                }
            }
            AuthType::ElyBy => {
                let Some(ref rt) = session.refresh_token else { return session; };
                match self.refresh_ely_token(&session.access_token, rt).await {
                    Ok(mut new_session) => {
                        // Also refresh OAuth2 token if we have one
                        if let Some(ref ort) = session.oauth_refresh_token {
                            if let Ok((oa, or2)) = self.refresh_ely_oauth_token(ort).await {
                                new_session.oauth_token = Some(oa);
                                new_session.oauth_refresh_token = or2;
                            } else {
                                new_session.oauth_token = session.oauth_token.clone();
                                new_session.oauth_refresh_token = session.oauth_refresh_token.clone();
                            }
                        }
                        let _ = self.save_session(&new_session).await;
                        new_session
                    }
                    Err(_) => session,
                }
            }
            AuthType::Local => session,
        }
    }

    async fn refresh_ms_token(&self, refresh_token: &str) -> Result<AuthSession> {
        let resp = self
            .client
            .post(MS_REFRESH_URL)
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("scope", SCOPE),
            ])
            .send()
            .await?
            .json::<MsTokenResponse>()
            .await?;

        let ms_token = resp.access_token.ok_or_else(|| {
            anyhow::anyhow!("Refresh failed: {}", resp.error.unwrap_or_default())
        })?;

        let xbl  = self.authenticate_xbl(&ms_token).await?;
        let uhs  = xbl.display_claims.xui.first().context("No XBL UHS")?.uhs.clone();
        let xsts = self.authenticate_xsts(&xbl.token).await?;
        let mc_token = self.authenticate_minecraft(&xsts.token, &uhs).await?;
        let profile  = self.get_profile(&mc_token).await?;

        Ok(AuthSession {
            username: profile.name,
            uuid: profile.id,
            access_token: mc_token,
            refresh_token: resp.refresh_token,
            auth_type: AuthType::Microsoft,
            oauth_token: None,
            oauth_refresh_token: None,
        })
    }

    pub async fn save_session(&self, session: &AuthSession) -> Result<()> {
        tokio::fs::create_dir_all(&self.sessions_dir).await?;
        tokio::fs::write(
            self.session_path(&session.uuid),
            serde_json::to_string_pretty(session)?,
        ).await?;
        Ok(())
    }

    /// Authenticate with ely.by using username + password.
    pub async fn authenticate_ely_by(&self, username: &str, password: &str) -> Result<AuthSession> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "clientToken": ELY_CLIENT_TOKEN,
            "requestUser": true
        });
        let resp = self
            .client
            .post(ELY_AUTH_URL)
            .json(&body)
            .send()
            .await
            .context("ely.by auth request failed")?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            anyhow::bail!("ely.by login failed (HTTP {}): {}", status, text);
        }

        let parsed: ElyAuthResponse = serde_json::from_str(&text)
            .with_context(|| format!("ely.by auth response parse error: {}", text))?;

        Ok(AuthSession {
            username: parsed.selected_profile.name,
            uuid: parsed.selected_profile.id,
            access_token: parsed.access_token,
            refresh_token: Some(parsed.client_token),
            auth_type: AuthType::ElyBy,
            oauth_token: None,
            oauth_refresh_token: None,
        })
    }

    async fn refresh_ely_token(&self, access_token: &str, client_token: &str) -> Result<AuthSession> {
        let body = serde_json::json!({
            "accessToken": access_token,
            "clientToken": client_token,
            "requestUser": true
        });
        let resp = self
            .client
            .post(ELY_REFRESH_URL)
            .json(&body)
            .send()
            .await?
            .json::<ElyRefreshResponse>()
            .await
            .context("ely.by token refresh failed")?;
        Ok(AuthSession {
            username: resp.selected_profile.name,
            uuid: resp.selected_profile.id,
            access_token: resp.access_token,
            refresh_token: Some(resp.client_token),
            auth_type: AuthType::ElyBy,
            oauth_token: None,
            oauth_refresh_token: None,
        })
    }

    pub async fn remove_session(&self, uuid: &str) -> Result<()> {
        let p = self.session_path(uuid);
        if p.exists() {
            tokio::fs::remove_file(p).await?;
        }
        Ok(())
    }

    /// Start ely.by OAuth2 device flow for skin API access.
    /// Returns the verification URL to open in a browser and the device_code to poll with.
    pub fn ely_oauth_device_url() -> String {
        format!(
            "{}?client_id={}&scope={}",
            ELY_OAUTH_DEVICE_URL,
            ELY_OAUTH_CLIENT_ID,
            ELY_OAUTH_SCOPE.replace(' ', "+")
        )
    }

    /// Poll ely.by OAuth2 token endpoint after user approves in browser.
    pub async fn poll_ely_oauth_token(&self, device_code: &str) -> Result<(String, Option<String>)> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        let wait = std::time::Duration::from_secs(5);
        loop {
            if std::time::Instant::now() > deadline {
                bail!("ely.by OAuth2 authorization timed out");
            }
            tokio::time::sleep(wait).await;
            let resp = self.client
                .post(ELY_OAUTH_TOKEN_URL)
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", ELY_OAUTH_CLIENT_ID),
                    ("client_secret", ELY_OAUTH_CLIENT_SECRET),
                    ("device_code", device_code),
                ])
                .send().await?
                .json::<ElyOAuthTokenResponse>().await?;
            match resp.error.as_deref() {
                None => return Ok((resp.access_token, resp.refresh_token)),
                Some("authorization_pending") => continue,
                Some("slow_down") => tokio::time::sleep(std::time::Duration::from_secs(5)).await,
                Some(e) => bail!("ely.by OAuth2 error: {}", e),
            }
        }
    }

    async fn refresh_ely_oauth_token(&self, refresh_token: &str) -> Result<(String, Option<String>)> {
        let resp = self.client
            .post(ELY_OAUTH_TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", ELY_OAUTH_CLIENT_ID),
                ("client_secret", ELY_OAUTH_CLIENT_SECRET),
                ("refresh_token", refresh_token),
            ])
            .send().await?
            .json::<ElyOAuthTokenResponse>().await?;
        if let Some(e) = resp.error {
            bail!("ely.by OAuth2 refresh failed: {}", e);
        }
        Ok((resp.access_token, resp.refresh_token))
    }

    /// Full Microsoft device-code authentication flow.
    /// Prints the user code and verification URL, then polls for completion.
    pub async fn authenticate(&self) -> Result<AuthSession> {
        // Step 1: Request device code
        let dc_resp = self
            .client
            .post(MS_DEVICE_CODE_URL)
            .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
            .send()
            .await?
            .json::<DeviceCodeResponse>()
            .await
            .context("Failed to get device code")?;

        println!();
        println!(
            "  Open: {}",
            dc_resp.verification_uri
        );
        println!("  Enter code: {}", dc_resp.user_code);
        if let Some(msg) = &dc_resp.message {
            println!("  {}", msg);
        }
        println!();

        // Step 2: Poll for token
        let (ms_token, ms_token_refresh) = self
            .poll_for_token(&dc_resp.device_code, dc_resp.interval, dc_resp.expires_in)
            .await?;

        // Step 3: Xbox Live
        let xbl = self.authenticate_xbl(&ms_token).await?;
        let uhs = xbl
            .display_claims
            .xui
            .first()
            .context("No XBL UHS")?
            .uhs
            .clone();

        // Step 4: XSTS
        let xsts = self.authenticate_xsts(&xbl.token).await?;

        // Step 5: Minecraft
        let mc_token = self
            .authenticate_minecraft(&xsts.token, &uhs)
            .await?;

        // Step 6: Profile
        let profile = self.get_profile(&mc_token).await?;

        Ok(AuthSession {
            username: profile.name,
            uuid: profile.id,
            access_token: mc_token,
            refresh_token: ms_token_refresh,
            auth_type: AuthType::Microsoft,
            oauth_token: None,
            oauth_refresh_token: None,
        })
    }

    async fn poll_for_token(&self, device_code: &str, interval: u64, expires_in: u64) -> Result<(String, Option<String>)> {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(expires_in);
        let wait = std::time::Duration::from_secs(interval.max(5));

        loop {
            if std::time::Instant::now() > deadline {
                bail!("Authentication timed out");
            }
            tokio::time::sleep(wait).await;

            let resp = self
                .client
                .post(MS_TOKEN_URL)
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", device_code),
                ])
                .send()
                .await?
                .json::<MsTokenResponse>()
                .await?;

            if let Some(token) = resp.access_token {
                return Ok((token, resp.refresh_token));
            }
            match resp.error.as_deref() {
                Some("authorization_pending") => continue,
                Some("slow_down") => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                Some(e) => bail!(
                    "Auth error: {} - {}",
                    e,
                    resp.error_description.unwrap_or_default()
                ),
                None => continue,
            }
        }
    }

    async fn authenticate_xbl(&self, ms_token: &str) -> Result<XblResponse> {
        let body = serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": ms_token
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        });
        self.client
            .post(XBL_AUTH_URL)
            .json(&body)
            .send()
            .await?
            .json::<XblResponse>()
            .await
            .context("XBL auth failed")
    }

    async fn authenticate_xsts(&self, xbl_token: &str) -> Result<XblResponse> {
        let body = serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        });
        self.client
            .post(XSTS_AUTH_URL)
            .json(&body)
            .send()
            .await?
            .json::<XblResponse>()
            .await
            .context("XSTS auth failed")
    }

    async fn authenticate_minecraft(&self, xsts_token: &str, uhs: &str) -> Result<String> {
        let body = serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
        });
        let resp = self
            .client
            .post(MC_AUTH_URL)
            .json(&body)
            .send()
            .await?
            .json::<McAuthResponse>()
            .await
            .context("Minecraft auth failed")?;
        Ok(resp.access_token)
    }

    async fn get_profile(&self, mc_token: &str) -> Result<McProfile> {
        self.client
            .get(MC_PROFILE_URL)
            .bearer_auth(mc_token)
            .send()
            .await?
            .json::<McProfile>()
            .await
            .context("Failed to get Minecraft profile")
    }

    /// Returns Ok(true) if the token is valid, Ok(false) if expired/invalid, Err on network failure.
    pub async fn validate_session(&self, session: &AuthSession) -> Result<bool> {
        if session.auth_type != AuthType::Microsoft {
            return Ok(true); // local/offline always valid
        }
        let resp = self.client
            .get(MC_PROFILE_URL)
            .bearer_auth(&session.access_token)
            .send()
            .await?;
        Ok(resp.status().is_success())
    }
}
