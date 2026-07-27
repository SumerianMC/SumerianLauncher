mod client;
mod launcher;
mod optimizer;
mod renderer;

use anyhow::Result;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::path::PathBuf;

use client::injection::{GameLauncher, LaunchOptions, detect_java_major, find_java_for_major};
use launcher::{
    auth::{AuthSession, AuthType, Authenticator, ProfileManager},
    backup::BackupManager,
    downloader::Downloader,
    history::{HistoryManager, LaunchRecord},
    instances::InstanceManager,
    manifest::VersionManifest,
    mods::ModManager,
    news,
    presets::{LaunchPreset, PresetManager},
    version::VersionManager,
};
use optimizer::OptimizationProfile;
use renderer::{
    pipeline::RenderPipeline,
    shaders::ShaderManager,
    textures::TextureManager,
};

fn theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

fn base_dir() -> PathBuf {
    let mut d = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    d.push("SumerianClient");
    d
}

fn game_dir() -> PathBuf {
    base_dir().join("game")
}

fn config_dir() -> PathBuf {
    base_dir().join("config")
}

fn bundled_shaders_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config").join("shaders")))
        .unwrap_or_else(|| PathBuf::from("config").join("shaders"))
}

fn print_banner() {
    println!();
    println!(
        "{}",
        style("  ███████╗██╗   ██╗███╗   ███╗███████╗██████╗ ██╗ █████╗ ███╗   ██╗").yellow()
    );
    println!(
        "{}",
        style("  ██╔════╝██║   ██║████╗ ████║██╔════╝██╔══██╗██║██╔══██╗████╗  ██║").yellow()
    );
    println!(
        "{}",
        style("  ███████╗██║   ██║██╔████╔██║█████╗  ██████╔╝██║███████║██╔██╗ ██║").yellow()
    );
    println!(
        "{}",
        style("  ╚════██║██║   ██║██║╚██╔╝██║██╔══╝  ██╔══██╗██║██╔══██║██║╚██╗██║").yellow()
    );
    println!(
        "{}",
        style("  ███████║╚██████╔╝██║ ╚═╝ ██║███████╗██║  ██║██║██║  ██║██║ ╚████║").yellow()
    );
    println!(
        "{}",
        style("  ╚══════╝ ╚═════╝ ╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝").yellow()
    );
    println!();
    println!(
        "  {} {}",
        style("Sumerian Client").cyan().bold(),
        style("v0.1.0 — Minecraft Legacy Launcher").dim()
    );
    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    print_banner();

    let base = base_dir();
    let game = game_dir();
    let config = config_dir();

    std::fs::create_dir_all(&game)?;
    std::fs::create_dir_all(&config)?;

    let http = reqwest::Client::builder()
        .user_agent("SumerianClient/0.1.0")
        .build()?;

    let texture_mgr = TextureManager::new(&base);
    texture_mgr.init().await?;

    let shader_mgr = ShaderManager::new(&bundled_shaders_dir());
    let version_mgr = VersionManager::new(&game);
    let downloader = Downloader::new(http.clone(), game.clone());
    let auth = Authenticator::new(http.clone(), &base);
    let profiles = ProfileManager::new(&base);
    let history_mgr = HistoryManager::new(&base);
    let preset_mgr = PresetManager::new(&base);
    let instance_mgr = InstanceManager::new(&base);
    let mod_mgr = ModManager::new(http.clone());
    let backup_mgr = BackupManager::new(&base);

    loop {
        let choice = Select::with_theme(&theme())
            .with_prompt("Main Menu")
            .items(&[
                "Install Version",
                "Launch Game",
                "Launch Preset",
                "Manage Presets",
                "Manage Accounts",
                "Manage Textures",
                "Manage Shaders",
                "Manage Instances",
                "Manage Mods",
                "View Installed Versions",
                "Launch History",
                "News",
                "Exit",
            ])
            .default(0)
            .interact()?;

        match choice {
            0 => install_version(&http, &downloader, &version_mgr, &game).await?,
            1 => launch_game(&downloader, &auth, &profiles, &version_mgr, &texture_mgr, &shader_mgr, &history_mgr, &instance_mgr, &game).await?,
            2 => launch_preset(&downloader, &auth, &profiles, &preset_mgr, &version_mgr, &texture_mgr, &shader_mgr, &history_mgr, &instance_mgr, &game).await?,
            3 => manage_presets(&preset_mgr, &version_mgr, &texture_mgr, &shader_mgr).await?,
            4 => manage_accounts(&auth, &profiles, &base).await?,
            5 => manage_textures(&texture_mgr, &game).await?,
            6 => manage_shaders(&shader_mgr, &game).await?,
            7 => manage_instances(&instance_mgr, &version_mgr, &backup_mgr, &game).await?,
            8 => manage_mods(&mod_mgr, &instance_mgr, &version_mgr, &game).await?,
            9 => list_installed(&version_mgr).await?,
            10 => view_launch_history(&history_mgr).await?,
            11 => view_news(&http).await?,
            12 => {
                println!("  Goodbye.");
                break;
            }
            _ => {}
        }
        println!();
    }

    Ok(())
}

async fn install_version(
    http: &reqwest::Client,
    downloader: &Downloader,
    version_mgr: &VersionManager,
    _game_dir: &PathBuf,
) -> Result<()> {
    println!("  {} Fetching version manifest...", style("→").cyan());
    let manifest = VersionManifest::fetch(http).await?;

    let type_names = ["release", "snapshot", "old_beta", "old_alpha"];
    let type_labels = ["Release", "Snapshot", "Beta", "Alpha / Classic"];

    let era_idx = Select::with_theme(&theme())
        .with_prompt("Version type")
        .items(&type_labels)
        .default(0)
        .interact()?;

    let versions = manifest.filter_by_type(type_names[era_idx]);
    if versions.is_empty() {
        println!("  No versions found for this type.");
        return Ok(());
    }

    let labels: Vec<String> = versions
        .iter()
        .map(|v| format!("{} ({})", v.id, v.release_time.get(..10).unwrap_or("")))
        .collect();

    let idx = Select::with_theme(&theme())
        .with_prompt("Select version")
        .items(&labels)
        .default(0)
        .interact()?;

    let entry = &versions[idx];
    println!(
        "  {} Fetching metadata for {}...",
        style("→").cyan(),
        entry.id
    );
    let meta = entry.fetch_meta(http).await?;

    // Save version JSON
    version_mgr.save_meta(&meta).await?;

    // Download client JAR
    println!("  {} Downloading client JAR...", style("→").cyan());
    downloader.download_version(&meta).await?;

    // Download libraries
    println!("  {} Downloading libraries...", style("→").cyan());
    downloader.download_libraries(&meta).await?;

    // Download assets
    if let Some(asset_index) = &meta.asset_index {
        println!("  {} Downloading assets...", style("→").cyan());
        downloader.download_assets(asset_index).await?;
    }

    println!(
        "  {} Version {} installed successfully!",
        style("✓").green(),
        meta.id
    );
    Ok(())
}

async fn launch_game(
    downloader: &Downloader,
    auth: &Authenticator,
    profiles: &ProfileManager,
    version_mgr: &VersionManager,
    texture_mgr: &TextureManager,
    shader_mgr: &ShaderManager,
    history_mgr: &HistoryManager,
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    // List installed versions
    let installed = version_mgr.list_installed().await?;
    if installed.is_empty() {
        println!("  No versions installed. Please install a version first.");
        return Ok(());
    }

    let labels: Vec<String> = installed
        .iter()
        .map(|v| format!("{} [{}]", v.id, v.version_type))
        .collect();

    let idx = Select::with_theme(&theme())
        .with_prompt("Select version to launch")
        .items(&labels)
        .default(0)
        .interact()?;

    let version_id = installed[idx].id.clone();
    let meta = version_mgr.load_meta(&version_id).await?;
    let era = client::injection::VersionEra::detect(&meta.id, &meta.version_type, &meta);

    // Instance selection
    let instances = instance_mgr.load_all().await?;
    let game_dir_override = if instances.is_empty() {
        None
    } else {
        let mut inst_labels: Vec<String> = vec!["Default (shared game dir)".into()];
        inst_labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i_idx = Select::with_theme(&theme())
            .with_prompt("Instance")
            .items(&inst_labels)
            .default(0)
            .interact()?;
        if i_idx == 0 { None } else { Some(instance_mgr.instance_dir(&instances[i_idx - 1].name)) }
    };

    // Ensure assets are complete before launching
    if let Some(asset_index) = &meta.asset_index {
        println!("  {} Verifying assets...", style("→").cyan());
        downloader.download_assets(asset_index).await?;
    }

    // Optimization profile
    let opt_profiles = OptimizationProfile::all();
    let profile_labels: Vec<String> = opt_profiles
        .iter()
        .map(|p| format!("{} — {}", p, p.description()))
        .collect();

    let profile_idx = Select::with_theme(&theme())
        .with_prompt("Optimization profile")
        .items(&profile_labels)
        .default(1)
        .interact()?;
    let profile = OptimizationProfile::from_index(profile_idx);

    // Texture pack selection
    let packs = texture_mgr.list_packs().await?;
    let texture_choice = if packs.is_empty() {
        None
    } else {
        let mut pack_labels: Vec<String> = vec!["None (vanilla)".into()];
        pack_labels.extend(packs.iter().map(|p| p.name.clone()));
        let t_idx = Select::with_theme(&theme())
            .with_prompt("Texture pack")
            .items(&pack_labels)
            .default(0)
            .interact()?;
        if t_idx == 0 {
            None
        } else {
            Some(packs[t_idx - 1].name.clone())
        }
    };

    // Shader preset selection
    let presets = shader_mgr.list_presets().await?;
    let shader_choice = if presets.is_empty() {
        None
    } else {
        let mut preset_labels: Vec<String> = vec!["None (vanilla)".into()];
        preset_labels.extend(presets.iter().cloned());
        let s_idx = Select::with_theme(&theme())
            .with_prompt("Shader preset")
            .items(&preset_labels)
            .default(0)
            .interact()?;
        if s_idx == 0 {
            None
        } else {
            Some(presets[s_idx - 1].clone())
        }
    };

    // Apply render pipeline
    let pipeline = RenderPipeline::new(texture_mgr, shader_mgr);
    pipeline
        .apply(
            texture_choice.as_deref(),
            shader_choice.as_deref(),
            game_dir,
            &era,
        )
        .await?;

    check_java_version(&meta);

    // ── Account selection ────────────────────────────────────────────────────
    let session = pick_session(auth, profiles).await?;
    println!(
        "  {} Playing as {} [{}]",
        style("✓").green(),
        style(&session.username).cyan(),
        match session.auth_type {
            AuthType::Local => style("local").yellow(),
            AuthType::Microsoft => style("microsoft").blue(),
        }
    );

    // Launch
    let launcher = GameLauncher::new(game_dir.clone());
    let opts = LaunchOptions {
        session: &session,
        profile: &profile,
        custom_jvm_args: &[],
        width: None,
        height: None,
        server: None,
        port: None,
        game_dir_override: game_dir_override,
    };
    let mut child = launcher.launch(&meta, &opts, version_mgr)?;

    println!(
        "  {} Game launched (PID {}). Waiting for exit...",
        style("✓").green(),
        child.id()
    );
    let started_at = chrono::Utc::now();
    let start = std::time::Instant::now();
    let status = child.wait()?;
    let duration_secs = start.elapsed().as_secs();
    let exit_code = status.code();

    history_mgr.push(LaunchRecord {
        version_id: meta.id.clone(),
        username: session.username.clone(),
        started_at,
        duration_secs,
        exit_code,
    }).await.ok();

    println!("  Game exited with status: {}", status);
    if exit_code != Some(0) {
        show_latest_crash_report(game_dir);
    }

    Ok(())
}

async fn manage_textures(texture_mgr: &TextureManager, game_dir: &PathBuf) -> Result<()> {
    let choice = Select::with_theme(&theme())
        .with_prompt("Texture Manager")
        .items(&["Import pack", "List packs", "Deactivate current", "Back"])
        .default(0)
        .interact()?;

    match choice {
        0 => {
            let path_str: String = Input::with_theme(&theme())
                .with_prompt("Path to resource pack (zip or folder)")
                .interact_text()?;
            let path = std::path::PathBuf::from(path_str.trim());
            let name = texture_mgr.import_pack(&path).await?;
            println!("  {} Imported '{}'", style("✓").green(), name);
        }
        1 => {
            let packs = texture_mgr.list_packs().await?;
            if packs.is_empty() {
                println!("  No packs installed.");
            } else {
                for p in &packs {
                    println!("  • {}", p.name);
                }
            }
        }
        2 => {
            texture_mgr.deactivate(game_dir).await?;
            println!("  {} Textures deactivated.", style("✓").green());
        }
        _ => {}
    }
    Ok(())
}

async fn manage_shaders(shader_mgr: &ShaderManager, game_dir: &PathBuf) -> Result<()> {
    let choice = Select::with_theme(&theme())
        .with_prompt("Shader Manager")
        .items(&["List presets", "Apply preset", "Disable shaders", "Back"])
        .default(0)
        .interact()?;

    match choice {
        0 => {
            let presets = shader_mgr.list_presets().await?;
            if presets.is_empty() {
                println!("  No shader presets found in config/shaders/");
            } else {
                for p in &presets {
                    if let Ok(cfg) = shader_mgr.load_preset(p).await {
                        println!(
                            "  • {} — shadows:{} water:{} bloom:{}",
                            style(p).cyan(),
                            cfg.shadow_quality,
                            cfg.water_reflections,
                            cfg.bloom
                        );
                    } else {
                        println!("  • {}", p);
                    }
                }
            }
        }
        1 => {
            let presets = shader_mgr.list_presets().await?;
            if presets.is_empty() {
                println!("  No presets available.");
                return Ok(());
            }
            let idx = Select::with_theme(&theme())
                .with_prompt("Select preset")
                .items(&presets)
                .default(0)
                .interact()?;
            shader_mgr.inject_shader(&presets[idx], game_dir).await?;
        }
        2 => {
            shader_mgr.disable_shaders(game_dir).await?;
            println!("  {} Shaders disabled.", style("✓").green());
        }
        _ => {}
    }
    Ok(())
}

async fn list_installed(version_mgr: &VersionManager) -> Result<()> {
    let installed = version_mgr.list_installed().await?;
    if installed.is_empty() {
        println!("  No versions installed.");
    } else {
        println!("  Installed versions:");
        for v in &installed {
            println!(
                "  • {} [{}]",
                style(&v.id).cyan(),
                v.version_type
            );
        }
    }
    Ok(())
}

/// Interactively pick an account (local profile or Microsoft) and return a session.
/// Selected Microsoft sessions are auto-refreshed before being returned.
async fn pick_session(auth: &Authenticator, profiles: &ProfileManager) -> Result<AuthSession> {
    let local = profiles.load_all().await?;
    let ms_accounts = auth.load_all_sessions().await;
    let ms_accounts: Vec<_> = ms_accounts
        .into_iter()
        .filter(|s| s.auth_type == AuthType::Microsoft)
        .collect();

    let mut labels: Vec<String> = Vec::new();
    for p in &local {
        labels.push(format!("  {} [local]", p.username));
    }
    for s in &ms_accounts {
        labels.push(format!("  {} [microsoft]", s.username));
    }
    labels.push("  Add local profile".into());
    labels.push("  Log in with Microsoft".into());

    let idx = Select::with_theme(&theme())
        .with_prompt("Select account")
        .items(&labels)
        .default(0)
        .interact()?;

    let local_count = local.len();
    let ms_count = ms_accounts.len();

    if idx < local_count {
        return Ok(local[idx].to_session());
    }
    let ms_start = local_count;
    if idx < ms_start + ms_count {
        let session = ms_accounts.into_iter().nth(idx - ms_start).unwrap();
        // Auto-refresh the token before returning
        let session = auth.try_refresh(session).await;
        return Ok(session);
    }
    let add_local_idx = ms_start + ms_count;
    let ms_login_idx  = add_local_idx + 1;

    if idx == add_local_idx {
        let name: String = Input::with_theme(&theme())
            .with_prompt("Username")
            .validate_with(|s: &String| {
                let s = s.trim();
                if s.is_empty() {
                    Err("Username cannot be empty.")
                } else if s.len() > 16 {
                    Err("Username must be 16 characters or fewer.")
                } else if !s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    Err("Only letters, numbers, and underscores allowed.")
                } else {
                    Ok(())
                }
            })
            .interact_text()?;
        let profile = profiles.add(name.trim()).await?;
        println!(
            "  {} Created local profile '{}' (UUID: {})",
            style("✓").green(),
            profile.username,
            profile.uuid
        );
        return Ok(profile.to_session());
    }

    if idx == ms_login_idx {
        println!("  {} Starting Microsoft login...", style("→").cyan());
        let session = auth.authenticate().await?;
        auth.save_session(&session).await?;
        println!(
            "  {} Logged in as {}",
            style("✓").green(),
            style(&session.username).cyan()
        );
        return Ok(session);
    }

    anyhow::bail!("No account selected.");
}

/// Full account management menu.
async fn manage_accounts(
    auth: &Authenticator,
    profiles: &ProfileManager,
    _base_dir: &PathBuf,
) -> Result<()> {
    loop {
        let local = profiles.load_all().await?;
        let ms_accounts = auth.load_all_sessions().await;
        let ms_accounts: Vec<_> = ms_accounts
            .into_iter()
            .filter(|s| s.auth_type == AuthType::Microsoft)
            .collect();

        println!();
        println!("  {} Accounts", style("◆").cyan());
        if local.is_empty() && ms_accounts.is_empty() {
            println!("  No accounts configured.");
        }
        for p in &local {
            println!("  • {} [local]  uuid: {}", style(&p.username).cyan(), p.uuid);
        }
        for s in &ms_accounts {
            println!("  • {} [microsoft]  uuid: {}", style(&s.username).blue(), s.uuid);
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Account Manager")
            .items(&[
                "Add local profile",
                "Rename local profile",
                "Remove local profile",
                "Log in with Microsoft (add account)",
                "Refresh Microsoft token",
                "Remove Microsoft account",
                "Back",
            ])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let name: String = Input::with_theme(&theme())
                    .with_prompt("Username")
                    .validate_with(|s: &String| {
                        let s = s.trim();
                        if s.is_empty() {
                            Err("Username cannot be empty.")
                        } else if s.len() > 16 {
                            Err("Username must be 16 characters or fewer.")
                        } else if !s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            Err("Only letters, numbers, and underscores allowed.")
                        } else {
                            Ok(())
                        }
                    })
                    .interact_text()?;
                match profiles.add(name.trim()).await {
                    Ok(p) => println!("  {} Added '{}' (UUID: {})", style("✓").green(), p.username, p.uuid),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if local.is_empty() { println!("  No local profiles to rename."); continue; }
                let names: Vec<&str> = local.iter().map(|p| p.username.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Profile to rename").items(&names).default(0).interact()?;
                let new_name: String = Input::with_theme(&theme())
                    .with_prompt("New username")
                    .validate_with(|s: &String| {
                        let s = s.trim();
                        if s.is_empty() { Err("Username cannot be empty.") }
                        else if s.len() > 16 { Err("Username must be 16 characters or fewer.") }
                        else if !s.chars().all(|c| c.is_alphanumeric() || c == '_') { Err("Only letters, numbers, and underscores allowed.") }
                        else { Ok(()) }
                    })
                    .interact_text()?;
                match profiles.rename(local[i].username.as_str(), new_name.trim()).await {
                    Ok(_)  => println!("  {} Renamed.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                if local.is_empty() { println!("  No local profiles to remove."); continue; }
                let names: Vec<&str> = local.iter().map(|p| p.username.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Profile to remove").items(&names).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Remove '{}'?", local[i].username))
                    .default(false).interact()?;
                if confirm {
                    profiles.remove(local[i].username.as_str()).await?;
                    println!("  {} Removed.", style("✓").green());
                }
            }
            3 => {
                println!("  {} Starting Microsoft login...", style("→").cyan());
                match auth.authenticate().await {
                    Ok(session) => {
                        auth.save_session(&session).await?;
                        println!("  {} Logged in as {}", style("✓").green(), style(&session.username).cyan());
                    }
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            4 => {
                if ms_accounts.is_empty() { println!("  No Microsoft accounts saved."); continue; }
                let labels: Vec<String> = ms_accounts.iter().map(|s| format!("{} ({})", s.username, &s.uuid[..8])).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select account to refresh").items(&labels).default(0).interact()?;
                let session = ms_accounts[i].clone();
                let has_refresh = session.refresh_token.is_some();
                if !has_refresh {
                    println!("  {} No refresh token stored for this account.", style("✗").red());
                    continue;
                }
                println!("  {} Refreshing token for {}...", style("→").cyan(), session.username);
                let refreshed = auth.try_refresh(session).await;
                println!("  {} Token refreshed for {}.", style("✓").green(), refreshed.username);
            }
            5 => {
                if ms_accounts.is_empty() { println!("  No Microsoft accounts to remove."); continue; }
                let labels: Vec<String> = ms_accounts.iter().map(|s| format!("{} ({})", s.username, &s.uuid[..8])).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select account to remove").items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Remove Microsoft account '{}'?", ms_accounts[i].username))
                    .default(false).interact()?;
                if confirm {
                    auth.remove_session(&ms_accounts[i].uuid).await?;
                    println!("  {} Removed.", style("✓").green());
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Preset helpers ────────────────────────────────────────────────────────────

/// Launch directly from a saved preset — skips all the individual selectors.
async fn launch_preset(
    downloader: &Downloader,
    auth: &Authenticator,
    profiles: &ProfileManager,
    preset_mgr: &PresetManager,
    version_mgr: &VersionManager,
    texture_mgr: &TextureManager,
    shader_mgr: &ShaderManager,
    history_mgr: &HistoryManager,
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let presets = preset_mgr.load_all().await?;
    if presets.is_empty() {
        println!("  No presets saved. Create one via Manage Presets first.");
        return Ok(());
    }

    let labels: Vec<String> = presets
        .iter()
        .map(|p| format!("{} — {}", style(&p.name).cyan(), p.summary()))
        .collect();

    let idx = Select::with_theme(&theme())
        .with_prompt("Select preset")
        .items(&labels)
        .default(0)
        .interact()?;

    let preset = &presets[idx];

    let meta = match version_mgr.load_meta(&preset.version_id).await {
        Ok(m) => m,
        Err(_) => {
            println!(
                "  {} Version '{}' is not installed. Install it first.",
                style("✗").red(),
                preset.version_id
            );
            return Ok(());
        }
    };
    let era = client::injection::VersionEra::detect(&meta.id, &meta.version_type, &meta);

    // Ensure assets are complete before launching
    if let Some(asset_index) = &meta.asset_index {
        println!("  {} Verifying assets...", style("→").cyan());
        downloader.download_assets(asset_index).await?;
    }

    println!(
        "  {} Preset: {}",
        style("→").cyan(),
        style(&preset.name).cyan().bold()
    );
    println!("     Version:  {}", meta.id);
    println!("     Profile:  {}", preset.optimization);
    println!(
        "     Textures: {}",
        preset.texture_pack.as_deref().unwrap_or("none")
    );
    println!(
        "     Shaders:  {}",
        preset.shader_preset.as_deref().unwrap_or("none")
    );
    println!();

    let pipeline = RenderPipeline::new(texture_mgr, shader_mgr);
    pipeline
        .apply(
            preset.texture_pack.as_deref(),
            preset.shader_preset.as_deref(),
            game_dir,
            &era,
        )
        .await?;

    check_java_version(&meta);

    let session = pick_session(auth, profiles).await?;
    println!(
        "  {} Playing as {} [{}]",
        style("✓").green(),
        style(&session.username).cyan(),
        match session.auth_type {
            AuthType::Local => style("local").yellow(),
            AuthType::Microsoft => style("microsoft").blue(),
        }
    );

    let game_dir_override = match &preset.instance {
        Some(name) => {
            let dir = instance_mgr.instance_dir(name);
            if dir.exists() {
                Some(dir)
            } else {
                println!("  {} Instance '{}' not found, using default game dir.", style("⚠").yellow(), name);
                None
            }
        }
        None => None,
    };

    let launcher = GameLauncher::new(game_dir.clone());
    let opts = LaunchOptions {
        session: &session,
        profile: &preset.optimization,
        custom_jvm_args: &preset.custom_jvm_args,
        width: preset.width,
        height: preset.height,
        server: preset.server.as_deref(),
        port: preset.port,
        game_dir_override,
    };
    let mut child = launcher.launch(&meta, &opts, version_mgr)?;

    println!(
        "  {} Game launched (PID {}). Waiting for exit...",
        style("✓").green(),
        child.id()
    );
    let started_at = chrono::Utc::now();
    let start = std::time::Instant::now();
    let status = child.wait()?;
    let duration_secs = start.elapsed().as_secs();
    let exit_code = status.code();

    history_mgr.push(LaunchRecord {
        version_id: meta.id.clone(),
        username: session.username.clone(),
        started_at,
        duration_secs,
        exit_code,
    }).await.ok();

    println!("  Game exited with status: {}", status);
    if exit_code != Some(0) {
        show_latest_crash_report(game_dir);
    }
    Ok(())
}

/// Create, edit, delete, and inspect saved launch presets.
async fn manage_presets(
    preset_mgr: &PresetManager,
    version_mgr: &VersionManager,
    texture_mgr: &TextureManager,
    shader_mgr: &ShaderManager,
) -> Result<()> {
    loop {
        let presets = preset_mgr.load_all().await?;

        println!();
        println!("  {} Presets ({})", style("◆").cyan(), presets.len());
        for p in &presets {
            println!("  • {} — {}", style(&p.name).cyan(), p.summary());
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Preset Manager")
            .items(&[
                "Create preset",
                "Edit preset",
                "Delete preset",
                "Back",
            ])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                match build_preset_interactive(version_mgr, texture_mgr, shader_mgr, None).await {
                    Ok(preset) => {
                        let name = preset.name.clone();
                        match preset_mgr.add(preset).await {
                            Ok(_) => println!("  {} Preset '{}' saved.", style("✓").green(), name),
                            Err(e) => println!("  {} {}", style("✗").red(), e),
                        }
                    }
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if presets.is_empty() {
                    println!("  No presets to edit.");
                    continue;
                }
                let labels: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select preset to edit")
                    .items(&labels)
                    .default(0)
                    .interact()?;

                match build_preset_interactive(
                    version_mgr,
                    texture_mgr,
                    shader_mgr,
                    Some(&presets[i]),
                )
                .await
                {
                    Ok(updated) => match preset_mgr.update(updated).await {
                        Ok(_) => println!("  {} Preset updated.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    },
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                if presets.is_empty() {
                    println!("  No presets to delete.");
                    continue;
                }
                let labels: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select preset to delete")
                    .items(&labels)
                    .default(0)
                    .interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Delete '{}'?", presets[i].name))
                    .default(false)
                    .interact()?;
                if confirm {
                    preset_mgr.remove(&presets[i].name).await?;
                    println!("  {} Deleted.", style("✓").green());
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Interactive wizard that builds a LaunchPreset.
/// Pass `existing` to pre-fill fields for editing.
async fn build_preset_interactive(
    version_mgr: &VersionManager,
    texture_mgr: &TextureManager,
    shader_mgr: &ShaderManager,
    existing: Option<&LaunchPreset>,
) -> Result<LaunchPreset> {
    // ── Name ──────────────────────────────────────────────────────────────────
    let name: String = Input::with_theme(&theme())
        .with_prompt("Preset name")
        .with_initial_text(existing.map(|p| p.name.as_str()).unwrap_or(""))
        .validate_with(|s: &String| {
            if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) }
        })
        .interact_text()?;

    // ── Version ───────────────────────────────────────────────────────────────
    let installed = version_mgr.list_installed().await?;
    if installed.is_empty() {
        anyhow::bail!("No versions installed. Install a version first.");
    }
    let version_labels: Vec<String> = installed
        .iter()
        .map(|v| format!("{} [{}]", v.id, v.version_type))
        .collect();
    let default_v = existing
        .and_then(|p| installed.iter().position(|v| v.id == p.version_id))
        .unwrap_or(0);
    let v_idx = Select::with_theme(&theme())
        .with_prompt("Version")
        .items(&version_labels)
        .default(default_v)
        .interact()?;
    let version_id = installed[v_idx].id.clone();

    // ── Optimization profile ──────────────────────────────────────────────────
    let opt_list = OptimizationProfile::all();
    let opt_labels: Vec<String> = opt_list
        .iter()
        .map(|p| format!("{} — {}", p, p.description()))
        .collect();
    let default_o = existing
        .and_then(|p| opt_list.iter().position(|o| o == &p.optimization))
        .unwrap_or(1);
    let o_idx = Select::with_theme(&theme())
        .with_prompt("Optimization profile")
        .items(&opt_labels)
        .default(default_o)
        .interact()?;
    let optimization = OptimizationProfile::from_index(o_idx);

    // ── Texture pack ──────────────────────────────────────────────────────────
    let packs = texture_mgr.list_packs().await?;
    let texture_pack = if packs.is_empty() {
        None
    } else {
        let mut pack_labels: Vec<String> = vec!["None (vanilla)".into()];
        pack_labels.extend(packs.iter().map(|p| p.name.clone()));
        let default_t = existing
            .and_then(|p| p.texture_pack.as_ref()
                .and_then(|t| packs.iter().position(|pk| &pk.name == t).map(|i| i + 1)))
            .unwrap_or(0);
        let t_idx = Select::with_theme(&theme())
            .with_prompt("Texture pack")
            .items(&pack_labels)
            .default(default_t)
            .interact()?;
        if t_idx == 0 { None } else { Some(packs[t_idx - 1].name.clone()) }
    };

    // ── Shader preset ─────────────────────────────────────────────────────────
    let shader_presets = shader_mgr.list_presets().await?;
    let shader_preset = if shader_presets.is_empty() {
        None
    } else {
        let mut shader_labels: Vec<String> = vec!["None (vanilla)".into()];
        shader_labels.extend(shader_presets.iter().cloned());
        let default_s = existing
            .and_then(|p| p.shader_preset.as_ref()
                .and_then(|s| shader_presets.iter().position(|sp| sp == s).map(|i| i + 1)))
            .unwrap_or(0);
        let s_idx = Select::with_theme(&theme())
            .with_prompt("Shader preset")
            .items(&shader_labels)
            .default(default_s)
            .interact()?;
        if s_idx == 0 { None } else { Some(shader_presets[s_idx - 1].clone()) }
    };

    // ── Resolution ────────────────────────────────────────────────────────────
    let use_res = Confirm::with_theme(&theme())
        .with_prompt("Set custom resolution?")
        .default(existing.map(|p| p.width.is_some()).unwrap_or(false))
        .interact()?;
    let (width, height) = if use_res {
        let w: String = Input::with_theme(&theme())
            .with_prompt("Width")
            .with_initial_text(existing.and_then(|p| p.width).map(|v| v.to_string()).unwrap_or_else(|| "1280".into()))
            .validate_with(|s: &String| s.trim().parse::<u32>().map(|_| ()).map_err(|_| "Must be a number"))
            .interact_text()?;
        let h: String = Input::with_theme(&theme())
            .with_prompt("Height")
            .with_initial_text(existing.and_then(|p| p.height).map(|v| v.to_string()).unwrap_or_else(|| "720".into()))
            .validate_with(|s: &String| s.trim().parse::<u32>().map(|_| ()).map_err(|_| "Must be a number"))
            .interact_text()?;
        (Some(w.trim().parse::<u32>().unwrap()), Some(h.trim().parse::<u32>().unwrap()))
    } else {
        (None, None)
    };

    // ── Server quick-join ─────────────────────────────────────────────────────
    let use_server = Confirm::with_theme(&theme())
        .with_prompt("Auto-join a server on launch?")
        .default(existing.map(|p| p.server.is_some()).unwrap_or(false))
        .interact()?;
    let (server, port) = if use_server {
        let srv: String = Input::with_theme(&theme())
            .with_prompt("Server address")
            .with_initial_text(existing.and_then(|p| p.server.as_deref()).unwrap_or(""))
            .validate_with(|s: &String| {
                if s.trim().is_empty() { Err("Address cannot be empty.") } else { Ok(()) }
            })
            .interact_text()?;
        let prt: String = Input::with_theme(&theme())
            .with_prompt("Port")
            .with_initial_text(existing.and_then(|p| p.port).map(|v| v.to_string()).unwrap_or_else(|| "25565".into()))
            .validate_with(|s: &String| s.trim().parse::<u16>().map(|_| ()).map_err(|_| "Must be 0–65535"))
            .interact_text()?;
        (Some(srv.trim().to_string()), Some(prt.trim().parse::<u16>().unwrap()))
    } else {
        (None, None)
    };

    // ── Instance ──────────────────────────────────────────────────────────────
    // Inline load — InstanceManager not passed in, so we construct a temporary one.
    let instance_mgr_tmp = InstanceManager::new(&base_dir());
    let all_instances = instance_mgr_tmp.load_all().await.unwrap_or_default();
    let instance = if all_instances.is_empty() {
        None
    } else {
        let mut inst_labels: Vec<String> = vec!["None (default game dir)".into()];
        inst_labels.extend(all_instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let default_i = existing
            .and_then(|p| p.instance.as_ref()
                .and_then(|n| all_instances.iter().position(|i| &i.name == n).map(|x| x + 1)))
            .unwrap_or(0);
        let i_idx = Select::with_theme(&theme())
            .with_prompt("Instance")
            .items(&inst_labels)
            .default(default_i)
            .interact()?;
        if i_idx == 0 { None } else { Some(all_instances[i_idx - 1].name.clone()) }
    };

    // ── Custom JVM args ───────────────────────────────────────────────────────
    let existing_args = existing
        .map(|p| p.custom_jvm_args.join(" "))
        .unwrap_or_default();
    let jvm_raw: String = Input::with_theme(&theme())
        .with_prompt("Extra JVM args (space-separated, leave blank for none)")
        .with_initial_text(&existing_args)
        .allow_empty(true)
        .interact_text()?;
    let custom_jvm_args: Vec<String> = jvm_raw
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    Ok(LaunchPreset {
        name: name.trim().to_string(),
        version_id,
        optimization,
        texture_pack,
        shader_preset,
        custom_jvm_args,
        width,
        height,
        server,
        port,
        instance,
    })
}

// ── Launch History ────────────────────────────────────────────────────────────

async fn view_launch_history(history_mgr: &HistoryManager) -> Result<()> {
    let records = history_mgr.load().await?;
    if records.is_empty() {
        println!("  No launch history yet.");
        return Ok(());
    }
    println!();
    println!("  {} Launch History (last {})", style("◆").cyan(), records.len());
    println!();
    for r in records.iter().rev() {
        let mins = r.duration_secs / 60;
        let secs = r.duration_secs % 60;
        let exit = match r.exit_code {
            Some(0) => style("ok").green().to_string(),
            Some(c) => style(format!("exit {}", c)).red().to_string(),
            None    => style("?").dim().to_string(),
        };
        println!(
            "  {} {}  {}  {}m{}s  [{}]",
            style("•").dim(),
            style(&r.version_id).cyan(),
            r.started_at.format("%Y-%m-%d %H:%M"),
            mins, secs,
            exit,
        );
    }
    Ok(())
}

// ── Crash Log Viewer ──────────────────────────────────────────────────────────

fn show_latest_crash_report(game_dir: &PathBuf) {
    let crash_dir = game_dir.join("crash-reports");
    let Ok(entries) = std::fs::read_dir(&crash_dir) else { return };

    // Find the most recently modified crash report
    let latest = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|x| x.to_str()) == Some("txt")
        })
        .max_by_key(|e| {
            e.metadata().and_then(|m| m.modified()).ok()
        });

    let Some(entry) = latest else { return };

    let Ok(content) = std::fs::read_to_string(entry.path()) else { return };

    println!();
    println!("  {} Crash report: {}", style("⚠").red(), entry.file_name().to_string_lossy());
    println!("  {}", style("─".repeat(60)).dim());

    // Print up to the first 60 lines — enough to see description + stack top
    for line in content.lines().take(60) {
        println!("  {}", line);
    }

    if content.lines().count() > 60 {
        println!("  {} ... (truncated, full report at {})", style("…").dim(), entry.path().display());
    }
    println!();
}

// ── Instance Manager ──────────────────────────────────────────────────────────

async fn manage_instances(
    instance_mgr: &InstanceManager,
    version_mgr: &VersionManager,
    backup_mgr: &BackupManager,
    game_dir: &PathBuf,
) -> Result<()> {
    loop {
        let instances = instance_mgr.load_all().await?;

        println!();
        println!("  {} Instances ({})", style("◆").cyan(), instances.len());
        for i in &instances {
            println!("  • {} [{}]  created: {}", style(&i.name).cyan(), i.version_id, &i.created_at[..10]);
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Instance Manager")
            .items(&["Create instance", "Delete instance", "Backup instance", "Restore backup", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let installed = version_mgr.list_installed().await?;
                if installed.is_empty() {
                    println!("  No versions installed. Install a version first.");
                    continue;
                }
                let name: String = Input::with_theme(&theme())
                    .with_prompt("Instance name")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) }
                    })
                    .interact_text()?;
                let version_labels: Vec<String> = installed
                    .iter()
                    .map(|v| format!("{} [{}]", v.id, v.version_type))
                    .collect();
                let v_idx = Select::with_theme(&theme())
                    .with_prompt("Version")
                    .items(&version_labels)
                    .default(0)
                    .interact()?;
                match instance_mgr.create(name.trim(), &installed[v_idx].id).await {
                    Ok(inst) => println!("  {} Created instance '{}'", style("✓").green(), inst.name),
                    Err(e)   => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if instances.is_empty() { println!("  No instances to delete."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select instance to delete")
                    .items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Delete '{}' and all its files?", instances[i].name))
                    .default(false).interact()?;
                if confirm {
                    match instance_mgr.delete(&instances[i].name).await {
                        Ok(_)  => println!("  {} Deleted.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            2 => {
                if instances.is_empty() { println!("  No instances to back up."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select instance to back up")
                    .items(&labels).default(0).interact()?;
                let inst_dir = instance_mgr.instance_dir(&instances[i].name);
                println!("  {} Creating backup...", style("→").cyan());
                match backup_mgr.create_backup(&instances[i].name, &inst_dir).await {
                    Ok(path) => println!("  {} Backup saved to {}", style("✓").green(), path.display()),
                    Err(e)   => println!("  {} {}", style("✗").red(), e),
                }
            }
            3 => {
                if instances.is_empty() { println!("  No instances available."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select instance to restore into")
                    .items(&labels).default(0).interact()?;
                let backups = backup_mgr.list_backups(&instances[i].name).await?;
                if backups.is_empty() {
                    println!("  No backups found for '{}'.", instances[i].name);
                    continue;
                }
                let backup_labels: Vec<String> = backups.iter()
                    .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                    .collect();
                let b = Select::with_theme(&theme())
                    .with_prompt("Select backup")
                    .items(&backup_labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Restore '{}' into '{}'? This overwrites current saves.",
                        backup_labels[b], instances[i].name))
                    .default(false).interact()?;
                if confirm {
                    let inst_dir = instance_mgr.instance_dir(&instances[i].name);
                    match backup_mgr.restore_backup(&backups[b], &inst_dir).await {
                        Ok(_)  => println!("  {} Restored.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            _ => break,
        }
    }
    // Also allow backing up the default game dir
    let _ = (backup_mgr, game_dir); // suppress unused warnings if no instance selected
    Ok(())
}

// ── News Feed ─────────────────────────────────────────────────────────────────

async fn view_news(http: &reqwest::Client) -> Result<()> {
    println!("  {} Fetching Minecraft news...", style("→").cyan());
    match news::fetch_news(http).await {
        Err(e) => {
            println!("  {} Failed to fetch news: {}", style("✗").red(), e);
            return Ok(());
        }
        Ok(entries) => {
            println!();
            println!("  {} Minecraft Patch Notes (latest {})", style("◆").cyan(), entries.len().min(10));
            println!();
            for entry in entries.iter().take(10) {
                let date = entry.date.as_deref().and_then(|d| d.get(..10)).unwrap_or("unknown");
                println!("  {} {}  {}", style("•").dim(), style(&entry.version).cyan(), style(date).dim());
                println!("    {}", entry.title);
                if let Some(body) = &entry.body {
                    // Print first 2 non-empty lines of the body as a teaser
                    for line in body.lines().filter(|l| !l.trim().is_empty()).take(2) {
                        println!("    {}", style(line.trim()).dim());
                    }
                }
                println!();
            }
        }
    }
    Ok(())
}

// ── Java version mismatch warning ─────────────────────────────────────────────

fn check_java_version(meta: &launcher::manifest::VersionMeta) {
    use client::injection::VersionEra;

    let era = VersionEra::detect(&meta.id, &meta.version_type, meta);
    let required = if era.requires_java8() {
        8
    } else {
        meta.java_version.as_ref().map(|j| j.major_version).unwrap_or(21)
    };

    let java_path = match find_java_for_major(required) {
        Some(p) => p,
        None => {
            println!();
            println!(
                "  {} Java {} is required for {} but no installation was found.",
                style("⚠").yellow().bold(),
                required,
                meta.id
            );
            println!("  The game will likely fail to start.");
            println!();
            return;
        }
    };

    if let Some(actual) = detect_java_major(&java_path) {
        if actual != required {
            println!();
            println!(
                "  {} Java version mismatch for {}",
                style("⚠").yellow().bold(),
                style(&meta.id).cyan()
            );
            println!(
                "     Required : Java {}",
                style(required).yellow()
            );
            println!(
                "     Found    : Java {} at {}",
                style(actual).red(),
                java_path.display()
            );
            println!("     The game may crash. Install Java {} to fix this.", required);
            println!();
        }
    }
}

// ── Mod Manager ───────────────────────────────────────────────────────────────

async fn manage_mods(
    mod_mgr: &ModManager,
    instance_mgr: &InstanceManager,
    version_mgr: &VersionManager,
    game_dir: &PathBuf,
) -> Result<()> {
    // Pick a mods directory: instance or default game dir
    let instances = instance_mgr.load_all().await?;
    let mods_dir = if instances.is_empty() {
        game_dir.join("mods")
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme())
            .with_prompt("Manage mods for")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 {
            game_dir.join("mods")
        } else {
            instance_mgr.instance_dir(&instances[i - 1].name).join("mods")
        }
    };

    // Need a game version for Modrinth search facets
    let installed = version_mgr.list_installed().await?;
    let game_version = if installed.is_empty() {
        "1.21".to_string()
    } else {
        let labels: Vec<String> = installed.iter().map(|v| v.id.clone()).collect();
        let i = Select::with_theme(&theme())
            .with_prompt("Game version (for search)")
            .items(&labels)
            .default(0)
            .interact()?;
        installed[i].id.clone()
    };

    loop {
        let installed_mods = ModManager::list_installed(&mods_dir).await?;

        println!();
        println!("  {} Mods ({}) — {}", style("◆").cyan(), installed_mods.len(), mods_dir.display());
        for m in &installed_mods {
            println!("  • {}", style(m).cyan());
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Mod Manager")
            .items(&["Search & install mod", "Remove mod", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let query: String = Input::with_theme(&theme())
                    .with_prompt("Search Modrinth")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("Query cannot be empty.") } else { Ok(()) }
                    })
                    .interact_text()?;

                println!("  {} Searching...", style("→").cyan());
                let hits = match mod_mgr.search(query.trim(), &game_version).await {
                    Ok(h) => h,
                    Err(e) => { println!("  {} {}", style("✗").red(), e); continue; }
                };
                if hits.is_empty() {
                    println!("  No results found.");
                    continue;
                }

                let hit_labels: Vec<String> = hits.iter()
                    .map(|h| format!("{} — {} ({} downloads)", h.title, h.description.chars().take(50).collect::<String>(), h.downloads))
                    .collect();
                let h = Select::with_theme(&theme())
                    .with_prompt("Select mod")
                    .items(&hit_labels)
                    .default(0)
                    .interact()?;

                let versions = match mod_mgr.get_versions(&hits[h].project_id, &game_version).await {
                    Ok(v) => v,
                    Err(e) => { println!("  {} {}", style("✗").red(), e); continue; }
                };
                if versions.is_empty() {
                    println!("  No compatible versions found for {}.", game_version);
                    continue;
                }

                let ver_labels: Vec<String> = versions.iter()
                    .map(|v| format!("{} ({})", v.name, v.game_versions.join(", ")))
                    .collect();
                let v = Select::with_theme(&theme())
                    .with_prompt("Select version")
                    .items(&ver_labels)
                    .default(0)
                    .interact()?;

                println!("  {} Downloading...", style("→").cyan());
                match mod_mgr.download_mod(&versions[v], &mods_dir).await {
                    Ok(path) => println!("  {} Installed to {}", style("✓").green(), path.display()),
                    Err(e)   => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if installed_mods.is_empty() {
                    println!("  No mods installed.");
                    continue;
                }
                let m = Select::with_theme(&theme())
                    .with_prompt("Select mod to remove")
                    .items(&installed_mods)
                    .default(0)
                    .interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Remove '{}'?", installed_mods[m]))
                    .default(false)
                    .interact()?;
                if confirm {
                    match ModManager::remove_mod(&mods_dir, &installed_mods[m]).await {
                        Ok(_)  => println!("  {} Removed.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}
