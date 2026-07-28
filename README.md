# Sumerian Client

> A Rust Minecraft launcher that brings every version — Classic through the latest snapshot — into the modern era with multi-account auth, instances, mods, shaders, textures, modpacks, and JVM optimization.

## Features

- **Version Manager** — Fetches Mojang's live manifest and installs any version: Classic, Alpha, Beta, Release, Snapshot. SHA1-verified downloads with progress bars.
- **Microsoft Authentication** — Full OAuth2 device-code flow (Microsoft → Xbox Live → Minecraft Services). Supports **multiple Microsoft accounts** simultaneously, per-account token storage, and automatic token refresh on launch.
- **Local Profiles** — Offline/local accounts with UUID generation (deterministic or random), rename, and remove.
- **Instance Manager** — Isolated game directories per instance, each with its own `mods/`, `saves/`, `resourcepacks/`, `shaderpacks/`, and `config/`.
  - **Clone** — Duplicate an existing instance (all files + profile) under a new name.
  - **Notes** — Attach a freeform text note to any instance, shown in the instance list.
  - **Per-instance mod profiles** — Enable/disable individual mods per instance by renaming `.jar` ↔ `.jar.disabled` without deleting them.
  - **Instance Profiles** — Per-instance overrides for optimization profile, Java binary, resolution, and custom JVM args stored as `instance_profile.json`.
- **Backup Manager** — Zip and restore the `saves/` directory of any instance with timestamped backups. Optional **auto-backup on launch** (toggle in Settings).
- **Mod Manager** — Search Modrinth by query + game version, browse results, pick a version, and download directly into any instance's `mods/` folder. List and remove installed mods. **Automatic dependency resolution** — required dependencies are fetched and installed alongside the selected mod.
- **Modpack Installer** — Search Modrinth for modpacks, select a version, and install the full `.mrpack` (overrides + mod files) into a named instance directory. Reports the required Minecraft version, Fabric loader, and Forge version.
- **Mod Loader Installer** — Install Fabric (via meta.fabricmc.net) or Forge (via official installer jar) directly from the launcher without leaving the CLI.
- **Mod Update Checker** — SHA1-hash each installed jar against Modrinth, detect newer versions, and apply updates in one step.
- **Server Browser** — Save favorite servers with name, address, and notes. TCP-pings each server and shows latency (or "offline") in the list.
- **Launch Presets** — Save named launch configurations with version, optimization profile, texture pack, shader preset, resolution, server quick-join, custom JVM args, and instance binding. Full create/edit/delete wizard.
- **Texture Injection** — Import resource packs (zip or folder) and inject them into `resourcepacks/`.
- **Shader Presets** — Four built-in presets (Vanilla Plus, Performance, Cinematic, Realistic) injected as OptiFine/Iris shaderpacks.
- **Optimization Profiles** — Five JVM profiles (Auto, Performance, Balanced, Quality, Potato). Auto detects system RAM and sets heap to 50% capped at 8 GB.
- **Settings / Config GUI** — Persistent launcher-wide settings: default optimization profile, auto-backup toggle, Discord RPC toggle, update-check toggle, default resolution, and per-major Java binary path overrides (Java 8 / 21 / 25).
- **Era Compatibility** — Detects Classic/Alpha/Beta/Release/Snapshot and selects the correct Java version automatically. Filters JVM flags that are invalid for older JVMs.
- **Java Auto-Install** — When the required Java version is missing, Sumerian downloads and extracts Temurin JDK automatically via the Adoptium API (Windows, macOS, Linux; x64 and aarch64). Falls back to direct GitHub release mirrors if the API is unavailable.
- **Java Version Mismatch Warning** — Detects the actual Java binary that will be used and warns visibly before launch if it doesn't match the version required by the manifest.
- **Asset Verification** — SHA1-checks existing asset objects before launch; re-downloads corrupt or missing files. Concurrent downloads capped at 32.
- **Launch History** — Records every launch (version, account, start time, duration, exit code). View newest-first with color-coded exit status.
- **Crash Log Parser** — On non-zero exit, finds the newest crash report, extracts description/exception/suspected mods, and diagnoses 9 known crash types with fix suggestions.
- **Discord Rich Presence** — Shows the current Minecraft version and username in Discord while the game is running (toggle in Settings).
- **Auto-Updater** — Checks GitHub Releases on startup and offers to atomically replace the running binary with the latest version.
- **News Feed** — Fetches and displays Mojang's latest Java patch notes from the launcher content API.
- **Skin Manager** — View, upload (Classic/Slim), and reset skins for Microsoft accounts via the Minecraft Services API.
- **World Manager** — List, rename, delete, and export (zip) save worlds per instance or the default game directory.
- **Screenshot Gallery** — Browse screenshots per instance, open individual images, or open the screenshots folder.
- **Multi-language UI** — English, Español, Français, Deutsch, Русский, 中文(简体). Language persists across restarts.

## Requirements

- Rust 1.75+ (`rustup update stable`)
- Java 8 and/or Java 21/25 installed — or let Sumerian auto-download Temurin
- A Microsoft account with a purchased copy of Minecraft (or a local offline profile)

## Build

```bash
cargo build --release
```

Binary: `target/release/sumerian.exe` (Windows) / `target/release/sumerian` (Linux/macOS)

## Run

```bash
cargo run --release
```

## Main Menu

```
  Install Version
  Install Mod Loader
  Launch Game
  Launch Preset
  Manage Presets
  Manage Accounts
  Manage Textures
  Manage Shaders
  Manage Instances
  Manage Mods
  Check Mod Updates
  Manage Skins
  Manage Worlds
  Screenshot Gallery
  View Installed Versions
  Launch History
  News
  Server Browser
  Install Modpack
  Settings
  Language / Idioma / Langue
  Exit
```

## Project Structure

```
Sumerian/
├── Cargo.toml
├── README.md
├── config/
│   └── shaders/
│       ├── vanilla_plus/shader.json
│       ├── performance/shader.json
│       ├── cinematic/shader.json
│       └── realistic/shader.json
└── src/
    ├── main.rs                    # Full CLI UI — all menus wired here
    ├── lang.rs                    # Multi-language strings (EN/ES/FR/DE/RU/ZH)
    ├── launcher/
    │   ├── mod.rs
    │   ├── auth.rs                # Microsoft OAuth2 + local profiles + multi-account
    │   ├── backup.rs              # Zip saves/ + restore
    │   ├── config.rs              # Global launcher settings (ConfigManager)
    │   ├── crash.rs               # Crash report parser + 9-pattern diagnostics
    │   ├── discord.rs             # Discord Rich Presence integration
    │   ├── downloader.rs          # SHA1-verified downloader, semaphore-capped assets
    │   ├── history.rs             # Launch records (last 100)
    │   ├── instances.rs           # Isolated instance dirs, clone, notes, mod profiles
    │   ├── loader.rs              # Fabric + Forge mod loader installer
    │   ├── manifest.rs            # Mojang version manifest + meta fetching
    │   ├── mod_updates.rs         # Modrinth hash-based update checker
    │   ├── modpacks.rs            # Modrinth mrpack modpack installer
    │   ├── mods.rs                # Modrinth search, download, dependency resolver
    │   ├── news.rs                # Mojang patch notes feed
    │   ├── presets.rs             # LaunchPreset (version/profile/tex/shader/res/server/jvm/instance)
    │   ├── screenshots.rs         # Screenshot gallery
    │   ├── servers.rs             # Server browser (favorites + TCP ping)
    │   ├── skins.rs               # Minecraft skin upload/reset via Services API
    │   ├── updater.rs             # GitHub Releases auto-updater
    │   └── version.rs             # Classpath + JVM arg builder
    ├── renderer/
    │   ├── mod.rs
    │   ├── textures.rs            # Resource pack import + injection
    │   ├── shaders.rs             # Shader preset loading + injection
    │   └── pipeline.rs            # Coordinates texture + shader application
    ├── optimizer/
    │   └── mod.rs                 # JVM optimization profiles + auto-tuner
    └── client/
        ├── mod.rs
        └── injection.rs           # Era detection, Java discovery + auto-install, process launcher
```

## Data Directory

| OS      | Path |
|---------|------|
| Windows | `%LOCALAPPDATA%\SumerianClient\` |
| macOS   | `~/Library/Application Support/SumerianClient/` |
| Linux   | `~/.local/share/SumerianClient/` |

```
SumerianClient/
├── game/
│   ├── versions/<id>/<id>.jar + <id>.json
│   ├── libraries/
│   ├── assets/indexes/ + objects/
│   ├── mods/
│   └── resourcepacks/
├── instances/
│   └── <name>/
│       ├── mods/
│       ├── saves/
│       ├── resourcepacks/
│       ├── shaderpacks/
│       ├── config/
│       ├── instance_profile.json
│       └── mod_profile.json
├── accounts/
│   └── <uuid>.json
├── backups/
│   └── <instance>_<timestamp>.zip
├── java/
│   └── <major>/bin/java(.exe)     # Auto-downloaded Temurin JDKs
├── textures/
│   ├── packs/
│   └── active/
├── config/
│   ├── shaders/<preset>/shader.json
│   ├── launcher.json              # Global settings
│   └── lang.json                  # Language preference
├── presets.json
├── instances.json
├── servers.json
├── profiles.json
└── launch_history.json
```

## Shader Presets

| Preset       | Shadows | Water | Bloom | AA   |
|--------------|---------|-------|-------|------|
| Vanilla Plus | 2       | ✓     | ✗     | FXAA |
| Performance  | 0       | ✗     | ✗     | None |
| Cinematic    | 4       | ✓     | ✓     | TAA  |
| Realistic    | 3       | ✓     | ✓     | MSAA |

Requires OptiFine or Iris installed in the game version.

## Optimization Profiles

| Profile     | Max Heap        | GC         | Use Case        |
|-------------|-----------------|------------|-----------------|
| Auto        | 50% RAM (≤8 GB) | G1GC tuned | Recommended     |
| Performance | 4 GB            | G1GC tuned | High-end PCs    |
| Balanced    | 2 GB            | G1GC       | Most systems    |
| Quality     | 6 GB            | G1GC large | High-res/modded |
| Potato      | 512 MB          | SerialGC   | Low-end PCs     |

## Authentication

Sumerian uses the official Microsoft device-code OAuth2 flow:

1. A code and URL are shown in the terminal
2. Open the URL in your browser and enter the code
3. The session is saved to `accounts/<uuid>.json`

Multiple Microsoft accounts are supported — each stored separately and selectable at launch. Tokens are automatically refreshed when an account is selected. Manual refresh is also available in Manage Accounts.

No passwords are stored — only the access token and refresh token returned by Microsoft.

## Java Discovery & Auto-Install

Sumerian automatically selects the correct Java version based on `javaVersion.majorVersion` in Mojang's version manifest:

- Classic / Alpha / Beta / legacy releases → Java 8
- 1.17–1.20 → Java 21
- 1.21+ → Java 25

Discovery order:
1. `JAVA8_HOME` / `JAVA21_HOME` / `JAVA25_HOME` environment variables
2. Per-instance Java path override (set in instance profile)
3. Global Java path override (set in Settings)
4. Sumerian-managed JDK directory (`SumerianClient/java/<major>/`)
5. Well-known install paths (Windows, macOS, Linux)
6. `java` on `PATH`

If no matching Java is found, Sumerian automatically downloads and extracts Temurin JDK via the Adoptium API. Falls back to direct GitHub release mirrors. A visible warning is shown before launch if the found binary doesn't match the required version.

## Mod Loader Installation

Fabric and Forge can be installed directly from the "Install Mod Loader" menu:

- **Fabric** — fetches available loader versions from meta.fabricmc.net, writes an inheriting version JSON, and downloads loader libraries into `game/libraries/`.
- **Forge** — downloads the official Forge installer jar and runs it headlessly with `--installClient`.

## Modpack Installation

Search Modrinth for modpacks, select a version, and Sumerian will:

1. Download the `.mrpack` archive
2. Extract `overrides/` and `client-overrides/` into the instance directory
3. Download all mod files listed in `modrinth.index.json`
4. Report the required Minecraft version and mod loader versions

## Mod Dependency Resolution

When installing a mod from Modrinth, Sumerian automatically checks the version's `dependencies` list and downloads any `required` dependencies that aren't already installed in the mods directory.

## Crash Diagnostics

When a game session exits with a non-zero code, Sumerian automatically parses the newest crash report and diagnoses the cause:

| Pattern              | Suggested Fix                              |
|----------------------|--------------------------------------------|
| OutOfMemoryError     | Increase heap / switch to Quality profile  |
| StackOverflowError   | Check for recursive mod code               |
| ClassNotFoundException | Missing mod dependency or wrong loader   |
| UnsupportedClassVersion | Java version too old for this MC build  |
| OpenGL / LWJGL error | Update GPU drivers                         |
| Mixin error          | Mod incompatibility — remove suspect mod   |
| NullPointerException | Likely mod bug — check suspected mods      |
| Display creation     | No display / headless environment          |
| Network error        | Check internet connection                  |

## Settings

Accessible from the main menu under **Settings**:

| Setting                  | Default   | Description                                      |
|--------------------------|-----------|--------------------------------------------------|
| Default optimization     | Balanced  | Profile used when no instance override is set    |
| Auto-backup on launch    | Off       | Zip saves/ before every game session             |
| Discord RPC              | On        | Show version + username in Discord               |
| Check updates on start   | On        | Check GitHub Releases at startup                 |
| Default resolution       | —         | Global width × height override                   |
| Java 8 / 21 / 25 path    | auto      | Override the binary used for each Java major     |

## Known Limitations

- **Shader rendering** requires OptiFine or Iris — Sumerian injects preset files but does not bundle GLSL shaders.
- **Forge headless install** may fail on some Forge versions that require GUI interaction.
- **Very old versions** (Classic, early Alpha) may not have full asset downloads available from Mojang's CDN.
- **Modpack auto-install** does not automatically install the required Minecraft version or mod loader — those must be installed separately via the launcher menus.
