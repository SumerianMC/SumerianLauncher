# Sumerian Client

> A Rust Minecraft launcher that brings every version — Classic through the latest snapshot — into the modern era with multi-account auth, instances, mods, shaders, textures, and JVM optimization.

## Features

- **Version Manager** — Fetches Mojang's live manifest and installs any version: Classic, Alpha, Beta, Release, Snapshot. SHA1-verified downloads with progress bars.
- **Microsoft Authentication** — Full OAuth2 device-code flow (Microsoft → Xbox Live → Minecraft Services). Supports **multiple Microsoft accounts** simultaneously, per-account token storage, and automatic token refresh on launch.
- **Local Profiles** — Offline/local accounts with UUID generation, rename, and remove.
- **Instance Manager** — Isolated game directories per instance, each with its own `mods/`, `saves/`, `resourcepacks/`, `shaderpacks/`, and `config/`.
- **Backup Manager** — Zip and restore the `saves/` directory of any instance with timestamped backups.
- **Mod Manager** — Search Modrinth by query + game version, browse results, pick a version, and download directly into any instance's `mods/` folder. List and remove installed mods.
- **Launch Presets** — Save named launch configurations with version, optimization profile, texture pack, shader preset, resolution, server quick-join, custom JVM args, and instance binding. Full create/edit/delete wizard.
- **Texture Injection** — Import resource packs (zip or folder) and inject them into `resourcepacks/`.
- **Shader Presets** — Four built-in presets (Vanilla Plus, Performance, Cinematic, Realistic) injected as OptiFine/Iris shaderpacks.
- **Optimization Profiles** — Four JVM profiles (Performance, Balanced, Quality, Potato) with tuned G1GC flags.
- **Era Compatibility** — Detects Classic/Alpha/Beta/Release/Snapshot and selects the correct Java version automatically. Filters JVM flags that are invalid for older JVMs (e.g. `--sun-misc-unsafe-memory-access` only on Java 23+).
- **Java Version Mismatch Warning** — Detects the actual Java binary that will be used and warns visibly before launch if it doesn't match the version required by the manifest.
- **Asset Verification** — SHA1-checks existing asset objects before launch; re-downloads corrupt or missing files. Concurrent downloads capped at 32.
- **Launch History** — Records every launch (version, account, start time, duration, exit code). View newest-first with color-coded exit status.
- **Crash Log Viewer** — On non-zero exit, automatically finds and prints the newest crash report (first 60 lines).
- **News Feed** — Fetches and displays Mojang's latest Java patch notes from the launcher content API.

## Requirements

- Rust 1.75+ (`rustup update stable`)
- Java 8 and/or Java 21/25 installed (matched automatically per version)
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
  Launch Game
  Launch Preset
  Manage Presets
  Manage Accounts
  Manage Textures
  Manage Shaders
  Manage Instances
  Manage Mods
  View Installed Versions
  Launch History
  News
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
    ├── launcher/
    │   ├── mod.rs
    │   ├── manifest.rs            # Mojang version manifest + meta fetching
    │   ├── downloader.rs          # SHA1-verified downloader, semaphore-capped assets
    │   ├── version.rs             # Classpath + JVM arg builder
    │   ├── auth.rs                # Microsoft OAuth2 + local profiles + multi-account
    │   ├── presets.rs             # LaunchPreset (version/profile/tex/shader/res/server/jvm/instance)
    │   ├── instances.rs           # Isolated instance directories
    │   ├── backup.rs              # Zip saves/ + restore
    │   ├── mods.rs                # Modrinth search, download, list, remove
    │   ├── history.rs             # Launch records (last 100)
    │   └── news.rs                # Mojang patch notes feed
    ├── renderer/
    │   ├── mod.rs
    │   ├── textures.rs            # Resource pack import + injection
    │   ├── shaders.rs             # Shader preset loading + injection
    │   └── pipeline.rs            # Coordinates texture + shader application
    ├── optimizer/
    │   └── mod.rs                 # JVM optimization profiles
    └── client/
        ├── mod.rs
        └── injection.rs           # Era detection, Java discovery, process launcher
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
│       └── config/
├── accounts/
│   └── <uuid>.json                # One file per Microsoft account
├── backups/
│   └── <instance>_<timestamp>.zip
├── textures/
│   ├── packs/
│   └── active/
├── config/
│   └── shaders/<preset>/shader.json
├── presets.json
├── instances.json
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

| Profile     | Max Heap | GC         | Use Case        |
|-------------|----------|------------|-----------------|
| Performance | 4 GB     | G1GC tuned | High-end PCs    |
| Balanced    | 2 GB     | G1GC       | Most systems    |
| Quality     | 6 GB     | G1GC large | High-res/modded |
| Potato      | 512 MB   | SerialGC   | Low-end PCs     |

## Authentication

Sumerian uses the official Microsoft device-code OAuth2 flow:

1. A code and URL are shown in the terminal
2. Open the URL in your browser and enter the code
3. The session is saved to `accounts/<uuid>.json`

Multiple Microsoft accounts are supported — each stored separately and selectable at launch. Tokens are automatically refreshed when an account is selected. Manual refresh is also available in Manage Accounts.

No passwords are stored — only the access token and refresh token returned by Microsoft.

## Java Discovery

Sumerian automatically selects the correct Java version based on `javaVersion.majorVersion` in Mojang's version manifest:

- Classic / Alpha / Beta / legacy releases → Java 8
- 1.17–1.20 → Java 21
- 1.21+ (e.g. 26.x snapshots) → Java 25

Discovery checks `JAVA8_HOME` / `JAVA21_HOME` / `JAVA25_HOME` env vars first, then well-known install paths, then falls back to `java` on `PATH`. A visible warning is shown before launch if the found binary doesn't match the required version.

## Known Limitations

- **Shader rendering** requires OptiFine or Iris installed in the game version. Sumerian injects the preset files; actual GLSL rendering is handled by those mods.
- **Very old versions** (Classic, early Alpha) may not have full asset downloads available from Mojang's CDN.
- **Offline mode** is not supported — a valid Microsoft account or local profile is required.
- **Mod loader** (Fabric/Forge) installation is not handled — install the loader manually into the version's JAR before using the mod manager.
