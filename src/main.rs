mod client;
mod lang;
mod launcher;
mod optimizer;
mod renderer;

use anyhow::Result;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::path::PathBuf;

use client::injection::{GameLauncher, LaunchOptions, detect_java_major, find_java_for_major, java_download_url, try_auto_install_java};
use launcher::{
    advancements,
    auth::{AuthSession, AuthType, Authenticator, ProfileManager},
    backup::{BackupManager, ScheduledBackupManager},
    benchmark,
    changelogviewer,
    config::ConfigManager,
    configexport,
    configsnapshot::ConfigSnapshotManager,
    conflicts,
    crashpatterns::CrashPatternManager,
    depgraph,
    doctor,
    downloader::Downloader,
    friends::{Friend, FriendList},
    history::{HistoryManager, LaunchRecord},
    instancediff,
    instancehealth,
    instances::{InstanceManager, InstanceProfile, WorldManager},
    jvmadvisor,
    jvmpresets::JvmPresetManager,
    lanscanner,
    leakdetector,
    loader,
    logtail,
    manifest::VersionManifest,
    mod_updates,
    modpacks::ModpackInstaller,
    modprofiles::ModProfileManager,
    mods::ModManager,
    modsearchhistory::ModSearchHistoryManager,
    news,
    playtime,
    playtimegoals::PlaytimeGoalManager,
    portforward,
    presets::{LaunchPreset, PresetManager},
    realms,
    recording,
    screenshots::ScreenshotGallery,
    seedreader,
    servers::ServerBrowser,
    sessiontimer,
    skins::SkinManager,
    splashscreen,
    servermanager,
    tips,
    trending,
    updater,
    version::VersionManager,
    discord::DiscordPresence,
    webhook::{self, WebhookEvent},
    worldinfo,
};use lang::{Lang, load_lang, save_lang};
use optimizer::OptimizationProfile;

use renderer::{
    packwizard::{self, PackSpec, ProceduralTexture},
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
        style(format!("v{} — Minecraft Legacy Launcher", env!("CARGO_PKG_VERSION"))).dim()
    );
    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    print_banner();

    // ── Quick-launch: `sumerian --last` ─────────────────────────────────────
    // Re-launch the most recent session without navigating any menus.
    {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--last") {
            let base_ql = base_dir();
            let game_ql = game_dir();
            let http_ql = reqwest::Client::builder()
                .user_agent(concat!("SumerianClient/", env!("CARGO_PKG_VERSION")))
                .build()?;
            let history_ql = HistoryManager::new(&base_ql);
            let version_mgr_ql = VersionManager::new(&game_ql);
            let downloader_ql = Downloader::new(http_ql.clone(), game_ql.clone());
            let auth_ql = Authenticator::new(http_ql.clone(), &base_ql);
            let profiles_ql = ProfileManager::new(&base_ql);
            let instance_mgr_ql = InstanceManager::new(&base_ql);
            let backup_mgr_ql = BackupManager::new(&base_ql);
            let config_mgr_ql = ConfigManager::new(&base_ql);
            let texture_mgr_ql = TextureManager::new(&base_ql);
            texture_mgr_ql.init().await?;
            let shader_mgr_ql = ShaderManager::new(&bundled_shaders_dir());
            let crash_mgr_ql = CrashPatternManager::new(&base_ql);
            let records = history_ql.load().await?;
            if let Some(last) = records.last() {
                println!(
                    "  {} Quick-launching last session: {} ({})",
                    style("→").cyan(),
                    style(&last.version_id).cyan().bold(),
                    last.username
                );
                println!();
                launch_game(
                    &http_ql, &downloader_ql, &auth_ql, &profiles_ql,
                    &version_mgr_ql, &texture_mgr_ql, &shader_mgr_ql,
                    &history_ql, &instance_mgr_ql, &backup_mgr_ql,
                    &config_mgr_ql, &crash_mgr_ql,
                    &ScheduledBackupManager::new(&base_ql),
                    &PlaytimeGoalManager::new(&base_ql),
                    &game_ql,
                ).await?;
            } else {
                println!("  {} No launch history found. Play a game first.", style("✗").red());
            }
            return Ok(());
        }
    }

    let base = base_dir();

    // ── Auto-update check ────────────────────────────────────────────────────
    {
        let check_http = reqwest::Client::builder()
            .user_agent("SumerianClient")
            .build()
            .unwrap();
        match updater::check_for_update(&check_http).await {
            Ok(Some((tag, url))) => {
                println!(
                    "  {} Update available: {} → {}",
                    style("↑").green().bold(),
                    style(updater::current_version()).dim(),
                    style(&tag).green().bold()
                );
                let do_update = Confirm::with_theme(&theme())
                    .with_prompt("Download and install update now?")
                    .default(true)
                    .interact()
                    .unwrap_or(false);
                if do_update {
                    println!("  {} Downloading {}...", style("→").cyan(), tag);
                    match updater::apply_update(&check_http, &url).await {
                        Ok(path) => {
                            println!("  {} Updated! Restart Sumerian to use the new version.", style("✓").green());
                            println!("  Installed to: {}", path.display());
                            return Ok(());
                        }
                        Err(e) => println!("  {} Update failed: {}", style("✗").red(), e),
                    }
                }
                println!();
            }
            Ok(None) => {} // already up-to-date, silent
            Err(_) => {}   // no network / API down, silent
        }
    }
    let game = game_dir();
    let config = config_dir();

    std::fs::create_dir_all(&game)?;
    std::fs::create_dir_all(&config)?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("SumerianClient/", env!("CARGO_PKG_VERSION")))
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
    let skin_mgr = SkinManager::new(http.clone());
    let server_browser = ServerBrowser::new(&base);
    let modpack_installer = ModpackInstaller::new(http.clone());
    let config_mgr = ConfigManager::new(&base);
    let friend_list = FriendList::new(&base);
    let crash_pattern_mgr = CrashPatternManager::new(&base);
    let config_snapshot_mgr = ConfigSnapshotManager::new(&base);
    let mod_profile_mgr = ModProfileManager::new(&base.join("instances"));
    let jvm_preset_mgr = JvmPresetManager::new(&base);
    let playtime_goal_mgr = PlaytimeGoalManager::new(&base);
    let scheduled_backup_mgr = ScheduledBackupManager::new(&base);
    let search_history_mgr = ModSearchHistoryManager::new(&base);
    let mut lang = load_lang(&base);

    // Print a startup tip below the banner
    println!("  {} {}", style("Tip:").cyan().bold(), style(tips::tip_of_the_session()).dim());
    println!();

    loop {
        let menu_items: Vec<&str> = vec![
            lang.menu_install_version.as_str(),       // 0
            lang.menu_install_mod_loader.as_str(),    // 1
            lang.menu_launch_game.as_str(),           // 2
            lang.menu_launch_preset.as_str(),         // 3
            lang.menu_manage_presets.as_str(),        // 4
            lang.menu_manage_accounts.as_str(),       // 5
            lang.menu_manage_textures.as_str(),       // 6
            lang.menu_manage_shaders.as_str(),        // 7
            lang.menu_manage_instances.as_str(),      // 8
            lang.menu_manage_mods.as_str(),           // 9
            lang.menu_check_mod_updates.as_str(),     // 10
            lang.menu_manage_skins.as_str(),          // 11
            lang.menu_manage_worlds.as_str(),         // 12
            lang.menu_screenshot_gallery.as_str(),    // 13
            lang.menu_view_installed.as_str(),        // 14
            lang.menu_launch_history.as_str(),        // 15
            lang.menu_news.as_str(),                  // 16
            "Server Browser",                         // 17
            "Install Modpack",                        // 18
            "Playtime",                               // 19
            "Friends",                                // 20
            "Trending Mods",                          // 21
            "Resource Pack Wizard",                   // 22
            "Port Forwarding Helper",                 // 23
            "Realm Browser",                          // 24
            "Advancement Tracker",                    // 25
            "LAN World Scanner",                      // 26
            "Whitelist & Op Manager",                 // 27
            "JVM Flag Advisor",                       // 28
            "Crash Pattern Report",                   // 29
            "Instance Diff",                          // 30
            "Recording Helper",                       // 31
            "Splash Screen Editor",                   // 32
            "Config Export / Import",                 // 33
            "Settings",                               // 34
            "Language / Idioma / Langue",             // 35
            lang.menu_exit.as_str(),                  // 36
            "Config Snapshot",                        // 37
            "Mod Profile Switcher",                   // 38
            "JVM Flag Presets",                       // 39
            "Log Tail",                               // 40
            "Doctor",                                 // 41
            "Performance Profiler",                   // 42
            "Playtime Goals",                         // 43
            "World Seed Reader",                      // 44
            "Instance Health Check",                  // 45
            "Resource Pack Preview",                  // 46
            "Mod Search History",                     // 47
            "Bulk Mod Toggle",                        // 48
            "Changelog Viewer",                       // 49
            "Mod Dependency Graph",                   // 50
        ];        let choice = Select::with_theme(&theme())
            .with_prompt("Main Menu")
            .items(&menu_items)
            .default(0)
            .interact()?;

        match choice {
            0  => install_version(&http, &downloader, &version_mgr, &game).await?,
            1  => install_mod_loader(&http, &version_mgr, &game).await?,
            2  => launch_game(&http, &downloader, &auth, &profiles, &version_mgr, &texture_mgr, &shader_mgr, &history_mgr, &instance_mgr, &backup_mgr, &config_mgr, &crash_pattern_mgr, &scheduled_backup_mgr, &playtime_goal_mgr, &game).await?,
            3  => launch_preset(&http, &downloader, &auth, &profiles, &preset_mgr, &version_mgr, &texture_mgr, &shader_mgr, &history_mgr, &instance_mgr, &backup_mgr, &config_mgr, &crash_pattern_mgr, &game).await?,
            4  => manage_presets(&preset_mgr, &version_mgr, &texture_mgr, &shader_mgr).await?,
            5  => manage_accounts(&auth, &profiles, &base).await?,
            6  => manage_textures(&texture_mgr, &game).await?,
            7  => manage_shaders(&shader_mgr, &game).await?,
            8  => manage_instances(&instance_mgr, &version_mgr, &backup_mgr, &game).await?,
            9  => manage_mods(&mod_mgr, &instance_mgr, &version_mgr, &game).await?,
            10 => check_mod_updates(&http, &instance_mgr, &version_mgr, &game).await?,
            11 => manage_skins(&skin_mgr, &auth, &profiles).await?,
            12 => manage_worlds(&instance_mgr, &game).await?,
            13 => screenshot_gallery(&http, &instance_mgr, &game).await?,
            14 => list_installed(&version_mgr).await?,
            15 => view_launch_history(&history_mgr).await?,
            16 => view_news(&http).await?,
            17 => server_browser_menu(&server_browser).await?,
            18 => install_modpack(&modpack_installer, &instance_mgr, &version_mgr).await?,
            19 => view_playtime(&history_mgr).await?,
            20 => friends_menu(&friend_list).await?,
            21 => trending_mods_menu(&http, &version_mgr).await?,
            22 => resource_pack_wizard(&texture_mgr).await?,
            23 => port_forwarding_menu(&game).await?,
            24 => realm_browser_menu(&http, &auth, &profiles, &version_mgr).await?,
            25 => advancement_tracker_menu(&instance_mgr, &game).await?,
            26 => lan_world_scanner_menu().await?,
            27 => whitelist_op_menu(&instance_mgr, &game).await?,
            28 => jvm_advisor_menu(&version_mgr).await?,
            29 => crash_pattern_report_menu(&crash_pattern_mgr).await?,
            30 => instance_diff_menu(&instance_mgr).await?,
            31 => recording_helper_menu(&instance_mgr, &game).await?,
            32 => splash_screen_editor_menu(&texture_mgr).await?,
            33 => config_export_import_menu(&base).await?,
            34 => settings_menu(&config_mgr).await?,
            35 => {
                let all = Lang::all();
                let names: Vec<&str> = all.iter().map(|l| l.name.as_str()).collect();
                let cur = all.iter().position(|l| l.code == lang.code).unwrap_or(0);
                let i = Select::with_theme(&theme())
                    .with_prompt("Select language")
                    .items(&names)
                    .default(cur)
                    .interact()?;
                lang = all[i].clone();
                let _ = save_lang(&base, &lang.code);
                println!("  {} Language set to {}", style("✓").green(), style(&lang.name).cyan());
            }
            36 => {
                println!("  {}", lang.goodbye);
                break;
            }
            37 => config_snapshot_menu(&config_snapshot_mgr, &instance_mgr, &base).await?,
            38 => mod_profile_switcher_menu(&mod_profile_mgr, &instance_mgr).await?,
            39 => jvm_presets_menu(&jvm_preset_mgr).await?,
            40 => log_tail_menu(&instance_mgr, &game).await?,
            41 => doctor_menu(&base, &game, &http, &auth, &version_mgr).await?,
            42 => performance_profiler_menu(&history_mgr).await?,
            43 => playtime_goals_menu(&playtime_goal_mgr, &history_mgr).await?,
            44 => world_seed_reader_menu(&instance_mgr, &game).await?,
            45 => instance_health_check_menu(&instance_mgr, &game).await?,
            46 => resource_pack_preview_menu(&texture_mgr).await?,
            47 => mod_search_history_menu(&search_history_mgr, &instance_mgr).await?,
            48 => bulk_mod_toggle_menu(&instance_mgr).await?,
            49 => changelog_viewer_menu(&http, &version_mgr).await?,
            50 => dep_graph_menu(&http, &instance_mgr, &version_mgr, &game).await?,
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
    let manifest = VersionManifest::fetch_with_cache(http, Some(&config_dir())).await?;

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
    http: &reqwest::Client,
    downloader: &Downloader,
    auth: &Authenticator,
    profiles: &ProfileManager,
    version_mgr: &VersionManager,
    texture_mgr: &TextureManager,
    shader_mgr: &ShaderManager,
    history_mgr: &HistoryManager,
    instance_mgr: &InstanceManager,
    backup_mgr: &BackupManager,
    config_mgr: &ConfigManager,
    crash_pattern_mgr: &CrashPatternManager,
    scheduled_backup_mgr: &ScheduledBackupManager,
    playtime_goal_mgr: &PlaytimeGoalManager,
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
    let (game_dir_override, inst_profile, selected_inst_name) = if instances.is_empty() {
        (None, InstanceProfile::default(), None)
    } else {
        let mut inst_labels: Vec<String> = vec!["Default (shared game dir)".into()];
        inst_labels.extend(instances.iter().map(|i| {
            let tags = if i.tags.is_empty() { String::new() } else { format!(" [{}]", i.tags.join(", ")) };
            let played = i.last_played.as_deref().and_then(|s| s.get(..10)).unwrap_or("never");
            format!("{} [{}]{} — last played: {}", i.name, i.version_id, tags, played)
        }));
        let i_idx = Select::with_theme(&theme())
            .with_prompt("Instance")
            .items(&inst_labels)
            .default(0)
            .interact()?;
        if i_idx == 0 {
            (None, InstanceProfile::default(), None)
        } else {
            let inst = &instances[i_idx - 1];
            let profile = instance_mgr.load_profile(&inst.name).await;
            (Some(instance_mgr.instance_dir(&inst.name)), profile, Some(inst.name.clone()))
        }
    };

    // Ensure assets are complete before launching
    if let Some(asset_index) = &meta.asset_index {
        println!("  {} Verifying assets...", style("→").cyan());
        downloader.download_assets(asset_index).await?;
    }

    // Conflict detection
    let mods_dir = game_dir_override.as_ref()
        .map(|d| d.join("mods"))
        .unwrap_or_else(|| game_dir.join("mods"));
    let conflict_warnings = conflicts::detect(&mods_dir);
    if !conflict_warnings.is_empty() {
        println!();
        println!("  {} Mod conflict warnings:", style("⚠").yellow().bold());
        for w in &conflict_warnings {
            let sev = match w.severity {
                conflicts::Severity::Critical => style("CRITICAL").red().bold(),
                conflicts::Severity::Warning  => style("WARNING").yellow(),
            };
            println!("    {} [{}] {}", style("•").dim(), sev, w.message);
        }
        let proceed = Confirm::with_theme(&theme())
            .with_prompt("Conflicts detected. Launch anyway?")
            .default(false)
            .interact()?;
        if !proceed { return Ok(()); }
        println!();
    }

    // Optimization profile
    let opt_profiles = OptimizationProfile::all();
    let ram_mb = optimizer::auto_heap_mb();
    let profile_labels: Vec<String> = opt_profiles
        .iter()
        .map(|p| {
            if *p == OptimizationProfile::Auto {
                format!("Auto — {}MB heap detected ({})", ram_mb, optimizer::auto_tune())
            } else {
                format!("{} — {}", p, p.description())
            }
        })
        .collect();

    let profile = if let Some(ref p) = inst_profile.optimization {
        println!("  {} Using instance optimization profile: {}", style("ℹ").cyan(), p);
        p.clone()
    } else {
        let profile_idx = Select::with_theme(&theme())
            .with_prompt("Optimization profile")
            .items(&profile_labels)
            .default(1)
            .interact()?;
        OptimizationProfile::from_index(profile_idx)
    };

    let (launch_width, launch_height) = if inst_profile.width.is_some() {
        (inst_profile.width, inst_profile.height)
    } else {
        (None, None)
    };
    let inst_jvm_args = inst_profile.custom_jvm_args.clone();

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
        if t_idx == 0 { None } else { Some(packs[t_idx - 1].name.clone()) }
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
        if s_idx == 0 { None } else { Some(presets[s_idx - 1].clone()) }
    };

    // Apply render pipeline
    let pipeline = RenderPipeline::new(texture_mgr, shader_mgr);
    pipeline
        .apply(texture_choice.as_deref(), shader_choice.as_deref(), game_dir, &era)
        .await?;

    check_java_version(&meta, &http).await;
    warn_if_low_ram(&profile);

    // Auto-backup on launch if enabled
    let cfg = config_mgr.load().await;
    if cfg.auto_backup_on_launch {
        let backup_dir = game_dir_override.as_ref().unwrap_or(game_dir);
        let inst_label = game_dir_override.as_ref()
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "default".to_string());
        print!("  {} Auto-backup... ", style("→").cyan());
        match backup_mgr.create_backup(&inst_label, backup_dir).await {
            Ok(p)  => println!("{} ({})", style("✓").green(), p.file_name().unwrap_or_default().to_string_lossy()),
            Err(e) => println!("{} {}", style("⚠").yellow(), e),
        }
    }

    // Account selection
    let mut session = pick_session(auth, profiles).await?;
    if session.auth_type == AuthType::Microsoft {
        print!("  {} Validating session... ", style("→").cyan());
        match auth.validate_session(&session).await {
            Ok(true)  => println!("{}", style("✓").green()),
            Ok(false) => {
                println!("{}", style("expired").yellow());
                println!("  {} Token expired — refreshing...", style("→").cyan());
                session = auth.try_refresh(session).await;
                println!("  {} Refreshed as {}", style("✓").green(), style(&session.username).cyan());
            }
            Err(e) => println!("{} ({})", style("skipped").dim(), e),
        }
    }
    println!(
        "  {} Playing as {} [{}]",
        style("✓").green(),
        style(&session.username).cyan(),
        match session.auth_type {
            AuthType::Local | AuthType::ElyBy => style("local").yellow(),
            AuthType::Microsoft => style("microsoft").blue(),
        }
    );

    // Webhook — game started
    if let Some(ref url) = cfg.webhook_url {
        webhook::fire(http, url, WebhookEvent::GameStarted, &meta.id, &session.username, None).await;
    }

    // Start benchmark sampler if enabled
    let benchmark_handle = if cfg.benchmark_mode {
        let effective_dir = game_dir_override.as_ref().unwrap_or(game_dir);
        println!("  {} Benchmark mode active — FPS/heap will be sampled from logs.", style("ℹ").cyan());
        Some(benchmark::start(effective_dir))
    } else {
        None
    };

    // Launch
    let launcher = GameLauncher::new(game_dir.clone());
    let opts = LaunchOptions {
        session: &session,
        profile: &profile,
        custom_jvm_args: &inst_jvm_args,
        width: launch_width,
        height: launch_height,
        server: None,
        port: None,
        game_dir_override: game_dir_override.clone(),
        launch_wrapper: cfg.launch_wrapper.as_deref(),
    };
    let mut child = launcher.launch(&meta, &opts, version_mgr)?;
    let mut discord = DiscordPresence::new();
    discord.connect();
    discord.set_playing(&meta.id, &session.username);
    let started_at = chrono::Utc::now();
    let start = std::time::Instant::now();
    // Start in-place session timer overlay on stderr.
    let timer_handle = sessiontimer::start(std::time::Duration::from_secs(60));
    let status = child.wait()?;
    timer_handle.stop();
    discord.clear();
    let duration_secs = start.elapsed().as_secs();
    let exit_code = status.code();

    // Collect benchmark stats + leak detection
    let bench_stats = benchmark_handle.map(|h| h.finish());
    if let Some(ref s) = bench_stats {
        if let Some(avg) = s.fps_avg() {
            println!(
                "  {} FPS — avg: {}  min: {}  max: {}",
                style("◆").cyan(),
                style(avg).green(),
                s.fps_min().unwrap_or(0),
                s.fps_max().unwrap_or(0),
            );
        }
        if let Some(peak) = s.peak_heap_mb() {
            println!("  {} Peak heap: {} MB", style("◆").cyan(), style(peak).yellow());
        }
        // Leak detection
        if let Some(report) = leakdetector::analyse(s, duration_secs) {
            if report.suspected {
                println!("  {} Memory leak suspected: {}", style("⚠").red().bold(), report.message);
            }
        }
    }

    // Webhook — game stopped
    if let Some(ref url) = cfg.webhook_url {
        webhook::fire(http, url, WebhookEvent::GameStopped, &meta.id, &session.username, Some(duration_secs)).await;
    }

    // Update instance last_played
    if let Some(ref inst_name) = selected_inst_name {
        let _ = instance_mgr.set_last_played(inst_name).await;
    }

    // Session note prompt
    let note_raw: String = Input::with_theme(&theme())
        .with_prompt("Session note (optional, Enter to skip)")
        .allow_empty(true)
        .interact_text()?;

    history_mgr.push(LaunchRecord {
        version_id: meta.id.clone(),
        username: session.username.clone(),
        started_at,
        duration_secs,
        exit_code,
        notes: if note_raw.trim().is_empty() { None } else { Some(note_raw.trim().to_string()) },
        fps_avg: bench_stats.as_ref().and_then(|s| s.fps_avg()),
        fps_min: bench_stats.as_ref().and_then(|s| s.fps_min()),
        fps_max: bench_stats.as_ref().and_then(|s| s.fps_max()),
        peak_heap_mb: bench_stats.as_ref().and_then(|s| s.peak_heap_mb()),
    }).await.ok();

    // Scheduled auto-backup tick.
    if cfg.scheduled_backup_hours > 0 {
        let backup_dir = game_dir_override.as_ref().unwrap_or(game_dir);
        let inst_label = game_dir_override.as_ref()
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "default".to_string());
        match scheduled_backup_mgr.tick(&inst_label, backup_dir, duration_secs, cfg.scheduled_backup_hours).await {
            Ok(Some(path)) => println!(
                "  {} Scheduled backup created: {}",
                style("✓").green(),
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            Ok(None) => {}
            Err(e) => println!("  {} Scheduled backup failed: {}", style("⚠").yellow(), e),
        }
    }

    // Playtime goal check.
    playtime_goal_mgr.check_after_session(history_mgr).await;

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
    let ms_accounts: Vec<_> = auth.load_all_sessions().await
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
                if s.is_empty() { Err("Username cannot be empty.") }
                else if s.len() > 16 { Err("Username must be 16 characters or fewer.") }
                else if !s.chars().all(|c| c.is_alphanumeric() || c == '_') { Err("Only letters, numbers, and underscores allowed.") }
                else { Ok(()) }
            })
            .interact_text()?;
        let uuid_choice = Select::with_theme(&theme())
            .with_prompt("UUID type")
            .items(&["Offline (deterministic, based on username)", "Random (new UUID each time)"])
            .default(0)
            .interact()?;
        let profile = if uuid_choice == 0 {
            launcher::auth::LocalProfile::new(name.trim().to_string())
        } else {
            launcher::auth::LocalProfile::new_random(name.trim().to_string())
        };
        profiles.add_profile(profile.clone()).await?;
        println!(
            "  {} Created local profile '{}' (UUID: {})",
            style("✓").green(), profile.username, profile.uuid
        );
        return Ok(profile.to_session());
    }

    if idx == ms_login_idx {
        println!("  {} Starting Microsoft login...", style("→").cyan());
        let session = auth.authenticate().await?;
        auth.save_session(&session).await?;
        println!("  {} Logged in as {}", style("✓").green(), style(&session.username).cyan());
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
        let ms_accounts: Vec<_> = auth.load_all_sessions().await
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
                        if s.is_empty() { Err("Username cannot be empty.") }
                        else if s.len() > 16 { Err("Username must be 16 characters or fewer.") }
                        else if !s.chars().all(|c| c.is_alphanumeric() || c == '_') { Err("Only letters, numbers, and underscores allowed.") }
                        else { Ok(()) }
                    })
                    .interact_text()?;
                let uuid_choice = Select::with_theme(&theme())
                    .with_prompt("UUID type")
                    .items(&["Offline (deterministic, based on username)", "Random (new UUID each time)"])
                    .default(0)
                    .interact()?;
                let profile = if uuid_choice == 0 {
                    launcher::auth::LocalProfile::new(name.trim().to_string())
                } else {
                    launcher::auth::LocalProfile::new_random(name.trim().to_string())
                };
                match profiles.add_profile(profile).await {
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
    http: &reqwest::Client,
    downloader: &Downloader,
    auth: &Authenticator,
    profiles: &ProfileManager,
    preset_mgr: &PresetManager,
    version_mgr: &VersionManager,
    texture_mgr: &TextureManager,
    shader_mgr: &ShaderManager,
    history_mgr: &HistoryManager,
    instance_mgr: &InstanceManager,
    backup_mgr: &BackupManager,
    config_mgr: &ConfigManager,
    crash_pattern_mgr: &CrashPatternManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let _ = crash_pattern_mgr; // reserved for future per-session recording
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

    check_java_version(&meta, &http).await;
    warn_if_low_ram(&preset.optimization);

    // Auto-backup on launch if enabled
    let cfg = config_mgr.load().await;
    if cfg.auto_backup_on_launch {
        let backup_dir = match &preset.instance {            Some(name) => instance_mgr.instance_dir(name),
            None => game_dir.clone(),
        };
        let inst_label = preset.instance.as_deref().unwrap_or("default");
        print!("  {} Auto-backup... ", style("→").cyan());
        match backup_mgr.create_backup(inst_label, &backup_dir).await {
            Ok(p) => println!("{} ({})", style("✓").green(), p.file_name().unwrap_or_default().to_string_lossy()),
            Err(e) => println!("{} {}", style("⚠").yellow(), e),
        }
    }

    let mut session = pick_session(auth, profiles).await?;
    // Session validation
    if session.auth_type == AuthType::Microsoft {
        print!("  {} Validating session... ", style("→").cyan());
        match auth.validate_session(&session).await {
            Ok(true) => println!("{}", style("✓").green()),
            Ok(false) => {
                println!("{}", style("expired").yellow());
                println!("  {} Token expired — refreshing...", style("→").cyan());
                session = auth.try_refresh(session).await;
                println!("  {} Refreshed as {}", style("✓").green(), style(&session.username).cyan());
            }
            Err(e) => println!("{} ({})", style("skipped").dim(), e),
        }
    }
    println!(
        "  {} Playing as {} [{}]",
        style("✓").green(),
        style(&session.username).cyan(),
        match session.auth_type {
            AuthType::Local | AuthType::ElyBy => style("local").yellow(),
            AuthType::Microsoft => style("microsoft").blue(),
        }
    );

    let game_dir_override = match &preset.instance {
        Some(name) => {
            let dir = instance_mgr.instance_dir(name);
            if dir.exists() { Some(dir) } else {
                println!("  {} Instance '{}' not found, using default game dir.", style("⚠").yellow(), name);
                None
            }
        }
        None => None,
    };

    // Conflict detection
    let mods_dir_check = game_dir_override.as_ref()
        .map(|d| d.join("mods"))
        .unwrap_or_else(|| game_dir.join("mods"));
    let conflict_warnings = conflicts::detect(&mods_dir_check);
    if !conflict_warnings.is_empty() {
        println!();
        println!("  {} Mod conflict warnings:", style("⚠").yellow().bold());
        for w in &conflict_warnings {
            let sev = match w.severity {
                conflicts::Severity::Critical => style("CRITICAL").red().bold(),
                conflicts::Severity::Warning  => style("WARNING").yellow(),
            };
            println!("    {} [{}] {}", style("•").dim(), sev, w.message);
        }
        let proceed = Confirm::with_theme(&theme())
            .with_prompt("Conflicts detected. Launch anyway?")
            .default(false)
            .interact()?;
        if !proceed { return Ok(()); }
        println!();
    }

    // Webhook — game started
    if let Some(ref url) = cfg.webhook_url {
        webhook::fire(http, url, WebhookEvent::GameStarted, &meta.id, &session.username, None).await;
    }

    // Benchmark sampler
    let benchmark_handle = if cfg.benchmark_mode {
        let effective_dir = game_dir_override.as_ref().unwrap_or(game_dir);
        println!("  {} Benchmark mode active.", style("ℹ").cyan());
        Some(benchmark::start(effective_dir))
    } else {
        None
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
        game_dir_override: game_dir_override.clone(),
        launch_wrapper: cfg.launch_wrapper.as_deref(),
    };
    let mut child = launcher.launch(&meta, &opts, version_mgr)?;

    println!(
        "  {} Game launched (PID {}). Waiting for exit...",
        style("✓").green(),
        child.id()
    );
    let mut discord = DiscordPresence::new();
    discord.connect();
    discord.set_playing(&meta.id, &session.username);
    let started_at = chrono::Utc::now();
    let start = std::time::Instant::now();
    let status = child.wait()?;
    discord.clear();
    let duration_secs = start.elapsed().as_secs();
    let exit_code = status.code();

    let bench_stats = benchmark_handle.map(|h| h.finish());
    if let Some(ref s) = bench_stats {
        if let Some(avg) = s.fps_avg() {
            println!("  {} FPS — avg: {}  min: {}  max: {}", style("◆").cyan(),
                style(avg).green(), s.fps_min().unwrap_or(0), s.fps_max().unwrap_or(0));
        }
        if let Some(peak) = s.peak_heap_mb() {
            println!("  {} Peak heap: {} MB", style("◆").cyan(), style(peak).yellow());
        }
    }

    // Webhook — game stopped
    if let Some(ref url) = cfg.webhook_url {
        webhook::fire(http, url, WebhookEvent::GameStopped, &meta.id, &session.username, Some(duration_secs)).await;
    }

    // Update instance last_played
    if let Some(ref inst_name) = preset.instance {
        let _ = instance_mgr.set_last_played(inst_name).await;
    }

    // Session note
    let note_raw: String = Input::with_theme(&theme())
        .with_prompt("Session note (optional, Enter to skip)")
        .allow_empty(true)
        .interact_text()?;

    history_mgr.push(LaunchRecord {
        version_id: meta.id.clone(),
        username: session.username.clone(),
        started_at,
        duration_secs,
        exit_code,
        notes: if note_raw.trim().is_empty() { None } else { Some(note_raw.trim().to_string()) },
        fps_avg: bench_stats.as_ref().and_then(|s| s.fps_avg()),
        fps_min: bench_stats.as_ref().and_then(|s| s.fps_min()),
        fps_max: bench_stats.as_ref().and_then(|s| s.fps_max()),
        peak_heap_mb: bench_stats.as_ref().and_then(|s| s.peak_heap_mb()),
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
    let ram_mb = optimizer::auto_heap_mb();
    let opt_labels: Vec<String> = opt_list
        .iter()
        .map(|p| {
            if *p == OptimizationProfile::Auto {
                format!("Auto — {}MB heap detected ({})", ram_mb, optimizer::auto_tune())
            } else {
                format!("{} — {}", p, p.description())
            }
        })
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
        // FPS stats
        if let Some(avg) = r.fps_avg {
            println!(
                "       FPS  avg: {}  min: {}  max: {}",
                style(avg).green(),
                r.fps_min.unwrap_or(0),
                r.fps_max.unwrap_or(0),
            );
        }
        // Heap peak
        if let Some(peak) = r.peak_heap_mb {
            println!("       Peak heap: {} MB", style(peak).yellow());
        }
        // Session note
        if let Some(ref note) = r.notes {
            println!("       Note: {}", style(note).italic().dim());
        }
    }
    Ok(())
}

// ── Crash Log Viewer ──────────────────────────────────────────────────────────

fn show_latest_crash_report(game_dir: &PathBuf) {
    use launcher::crash;

    let Some(path) = crash::find_latest(game_dir) else { return };
    let Some(report) = crash::parse(&path) else {
        println!("  {} Could not parse crash report.", style("⚠").red());
        return;
    };

    println!();
    println!("  {} Crash detected: {}", style("⚠").red().bold(), style(&report.description).red());
    println!("  {}", style("─".repeat(60)).dim());

    if let Some(ref ex) = report.exception {
        println!("  {} {}", style("Exception:").yellow(), style(ex).red());
    }

    if !report.suspected_mods.is_empty() {
        println!();
        println!("  {} Suspected mods / classes in stack trace:", style("◆").yellow());
        for m in report.suspected_mods.iter().take(5) {
            println!("    {} {}", style("•").dim(), style(m).yellow());
        }
    }

    println!();
    println!("  {} Diagnos{}:", style("◆").cyan(), if report.diagnoses.len() == 1 { "is" } else { "es" });
    for d in &report.diagnoses {
        println!("    {} {}", style("•").red(), style(d.cause).red().bold());
        println!("      {} {}", style("→").cyan(), d.fix);
    }

    println!();
    println!("  Full report: {}", style(report.path.display()).dim());
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
            let notes_str = if i.notes.is_empty() { String::new() } else { format!("  — {}", style(&i.notes).dim()) };
            let tags_str  = if i.tags.is_empty()  { String::new() } else { format!("  [{}]", i.tags.join(", ")) };
            let played    = i.last_played.as_deref().and_then(|s| s.get(..10)).unwrap_or("never");
            println!("  • {} [{}]  last: {}{}{}", style(&i.name).cyan(), i.version_id, played, tags_str, notes_str);
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Instance Manager")
            .items(&["Create instance", "Clone instance", "Delete instance", "Edit profile", "Edit notes", "Manage tags", "Manage mod profile", "Backup instance", "Restore backup", "Export instance", "Import instance", "Back"])
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
                if instances.is_empty() { println!("  No instances to clone."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Instance to clone").items(&labels).default(0).interact()?;
                let new_name: String = Input::with_theme(&theme())
                    .with_prompt("New instance name")
                    .validate_with(|s: &String| if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) })
                    .interact_text()?;
                println!("  {} Cloning '{}' → '{}'...", style("→").cyan(), instances[i].name, new_name.trim());
                match instance_mgr.clone_instance(&instances[i].name, new_name.trim()).await {
                    Ok(inst) => println!("  {} Cloned as '{}'", style("✓").green(), inst.name),
                    Err(e)   => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
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
            3 => {
                if instances.is_empty() { println!("  No instances to edit."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select instance")
                    .items(&labels).default(0).interact()?;
                let name = &instances[i].name;
                let current = instance_mgr.load_profile(name).await;
                let updated = edit_instance_profile(name, current).await?;
                match instance_mgr.save_profile(name, &updated).await {
                    Ok(_)  => println!("  {} Profile saved for '{}'.", style("✓").green(), name),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            4 => {
                if instances.is_empty() { println!("  No instances."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select instance").items(&labels).default(0).interact()?;
                let current_notes = instances[i].notes.clone();
                println!("  Current notes: {}", if current_notes.is_empty() { style("(none)".to_string()).dim().to_string() } else { current_notes.clone() });
                let notes: String = Input::with_theme(&theme())
                    .with_prompt("Notes (leave blank to clear)")
                    .with_initial_text(&current_notes)
                    .allow_empty(true)
                    .interact_text()?;
                match instance_mgr.set_notes(&instances[i].name, notes.trim()).await {
                    Ok(_)  => println!("  {} Notes saved.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            5 => {
                // Manage tags
                if instances.is_empty() { println!("  No instances."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select instance").items(&labels).default(0).interact()?;
                let current_tags = instances[i].tags.join(", ");
                println!("  Current tags: {}", if current_tags.is_empty() { style("(none)".to_string()).dim().to_string() } else { current_tags.clone() });
                let tags_raw: String = Input::with_theme(&theme())
                    .with_prompt("Tags (comma-separated, blank to clear)")
                    .with_initial_text(&current_tags)
                    .allow_empty(true)
                    .interact_text()?;
                let new_tags: Vec<String> = tags_raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match instance_mgr.set_tags(&instances[i].name, new_tags).await {
                    Ok(_)  => println!("  {} Tags saved.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            6 => {
                if instances.is_empty() { println!("  No instances."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select instance").items(&labels).default(0).interact()?;
                manage_mod_profile(&instance_mgr, &instances[i].name).await?;
            }
            7 => {
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
            8 => {
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
            9 => {
                if instances.is_empty() { println!("  No instances to export."); continue; }
                let labels: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Select instance to export")
                    .items(&labels).default(0).interact()?;
                let dest: String = Input::with_theme(&theme())
                    .with_prompt("Save zip to path")
                    .with_initial_text(format!("{}.zip", instances[i].name))
                    .interact_text()?;
                match instance_mgr.export(&instances[i].name, &std::path::PathBuf::from(dest.trim())).await {
                    Ok(_)  => println!("  {} Exported.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            10 => {
                let src: String = Input::with_theme(&theme())
                    .with_prompt("Path to instance zip")
                    .interact_text()?;
                match instance_mgr.import(&std::path::PathBuf::from(src.trim())).await {
                    Ok(inst) => println!("  {} Imported instance '{}'", style("✓").green(), inst.name),
                    Err(e)   => println!("  {} {}", style("✗").red(), e),
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

// ── Java version mismatch warning + auto-install ──────────────────────────────

async fn check_java_version(meta: &launcher::manifest::VersionMeta, http: &reqwest::Client) {
    use client::injection::VersionEra;

    let era = VersionEra::detect(&meta.id, &meta.version_type, meta);
    let required = if era.requires_java8() {
        8
    } else {
        meta.java_version.as_ref().map(|j| j.major_version).unwrap_or(21)
    };

    // Check if we already have a matching Java
    let found = find_java_for_major(required)
        .and_then(|p| detect_java_major(&p).filter(|&v| v == required).map(|_| p));

    if found.is_some() {
        return; // correct version present, nothing to do
    }

    println!();
    println!(
        "  {} Java {} not found for {}. Attempting automatic download...",
        style("⚠").yellow().bold(), required, meta.id
    );

    let java_dir = base_dir().join("java").join(required.to_string());
    match try_auto_install_java(http, required, &java_dir).await {
        Ok(java_bin) => {
            // Point the env var so find_java_for_major picks it up for the actual launch
            let env_key = match required {
                8  => "JAVA8_HOME",
                21 => "JAVA21_HOME",
                25 => "JAVA25_HOME",
                _  => "JAVA_HOME",
            };
            std::env::set_var(env_key, java_dir);
            println!(
                "  {} Java {} installed at {}",
                style("✓").green(), required, java_bin.display()
            );
        }
        Err(e) => {
            println!(
                "  {} Auto-install failed: {}",
                style("✗").red(), e
            );
            println!(
                "  {} Please download Java {} manually:",
                style("→").cyan(), required
            );
            println!("     {}", style(java_download_url(required)).cyan().underlined());
            let _ = open::that(java_download_url(required));
            println!();
        }
    }
}

// ── Instance Profile Editor ─────────────────────────────────────────────────────

async fn edit_instance_profile(
    instance_name: &str,
    current: InstanceProfile,
) -> Result<InstanceProfile> {
    println!();
    println!("  {} Instance profile: {}", style("◆").cyan(), style(instance_name).cyan().bold());
    println!("  Leave fields blank / unchanged to keep current values.");
    println!();

    // ── Optimization profile ─────────────────────────────────────────────────────
    let opt_list = OptimizationProfile::all();
    let ram_mb = optimizer::auto_heap_mb();
    let mut opt_labels: Vec<String> = vec!["Inherit (use global setting)".into()];
    opt_labels.extend(opt_list.iter().map(|p| {
        if *p == OptimizationProfile::Auto {
            format!("Auto — {}MB heap detected ({})", ram_mb, optimizer::auto_tune())
        } else {
            format!("{} — {}", p, p.description())
        }
    }));
    let default_o = current.optimization.as_ref()
        .and_then(|o| opt_list.iter().position(|p| p == o).map(|i| i + 1))
        .unwrap_or(0);
    let o_idx = Select::with_theme(&theme())
        .with_prompt("Optimization profile")
        .items(&opt_labels)
        .default(default_o)
        .interact()?;
    let optimization = if o_idx == 0 { None } else { Some(OptimizationProfile::from_index(o_idx - 1)) };

    // ── Java override ───────────────────────────────────────────────────────────────
    let java_raw: String = Input::with_theme(&theme())
        .with_prompt("Java binary path override (leave blank to auto-detect)")
        .with_initial_text(current.java_path.as_deref().unwrap_or(""))
        .allow_empty(true)
        .interact_text()?;
    let java_path = if java_raw.trim().is_empty() { None } else { Some(java_raw.trim().to_string()) };

    // ── Resolution ──────────────────────────────────────────────────────────────────
    let use_res = Confirm::with_theme(&theme())
        .with_prompt("Set custom resolution for this instance?")
        .default(current.width.is_some())
        .interact()?;
    let (width, height) = if use_res {
        let w: String = Input::with_theme(&theme())
            .with_prompt("Width")
            .with_initial_text(current.width.map(|v| v.to_string()).unwrap_or_else(|| "1280".into()))
            .validate_with(|s: &String| s.trim().parse::<u32>().map(|_| ()).map_err(|_| "Must be a number"))
            .interact_text()?;
        let h: String = Input::with_theme(&theme())
            .with_prompt("Height")
            .with_initial_text(current.height.map(|v| v.to_string()).unwrap_or_else(|| "720".into()))
            .validate_with(|s: &String| s.trim().parse::<u32>().map(|_| ()).map_err(|_| "Must be a number"))
            .interact_text()?;
        (Some(w.trim().parse::<u32>().unwrap()), Some(h.trim().parse::<u32>().unwrap()))
    } else {
        (None, None)
    };

    // ── Custom JVM args ───────────────────────────────────────────────────────────────
    let jvm_raw: String = Input::with_theme(&theme())
        .with_prompt("Extra JVM args (space-separated, leave blank for none)")
        .with_initial_text(&current.custom_jvm_args.join(" "))
        .allow_empty(true)
        .interact_text()?;
    let custom_jvm_args: Vec<String> = jvm_raw.split_whitespace().map(|s| s.to_string()).collect();

    Ok(InstanceProfile { optimization, java_path, width, height, custom_jvm_args })
}

// ── Mod Loader Installer ─────────────────────────────────────────────────────

async fn install_mod_loader(
    http: &reqwest::Client,
    version_mgr: &VersionManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let installed = version_mgr.list_installed().await?;
    if installed.is_empty() {
        println!("  No versions installed. Install a Minecraft version first.");
        return Ok(());
    }

    let version_labels: Vec<String> = installed
        .iter()
        .map(|v| format!("{} [{}]", v.id, v.version_type))
        .collect();
    let v_idx = Select::with_theme(&theme())
        .with_prompt("Minecraft version")
        .items(&version_labels)
        .default(0)
        .interact()?;
    let mc_version = &installed[v_idx].id;

    let loader_choice = Select::with_theme(&theme())
        .with_prompt("Mod loader")
        .items(&["Fabric", "Forge", "Quilt", "NeoForge"])
        .default(0)
        .interact()?;

    match loader_choice {
        0 => {
            println!("  {} Fetching Fabric loader versions...", style("→").cyan());
            let versions = match loader::fabric_loader_versions(http, mc_version).await {
                Ok(v) => v,
                Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
            };
            if versions.is_empty() {
                println!("  No Fabric loader versions found for {}.", mc_version);
                return Ok(());
            }
            let l_idx = Select::with_theme(&theme())
                .with_prompt("Loader version")
                .items(&versions)
                .default(0)
                .interact()?;
            println!("  {} Installing Fabric {}...", style("→").cyan(), versions[l_idx]);
            match loader::install_fabric(http, game_dir, mc_version, &versions[l_idx]).await {
                Ok(id) => println!("  {} Installed as version '{}'. Select it in Launch Game.", style("✓").green(), id),
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
        1 => {
            println!("  {} Fetching Forge versions...", style("→").cyan());
            let versions = match loader::forge_versions(http, mc_version).await {
                Ok(v) => v,
                Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
            };
            if versions.is_empty() {
                println!("  No Forge versions found for {}.", mc_version);
                return Ok(());
            }
            let f_idx = Select::with_theme(&theme())
                .with_prompt("Forge version")
                .items(&versions)
                .default(0)
                .interact()?;
            println!("  {} Running Forge installer (this may take a minute)...", style("→").cyan());
            match loader::install_forge(http, game_dir, &versions[f_idx]).await {
                Ok(id) => println!("  {} Installed as version '{}'. Select it in Launch Game.", style("✓").green(), id),
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
        2 => {
            println!("  {} Fetching Quilt loader versions...", style("→").cyan());
            let versions = match loader::quilt_loader_versions(http, mc_version).await {
                Ok(v) => v,
                Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
            };
            if versions.is_empty() {
                println!("  No Quilt loader versions found for {}.", mc_version);
                return Ok(());
            }
            let l_idx = Select::with_theme(&theme())
                .with_prompt("Loader version")
                .items(&versions)
                .default(0)
                .interact()?;
            println!("  {} Installing Quilt {}...", style("→").cyan(), versions[l_idx]);
            match loader::install_quilt(http, game_dir, mc_version, &versions[l_idx]).await {
                Ok(id) => println!("  {} Installed as version '{}'. Select it in Launch Game.", style("✓").green(), id),
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
        _ => {
            println!("  {} Fetching NeoForge versions...", style("→").cyan());
            let versions = match loader::neoforge_versions(http, mc_version).await {
                Ok(v) => v,
                Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
            };
            if versions.is_empty() {
                println!("  No NeoForge versions found for {}. NeoForge only supports Minecraft 1.20.2+.", mc_version);
                return Ok(());
            }
            let nf_idx = Select::with_theme(&theme())
                .with_prompt("NeoForge version")
                .items(&versions)
                .default(0)
                .interact()?;
            println!("  {} Running NeoForge installer (this may take a minute)...", style("→").cyan());
            match loader::install_neoforge(http, game_dir, mc_version, &versions[nf_idx]).await {
                Ok(id) => println!("  {} Installed as version '{}'. Select it in Launch Game.", style("✓").green(), id),
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
    }
    Ok(())
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
                    Ok(path) => {
                        println!("  {} Installed to {}", style("✓").green(), path.display());
                        // Resolve and install required dependencies
                        let already = ModManager::list_installed(&mods_dir).await.unwrap_or_default();
                        match mod_mgr.resolve_dependencies(&versions[v], &game_version, &mods_dir, &already).await {
                            Ok(deps) if !deps.is_empty() => println!("  {} Installed {} dependenc{}.", style("✓").green(), deps.len(), if deps.len() == 1 { "y" } else { "ies" }),
                            Ok(_) => {}
                            Err(e) => println!("  {} Dependency resolution failed: {}", style("⚠").yellow(), e),
                        }
                    }
                    Err(e) => println!("  {} {}", style("✗").red(), e),
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

// ── RAM Warning ───────────────────────────────────────────────────────────────

fn warn_if_low_ram(profile: &OptimizationProfile) {
    let required_mb: u64 = match profile {
        OptimizationProfile::Quality     => 6144,
        OptimizationProfile::Performance => 4096,
        OptimizationProfile::Balanced    => 2048,
        OptimizationProfile::Potato      => 512,
        OptimizationProfile::Auto        => optimizer::auto_heap_mb() as u64,
    };
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let free_mb = sys.available_memory() / 1024 / 1024;
    if free_mb < required_mb {
        println!();
        println!(
            "  {} Low RAM: profile needs {}MB but only {}MB is free.",
            style("⚠").yellow().bold(), required_mb, free_mb
        );
        println!("     Consider switching to a lighter optimization profile.");
        println!();
    }
}

// ── Skin Manager ──────────────────────────────────────────────────────────────

async fn manage_skins(
    skin_mgr: &SkinManager,
    auth: &Authenticator,
    profiles: &ProfileManager,
) -> Result<()> {
    let session = pick_session(auth, profiles).await?;
    if session.auth_type != launcher::auth::AuthType::Microsoft {
        println!("  {} Skin management requires a Microsoft account.", style("✗").red());
        return Ok(());
    }

    let token = session.effective_token();
    let choice = Select::with_theme(&theme())
        .with_prompt("Skin Manager")
        .items(&["View current skin", "Upload skin", "Reset to default", "Back"])
        .default(0)
        .interact()?;
    match choice {
        0 => {
            match skin_mgr.get_profile(token).await {
                Ok(p) => {
                    println!("  {} {}", style("Player:").dim(), style(&p.name).cyan());
                    if let Some(skin) = p.skins.iter().find(|s| s.state == "ACTIVE") {
                        println!("  {} {}", style("Skin URL:").dim(), style(&skin.url).cyan());
                        println!("  {} {}", style("Variant:").dim(), &skin.variant);
                    } else {
                        println!("  No active skin found.");
                    }
                }
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
        1 => {
            let path_str: String = Input::with_theme(&theme()).with_prompt("Path to skin PNG").interact_text()?;
            let variant_idx = Select::with_theme(&theme()).with_prompt("Skin variant").items(&["Classic (Steve)", "Slim (Alex)"]).default(0).interact()?;
            let variant = if variant_idx == 0 { "classic" } else { "slim" };
            match skin_mgr.upload_skin(token, &PathBuf::from(path_str.trim()), variant).await {
                Ok(_)  => println!("  {} Skin uploaded.", style("✓").green()),
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
        2 => {
            if Confirm::with_theme(&theme()).with_prompt("Reset skin to default?").default(false).interact()? {
                match skin_mgr.reset_skin(token, &session.uuid).await {
                    Ok(_)  => println!("  {} Skin reset.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Mod Update Checker ────────────────────────────────────────────────────────

async fn check_mod_updates(
    http: &reqwest::Client,
    instance_mgr: &InstanceManager,
    version_mgr: &VersionManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let mods_dir = if instances.is_empty() {
        game_dir.join("mods")
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme()).with_prompt("Check updates for").items(&labels).default(0).interact()?;
        if i == 0 { game_dir.join("mods") } else { instance_mgr.instance_dir(&instances[i - 1].name).join("mods") }
    };
    let installed = version_mgr.list_installed().await?;
    let game_version = if installed.is_empty() { "1.21".to_string() } else {
        let labels: Vec<String> = installed.iter().map(|v| v.id.clone()).collect();
        let i = Select::with_theme(&theme()).with_prompt("Game version").items(&labels).default(0).interact()?;
        installed[i].id.clone()
    };
    let loader_idx = Select::with_theme(&theme()).with_prompt("Mod loader").items(&["fabric", "forge", "quilt"]).default(0).interact()?;
    let loader = ["fabric", "forge", "quilt"][loader_idx];
    println!("  {} Checking for updates...", style("→").cyan());
    let updates = match mod_updates::check_updates(http, &mods_dir, &game_version, loader).await {
        Ok(u) => u,
        Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
    };
    if updates.is_empty() {
        println!("  {} All mods are up to date.", style("✓").green());
        return Ok(());
    }
    println!();
    println!("  {} {} update(s) available:", style("◆").cyan(), updates.len());
    for u in &updates {
        println!("  • {} {} → {}", style(&u.filename).cyan(), style(&u.current_version).dim(), style(&u.latest_version).green());
        // Show changelog if present
        if !u.changelog.is_empty() {
            println!("    {} Changelog:", style("◆").dim());
            for line in u.changelog.lines().take(8) {
                if !line.trim().is_empty() {
                    println!("      {}", style(line.trim()).dim());
                }
            }
        }
        println!();
    }
    if Confirm::with_theme(&theme()).with_prompt("Update all?").default(true).interact()? {
        for u in &updates {
            print!("  {} Updating {}... ", style("→").cyan(), u.filename);
            match mod_updates::apply_update(http, &mods_dir, u).await {
                Ok(_)  => println!("{}", style("✓").green()),
                Err(e) => println!("{} {}", style("✗").red(), e),
            }
        }
    }
    Ok(())
}

// ── World Manager ────────────────────────────────────────────────────────────

async fn manage_worlds(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let saves_dir = if instances.is_empty() {
        game_dir.join("saves")
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme()).with_prompt("Worlds from").items(&labels).default(0).interact()?;
        if i == 0 { game_dir.join("saves") } else { instance_mgr.instance_dir(&instances[i - 1].name).join("saves") }
    };

    loop {
        let worlds = WorldManager::list(&saves_dir).await?;
        println!();
        println!("  {} Worlds ({}) — {}", style("◆").cyan(), worlds.len(), saves_dir.display());
        for w in &worlds {
            let played = w.last_played.as_deref().unwrap_or("unknown");
            println!("  • {}  {}", style(&w.name).cyan(), style(played).dim());
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("World Manager")
            .items(&["Rename world", "Delete world", "Export world (zip)", "Open saves folder", "World Info (seed/mode/day)", "Back"])
            .default(0)
            .interact()?;

        if worlds.is_empty() && choice < 4 {
            println!("  No worlds found.");
            continue;
        }

        match choice {
            0 => {
                let labels: Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select world").items(&labels).default(0).interact()?;
                let new_name: String = Input::with_theme(&theme())
                    .with_prompt("New name")
                    .validate_with(|s: &String| if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) })
                    .interact_text()?;
                match WorldManager::rename(&saves_dir, &worlds[i].name, new_name.trim()).await {
                    Ok(_)  => println!("  {} Renamed.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                let labels: Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select world").items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Permanently delete '{}'?", worlds[i].name))
                    .default(false).interact()?;
                if confirm {
                    match WorldManager::delete(&saves_dir, &worlds[i].name).await {
                        Ok(_)  => println!("  {} Deleted.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            2 => {
                let labels: Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select world").items(&labels).default(0).interact()?;
                let dest: String = Input::with_theme(&theme())
                    .with_prompt("Save zip to path")
                    .with_initial_text(format!("{}.zip", worlds[i].name))
                    .interact_text()?;
                match WorldManager::export(&saves_dir, &worlds[i].name, &PathBuf::from(dest.trim())).await {
                    Ok(_)  => println!("  {} Exported.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            3 => { let _ = open::that(&saves_dir); }
            4 => {
                // World Info — seed, game mode, day
                if worlds.is_empty() { println!("  No worlds found."); continue; }
                let labels: Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select world").items(&labels).default(0).interact()?;
                let world_dir = &worlds[i].path;
                let info = worldinfo::read(world_dir);
                println!();
                println!("  {} World Info — {}", style("◆").cyan(), style(&info.name).cyan().bold());
                println!("  Seed      : {}", info.seed.map(|s| s.to_string()).unwrap_or_else(|| "unknown".into()));
                println!("  Game mode : {}", style(info.game_mode_name()).cyan());
                println!("  Day       : {}", info.day.map(|d| d.to_string()).unwrap_or_else(|| "unknown".into()));
                if let Some(dv) = info.data_version {
                    println!("  Data ver  : {}", style(dv).dim());
                }
                println!();
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Mod Profile Manager ──────────────────────────────────────────────────────

async fn manage_mod_profile(instance_mgr: &InstanceManager, instance: &str) -> Result<()> {
    loop {
        let mods_dir = instance_mgr.instance_dir(instance).join("mods");
        let mp = instance_mgr.load_mod_profile(instance).await;

        // List active jars
        let mut active: Vec<String> = Vec::new();
        let mut disabled: Vec<String> = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(&mods_dir).await {
            while let Ok(Some(e)) = rd.next_entry().await {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".jar.disabled") {
                    disabled.push(name.trim_end_matches(".disabled").to_string());
                } else if name.ends_with(".jar") {
                    active.push(name);
                }
            }
        }
        active.sort(); disabled.sort();

        println!();
        println!("  {} Mod Profile: {}", style("◆").cyan(), style(instance).cyan().bold());
        println!("  Active ({}):", active.len());
        for m in &active { println!("    {} {}", style("•").green(), m); }
        println!("  Disabled ({}):", disabled.len());
        for m in &disabled { println!("    {} {}", style("•").dim(), m); }
        println!();
        let _ = mp;

        let choice = Select::with_theme(&theme())
            .with_prompt("Mod Profile")
            .items(&["Disable a mod", "Enable a mod", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                if active.is_empty() { println!("  No active mods."); continue; }
                let i = Select::with_theme(&theme()).with_prompt("Disable mod").items(&active).default(0).interact()?;
                match instance_mgr.disable_mod(instance, &active[i]).await {
                    Ok(_)  => println!("  {} Disabled '{}'.", style("✓").green(), active[i]),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if disabled.is_empty() { println!("  No disabled mods."); continue; }
                let i = Select::with_theme(&theme()).with_prompt("Enable mod").items(&disabled).default(0).interact()?;
                match instance_mgr.enable_mod(instance, &disabled[i]).await {
                    Ok(_)  => println!("  {} Enabled '{}'.", style("✓").green(), disabled[i]),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Server Browser ────────────────────────────────────────────────────────────

async fn server_browser_menu(browser: &ServerBrowser) -> Result<()> {
    loop {
        let servers = browser.load().await?;
        println!();
        println!("  {} Server Browser ({})", style("◆").cyan(), servers.len());
        for (idx, s) in servers.iter().enumerate() {
            let ping = ServerBrowser::ping(s)
                .map(|ms| format!("{}ms", ms))
                .unwrap_or_else(|| style("offline").red().to_string());
            println!("  {}. {} — {}  [{}]", idx + 1, style(&s.name).cyan(), s.address, ping);
            if !s.notes.is_empty() { println!("     {}", style(&s.notes).dim()); }
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Server Browser")
            .items(&["Add server", "Remove server", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let name: String = Input::with_theme(&theme()).with_prompt("Server name").interact_text()?;
                let address: String = Input::with_theme(&theme()).with_prompt("Address (host or host:port)").interact_text()?;
                let notes: String = Input::with_theme(&theme()).with_prompt("Notes (optional)").allow_empty(true).interact_text()?;
                browser.add(launcher::servers::ServerEntry {
                    name: name.trim().to_string(),
                    address: address.trim().to_string(),
                    port: 0,
                    notes: notes.trim().to_string(),
                }).await?;
                println!("  {} Added.", style("✓").green());
            }
            1 => {
                if servers.is_empty() { println!("  No servers saved."); continue; }
                let labels: Vec<String> = servers.iter().map(|s| format!("{} ({})", s.name, s.address)).collect();
                let i = Select::with_theme(&theme()).with_prompt("Remove server").items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme()).with_prompt(format!("Remove '{}'?", servers[i].name)).default(false).interact()?;
                if confirm { browser.remove(i).await?; println!("  {} Removed.", style("✓").green()); }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Modpack Installer ─────────────────────────────────────────────────────────

async fn install_modpack(
    installer: &ModpackInstaller,
    instance_mgr: &InstanceManager,
    version_mgr: &VersionManager,
) -> Result<()> {
    let installed = version_mgr.list_installed().await?;
    let game_version = if installed.is_empty() {
        Input::with_theme(&theme()).with_prompt("Game version (e.g. 1.21.1)").interact_text()?
    } else {
        let labels: Vec<String> = installed.iter().map(|v| v.id.clone()).collect();
        let i = Select::with_theme(&theme()).with_prompt("Game version").items(&labels).default(0).interact()?;
        installed[i].id.clone()
    };

    let query: String = Input::with_theme(&theme())
        .with_prompt("Search Modrinth modpacks")
        .validate_with(|s: &String| if s.trim().is_empty() { Err("Query cannot be empty.") } else { Ok(()) })
        .interact_text()?;

    println!("  {} Searching...", style("→").cyan());
    let hits = match installer.search(query.trim(), &game_version).await {
        Ok(h) => h,
        Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
    };
    if hits.is_empty() { println!("  No modpacks found."); return Ok(()); }

    let hit_labels: Vec<String> = hits.iter()
        .map(|h| format!("{} — {} ({} downloads)", h.title, h.description.chars().take(50).collect::<String>(), h.downloads))
        .collect();
    let h = Select::with_theme(&theme()).with_prompt("Select modpack").items(&hit_labels).default(0).interact()?;

    let versions = match installer.get_versions(&hits[h].project_id, &game_version).await {
        Ok(v) => v,
        Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
    };
    if versions.is_empty() { println!("  No compatible versions found."); return Ok(()); }

    let ver_labels: Vec<String> = versions.iter()
        .map(|v| format!("{} ({})", v.name, v.game_versions.join(", ")))
        .collect();
    let v = Select::with_theme(&theme()).with_prompt("Select version").items(&ver_labels).default(0).interact()?;

    let inst_name: String = Input::with_theme(&theme())
        .with_prompt("Instance name for this modpack")
        .with_initial_text(&hits[h].title.replace(' ', "-").to_lowercase())
        .validate_with(|s: &String| if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) })
        .interact_text()?;

    let inst_dir = instance_mgr.instance_dir(inst_name.trim());
    match installer.install_mrpack(&versions[v], &inst_dir).await {
        Ok((mc_ver, loaders)) => {
            println!("  {} Modpack installed to '{}'", style("✓").green(), inst_name.trim());
            println!("     Minecraft: {}", mc_ver);
            if let Some(f) = loaders.fabric   { println!("     Fabric loader: {}", f); }
            if let Some(f) = loaders.forge    { println!("     Forge: {}", f); }
            if let Some(f) = loaders.quilt    { println!("     Quilt loader: {}", f); }
            if let Some(f) = loaders.neoforge { println!("     NeoForge: {}", f); }
            println!("  {} Install the matching Minecraft version and mod loader, then create an instance pointing to this directory.", style("ℹ").cyan());
        }
        Err(e) => println!("  {} {}", style("✗").red(), e),
    }
    Ok(())
}

// ── Settings / Config GUI ─────────────────────────────────────────────────────

async fn settings_menu(config_mgr: &ConfigManager) -> Result<()> {
    loop {
        let mut cfg = config_mgr.load().await;
        println!();
        println!("  {} Settings", style("◆").cyan());
        println!("  Default optimization : {}", style(&cfg.default_optimization).cyan());
        println!("  Auto-backup on launch: {}", if cfg.auto_backup_on_launch { style("on").green() } else { style("off").dim() });
        println!("  Discord RPC          : {}", if cfg.discord_rpc { style("on").green() } else { style("off").dim() });
        println!("  Check updates        : {}", if cfg.check_updates_on_start { style("on").green() } else { style("off").dim() });
        println!("  Benchmark mode       : {}", if cfg.benchmark_mode { style("on").green() } else { style("off").dim() });
        println!("  Webhook URL          : {}", cfg.webhook_url.as_deref().unwrap_or("(not set)"));
        println!("  Launch wrapper       : {}", cfg.launch_wrapper.as_deref().unwrap_or("(none)"));
        println!("  Scheduled backup     : {}", if cfg.scheduled_backup_hours == 0 { "off".into() } else { format!("every {}h of playtime", cfg.scheduled_backup_hours) });
        println!("  Default resolution   : {}",
            match (cfg.default_width, cfg.default_height) {
                (Some(w), Some(h)) => format!("{}x{}", w, h),
                _ => "default".into(),
            }
        );
        println!("  Java 8 path          : {}", cfg.java8_path.as_deref().unwrap_or("auto"));
        println!("  Java 21 path         : {}", cfg.java21_path.as_deref().unwrap_or("auto"));
        println!("  Java 25 path         : {}", cfg.java25_path.as_deref().unwrap_or("auto"));
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Settings")
            .items(&[
                "Default optimization profile",
                "Toggle auto-backup on launch",
                "Toggle Discord RPC",
                "Toggle update check on start",
                "Toggle benchmark mode (FPS + heap sampling)",
                "Set webhook URL",
                "Set launch wrapper (mangohud, gamescope, etc.)",
                "Set scheduled backup interval",
                "Set default resolution",
                "Set Java 8 path",
                "Set Java 21 path",
                "Set Java 25 path",
                "Back",
            ])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let opts = OptimizationProfile::all();
                let labels: Vec<String> = opts.iter().map(|p| format!("{} — {}", p, p.description())).collect();
                let cur = opts.iter().position(|p| p == &cfg.default_optimization).unwrap_or(0);
                let i = Select::with_theme(&theme()).with_prompt("Default profile").items(&labels).default(cur).interact()?;
                cfg.default_optimization = OptimizationProfile::from_index(i);
            }
            1 => { cfg.auto_backup_on_launch = !cfg.auto_backup_on_launch; }
            2 => { cfg.discord_rpc = !cfg.discord_rpc; }
            3 => { cfg.check_updates_on_start = !cfg.check_updates_on_start; }
            4 => {
                cfg.benchmark_mode = !cfg.benchmark_mode;
                if cfg.benchmark_mode {
                    println!("  {} Benchmark mode on — FPS and heap will be sampled from logs/latest.log during each session.", style("ℹ").cyan());
                }
            }
            5 => {
                println!("  Enter the URL to receive POST notifications on game start/stop.");
                println!("  Works with Discord webhooks, Slack, ntfy.sh, or any HTTP endpoint.");
                println!("  Leave blank to disable.");
                let url: String = Input::with_theme(&theme())
                    .with_prompt("Webhook URL")
                    .with_initial_text(cfg.webhook_url.as_deref().unwrap_or(""))
                    .allow_empty(true)
                    .interact_text()?;
                cfg.webhook_url = if url.trim().is_empty() { None } else { Some(url.trim().to_string()) };
            }
            6 => {
                let w: String = Input::with_theme(&theme())
                    .with_prompt("Launch wrapper binary (e.g. mangohud, gamescope; blank to disable)")
                    .with_initial_text(cfg.launch_wrapper.as_deref().unwrap_or(""))
                    .allow_empty(true)
                    .interact_text()?;
                cfg.launch_wrapper = if w.trim().is_empty() { None } else { Some(w.trim().to_string()) };
            }
            7 => {
                let h: String = Input::with_theme(&theme())
                    .with_prompt("Backup every N hours of playtime (0 = off)")
                    .with_initial_text(&cfg.scheduled_backup_hours.to_string())
                    .validate_with(|s: &String| s.trim().parse::<u64>().map(|_| ()).map_err(|_| "Must be a number"))
                    .interact_text()?;
                cfg.scheduled_backup_hours = h.trim().parse().unwrap_or(0);
            }
            8 => {
                let use_res = Confirm::with_theme(&theme()).with_prompt("Set custom resolution?").default(cfg.default_width.is_some()).interact()?;
                if use_res {
                    let w: String = Input::with_theme(&theme()).with_prompt("Width")
                        .with_initial_text(cfg.default_width.map(|v| v.to_string()).unwrap_or_else(|| "1280".into()))
                        .validate_with(|s: &String| s.trim().parse::<u32>().map(|_| ()).map_err(|_| "Must be a number"))
                        .interact_text()?;
                    let h: String = Input::with_theme(&theme()).with_prompt("Height")
                        .with_initial_text(cfg.default_height.map(|v| v.to_string()).unwrap_or_else(|| "720".into()))
                        .validate_with(|s: &String| s.trim().parse::<u32>().map(|_| ()).map_err(|_| "Must be a number"))
                        .interact_text()?;
                    cfg.default_width = Some(w.trim().parse().unwrap());
                    cfg.default_height = Some(h.trim().parse().unwrap());
                } else {
                    cfg.default_width = None;
                    cfg.default_height = None;
                }
            }
            9 => {
                let p: String = Input::with_theme(&theme()).with_prompt("Java 8 binary path (blank = auto)").allow_empty(true)
                    .with_initial_text(cfg.java8_path.as_deref().unwrap_or("")).interact_text()?;
                cfg.java8_path = if p.trim().is_empty() { None } else { Some(p.trim().to_string()) };
                if let Some(ref path) = cfg.java8_path { std::env::set_var("JAVA8_HOME", std::path::Path::new(path).parent().and_then(|p| p.parent()).unwrap_or(std::path::Path::new(path))); }
            }
            10 => {
                let p: String = Input::with_theme(&theme()).with_prompt("Java 21 binary path (blank = auto)").allow_empty(true)
                    .with_initial_text(cfg.java21_path.as_deref().unwrap_or("")).interact_text()?;
                cfg.java21_path = if p.trim().is_empty() { None } else { Some(p.trim().to_string()) };
                if let Some(ref path) = cfg.java21_path { std::env::set_var("JAVA21_HOME", std::path::Path::new(path).parent().and_then(|p| p.parent()).unwrap_or(std::path::Path::new(path))); }
            }
            11 => {
                let p: String = Input::with_theme(&theme()).with_prompt("Java 25 binary path (blank = auto)").allow_empty(true)
                    .with_initial_text(cfg.java25_path.as_deref().unwrap_or("")).interact_text()?;
                cfg.java25_path = if p.trim().is_empty() { None } else { Some(p.trim().to_string()) };
                if let Some(ref path) = cfg.java25_path { std::env::set_var("JAVA25_HOME", std::path::Path::new(path).parent().and_then(|p| p.parent()).unwrap_or(std::path::Path::new(path))); }
            }
            _ => break,
        }
        match config_mgr.save(&cfg).await {
            Ok(_)  => println!("  {} Settings saved.", style("✓").green()),
            Err(e) => println!("  {} {}", style("✗").red(), e),
        }
    }
    Ok(())
}

// ── Screenshot Gallery (with clipboard/upload) ───────────────────────────────



async fn view_playtime(history_mgr: &HistoryManager) -> Result<()> {
    use launcher::playtime::{bar, fmt_duration, week_label, PlaytimeTracker};

    let tracker = PlaytimeTracker::new(history_mgr);
    let s = tracker.summary().await?;

    if s.total_secs == 0 {
        println!("  No playtime recorded yet.");
        return Ok(());
    }

    println!();
    println!("  {} Playtime Summary", style("◆").cyan());
    println!();
    println!(
        "  Total playtime   : {}",
        style(fmt_duration(s.total_secs)).cyan().bold()
    );
    println!(
        "  This week        : {}",
        style(fmt_duration(s.this_week_secs)).green()
    );

    // ── By version ────────────────────────────────────────────────────────────
    if !s.by_version.is_empty() {
        println!();
        println!("  {} By version:", style("◆").cyan());
        let max = s.by_version[0].1;
        for (ver, secs) in s.by_version.iter().take(8) {
            println!(
                "    {:<18} {} {}",
                style(ver).cyan(),
                bar(*secs, max, 20),
                style(fmt_duration(*secs)).dim()
            );
        }
    }

    // ── By account ────────────────────────────────────────────────────────────
    if !s.by_account.is_empty() {
        println!();
        println!("  {} By account:", style("◆").cyan());
        let max = s.by_account[0].1;
        for (acc, secs) in s.by_account.iter().take(5) {
            println!(
                "    {:<18} {} {}",
                style(acc).cyan(),
                bar(*secs, max, 20),
                style(fmt_duration(*secs)).dim()
            );
        }
    }

    // ── Weekly chart ─────────────────────────────────────────────────────────
    if !s.weekly.is_empty() {
        println!();
        println!("  {} Weekly (last {} weeks):", style("◆").cyan(), s.weekly.len());
        let max = s.weekly.iter().map(|(_, v)| *v).max().unwrap_or(1);
        for ((year, week), secs) in &s.weekly {
            println!(
                "    {:<10} {} {}",
                style(week_label(*year, *week)).dim(),
                bar(*secs, max, 24),
                style(fmt_duration(*secs)).dim()
            );
        }
    }

    // ── FPS & Heap history ────────────────────────────────────────────────────
    let records = history_mgr.load().await?;
    let fps_records: Vec<_> = records.iter()
        .filter(|r| r.fps_avg.is_some())
        .rev()
        .take(10)
        .collect();
    if !fps_records.is_empty() {
        println!();
        println!("  {} Recent FPS (last {} sessions with benchmark data):", style("◆").cyan(), fps_records.len());
        let max_fps = fps_records.iter().filter_map(|r| r.fps_max).max().unwrap_or(1) as u64;
        for r in fps_records.iter().rev() {
            if let Some(avg) = r.fps_avg {
                let date = r.started_at.format("%m-%d").to_string();
                println!(
                    "    {:<6} {} {}  min:{} max:{}",
                    style(&date).dim(),
                    bar(avg as u64, max_fps, 20),
                    style(format!("{}fps avg", avg)).green(),
                    r.fps_min.unwrap_or(0),
                    r.fps_max.unwrap_or(0),
                );
            }
        }
    }

    let heap_records: Vec<_> = records.iter()
        .filter(|r| r.peak_heap_mb.is_some())
        .rev()
        .take(10)
        .collect();
    if !heap_records.is_empty() {
        println!();
        println!("  {} Peak Heap Usage (last {} sessions):", style("◆").cyan(), heap_records.len());
        let max_heap = heap_records.iter().filter_map(|r| r.peak_heap_mb).max().unwrap_or(1);
        for r in heap_records.iter().rev() {
            if let Some(peak) = r.peak_heap_mb {
                let date = r.started_at.format("%m-%d").to_string();
                println!(
                    "    {:<6} {} {}",
                    style(&date).dim(),
                    bar(peak, max_heap, 20),
                    style(format!("{} MB", peak)).yellow(),
                );
            }
        }
    }

    println!();
    Ok(())
}

// ── Friends List ──────────────────────────────────────────────────────────────

async fn friends_menu(friend_list: &FriendList) -> Result<()> {
    loop {
        let friends = friend_list.load().await?;

        println!();
        println!("  {} Friends ({})", style("◆").cyan(), friends.len());
        for (idx, f) in friends.iter().enumerate() {
            let status = match f.ping() {
                Some(ms) => style(format!("{}ms", ms)).green().to_string(),
                None     => match f.server {
                    Some(_) => style("offline").red().to_string(),
                    None    => style("no server set").dim().to_string(),
                },
            };
            let server_label = f.server.as_deref().unwrap_or("—");
            println!(
                "  {}. {} — {}  [{}]",
                idx + 1,
                style(&f.name).cyan(),
                server_label,
                status
            );
            if !f.notes.is_empty() {
                println!("     {}", style(&f.notes).dim());
            }
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Friends")
            .items(&["Add friend", "Edit friend", "Remove friend", "Refresh status", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let name: String = Input::with_theme(&theme())
                    .with_prompt("Friend's name")
                    .validate_with(|s: &String| if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) })
                    .interact_text()?;
                let server: String = Input::with_theme(&theme())
                    .with_prompt("Their server address (host:port or blank)")
                    .allow_empty(true)
                    .interact_text()?;
                let notes: String = Input::with_theme(&theme())
                    .with_prompt("Notes (optional)")
                    .allow_empty(true)
                    .interact_text()?;
                friend_list.add(Friend {
                    name: name.trim().to_string(),
                    server: if server.trim().is_empty() { None } else { Some(server.trim().to_string()) },
                    notes: notes.trim().to_string(),
                }).await?;
                println!("  {} Added.", style("✓").green());
            }
            1 => {
                if friends.is_empty() { println!("  No friends to edit."); continue; }
                let labels: Vec<String> = friends.iter().map(|f| f.name.clone()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Select friend").items(&labels).default(0).interact()?;
                let name: String = Input::with_theme(&theme())
                    .with_prompt("Name")
                    .with_initial_text(&friends[i].name)
                    .validate_with(|s: &String| if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) })
                    .interact_text()?;
                let server: String = Input::with_theme(&theme())
                    .with_prompt("Server address (blank to clear)")
                    .with_initial_text(friends[i].server.as_deref().unwrap_or(""))
                    .allow_empty(true)
                    .interact_text()?;
                let notes: String = Input::with_theme(&theme())
                    .with_prompt("Notes")
                    .with_initial_text(&friends[i].notes)
                    .allow_empty(true)
                    .interact_text()?;
                friend_list.update(i, Friend {
                    name: name.trim().to_string(),
                    server: if server.trim().is_empty() { None } else { Some(server.trim().to_string()) },
                    notes: notes.trim().to_string(),
                }).await?;
                println!("  {} Updated.", style("✓").green());
            }
            2 => {
                if friends.is_empty() { println!("  No friends to remove."); continue; }
                let labels: Vec<String> = friends.iter().map(|f| f.name.clone()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Remove friend").items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Remove '{}'?", friends[i].name))
                    .default(false).interact()?;
                if confirm {
                    friend_list.remove(i).await?;
                    println!("  {} Removed.", style("✓").green());
                }
            }
            3 => {
                // Re-entering the loop will re-ping automatically.
                continue;
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Trending Mods ─────────────────────────────────────────────────────────────

async fn trending_mods_menu(http: &reqwest::Client, version_mgr: &VersionManager) -> Result<()> {
    let installed = version_mgr.list_installed().await?;
    let game_version = if installed.is_empty() {
        Input::with_theme(&theme())
            .with_prompt("Game version (e.g. 1.21.1)")
            .interact_text()?
    } else {
        let labels: Vec<String> = installed.iter().map(|v| v.id.clone()).collect();
        let i = Select::with_theme(&theme()).with_prompt("Game version").items(&labels).default(0).interact()?;
        installed[i].id.clone()
    };

    let mut page = 0usize;
    loop {
        println!("  {} Fetching trending mods for {}...", style("→").cyan(), game_version);
        let hits = match trending::fetch_trending(http, &game_version, page, 10).await {
            Ok(h) => h,
            Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
        };

        if hits.is_empty() {
            println!("  No results found.");
            return Ok(());
        }

        println!();
        println!(
            "  {} Trending Mods — {} (page {})",
            style("◆").cyan(), game_version, page + 1
        );
        println!();
        for (i, h) in hits.iter().enumerate() {
            println!(
                "  {}. {} {} ↓{}  ♥{}",
                i + 1,
                style(&h.title).cyan(),
                if h.categories.is_empty() {
                    String::new()
                } else {
                    format!("[{}]", h.categories.join(", "))
                },
                h.downloads,
                h.follows,
            );
            let desc: String = h.description.chars().take(80).collect();
            println!("     {}", style(desc).dim());
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Trending Mods")
            .items(&["Next page", "Previous page", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => { page += 1; }
            1 => { page = page.saturating_sub(1); }
            _ => break,
        }
    }
    Ok(())
}

// ── Resource Pack Wizard ──────────────────────────────────────────────────────

async fn resource_pack_wizard(texture_mgr: &TextureManager) -> Result<()> {
    println!();
    println!("  {} Resource Pack Wizard", style("◆").cyan());
    println!("  Creates a new pack with procedurally generated block textures.");
    println!();

    let name: String = Input::with_theme(&theme())
        .with_prompt("Pack name (no spaces recommended)")
        .validate_with(|s: &String| {
            if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) }
        })
        .interact_text()?;

    let description: String = Input::with_theme(&theme())
        .with_prompt("Pack description")
        .with_initial_text("A custom Sumerian resource pack")
        .interact_text()?;

    // Ask which textures to generate
    let all_textures = ProceduralTexture::all();
    let tex_labels: Vec<&str> = all_textures.iter().map(|t| t.display_name()).collect();
    println!();
    println!("  Select textures to generate (spacebar to toggle, enter to confirm):");

    // Use MultiSelect for texture picking — fall back to all if unavailable.
    let chosen_indices: Vec<usize> = dialoguer::MultiSelect::with_theme(&theme())
        .with_prompt("Textures to generate")
        .items(&tex_labels)
        .defaults(&vec![true; tex_labels.len()])
        .interact()?;

    let generate_textures: Vec<ProceduralTexture> = chosen_indices
        .into_iter()
        .map(|i| all_textures[i].clone())
        .collect();

    let spec = PackSpec {
        name: name.trim().to_string(),
        description: description.trim().to_string(),
        generate_textures,
    };

    // The TextureManager stores packs in base/textures/packs/
    let packs_base = texture_mgr.packs_dir();
    println!("  {} Creating pack...", style("→").cyan());
    match packwizard::create_pack(&packs_base, &spec).await {
        Ok(path) => {
            println!(
                "  {} Pack '{}' created at {}",
                style("✓").green(),
                spec.name,
                path.display()
            );
            println!(
                "  {} Use 'Manage Textures → Import pack' to load it into the game.",
                style("ℹ").cyan()
            );
        }
        Err(e) => println!("  {} {}", style("✗").red(), e),
    }
    Ok(())
}

// ── Port Forwarding Helper ────────────────────────────────────────────────────

async fn port_forwarding_menu(game_dir: &PathBuf) -> Result<()> {
    let info = portforward::detect(game_dir);

    println!();
    println!("  {} Port Forwarding Helper", style("◆").cyan());
    println!();
    println!("  Local IP   : {}", style(&info.local_ip).cyan());
    println!("  Server port: {}", style(info.port).green());
    println!("  LAN address: {}", style(info.lan_address()).cyan());
    println!();

    let choice = Select::with_theme(&theme())
        .with_prompt("How do you want to share your server?")
        .items(&["ngrok instructions", "playit.gg instructions", "Back"])
        .default(0)
        .interact()?;

    match choice {
        0 => {
            println!();
            println!("  {} ngrok", style("◆").cyan().bold());
            println!();
            for line in info.ngrok_instructions().lines() {
                println!("  {}", style(line).cyan());
            }
            println!();
            println!("  Press Enter to open the ngrok download page in your browser...");
            let _: String = Input::with_theme(&theme()).allow_empty(true).with_prompt("").interact_text()?;
            let _ = open::that("https://ngrok.com/download");
        }
        1 => {
            println!();
            println!("  {} playit.gg", style("◆").cyan().bold());
            println!();
            for line in info.playit_instructions().lines() {
                println!("  {}", style(line).cyan());
            }
            println!();
            println!("  Press Enter to open the playit.gg download page...");
            let _: String = Input::with_theme(&theme()).allow_empty(true).with_prompt("").interact_text()?;
            let _ = open::that("https://playit.gg/download");
        }
        _ => {}
    }
    Ok(())
}

// ── Realm Browser ────────────────────────────────────────────────────────────

async fn realm_browser_menu(
    http: &reqwest::Client,
    auth: &Authenticator,
    profiles: &ProfileManager,
    version_mgr: &VersionManager,
) -> Result<()> {
    let session = pick_session(auth, profiles).await?;
    if session.auth_type != launcher::auth::AuthType::Microsoft {
        println!("  {} Realm Browser requires a Microsoft account.", style("✗").red());
        return Ok(());
    }

    let installed = version_mgr.list_installed().await?;
    let game_version = installed.first().map(|v| v.id.as_str()).unwrap_or("1.21").to_string();

    println!("  {} Fetching Realms...", style("→").cyan());
    let realm_list = match realms::list_realms(
        http,
        session.effective_token(),
        &session.uuid,
        &session.username,
        &game_version,
    ).await {
        Ok(r) => r,
        Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
    };

    if realm_list.is_empty() {
        println!("  No active Realms found for this account.");
        return Ok(());
    }

    println!();
    println!("  {} Your Realms ({})", style("◆").cyan(), realm_list.len());
    let labels: Vec<String> = realm_list.iter().map(|r| {
        let state = if r.state == "OPEN" { style("open").green() } else { style("closed").dim() };
        format!("{} [{}] — {} ({})", r.name, state, r.owner, r.motd.as_deref().unwrap_or(""))
    }).collect();

    let mut items = labels.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    items.push("Back");

    let choice = Select::with_theme(&theme())
        .with_prompt("Select Realm to get join address")
        .items(&items)
        .default(0)
        .interact()?;

    if choice >= realm_list.len() { return Ok(()); }

    let realm = &realm_list[choice];
    println!("  {} Fetching join address...", style("→").cyan());
    match realms::get_address(http, session.effective_token(), realm.id, &game_version).await {
        Ok(addr) => {
            println!("  {} Realm address: {}", style("✓").green(), style(&addr).cyan());
            println!("  {} Use this address in 'Launch Game' → server quick-join, or add it to Server Browser.", style("ℹ").cyan());
        }
        Err(e) => println!("  {} {}", style("✗").red(), e),
    }
    Ok(())
}

// ── Advancement Tracker ───────────────────────────────────────────────────────

async fn advancement_tracker_menu(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let saves_dir = if instances.is_empty() {
        game_dir.join("saves")
    } else {
        let mut labels = vec!["Default game dir".to_string()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme()).with_prompt("Saves from").items(&labels).default(0).interact()?;
        if i == 0 { game_dir.join("saves") } else { instance_mgr.instance_dir(&instances[i - 1].name).join("saves") }
    };

    // List worlds
    let worlds = WorldManager::list(&saves_dir).await?;
    if worlds.is_empty() { println!("  No worlds found."); return Ok(()); }

    let world_labels: Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
    let wi = Select::with_theme(&theme()).with_prompt("Select world").items(&world_labels).default(0).interact()?;
    let world_dir = &worlds[wi].path;

    let adv_path = match advancements::find_latest_advancements(world_dir) {
        Some(p) => p,
        None => { println!("  No advancements file found (1.12+ worlds only)."); return Ok(()); }
    };

    let cats = match advancements::parse_advancements(&adv_path) {
        Ok(c) => c,
        Err(e) => { println!("  {} Failed to parse advancements: {}", style("✗").red(), e); return Ok(()); }
    };

    let total_done: usize = cats.iter().map(|c| c.done).sum();
    let total_all: usize  = cats.iter().map(|c| c.total).sum();

    println!();
    println!("  {} Advancements — {}", style("◆").cyan(), worlds[wi].name);
    println!("  Overall: {}/{} ({:.0}%)", style(total_done).green(), total_all, if total_all > 0 { total_done as f64 / total_all as f64 * 100.0 } else { 0.0 });
    println!();
    for cat in &cats {
        let filled = if cat.total > 0 { (cat.done as f64 / cat.total as f64 * 20.0) as usize } else { 0 };
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(20 - filled));
        println!("  {:<30} {} {}/{}", style(&cat.namespace).cyan(), bar, cat.done, cat.total);
    }
    Ok(())
}

// ── LAN World Scanner ─────────────────────────────────────────────────────────

async fn lan_world_scanner_menu() -> Result<()> {
    println!();
    println!("  {} LAN World Scanner", style("◆").cyan());
    println!("  Scanning for open LAN worlds on your local network (4 seconds)...");
    println!();

    let worlds = match tokio::task::spawn_blocking(lanscanner::scan).await? {
        Ok(w) => w,
        Err(e) => { println!("  {} Scan failed: {}", style("✗").red(), e); return Ok(()); }
    };

    if worlds.is_empty() {
        println!("  No LAN worlds found. Make sure someone has opened their world to LAN in Minecraft.");
        return Ok(());
    }

    println!("  {} Found {} LAN world(s):", style("◆").cyan(), worlds.len());
    println!();
    for (i, w) in worlds.iter().enumerate() {
        println!("  {}. {} — {}", i + 1, style(&w.motd).cyan(), style(&w.address).green());
    }
    println!();
    println!("  {} Use the address above as the server address in 'Launch Game' or 'Server Browser'.", style("ℹ").cyan());
    Ok(())
}

// ── Whitelist & Op Manager ────────────────────────────────────────────────────

async fn whitelist_op_menu(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let server_dir = if instances.is_empty() {
        game_dir.clone()
    } else {
        let mut labels = vec!["Default game dir".to_string()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme()).with_prompt("Server directory").items(&labels).default(0).interact()?;
        if i == 0 { game_dir.clone() } else { instance_mgr.instance_dir(&instances[i - 1].name) }
    };

    loop {
        let whitelist = servermanager::load_whitelist(&server_dir).await.unwrap_or_default();
        let ops = servermanager::load_ops(&server_dir).await.unwrap_or_default();

        println!();
        println!("  {} Whitelist ({}) | Ops ({})", style("◆").cyan(), whitelist.len(), ops.len());
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Whitelist & Op Manager")
            .items(&[
                "List whitelist",
                "Add to whitelist",
                "Remove from whitelist",
                "List ops",
                "Add op",
                "Remove op",
                "Back",
            ])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                if whitelist.is_empty() { println!("  Whitelist is empty."); }
                for e in &whitelist { println!("  • {} ({})", style(&e.name).cyan(), &e.uuid[..8]); }
            }
            1 => {
                let name: String = Input::with_theme(&theme()).with_prompt("Player name").interact_text()?;
                let uuid: String = Input::with_theme(&theme()).with_prompt("UUID (leave blank to use offline UUID)").allow_empty(true).interact_text()?;
                let effective_uuid = if uuid.trim().is_empty() {
                    format!("{:x}", uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, format!("OfflinePlayer:{}", name.trim()).as_bytes()))
                } else { uuid.trim().to_string() };
                match servermanager::add_whitelist(&server_dir, name.trim(), &effective_uuid).await {
                    Ok(_) => println!("  {} Added.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                if whitelist.is_empty() { println!("  Whitelist is empty."); continue; }
                let names: Vec<&str> = whitelist.iter().map(|e| e.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Remove player").items(&names).default(0).interact()?;
                match servermanager::remove_whitelist(&server_dir, &whitelist[i].name).await {
                    Ok(_) => println!("  {} Removed.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            3 => {
                if ops.is_empty() { println!("  No ops."); }
                for e in &ops { println!("  • {} (level {}) ({})", style(&e.name).cyan(), e.level, &e.uuid[..8]); }
            }
            4 => {
                let name: String = Input::with_theme(&theme()).with_prompt("Player name").interact_text()?;
                let uuid: String = Input::with_theme(&theme()).with_prompt("UUID (blank = offline UUID)").allow_empty(true).interact_text()?;
                let effective_uuid = if uuid.trim().is_empty() {
                    format!("{:x}", uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, format!("OfflinePlayer:{}", name.trim()).as_bytes()))
                } else { uuid.trim().to_string() };
                let level_idx = Select::with_theme(&theme()).with_prompt("Op level").items(&["1 — Bypass spawn protection", "2 — Commands + singleplayer", "3 — Kick/ban/op", "4 — Full operator"]).default(1).interact()?;
                match servermanager::add_op(&server_dir, name.trim(), &effective_uuid, level_idx as u8 + 1).await {
                    Ok(_) => println!("  {} Added.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            5 => {
                if ops.is_empty() { println!("  No ops."); continue; }
                let names: Vec<&str> = ops.iter().map(|e| e.name.as_str()).collect();
                let i = Select::with_theme(&theme()).with_prompt("Remove op").items(&names).default(0).interact()?;
                match servermanager::remove_op(&server_dir, &ops[i].name).await {
                    Ok(_) => println!("  {} Removed.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── JVM Flag Advisor ──────────────────────────────────────────────────────────

async fn jvm_advisor_menu(version_mgr: &VersionManager) -> Result<()> {
    use client::injection::VersionEra;

    let installed = version_mgr.list_installed().await?;
    let java_major = if installed.is_empty() {
        21u32
    } else {
        let labels: Vec<String> = installed.iter().map(|v| format!("{} [{}]", v.id, v.version_type)).collect();
        let i = Select::with_theme(&theme()).with_prompt("Version to advise for").items(&labels).default(0).interact()?;
        let meta = version_mgr.load_meta(&installed[i].id).await?;
        let era = VersionEra::detect(&meta.id, &meta.version_type, &meta);
        if era.requires_java8() { 8 } else { meta.java_version.as_ref().map(|j| j.major_version).unwrap_or(21) }
    };

    let opt_list = OptimizationProfile::all();
    let opt_labels: Vec<String> = opt_list.iter().map(|p| format!("{} — {}", p, p.description())).collect();
    let o_idx = Select::with_theme(&theme()).with_prompt("Optimization profile").items(&opt_labels).default(2).interact()?;
    let profile = OptimizationProfile::from_index(o_idx);

    let advice = jvmadvisor::advise(java_major, &profile);

    println!();
    println!("  {} JVM Flag Advisor — Java {} / {}", style("◆").cyan(), java_major, profile);
    println!();
    for fa in &advice.flags {
        println!("  {} {}", style("•").green(), style(&fa.flag).cyan());
        println!("    {}", style(&fa.reason).dim());
        println!();
    }
    println!("  {} Copyable flags:", style("◆").cyan());
    println!();
    println!("  {}", style(&advice.command_line).cyan());
    println!();
    println!("  {} Paste these into Settings → Java path → Custom JVM args, or an instance profile.", style("ℹ").cyan());
    Ok(())
}

// ── Crash Pattern Report ──────────────────────────────────────────────────────

async fn crash_pattern_report_menu(mgr: &launcher::crashpatterns::CrashPatternManager) -> Result<()> {
    let store = mgr.load().await;

    println!();
    println!("  {} Crash Pattern Report", style("◆").cyan());
    println!("  Total crashes recorded: {}", style(store.total_crashes).yellow());
    println!();

    if store.total_crashes == 0 {
        println!("  No crashes recorded yet.");
        return Ok(());
    }

    let suspects = store.top_suspects(10);
    if !suspects.is_empty() {
        println!("  {} Top suspected mods (by crash co-occurrence):", style("◆").cyan());
        for (mod_fragment, count) in &suspects {
            println!("    {} {} — {} crash(es)", style("•").red(), style(mod_fragment).cyan(), count);
        }
        println!();
    }

    let causes = store.top_causes(8);
    if !causes.is_empty() {
        println!("  {} Most common crash causes:", style("◆").cyan());
        for (cause, count) in &causes {
            println!("    {} {} — {} time(s)", style("•").yellow(), cause, count);
        }
        println!();
    }

    let choice = Select::with_theme(&theme())
        .with_prompt("Crash Pattern Report")
        .items(&["Reset all patterns", "Back"])
        .default(1)
        .interact()?;

    if choice == 0 {
        let confirm = Confirm::with_theme(&theme())
            .with_prompt("Reset all crash pattern data?")
            .default(false)
            .interact()?;
        if confirm {
            mgr.reset().await?;
            println!("  {} Reset.", style("✓").green());
        }
    }
    Ok(())
}

// ── Instance Diff ─────────────────────────────────────────────────────────────

async fn instance_diff_menu(instance_mgr: &InstanceManager) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    if instances.len() < 2 {
        println!("  You need at least 2 instances to compare.");
        return Ok(());
    }

    let labels: Vec<String> = instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)).collect();

    let a_idx = Select::with_theme(&theme()).with_prompt("Instance A").items(&labels).default(0).interact()?;
    let b_idx = Select::with_theme(&theme()).with_prompt("Instance B").items(&labels).default(1).interact()?;

    if a_idx == b_idx {
        println!("  Select two different instances.");
        return Ok(());
    }

    let dir_a = instance_mgr.instance_dir(&instances[a_idx].name);
    let dir_b = instance_mgr.instance_dir(&instances[b_idx].name);
    let report = instancediff::diff(&dir_a, &dir_b);

    println!();
    println!(
        "  {} Instance Diff: {} vs {}",
        style("◆").cyan(),
        style(&instances[a_idx].name).cyan(),
        style(&instances[b_idx].name).cyan()
    );
    println!("  Shared mods: {}", style(report.shared_mod_count).green());
    println!();

    if report.mod_diffs.is_empty() {
        println!("  No mod differences found.");
    } else {
        println!("  {} Mod differences ({}):", style("◆").cyan(), report.mod_diffs.len());
        for d in &report.mod_diffs {
            if d.only_in_a {
                println!("    {} {} — only in {}", style("A").cyan().bold(), d.name, instances[a_idx].name);
            } else if d.only_in_b {
                println!("    {} {} — only in {}", style("B").cyan().bold(), d.name, instances[b_idx].name);
            } else if d.version_mismatch {
                println!("    {} {} — version mismatch", style("≠").yellow(), d.name);
                println!("       A: {}  B: {}",
                    d.version_a.as_deref().unwrap_or("?"),
                    d.version_b.as_deref().unwrap_or("?"));
            }
        }
    }

    if !report.config_diffs.is_empty() {
        println!();
        println!("  {} Config differences ({}):", style("◆").cyan(), report.config_diffs.len());
        for d in &report.config_diffs {
            if d.only_in_a {
                println!("    {} {} — only in {}", style("A").cyan().bold(), d.filename, instances[a_idx].name);
            } else {
                println!("    {} {} — only in {}", style("B").cyan().bold(), d.filename, instances[b_idx].name);
            }
        }
    }
    Ok(())
}

// ── Recording Helper ──────────────────────────────────────────────────────────

async fn recording_helper_menu(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let mods_dir = if instances.is_empty() {
        game_dir.join("mods")
    } else {
        let mut labels = vec!["Default game dir".to_string()];
        labels.extend(instances.iter().map(|i| i.name.clone()));
        let i = Select::with_theme(&theme()).with_prompt("Check mods in").items(&labels).default(0).interact()?;
        if i == 0 { game_dir.join("mods") } else { instance_mgr.instance_dir(&instances[i - 1].name).join("mods") }
    };

    let recorder = recording::detect(&mods_dir);

    println!();
    println!("  {} Recording Helper", style("◆").cyan());
    println!();

    match &recorder {
        recording::Recorder::Obs => {
            println!("  {} OBS Studio detected on this system.", style("✓").green());
            println!();
            for line in recording::obs_setup_guide().lines() {
                if line.is_empty() { println!(); } else { println!("  {}", line); }
            }
        }
        recording::Recorder::ReplayMod => {
            println!("  {} ReplayMod detected in this instance.", style("✓").green());
            println!();
            for line in recording::replay_mod_setup_guide().lines() {
                if line.is_empty() { println!(); } else { println!("  {}", line); }
            }
        }
        recording::Recorder::None => {
            println!("  {} No recording tool detected.", style("ℹ").yellow());
            println!();
            for line in recording::no_recorder_guide().lines() {
                if line.is_empty() { println!(); } else { println!("  {}", line); }
            }
            println!();
            let choice = Select::with_theme(&theme())
                .with_prompt("Open download page")
                .items(&["OBS Studio", "ReplayMod", "Windows Game Bar info", "Back"])
                .default(0)
                .interact()?;
            match choice {
                0 => { let _ = open::that("https://obsproject.com"); }
                1 => { let _ = open::that("https://www.replaymod.com"); }
                2 => { let _ = open::that("https://xbox.com/en-US/apps/xbox-game-bar"); }
                _ => {}
            }
        }
    }
    Ok(())
}

// ── Splash Screen Editor ──────────────────────────────────────────────────────

async fn splash_screen_editor_menu(texture_mgr: &TextureManager) -> Result<()> {
    let packs = texture_mgr.list_packs().await?;
    if packs.is_empty() {
        println!("  No resource packs found. Create one first via 'Resource Pack Wizard'.");
        return Ok(());
    }

    let pack_names: Vec<&str> = packs.iter().map(|p| p.name.as_str()).collect();
    let p_idx = Select::with_theme(&theme()).with_prompt("Select pack").items(&pack_names).default(0).interact()?;
    let pack_dir = &packs[p_idx].path;

    loop {
        let lines = splashscreen::load(pack_dir).await.unwrap_or_default();
        println!();
        println!("  {} Splash Screen Editor — {}", style("◆").cyan(), packs[p_idx].name);
        println!("  {} splash(es):", lines.len());
        for (i, l) in lines.iter().enumerate() {
            println!("  {}. {}", i + 1, style(l).cyan());
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Splash Editor")
            .items(&["Add splash", "Remove splash", "Import from file", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let text: String = Input::with_theme(&theme())
                    .with_prompt("Splash text (max 256 chars)")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("Cannot be empty.") }
                        else if s.len() > 256 { Err("Max 256 characters.") }
                        else { Ok(()) }
                    })
                    .interact_text()?;
                match splashscreen::add(pack_dir, text.trim()).await {
                    Ok(_)  => println!("  {} Added.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if lines.is_empty() { println!("  No splashes to remove."); continue; }
                let line_labels: Vec<String> = lines.iter().enumerate()
                    .map(|(i, l)| format!("{}. {}", i + 1, l.chars().take(60).collect::<String>()))
                    .collect();
                let i = Select::with_theme(&theme()).with_prompt("Remove splash").items(&line_labels).default(0).interact()?;
                match splashscreen::remove(pack_dir, i).await {
                    Ok(_)  => println!("  {} Removed.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                let path_str: String = Input::with_theme(&theme()).with_prompt("Path to text file (one splash per line)").interact_text()?;
                match splashscreen::import_file(pack_dir, &PathBuf::from(path_str.trim())).await {
                    Ok(n)  => println!("  {} Imported {} new splash(es).", style("✓").green(), n),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Config Export / Import ────────────────────────────────────────────────────

async fn config_export_import_menu(data_dir: &PathBuf) -> Result<()> {
    println!();
    println!("  {} Config Export / Import", style("◆").cyan());
    println!("  Exports/imports the entire launcher data directory (accounts excluded).");
    println!();

    let choice = Select::with_theme(&theme())
        .with_prompt("Config Export / Import")
        .items(&["Export config to zip", "Import config from zip", "Back"])
        .default(0)
        .interact()?;

    match choice {
        0 => {
            let dest: String = Input::with_theme(&theme())
                .with_prompt("Save zip to path")
                .with_initial_text("SumerianConfig.zip")
                .interact_text()?;
            let dest_path = PathBuf::from(dest.trim());
            println!("  {} Exporting...", style("→").cyan());
            match tokio::task::spawn_blocking({
                let d = data_dir.clone();
                let p = dest_path.clone();
                move || configexport::export(&d, &p)
            }).await? {
                Ok(path) => println!("  {} Exported to {}", style("✓").green(), path.display()),
                Err(e)   => println!("  {} {}", style("✗").red(), e),
            }
        }
        1 => {
            let src: String = Input::with_theme(&theme())
                .with_prompt("Path to config zip")
                .interact_text()?;
            let src_path = PathBuf::from(src.trim());
            let confirm = Confirm::with_theme(&theme())
                .with_prompt("This will overwrite existing launcher files (accounts preserved). Continue?")
                .default(false)
                .interact()?;
            if confirm {
                println!("  {} Importing...", style("→").cyan());
                match tokio::task::spawn_blocking({
                    let d = data_dir.clone();
                    let p = src_path.clone();
                    move || configexport::import(&p, &d)
                }).await? {
                    Ok(n)  => println!("  {} Imported {} file(s).", style("✓").green(), n),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Screenshot Gallery (updated with clipboard/upload) ────────────────────────

async fn screenshot_gallery(
    http: &reqwest::Client,
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let dir = if instances.is_empty() {
        game_dir.clone()
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| i.name.clone()));
        let i = Select::with_theme(&theme()).with_prompt("Screenshots from").items(&labels).default(0).interact()?;
        if i == 0 { game_dir.clone() } else { instance_mgr.instance_dir(&instances[i - 1].name) }
    };

    let shots = ScreenshotGallery::list(&dir).await?;
    if shots.is_empty() {
        println!("  No screenshots found in {}", dir.join("screenshots").display());
        return Ok(());
    }

    let mut labels: Vec<String> = shots.iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();
    labels.push("Open folder".into());
    labels.push("Back".into());

    println!();
    println!("  {} Screenshots ({})", style("◆").cyan(), shots.len());
    println!();
    let idx = Select::with_theme(&theme()).with_prompt("Select screenshot").items(&labels).default(0).interact()?;

    if idx == shots.len() {
        let _ = ScreenshotGallery::open_folder(&dir);
    } else if idx < shots.len() {
        let action_choice = Select::with_theme(&theme())
            .with_prompt("Action")
            .items(&["Open in viewer", "Copy path to clipboard", "Upload to 0x0.st (get URL)", "Back"])
            .default(0)
            .interact()?;
        match action_choice {
            0 => {
                match ScreenshotGallery::open(&shots[idx]) {
                    Ok(_)  => println!("  {} Opened.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                match ScreenshotGallery::copy_path_to_clipboard(&shots[idx]) {
                    Ok(true)  => println!("  {} Path copied to clipboard.", style("✓").green()),
                    Ok(false) => println!("  {} No clipboard tool found (clip/pbcopy/xclip).", style("⚠").yellow()),
                    Err(e)    => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                println!("  {} Uploading to 0x0.st...", style("→").cyan());
                match ScreenshotGallery::upload_to_paste(http, &shots[idx]).await {
                    Ok(url) => {
                        println!("  {} {}", style("✓").green(), style(&url).cyan().underlined());
                        // Also copy to clipboard if possible
                        let _ = ScreenshotGallery::copy_path_to_clipboard(std::path::Path::new(&url));
                        println!("  {} URL copied to clipboard.", style("ℹ").cyan());
                    }
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// ── Config Snapshot ──────────────────────────────────────────────────────────

async fn config_snapshot_menu(
    snap_mgr: &ConfigSnapshotManager,
    instance_mgr: &InstanceManager,
    base: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;

    // Pick an instance (or default game config).
    let instance_name: String = if instances.is_empty() {
        "default".to_string()
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme())
            .with_prompt("Config snapshot for")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 {
            "default".to_string()
        } else {
            instances[i - 1].name.clone()
        }
    };

    // Resolve the config/ directory we're snapshotting.
    let config_dir = if instance_name == "default" {
        base.join("config")
    } else {
        instance_mgr.instance_dir(&instance_name).join("config")
    };

    loop {
        let snapshots = snap_mgr.list(&instance_name).await;

        println!();
        println!(
            "  {} Config Snapshots — {} ({} saved)",
            style("◆").cyan(),
            style(&instance_name).cyan().bold(),
            snapshots.len()
        );
        for s in &snapshots {
            let date = s.created_at.get(..16).unwrap_or(&s.created_at);
            println!("  • {} — {}", style(&s.name).cyan(), style(date).dim());
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Config Snapshot")
            .items(&["Save snapshot", "Restore snapshot", "Delete snapshot", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let label: String = Input::with_theme(&theme())
                    .with_prompt("Snapshot name")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) }
                    })
                    .interact_text()?;
                match snap_mgr.create(&instance_name, &config_dir, label.trim()).await {
                    Ok(s) => println!("  {} Snapshot '{}' saved.", style("✓").green(), s.name),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if snapshots.is_empty() {
                    println!("  No snapshots to restore.");
                    continue;
                }
                let labels: Vec<String> = snapshots.iter()
                    .map(|s| format!("{} ({})", s.name, s.created_at.get(..16).unwrap_or("")))
                    .collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Restore snapshot")
                    .items(&labels)
                    .default(0)
                    .interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!(
                        "Restore '{}' into {}? This will overwrite the current config/ directory.",
                        snapshots[i].name, instance_name
                    ))
                    .default(false)
                    .interact()?;
                if confirm {
                    match snap_mgr.restore(&instance_name, &snapshots[i].name, &config_dir).await {
                        Ok(_) => println!("  {} Restored.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            2 => {
                if snapshots.is_empty() {
                    println!("  No snapshots to delete.");
                    continue;
                }
                let labels: Vec<&str> = snapshots.iter().map(|s| s.name.as_str()).collect();
                let i = Select::with_theme(&theme())
                    .with_prompt("Delete snapshot")
                    .items(&labels)
                    .default(0)
                    .interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Delete snapshot '{}'?", snapshots[i].name))
                    .default(false)
                    .interact()?;
                if confirm {
                    match snap_mgr.delete(&instance_name, &snapshots[i].name).await {
                        Ok(_) => println!("  {} Deleted.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Mod Profile Switcher ──────────────────────────────────────────────────────

async fn mod_profile_switcher_menu(
    mp_mgr: &ModProfileManager,
    instance_mgr: &InstanceManager,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    if instances.is_empty() {
        println!("  No instances found. Create an instance first.");
        return Ok(());
    }

    let inst_labels: Vec<String> = instances.iter()
        .map(|i| format!("{} [{}]", i.name, i.version_id))
        .collect();
    let i_idx = Select::with_theme(&theme())
        .with_prompt("Select instance")
        .items(&inst_labels)
        .default(0)
        .interact()?;
    let instance_name = &instances[i_idx].name;

    loop {
        let profiles = mp_mgr.load_all(instance_name).await;

        println!();
        println!(
            "  {} Mod Profiles — {} ({} saved)",
            style("◆").cyan(),
            style(instance_name).cyan().bold(),
            profiles.len()
        );
        for p in &profiles {
            let date = p.updated_at.get(..16).unwrap_or(&p.updated_at);
            println!(
                "  • {}  {} mod(s) disabled  {}",
                style(&p.name).cyan(),
                p.disabled.len(),
                style(date).dim()
            );
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Mod Profile Switcher")
            .items(&[
                "Save current state as profile",
                "Apply profile",
                "Show profile diff",
                "Delete profile",
                "Back",
            ])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let name: String = Input::with_theme(&theme())
                    .with_prompt("Profile name (e.g. speedrun, casual)")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) }
                    })
                    .interact_text()?;
                match mp_mgr.save_current(instance_name, name.trim()).await {
                    Ok(p) => println!(
                        "  {} Saved '{}' ({} disabled).",
                        style("✓").green(), p.name, p.disabled.len()
                    ),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if profiles.is_empty() {
                    println!("  No profiles saved yet.");
                    continue;
                }
                let labels: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
                let pi = Select::with_theme(&theme())
                    .with_prompt("Apply profile")
                    .items(&labels)
                    .default(0)
                    .interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!(
                        "Apply '{}' to {}? This will rename mod files on disk.",
                        profiles[pi].name, instance_name
                    ))
                    .default(true)
                    .interact()?;
                if confirm {
                    match mp_mgr.apply(instance_name, &profiles[pi].name).await {
                        Ok(_) => println!("  {} Profile applied.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            2 => {
                if profiles.len() < 2 {
                    println!("  Need at least 2 profiles to diff.");
                    continue;
                }
                let labels: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
                let a = Select::with_theme(&theme()).with_prompt("Profile A").items(&labels).default(0).interact()?;
                let b = Select::with_theme(&theme()).with_prompt("Profile B").items(&labels).default(1).interact()?;
                match mp_mgr.diff(instance_name, &profiles[a].name, &profiles[b].name).await {
                    Ok(diffs) if diffs.is_empty() => println!("  Profiles are identical."),
                    Ok(diffs) => {
                        println!();
                        for d in &diffs { println!("{}", d); }
                    }
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            3 => {
                if profiles.is_empty() {
                    println!("  No profiles to delete.");
                    continue;
                }
                let labels: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
                let pi = Select::with_theme(&theme()).with_prompt("Delete profile").items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Delete profile '{}'?", profiles[pi].name))
                    .default(false)
                    .interact()?;
                if confirm {
                    match mp_mgr.delete(instance_name, &profiles[pi].name).await {
                        Ok(_) => println!("  {} Deleted.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── JVM Flag Presets ──────────────────────────────────────────────────────────

async fn jvm_presets_menu(preset_mgr: &JvmPresetManager) -> Result<()> {
    loop {
        let presets = preset_mgr.load_all().await;

        println!();
        println!("  {} JVM Flag Presets ({})", style("◆").cyan(), presets.len());
        for p in &presets {
            println!(
                "  • {}  {}  — {}",
                style(&p.name).cyan(),
                style(p.flags.join(" ")).dim(),
                p.description
            );
        }
        println!();
        println!("  {} Presets are appended to the JVM arg list at launch (per-instance).", style("ℹ").dim());
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("JVM Flag Presets")
            .items(&["Create preset", "Edit preset", "Delete preset", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let name: String = Input::with_theme(&theme())
                    .with_prompt("Preset name")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("Name cannot be empty.") } else { Ok(()) }
                    })
                    .interact_text()?;
                let flags_raw: String = Input::with_theme(&theme())
                    .with_prompt("JVM flags (space-separated)")
                    .validate_with(|s: &String| {
                        if s.trim().is_empty() { Err("At least one flag required.") } else { Ok(()) }
                    })
                    .interact_text()?;
                let flags: Vec<String> = flags_raw.split_whitespace().map(|s| s.to_string()).collect();
                let desc: String = Input::with_theme(&theme())
                    .with_prompt("Description (optional)")
                    .allow_empty(true)
                    .interact_text()?;
                match preset_mgr.create(name.trim(), flags, desc.trim()).await {
                    Ok(p) => println!("  {} Created '{}' ({} flag(s)).", style("✓").green(), p.name, p.flags.len()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                if presets.is_empty() {
                    println!("  No presets to edit.");
                    continue;
                }
                let labels: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
                let pi = Select::with_theme(&theme()).with_prompt("Edit preset").items(&labels).default(0).interact()?;
                let flags_raw: String = Input::with_theme(&theme())
                    .with_prompt("New flags (space-separated)")
                    .with_initial_text(&presets[pi].flags.join(" "))
                    .interact_text()?;
                let new_flags: Vec<String> = flags_raw.split_whitespace().map(|s| s.to_string()).collect();
                let desc: String = Input::with_theme(&theme())
                    .with_prompt("Description")
                    .with_initial_text(&presets[pi].description)
                    .allow_empty(true)
                    .interact_text()?;
                match preset_mgr.update(&presets[pi].name, Some(new_flags), Some(desc.trim())).await {
                    Ok(_) => println!("  {} Updated.", style("✓").green()),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                if presets.is_empty() {
                    println!("  No presets to delete.");
                    continue;
                }
                let labels: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
                let pi = Select::with_theme(&theme()).with_prompt("Delete preset").items(&labels).default(0).interact()?;
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Delete preset '{}'?", presets[pi].name))
                    .default(false)
                    .interact()?;
                if confirm {
                    match preset_mgr.delete(&presets[pi].name).await {
                        Ok(_) => println!("  {} Deleted.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Log Tail ──────────────────────────────────────────────────────────────────

async fn log_tail_menu(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;

    let target_dir = if instances.is_empty() {
        game_dir.clone()
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme())
            .with_prompt("Tail logs from")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 {
            game_dir.clone()
        } else {
            instance_mgr.instance_dir(&instances[i - 1].name)
        }
    };

    let idle_choices = ["10s (default)", "30s", "60s", "5 minutes"];
    let idle_idx = Select::with_theme(&theme())
        .with_prompt("Stop after idle for")
        .items(&idle_choices)
        .default(0)
        .interact()?;
    let idle_secs = match idle_idx {
        1 => 30,
        2 => 60,
        3 => 300,
        _ => 10,
    };

    logtail::tail(&target_dir, std::time::Duration::from_secs(idle_secs))?;
    Ok(())
}

// ── Doctor ────────────────────────────────────────────────────────────────────

async fn doctor_menu(
    base: &PathBuf,
    game_dir: &PathBuf,
    http: &reqwest::Client,
    auth: &Authenticator,
    version_mgr: &VersionManager,
) -> Result<()> {
    println!();
    let _ = doctor::run(base, game_dir, http, auth, version_mgr).await?;
    Ok(())
}

// ── Performance Profiler ──────────────────────────────────────────────────────

async fn performance_profiler_menu(history_mgr: &HistoryManager) -> Result<()> {
    let records = history_mgr.load().await?;

    // Only sessions that have at least FPS or heap data.
    let profiled: Vec<_> = records.iter()
        .rev()
        .filter(|r| r.fps_avg.is_some() || r.peak_heap_mb.is_some())
        .collect();

    println!();
    println!("  {} Performance Profiler", style("◆").cyan().bold());
    println!("  Shows sessions with benchmark data (enable Benchmark Mode in Settings).");
    println!();

    if profiled.is_empty() {
        println!("  No performance data recorded yet.");
        println!("  Enable {} in Settings, then play a session.", style("Benchmark Mode").cyan());
        return Ok(());
    }

    // ── Summary table ─────────────────────────────────────────────────────────
    println!(
        "  {:<24} {:<12} {:>8} {:>8} {:>8} {:>10}",
        style("Version").bold(),
        style("Date").bold(),
        style("FPS avg").bold(),
        style("FPS min").bold(),
        style("FPS max").bold(),
        style("Peak heap").bold(),
    );
    println!("  {}", style("─".repeat(76)).dim());

    for r in &profiled {
        let date = r.started_at.format("%Y-%m-%d %H:%M").to_string();
        let fps_avg = r.fps_avg.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        let fps_min = r.fps_min.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        let fps_max = r.fps_max.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        let heap   = r.peak_heap_mb.map(|v| format!("{} MB", v)).unwrap_or_else(|| "—".into());
        println!(
            "  {:<24} {:<12} {:>8} {:>8} {:>8} {:>10}",
            style(&r.version_id).cyan(),
            style(&date).dim(),
            style(&fps_avg).green(),
            style(&fps_min).yellow(),
            style(&fps_max).green(),
            style(&heap).yellow(),
        );
    }

    println!();

    // ── Aggregated stats across all profiled sessions ─────────────────────────
    let fps_avgs: Vec<u32> = profiled.iter().filter_map(|r| r.fps_avg).collect();
    let heaps:    Vec<u64> = profiled.iter().filter_map(|r| r.peak_heap_mb).collect();

    if !fps_avgs.is_empty() {
        let overall_avg = fps_avgs.iter().sum::<u32>() / fps_avgs.len() as u32;
        let overall_min = fps_avgs.iter().copied().min().unwrap_or(0);
        let overall_max = fps_avgs.iter().copied().max().unwrap_or(0);
        println!(
            "  {} All-session FPS  avg: {}  min: {}  max: {}",
            style("◆").cyan(),
            style(overall_avg).green().bold(),
            style(overall_min).yellow(),
            style(overall_max).green(),
        );
    }
    if !heaps.is_empty() {
        let peak = heaps.iter().copied().max().unwrap_or(0);
        let avg  = heaps.iter().sum::<u64>() / heaps.len() as u64;
        println!(
            "  {} Heap — avg peak: {} MB  all-time peak: {} MB",
            style("◆").cyan(),
            style(avg).yellow(),
            style(peak).red(),
        );
    }

    println!();
    println!(
        "  {} {} session(s) with performance data shown.",
        style("ℹ").dim(), profiled.len()
    );

    Ok(())
}

// ── Playtime Goals ────────────────────────────────────────────────────────────

async fn playtime_goals_menu(
    goal_mgr: &PlaytimeGoalManager,
    history_mgr: &HistoryManager,
) -> Result<()> {
    loop {
        let goals = goal_mgr.load().await;
        println!();
        goal_mgr.print_progress(history_mgr).await?;

        let choice = Select::with_theme(&theme())
            .with_prompt("Playtime Goals")
            .items(&["Set daily goal", "Set weekly goal", "Clear all goals", "Back"])
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let raw: String = Input::with_theme(&theme())
                    .with_prompt("Daily goal (e.g. 1h30m, 90m, or 0 to disable)")
                    .with_initial_text(&fmt_goal_duration(goals.daily_secs))
                    .interact_text()?;
                let secs = parse_goal_duration(raw.trim());
                let mut g = goal_mgr.load().await;
                g.daily_secs = secs;
                match goal_mgr.save(&g).await {
                    Ok(_) => println!("  {} Daily goal set to {}.", style("✓").green(), fmt_goal_duration(secs)),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            1 => {
                let raw: String = Input::with_theme(&theme())
                    .with_prompt("Weekly goal (e.g. 10h, 600m, or 0 to disable)")
                    .with_initial_text(&fmt_goal_duration(goals.weekly_secs))
                    .interact_text()?;
                let secs = parse_goal_duration(raw.trim());
                let mut g = goal_mgr.load().await;
                g.weekly_secs = secs;
                match goal_mgr.save(&g).await {
                    Ok(_) => println!("  {} Weekly goal set to {}.", style("✓").green(), fmt_goal_duration(secs)),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
            2 => {
                let mut g = goal_mgr.load().await;
                g.daily_secs = 0;
                g.weekly_secs = 0;
                goal_mgr.save(&g).await.ok();
                println!("  {} Goals cleared.", style("✓").green());
            }
            _ => break,
        }
    }
    Ok(())
}

fn fmt_goal_duration(secs: u64) -> String {
    if secs == 0 { return "0".into(); }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 && m > 0 { format!("{}h{}m", h, m) }
    else if h > 0 { format!("{}h", h) }
    else { format!("{}m", m) }
}

fn parse_goal_duration(s: &str) -> u64 {
    // Accept: "0", "90m", "1h30m", "1h", "5400" (bare seconds)
    if s == "0" { return 0; }
    let lower = s.to_lowercase();
    let mut total = 0u64;
    let mut num = String::new();
    for c in lower.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else if c == 'h' {
            total += num.trim().parse::<u64>().unwrap_or(0) * 3600;
            num.clear();
        } else if c == 'm' {
            total += num.trim().parse::<u64>().unwrap_or(0) * 60;
            num.clear();
        }
    }
    // Bare number treated as minutes
    if !num.trim().is_empty() {
        total += num.trim().parse::<u64>().unwrap_or(0) * 60;
    }
    total
}

// ── World Seed Reader ─────────────────────────────────────────────────────────

async fn world_seed_reader_menu(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;

    let base_saves = if instances.is_empty() {
        game_dir.join("saves")
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme())
            .with_prompt("Read seeds from")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 { game_dir.join("saves") }
        else { instance_mgr.instance_dir(&instances[i - 1].name).join("saves") }
    };

    if !base_saves.exists() {
        println!("  No saves directory found.");
        return Ok(());
    }

    let worlds = launcher::instances::WorldManager::list(&base_saves).await?;
    if worlds.is_empty() {
        println!("  No worlds found.");
        return Ok(());
    }

    let labels: Vec<String> = worlds.iter().map(|w| {
        let date = w.last_played.as_deref().unwrap_or("unknown");
        format!("{} — last played: {}", w.name, date)
    }).collect();

    let i = Select::with_theme(&theme())
        .with_prompt("Select world")
        .items(&labels)
        .default(0)
        .interact()?;

    match seedreader::read_seed(&worlds[i].path) {
        Ok(seed) => {
            println!();
            println!(
                "  {} World: {}",
                style("◆").cyan(),
                style(&worlds[i].name).cyan().bold()
            );
            println!(
                "  {} Seed:  {}",
                style("◆").cyan(),
                style(seedreader::format_seed(seed)).green().bold()
            );
            println!();
            println!("  {} Use /seed in-game to verify.", style("ℹ").dim());
        }
        Err(e) => println!("  {} {}", style("✗").red(), e),
    }
    Ok(())
}

// ── Instance Health Check ─────────────────────────────────────────────────────

async fn instance_health_check_menu(
    instance_mgr: &InstanceManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;

    let mods_dir = if instances.is_empty() {
        game_dir.join("mods")
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme())
            .with_prompt("Check health for")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 { game_dir.join("mods") }
        else { instance_mgr.instance_dir(&instances[i - 1].name).join("mods") }
    };

    println!();
    println!("  {} Instance Health Check", style("◆").cyan().bold());
    println!("  Scanning: {}", style(mods_dir.display().to_string()).dim());
    println!();

    let issues = instancehealth::check(&mods_dir);
    if issues.is_empty() {
        println!("  {} No issues found — all clear!", style("✓").green().bold());
    } else {
        println!("  {} {} issue(s) found:", style("⚠").yellow().bold(), issues.len());
        println!();
        for issue in &issues {
            let sev = if issue.severity() == "ERROR" {
                style(issue.severity()).red().bold()
            } else {
                style(issue.severity()).yellow().bold()
            };
            println!("  [{}] {}", sev, issue.message());
        }
    }
    Ok(())
}

// ── Resource Pack Preview ─────────────────────────────────────────────────────

async fn resource_pack_preview_menu(texture_mgr: &TextureManager) -> Result<()> {
    let packs = texture_mgr.list_packs().await?;
    if packs.is_empty() {
        println!("  No resource packs installed.");
        return Ok(());
    }

    let labels: Vec<String> = packs.iter().map(|p| p.name.clone()).collect();
    let i = Select::with_theme(&theme())
        .with_prompt("Preview pack")
        .items(&labels)
        .default(0)
        .interact()?;

    match texture_mgr.preview_pack(&packs[i].name).await {
        Ok(preview) => {
            println!();
            println!("  {} {}", style("◆").cyan(), style(&packs[i].name).cyan().bold());
            println!(
                "  Pack format : {} (MC {})",
                style(preview.format).green(),
                preview.format_label()
            );
            println!(
                "  Description : {}",
                style(preview.clean_description()).italic()
            );
        }
        Err(e) => println!("  {} {}", style("✗").red(), e),
    }
    Ok(())
}

// ── Mod Search History ────────────────────────────────────────────────────────

async fn mod_search_history_menu(
    history_mgr: &ModSearchHistoryManager,
    instance_mgr: &InstanceManager,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    let instance_key = if instances.is_empty() {
        "default".to_string()
    } else {
        let mut labels: Vec<String> = vec!["Default".into()];
        labels.extend(instances.iter().map(|i| i.name.clone()));
        let i = Select::with_theme(&theme())
            .with_prompt("History for")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 { "default".to_string() } else { instances[i - 1].name.clone() }
    };

    loop {
        let queries = history_mgr.get(&instance_key).await;
        println!();
        println!(
            "  {} Mod Search History — {} ({} entries)",
            style("◆").cyan(),
            style(&instance_key).cyan().bold(),
            queries.len()
        );
        if queries.is_empty() {
            println!("  No search history yet.");
        } else {
            for (i, q) in queries.iter().rev().enumerate() {
                println!("  {}. {}", i + 1, style(q).cyan());
            }
        }
        println!();

        let choice = Select::with_theme(&theme())
            .with_prompt("Mod Search History")
            .items(&["Clear history", "Back"])
            .default(1)
            .interact()?;

        match choice {
            0 => {
                let confirm = Confirm::with_theme(&theme())
                    .with_prompt(format!("Clear search history for '{}'?", instance_key))
                    .default(false)
                    .interact()?;
                if confirm {
                    match history_mgr.clear(&instance_key).await {
                        Ok(_) => println!("  {} Cleared.", style("✓").green()),
                        Err(e) => println!("  {} {}", style("✗").red(), e),
                    }
                }
            }
            _ => break,
        }
    }
    Ok(())
}

// ── Bulk Mod Toggle ───────────────────────────────────────────────────────────

async fn bulk_mod_toggle_menu(instance_mgr: &InstanceManager) -> Result<()> {
    let instances = instance_mgr.load_all().await?;
    if instances.is_empty() {
        println!("  No instances found.");
        return Ok(());
    }

    let labels: Vec<String> = instances.iter()
        .map(|i| format!("{} [{}]", i.name, i.version_id))
        .collect();
    let i = Select::with_theme(&theme())
        .with_prompt("Select instance")
        .items(&labels)
        .default(0)
        .interact()?;
    let instance_name = &instances[i].name;

    let choice = Select::with_theme(&theme())
        .with_prompt(format!("Bulk mod toggle — {}", instance_name))
        .items(&["Disable ALL mods", "Enable ALL mods", "Back"])
        .default(0)
        .interact()?;

    match choice {
        0 => {
            let confirm = Confirm::with_theme(&theme())
                .with_prompt(format!("Disable ALL mods in '{}'? (renames .jar → .jar.disabled)", instance_name))
                .default(false)
                .interact()?;
            if confirm {
                match instance_mgr.disable_all_mods(instance_name).await {
                    Ok(n) => println!("  {} Disabled {} mod(s).", style("✓").green(), n),
                    Err(e) => println!("  {} {}", style("✗").red(), e),
                }
            }
        }
        1 => {
            match instance_mgr.enable_all_mods(instance_name).await {
                Ok(n) => println!("  {} Enabled {} mod(s).", style("✓").green(), n),
                Err(e) => println!("  {} {}", style("✗").red(), e),
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Changelog Viewer ──────────────────────────────────────────────────────────

async fn changelog_viewer_menu(
    http: &reqwest::Client,
    version_mgr: &VersionManager,
) -> Result<()> {
    let installed = version_mgr.list_installed().await?;
    if installed.is_empty() {
        println!("  No versions installed.");
        return Ok(());
    }

    let labels: Vec<String> = installed.iter()
        .map(|v| format!("{} [{}]", v.id, v.version_type))
        .collect();
    let i = Select::with_theme(&theme())
        .with_prompt("View changelog for")
        .items(&labels)
        .default(0)
        .interact()?;
    let version_id = &installed[i].id;

    println!("  {} Fetching changelog for {}...", style("→").cyan(), version_id);
    let entries = changelogviewer::find_entries(http, version_id).await;
    match entries {
        Err(e) => { println!("  {} {}", style("✗").red(), e); return Ok(()); }
        Ok(ref v) if v.is_empty() => {
            println!("  No changelog found for '{}'.", version_id);
            return Ok(());
        }
        Ok(ref v) => {
            // Show the first (most recent) matching entry.
            let entry = &v[0];
            println!();
            println!("  {} {} — {}", style("◆").cyan(), style(&entry.version).cyan().bold(), entry.date.get(..10).unwrap_or(&entry.date));
            if !entry.title.is_empty() {
                println!("  {}", style(&entry.title).bold());
            }
            println!();
            match changelogviewer::fetch_body(http, entry).await {
                Ok(body) => {
                    for line in body.lines().take(60) {
                        println!("  {}", line);
                    }
                    if body.lines().count() > 60 {
                        println!("  {} (truncated — open launchercontent.mojang.com for full notes)", style("…").dim());
                    }
                }
                Err(e) => println!("  {} Could not fetch full body: {}", style("⚠").yellow(), e),
            }
        }
    }
    Ok(())
}

// ── Mod Dependency Graph ──────────────────────────────────────────────────────

async fn dep_graph_menu(
    http: &reqwest::Client,
    instance_mgr: &InstanceManager,
    version_mgr: &VersionManager,
    game_dir: &PathBuf,
) -> Result<()> {
    let instances = instance_mgr.load_all().await?;

    let mods_dir = if instances.is_empty() {
        game_dir.join("mods")
    } else {
        let mut labels: Vec<String> = vec!["Default game dir".into()];
        labels.extend(instances.iter().map(|i| format!("{} [{}]", i.name, i.version_id)));
        let i = Select::with_theme(&theme())
            .with_prompt("Build dependency graph for")
            .items(&labels)
            .default(0)
            .interact()?;
        if i == 0 { game_dir.join("mods") }
        else { instance_mgr.instance_dir(&instances[i - 1].name).join("mods") }
    };

    // Pick game version for Modrinth lookups.
    let installed = version_mgr.list_installed().await?;
    let game_version = if installed.is_empty() {
        "1.21".to_string()
    } else {
        let v_labels: Vec<String> = installed.iter().map(|v| v.id.clone()).collect();
        let vi = Select::with_theme(&theme())
            .with_prompt("Game version (for Modrinth lookup)")
            .items(&v_labels)
            .default(0)
            .interact()?;
        installed[vi].id.clone()
    };

    println!("  {} Building dependency graph (may take a moment)...", style("→").cyan());
    match depgraph::build(http, &mods_dir, &game_version).await {
        Ok(nodes) if nodes.is_empty() => println!("  No mods found in directory."),
        Ok(nodes) => {
            println!();
            println!(
                "  {} Dependency Graph  {} = required  {} = optional",
                style("◆").cyan(),
                style("●").green(),
                style("○").yellow()
            );
            println!();
            depgraph::print_tree(&nodes);
        }
        Err(e) => println!("  {} {}", style("✗").red(), e),
    }
    Ok(())
}
