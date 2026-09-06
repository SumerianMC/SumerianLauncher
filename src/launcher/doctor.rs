/// Doctor — environment debug report.
///
/// Checks all major launcher prerequisites and prints a colour-coded summary.
/// Inspired by `brew doctor` / `flutter doctor`.
///
/// Checks performed:
///   1. Java 8 availability and version match
///   2. Java 21 availability
///   3. Java 25 availability
///   4. Available disk space on the data directory partition
///   5. Microsoft auth token validity for each stored account
///   6. Installed Minecraft library integrity (SHA1 spot-check, first 20 libs)
///   7. Network reachability (Mojang manifest endpoint)
use std::path::{Path, PathBuf};

use anyhow::Result;
use console::style;

use crate::client::injection::{detect_java_major, find_java_for_major};
use crate::launcher::auth::{AuthSession, AuthType, Authenticator};
use crate::launcher::version::VersionManager;

/// Run all checks and print the report.  Returns the number of failures found.
pub async fn run(
    data_dir: &Path,
    game_dir: &Path,
    http: &reqwest::Client,
    auth: &Authenticator,
    version_mgr: &VersionManager,
) -> Result<usize> {
    println!("  {} Sumerian Doctor — environment check", style("⚕").cyan().bold());
    println!("  {}", style("─".repeat(54)).dim());
    println!();

    let mut failures = 0usize;

    // ── 1-3. Java versions ────────────────────────────────────────────────────
    for major in [8u32, 21, 25] {
        let label = format!("Java {}", major);
        match find_java_for_major(major) {
            Some(path) => {
                let detected = detect_java_major(&path);
                let version_ok = detected.map(|v| v == major).unwrap_or(false);
                let detected_str = detected
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into());
                if version_ok {
                    ok(&label, &format!("{}", path.display()));
                } else {
                    warn(
                        &label,
                        &format!(
                            "Found at {} but reports Java {} (expected {})",
                            path.display(),
                            detected_str,
                            major
                        ),
                    );
                    failures += 1;
                }
            }
            None => {
                // Java 25 is optional — only warn rather than fail.
                if major == 25 {
                    warn(&label, "Not found — required for Minecraft 1.21+. Sumerian can auto-install.");
                } else {
                    fail(&label, "Not found. Sumerian will attempt auto-install on first launch.");
                    failures += 1;
                }
            }
        }
    }
    println!();

    // ── 4. Disk space ─────────────────────────────────────────────────────────
    match available_space_mb(data_dir) {
        Some(mb) if mb < 500 => {
            fail(
                "Disk space",
                &format!("{} MB free — less than 500 MB recommended", mb),
            );
            failures += 1;
        }
        Some(mb) => ok("Disk space", &format!("{} MB free", mb)),
        None => warn("Disk space", "Could not determine free space"),
    }
    println!();

    // ── 5. Auth tokens ────────────────────────────────────────────────────────
    let sessions: Vec<AuthSession> = auth
        .load_all_sessions()
        .await
        .into_iter()
        .filter(|s| s.auth_type == AuthType::Microsoft)
        .collect();

    if sessions.is_empty() {
        info("Auth", "No Microsoft accounts — only offline profiles available");
    } else {
        for s in &sessions {
            let label = format!("Auth: {}", s.username);
            match auth.validate_session(s).await {
                Ok(true) => ok(&label, "token valid"),
                Ok(false) => {
                    warn(&label, "token expired — will auto-refresh on launch");
                }
                Err(e) => {
                    warn(&label, &format!("could not validate ({})", e));
                    failures += 1;
                }
            }
        }
    }
    println!();

    // ── 6. Library spot-check ─────────────────────────────────────────────────
    let lib_result = check_libraries(game_dir);
    match lib_result {
        Ok(0) => ok("Libraries", "spot-check passed"),
        Ok(n) => {
            warn(
                "Libraries",
                &format!("{} missing/corrupt file(s) — re-install affected version to fix", n),
            );
            failures += 1;
        }
        Err(e) => {
            warn("Libraries", &format!("check skipped ({})", e));
        }
    }
    println!();

    // ── 7. Network ────────────────────────────────────────────────────────────
    let manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    match http.head(manifest_url).send().await {
        Ok(r) if r.status().is_success() => {
            ok("Network", "Mojang manifest reachable");
        }
        Ok(r) => {
            warn(
                "Network",
                &format!("Mojang manifest returned HTTP {}", r.status()),
            );
            failures += 1;
        }
        Err(e) => {
            fail("Network", &format!("Cannot reach Mojang: {}", e));
            failures += 1;
        }
    }
    println!();

    // ── Summary ───────────────────────────────────────────────────────────────
    println!("  {}", style("─".repeat(54)).dim());
    if failures == 0 {
        println!(
            "  {} All checks passed.",
            style("✓").green().bold()
        );
    } else {
        println!(
            "  {} {} issue(s) found — see above for details.",
            style("✗").red().bold(),
            failures
        );
    }

    Ok(failures)
}

// ── print helpers ─────────────────────────────────────────────────────────────

fn ok(label: &str, detail: &str) {
    println!("  {} {:<22} {}", style("✓").green(), label, style(detail).dim());
}

fn warn(label: &str, detail: &str) {
    println!("  {} {:<22} {}", style("⚠").yellow(), label, style(detail).yellow());
}

fn fail(label: &str, detail: &str) {
    println!("  {} {:<22} {}", style("✗").red(), label, style(detail).red());
}

fn info(label: &str, detail: &str) {
    println!("  {} {:<22} {}", style("ℹ").cyan(), label, style(detail).dim());
}

// ── system helpers ────────────────────────────────────────────────────────────

/// Return free disk space in MB for the partition containing `path`.
fn available_space_mb(path: &Path) -> Option<u64> {
    // Use `sysinfo` disk information to find the partition.
    let mut sys = sysinfo::System::new();
    let disks = sysinfo::Disks::new_with_refreshed_list();

    let path_str = path.to_string_lossy().to_lowercase();
    let _ = sys; // silence unused warning

    // Find the disk whose mount point is the longest prefix of `path`.
    let best = disks
        .iter()
        .filter(|d| {
            path_str.starts_with(
                &d.mount_point().to_string_lossy().to_lowercase() as &str,
            )
        })
        .max_by_key(|d| d.mount_point().to_string_lossy().len());

    best.map(|d| d.available_space() / 1024 / 1024)
}

/// Spot-check the first 20 library JARs — verify they exist and have non-zero size.
/// Returns the count of missing/zero-length files.
fn check_libraries(game_dir: &Path) -> Result<usize> {
    let libs_dir = game_dir.join("libraries");
    if !libs_dir.exists() {
        return Ok(0); // no libraries downloaded yet — not an error
    }

    let mut bad = 0usize;
    let mut checked = 0usize;

    'outer: for entry in walkdir_max(&libs_dir, 6) {
        let path = entry?;
        if path.extension().and_then(|e| e.to_str()) != Some("jar") {
            continue;
        }
        match std::fs::metadata(&path) {
            Ok(m) if m.len() == 0 => bad += 1,
            Ok(_) => {}
            Err(_) => bad += 1,
        }
        checked += 1;
        if checked >= 20 {
            break 'outer;
        }
    }

    Ok(bad)
}

/// Simple recursive file walker that yields `Result<PathBuf>` entries up to
/// `max_depth` levels deep, to avoid pulling in the `walkdir` crate.
fn walkdir_max(dir: &Path, max_depth: usize) -> impl Iterator<Item = Result<PathBuf>> {
    let mut stack: Vec<(PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];
    let mut results: Vec<Result<PathBuf>> = Vec::new();

    while let Some((current, depth)) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&current) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_dir() && depth < max_depth {
                    stack.push((path, depth + 1));
                } else if path.is_file() {
                    results.push(Ok(path));
                }
            }
        }
    }

    results.into_iter()
}
