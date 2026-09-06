/// Instance Health Check — scan an instance's `mods/` directory for
/// common problems before launch.
///
/// Checks performed:
///   1. Zero-byte JAR files (incomplete downloads)
///   2. Duplicate mod basenames (same mod installed twice under different versions)
///   3. JARs that do not appear to be valid ZIP/JAR archives (bad magic bytes)
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum HealthIssue {
    /// A JAR file has zero bytes — likely a failed download.
    EmptyJar { filename: String },
    /// Two or more JARs share the same apparent mod ID (prefix before the first `-`).
    DuplicateMod { filenames: Vec<String> },
    /// File magic bytes do not start with `PK\x03\x04` — not a valid ZIP/JAR.
    CorruptJar { filename: String },
}

impl HealthIssue {
    pub fn severity(&self) -> &'static str {
        match self {
            Self::EmptyJar { .. } | Self::CorruptJar { .. } => "ERROR",
            Self::DuplicateMod { .. } => "WARN",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::EmptyJar { filename } =>
                format!("Empty JAR (0 bytes): {} — likely a failed download", filename),
            Self::DuplicateMod { filenames } =>
                format!("Possible duplicate mod: {}", filenames.join(", ")),
            Self::CorruptJar { filename } =>
                format!("Corrupt/invalid JAR (bad magic bytes): {}", filename),
        }
    }
}

/// Run all health checks on `mods_dir`.  Returns the list of issues found.
/// An empty list means no problems detected.
pub fn check(mods_dir: &Path) -> Vec<HealthIssue> {
    let mut issues = Vec::new();

    let entries: Vec<_> = match std::fs::read_dir(mods_dir) {
        Ok(rd) => rd.flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jar"))
            .collect(),
        Err(_) => return issues, // mods dir doesn't exist — nothing to check
    };

    // ── 1 & 3: Zero-byte and corrupt JARs ────────────────────────────────────
    for path in &entries {
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        match std::fs::metadata(path) {
            Ok(m) if m.len() == 0 => {
                issues.push(HealthIssue::EmptyJar { filename });
                continue;
            }
            Ok(_) => {}
            Err(_) => continue,
        }
        // Check ZIP magic bytes (first 4 bytes must be PK\x03\x04)
        if let Ok(mut f) = std::fs::File::open(path) {
            use std::io::Read;
            let mut magic = [0u8; 4];
            if f.read_exact(&mut magic).is_ok() && &magic != b"PK\x03\x04" {
                issues.push(HealthIssue::CorruptJar { filename });
            }
        }
    }

    // ── 2: Duplicate mods (same base name, different version suffix) ──────────
    // Heuristic: group by the part of the filename before the first digit sequence
    // that looks like a version number (e.g. "sodium-0.5.8" → key "sodium").
    let mut by_key: HashMap<String, Vec<String>> = HashMap::new();
    for path in &entries {
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let stem = filename.trim_end_matches(".jar");
        let key = mod_base_name(stem);
        by_key.entry(key).or_default().push(filename);
    }
    for (_, filenames) in by_key {
        if filenames.len() > 1 {
            issues.push(HealthIssue::DuplicateMod { filenames });
        }
    }

    issues
}

/// Extract a stable "mod ID" from a JAR stem by stripping the version suffix.
/// e.g. "sodium-mc1.21-0.5.8+build.1" → "sodium"
///      "fabric-api-0.91.0+1.21"       → "fabric-api"
fn mod_base_name(stem: &str) -> String {
    // Split on `-` and take parts until we hit one starting with a digit or `mc`.
    let parts: Vec<&str> = stem.split('-').collect();
    let mut key_parts = Vec::new();
    for part in &parts {
        if part.starts_with(|c: char| c.is_ascii_digit()) || part.starts_with("mc") {
            break;
        }
        key_parts.push(*part);
    }
    if key_parts.is_empty() {
        stem.to_string()
    } else {
        key_parts.join("-").to_lowercase()
    }
}
