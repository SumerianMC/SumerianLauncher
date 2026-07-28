use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::launcher::auth::AuthSession;
use crate::launcher::manifest::VersionMeta;
use crate::launcher::version::VersionManager;
use crate::optimizer::OptimizationProfile;

/// Download URL shown to the user when auto-install fails.
pub fn java_download_url(major: u32) -> &'static str {
    match major {
        8  => "https://adoptium.net/temurin/releases/?version=8",
        21 => "https://adoptium.net/temurin/releases/?version=21",
        25 => "https://adoptium.net/temurin/releases/?version=25",
        _  => "https://adoptium.net/temurin/releases/",
    }
}

/// Try to download and extract Temurin JDK for `major` into `dest_dir`.
/// Tries Adoptium API first, then falls back to direct archive mirrors.
pub async fn try_auto_install_java(
    http: &reqwest::Client,
    major: u32,
    dest_dir: &PathBuf,
) -> Result<PathBuf> {
    let os = if cfg!(target_os = "windows") { "windows" }
             else if cfg!(target_os = "macos") { "mac" }
             else { "linux" };
    let arch = if cfg!(target_arch = "x86_64") { "x64" } else { "aarch64" };
    let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };

    // Build candidate URLs: Adoptium API first, then direct fallback mirrors
    let mut urls: Vec<String> = Vec::new();

    // 1. Adoptium API
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?os={os}&architecture={arch}&image_type=jdk"
    );
    #[derive(serde::Deserialize)]
    struct AdoptiumAsset { binary: AdoptiumBinary }
    #[derive(serde::Deserialize)]
    struct AdoptiumBinary { package: AdoptiumPackage }
    #[derive(serde::Deserialize)]
    struct AdoptiumPackage { link: String }

    if let Ok(resp) = http.get(&api).send().await {
        if let Ok(assets) = resp.json::<Vec<AdoptiumAsset>>().await {
            if let Some(asset) = assets.into_iter().next() {
                urls.push(asset.binary.package.link);
            }
        }
    }

    // 2. Direct Adoptium archive fallback mirrors
    let (win_zip, linux_tar) = match major {
        8  => (
            "https://github.com/adoptium/temurin8-binaries/releases/download/jdk8u392-b08/OpenJDK8U-jdk_x64_windows_hotspot_8u392b08.zip",
            "https://github.com/adoptium/temurin8-binaries/releases/download/jdk8u392-b08/OpenJDK8U-jdk_x64_linux_hotspot_8u392b08.tar.gz",
        ),
        16 => (
            "https://github.com/adoptium/temurin16-binaries/releases/download/jdk-16.0.2%2B7/OpenJDK16U-jdk_x64_windows_hotspot_16.0.2_7.zip",
            "https://github.com/adoptium/temurin16-binaries/releases/download/jdk-16.0.2%2B7/OpenJDK16U-jdk_x64_linux_hotspot_16.0.2_7.tar.gz",
        ),
        17 => (
            "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.9%2B9/OpenJDK17U-jdk_x64_windows_hotspot_17.0.9_9.zip",
            "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.9%2B9/OpenJDK17U-jdk_x64_linux_hotspot_17.0.9_9.tar.gz",
        ),
        21 => (
            "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.1%2B12/OpenJDK21U-jdk_x64_windows_hotspot_21.0.1_12.zip",
            "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.1%2B12/OpenJDK21U-jdk_x64_linux_hotspot_21.0.1_12.tar.gz",
        ),
        _ => ("", ""),
    };
    let fallback = if cfg!(target_os = "windows") { win_zip } else { linux_tar };
    if !fallback.is_empty() { urls.push(fallback.to_string()); }

    // Try each URL in order
    let mut last_err = anyhow::anyhow!("No download URLs available for Java {major}");
    for url in &urls {
        println!("  → Downloading Java {major}...");
        match http.get(url).send().await.and_then(|r| Ok(r)) {
            Ok(resp) if resp.status().is_success() => {
                let bytes = resp.bytes().await?;
                std::fs::create_dir_all(dest_dir)?;
                let archive_path = dest_dir.join(format!("jdk{major}.{ext}"));
                std::fs::write(&archive_path, &bytes)?;

                println!("  → Extracting Java {major}...");
                if cfg!(target_os = "windows") {
                    let file = std::fs::File::open(&archive_path)?;
                    let mut zip = zip::ZipArchive::new(file)?;
                    zip.extract(dest_dir)?;
                } else {
                    let status = Command::new("tar")
                        .args(["-xzf", archive_path.to_str().unwrap(), "-C", dest_dir.to_str().unwrap(), "--strip-components=1"])
                        .status()?;
                    if !status.success() { bail!("tar extraction failed"); }
                }
                let _ = std::fs::remove_file(&archive_path);

                // Find binary up to 2 levels deep
                let bin_name = if cfg!(target_os = "windows") { "java.exe" } else { "java" };
                let candidate = dest_dir.join("bin").join(bin_name);
                if candidate.exists() { return Ok(candidate); }
                for entry in std::fs::read_dir(dest_dir)?.flatten() {
                    let p = entry.path().join("bin").join(bin_name);
                    if p.exists() { return Ok(p); }
                    if entry.path().is_dir() {
                        for inner in std::fs::read_dir(entry.path())?.flatten() {
                            let p2 = inner.path().join("bin").join(bin_name);
                            if p2.exists() { return Ok(p2); }
                        }
                    }
                }
                bail!("Could not locate java binary after extraction");
            }
            Ok(resp) => { last_err = anyhow::anyhow!("HTTP {}", resp.status()); }
            Err(e) => { last_err = e.into(); }
        }
    }
    Err(last_err)
}

/// Era-specific compatibility adapter.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionEra {
    Classic,
    Alpha,
    Beta,
    /// Release versions that use launchwrapper and require Java 8 (1.0 – 1.16.x)
    ReleaseLegacy,
    /// Modern releases with the new launcher format (1.17+)
    Release,
    Snapshot,
}

impl VersionEra {
    pub fn detect(version_id: &str, version_type: &str, meta: &VersionMeta) -> Self {
        if version_id.starts_with("c") || version_id.starts_with("rd-") {
            return Self::Classic;
        }
        if version_id.starts_with("a") || version_type == "old_alpha" {
            return Self::Alpha;
        }
        if version_id.starts_with("b") || version_type == "old_beta" {
            return Self::Beta;
        }

        // Mojang sets javaVersion.majorVersion = 16 for 1.17, 17 for 1.18+.
        // Anything below that (or absent) is a legacy release needing Java 8.
        let needs_java8 = meta
            .java_version
            .as_ref()
            .map(|j| j.major_version < 16)
            .unwrap_or(true); // very old versions have no javaVersion field

        if version_type == "snapshot" {
            // Snapshots before 1.17 also need Java 8
            return if needs_java8 {
                Self::ReleaseLegacy
            } else {
                Self::Snapshot
            };
        }

        if needs_java8 {
            Self::ReleaseLegacy
        } else {
            Self::Release
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Alpha => "Alpha",
            Self::Beta => "Beta",
            Self::ReleaseLegacy => "Release (legacy)",
            Self::Release => "Release",
            Self::Snapshot => "Snapshot",
        }
    }

    /// Returns true for any era that uses launchwrapper / URLClassLoader
    /// and therefore requires Java 8.
    pub fn requires_java8(&self) -> bool {
        matches!(self, Self::Classic | Self::Alpha | Self::Beta | Self::ReleaseLegacy)
    }

    /// JVM flags that fix known incompatibilities for this era.
    pub fn extra_jvm_args(&self) -> Vec<String> {
        match self {
            // Java 8 eras: legacy sort + blank proxy host (avoids NPE in old net code)
            Self::Classic | Self::Alpha | Self::Beta | Self::ReleaseLegacy => vec![
                "-Djava.util.Arrays.useLegacyMergeSort=true".into(),
                "-Dhttp.proxyHost=".into(),
            ],
            _ => vec![],
        }
    }
}

pub struct LaunchOptions<'a> {
    pub session: &'a AuthSession,
    pub profile: &'a OptimizationProfile,
    pub custom_jvm_args: &'a [String],
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub server: Option<&'a str>,
    pub port: Option<u16>,
    pub game_dir_override: Option<PathBuf>,
}

pub struct GameLauncher {
    pub game_dir: PathBuf,
}

impl GameLauncher {
    pub fn new(game_dir: PathBuf) -> Self {
        Self { game_dir }
    }

    pub fn launch(
        &self,
        meta: &VersionMeta,
        opts: &LaunchOptions,
        version_manager: &VersionManager,
    ) -> Result<std::process::Child> {
        let jar_path = self
            .game_dir
            .join("versions")
            .join(&meta.id)
            .join(format!("{}.jar", meta.id));

        if !jar_path.exists() {
            bail!("Client JAR not found: {}", jar_path.display());
        }

        let effective_game_dir = opts.game_dir_override.clone().unwrap_or_else(|| self.game_dir.clone());

        let natives_dir = self
            .game_dir
            .join("versions")
            .join(&meta.id)
            .join("natives");

        let era = VersionEra::detect(&meta.id, &meta.version_type, meta);

        let required_java = if era.requires_java8() {
            8
        } else {
            meta.java_version.as_ref().map(|j| j.major_version).unwrap_or(21)
        };

        let java_path = find_java_for_major(required_java).unwrap_or_else(|| {
            eprintln!();
            eprintln!("  ⚠  WARNING: {} requires Java {} but no matching installation was found.", meta.id, required_java);
            eprintln!();
            PathBuf::from(if cfg!(target_os = "windows") { "java.exe" } else { "java" })
        });

        // Warn if the Java we found doesn't match what the manifest requests
        if let Some(actual) = detect_java_major(&java_path) {
            if actual != required_java {
                eprintln!();
                eprintln!("  ⚠  WARNING: Manifest requires Java {} but found Java {} at {}",
                    required_java, actual, java_path.display());
                eprintln!("     The game may crash. Install Java {} to fix this.", required_java);
                eprintln!();
            }
        }

        let classpath =
            version_manager.build_classpath(meta, &self.game_dir, &jar_path);

        let mut jvm_args =
            version_manager.build_jvm_args(meta, &self.game_dir, &natives_dir, opts.profile, required_java);

        jvm_args.extend(era.extra_jvm_args());

        // Redirect Mojang auth API calls to ely.by when using an ely.by account
        if opts.session.auth_type == crate::launcher::auth::AuthType::ElyBy {
            jvm_args.push("-Dminecraft.api.auth.host=https://authserver.ely.by".into());
            jvm_args.push("-Dminecraft.api.session.host=https://sessionserver.ely.by".into());
            jvm_args.push("-Dminecraft.api.services.host=https://api.ely.by".into());
            jvm_args.push("-Dminecraft.api.profiles.host=https://sessionserver.ely.by".into());
        }

        jvm_args.extend_from_slice(opts.custom_jvm_args);

        let mut game_args = version_manager.build_game_args(
            meta,
            &effective_game_dir,
            &opts.session.username,
            opts.session.effective_token(),
            &opts.session.uuid,
        );

        if let (Some(w), Some(h)) = (opts.width, opts.height) {
            game_args.push("--width".into());
            game_args.push(w.to_string());
            game_args.push("--height".into());
            game_args.push(h.to_string());
        }

        if let Some(srv) = opts.server {
            game_args.push("--server".into());
            game_args.push(srv.into());
            if let Some(p) = opts.port {
                game_args.push("--port".into());
                game_args.push(p.to_string());
            }
        }

        let mut cmd = Command::new(&java_path);
        cmd.args(&jvm_args);
        cmd.arg("-cp").arg(&classpath);
        cmd.arg(&meta.main_class);
        cmd.args(&game_args);
        cmd.current_dir(&effective_game_dir);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        println!();
        println!("  Launching {} ({})...", meta.id, era.display_name());
        println!("  Java:    {} (Java {})", java_path.display(), required_java);
        println!("  Profile: {}", opts.profile);
        println!();

        let child = cmd.spawn().context("Failed to spawn Java process")?;
        Ok(child)
    }
}

// ── Java discovery ────────────────────────────────────────────────────────────

/// Find a Java binary matching the requested major version.
/// Falls back to the next best available version if exact match not found.
pub fn find_java_for_major(major: u32) -> Option<PathBuf> {
    // Env var overrides: JAVA8_HOME, JAVA21_HOME, JAVA25_HOME, JAVA_HOME
    let env_var = match major {
        8  => "JAVA8_HOME",
        16 => "JAVA16_HOME",
        21 => "JAVA21_HOME",
        25 => "JAVA25_HOME",
        _  => "JAVA_HOME",
    };
    if let Ok(home) = std::env::var(env_var) {
        let p = java_bin_in(&home);
        if p.exists() { return Some(p); }
    }

    // Check SumerianClient managed java dir (handles auto-downloaded JDKs)
    let managed = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("SumerianClient").join("java").join(major.to_string());
    if managed.exists() {
        let bin_name = if cfg!(target_os = "windows") { "java.exe" } else { "java" };
        // Check direct, one level, and two levels deep
        let direct = managed.join("bin").join(bin_name);
        if direct.exists() { return Some(direct); }
        if let Ok(entries) = std::fs::read_dir(&managed) {
            for entry in entries.flatten() {
                let p = entry.path().join("bin").join(bin_name);
                if p.exists() { return Some(p); }
                if entry.path().is_dir() {
                    if let Ok(inner_entries) = std::fs::read_dir(entry.path()) {
                        for inner in inner_entries.flatten() {
                            let p2 = inner.path().join("bin").join(bin_name);
                            if p2.exists() { return Some(p2); }
                        }
                    }
                }
            }
        }
    }

    // Ordered candidate lists per major version
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        match major {
            8 => &[
                r"C:\Program Files\BellSoft\LibericaJDK-8\bin\java.exe",
                r"C:\Program Files\BellSoft\LibericaJRE-8\bin\java.exe",
                r"C:\Program Files\Eclipse Adoptium\jre-8.0.392.8-hotspot\bin\java.exe",
                r"C:\Program Files\Eclipse Adoptium\jdk-8.0.392.8-hotspot\bin\java.exe",
                r"C:\Program Files\Java\jre1.8.0_391\bin\java.exe",
                r"C:\Program Files\Java\jre1.8.0_381\bin\java.exe",
                r"C:\Program Files\Java\jdk1.8.0_391\bin\java.exe",
                r"C:\Program Files\Java\jdk1.8.0_381\bin\java.exe",
                r"C:\Program Files\Amazon Corretto\jre8\bin\java.exe",
                r"C:\Program Files\Amazon Corretto\jdk8\bin\java.exe",
            ],
            21 => &[
                r"C:\Program Files\Java\jdk-21.0.10\bin\java.exe",
                r"C:\Program Files\Eclipse Adoptium\jdk-21.0.1.12-hotspot\bin\java.exe",
                r"C:\Program Files\Eclipse Adoptium\jre-21.0.1.12-hotspot\bin\java.exe",
                r"C:\Program Files\Microsoft\jdk-21.0.1.12-hotspot\bin\java.exe",
                r"C:\Program Files\Java\jdk-21\bin\java.exe",
                r"C:\Program Files\Java\jre-21\bin\java.exe",
                r"C:\Program Files\Amazon Corretto\jdk21\bin\java.exe",
            ],
            25 => &[
                r"C:\Program Files\Java\jdk-25.0.2\bin\java.exe",
                r"C:\Program Files\Java\jdk-25\bin\java.exe",
            ],
            _ => &[],
        }
    } else if cfg!(target_os = "macos") {
        match major {
            8  => &[
                "/Library/Java/JavaVirtualMachines/temurin-8.jdk/Contents/Home/bin/java",
                "/Library/Java/JavaVirtualMachines/jdk1.8.0_391.jdk/Contents/Home/bin/java",
                "/Library/Java/JavaVirtualMachines/amazon-corretto-8.jdk/Contents/Home/bin/java",
            ],
            21 => &[
                "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java",
                "/usr/bin/java",
            ],
            25 => &[
                "/Library/Java/JavaVirtualMachines/temurin-25.jdk/Contents/Home/bin/java",
            ],
            _  => &["/usr/bin/java"],
        }
    } else {
        match major {
            8  => &[
                "/usr/lib/jvm/java-8-openjdk-amd64/bin/java",
                "/usr/lib/jvm/java-8-openjdk/bin/java",
                "/usr/lib/jvm/temurin-8/bin/java",
                "/usr/lib/jvm/java-8-amazon-corretto/bin/java",
            ],
            21 => &[
                "/usr/lib/jvm/java-21-openjdk-amd64/bin/java",
                "/usr/bin/java",
            ],
            25 => &[
                "/usr/lib/jvm/java-25-openjdk-amd64/bin/java",
            ],
            _  => &["/usr/bin/java"],
        }
    };

    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() { return Some(p); }
    }

    find_java_on_path_with_version(major)
}

fn java_bin_in(home: &str) -> PathBuf {
    PathBuf::from(home)
        .join("bin")
        .join(if cfg!(target_os = "windows") { "java.exe" } else { "java" })
}

/// Run `java -version` on a specific binary and return its major version.
pub fn detect_java_major(java_path: &PathBuf) -> Option<u32> {
    let output = Command::new(java_path)
        .arg("-version")
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    parse_java_major_version(&text)
}

/// Run `java -version` on the PATH java and return it if its major version matches.
fn find_java_on_path_with_version(want_major: u32) -> Option<PathBuf> {
    let java = if cfg!(target_os = "windows") { "java.exe" } else { "java" };
    let output = Command::new(java)
        .arg("-version")
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .ok()?;
    // `java -version` prints to stderr: 'java version "1.8.0_391"' or 'openjdk version "21.0.1"'
    let text = String::from_utf8_lossy(&output.stderr);
    let major = parse_java_major_version(&text)?;
    if major == want_major {
        Some(PathBuf::from(java))
    } else {
        None
    }
}

/// Parse the major version out of `java -version` stderr output.
/// Handles both legacy "1.8.x" and modern "21.x" version strings.
fn parse_java_major_version(text: &str) -> Option<u32> {
    // Find the quoted version string, e.g. "1.8.0_391" or "21.0.1"
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    let version = &text[start..end];

    let first = version.split('.').next()?;
    let major: u32 = first.parse().ok()?;

    // Legacy scheme: "1.8" → major 8
    if major == 1 {
        let second: u32 = version.split('.').nth(1)?.parse().ok()?;
        Some(second)
    } else {
        Some(major)
    }
}
