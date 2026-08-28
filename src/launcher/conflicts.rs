/// Pre-launch mod conflict detector.
///
/// Scans the mods directory for:
///   1. Duplicate mod IDs (same mod installed twice with different filenames).
///   2. Known incompatible mod pairs (hard-coded table, easily extendable).
///
/// Returns a list of human-readable warning strings. An empty list means no
/// conflicts were detected.
use std::path::Path;

/// A known incompatible pair: if both mod name fragments are present, emit a warning.
static KNOWN_CONFLICTS: &[(&str, &str, &str)] = &[
    // (fragment_a, fragment_b, reason)
    ("optifine",      "iris",          "OptiFine and Iris cannot be loaded at the same time"),
    ("optifine",      "sodium",        "OptiFine and Sodium conflict — use Iris+Sodium instead"),
    ("forge",         "fabric",        "Forge and Fabric mod loaders cannot coexist"),
    ("forge",         "quilt",         "Forge and Quilt mod loaders cannot coexist"),
    ("forge",         "neoforge",      "Forge and NeoForge mod loaders cannot coexist"),
    ("neoforge",      "fabric",        "NeoForge and Fabric mod loaders cannot coexist"),
    ("neoforge",      "quilt",         "NeoForge and Quilt mod loaders cannot coexist"),
    ("fabric-api",    "qsl",           "Fabric API and the Quilt Standard Libraries both provide loader hooks — use the one matching your loader"),
    ("lwjgl3ify",     "lwjgl",         "lwjgl3ify replaces LWJGL — remove the standalone LWJGL jar"),
    ("mixin",         "mixinbooter",   "Multiple Mixin bootstrappers detected — keep only one"),
    ("journeymap",    "voxelmap",      "JourneyMap and VoxelMap both provide a minimap — use only one"),
    ("journeymap",    "xaeros",        "JourneyMap and Xaero's Map both provide a minimap — use only one"),
    ("voxelmap",      "xaeros",        "VoxelMap and Xaero's Map both provide a minimap — use only one"),
    ("betterfoliage", "dynamictrees",  "BetterFoliage can conflict with Dynamic Trees — test carefully"),
    ("phosphor",      "starlight",     "Phosphor and Starlight both replace the lighting engine — use only one"),
    ("rubidium",      "sodium",        "Rubidium is a Sodium fork — do not load both"),
    ("embeddium",     "sodium",        "Embeddium is a Sodium fork — do not load both"),
    ("embeddium",     "rubidium",      "Embeddium and Rubidium are both Sodium forks — use only one"),
];

#[derive(Debug, Clone)]
pub struct ConflictWarning {
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    /// Will almost certainly crash.
    Critical,
    /// May cause issues, worth a warning.
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::Warning  => write!(f, "WARNING"),
        }
    }
}

/// Scan `mods_dir` and return any detected conflicts.
pub fn detect(mods_dir: &Path) -> Vec<ConflictWarning> {
    let mut warnings = Vec::new();

    // Collect lowercase filenames (only .jar files).
    let jar_names: Vec<String> = match std::fs::read_dir(mods_dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_lowercase())
            .filter(|n| n.ends_with(".jar"))
            .collect(),
        Err(_) => return warnings,
    };

    // ── 1. Duplicate mod ID detection ────────────────────────────────────────
    // Heuristic: strip version numbers and common suffixes to get a "slug".
    // If two jars resolve to the same slug they are probably the same mod.
    let slugs: Vec<String> = jar_names.iter().map(|n| slugify(n)).collect();

    let mut seen: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (slug, name) in slugs.iter().zip(jar_names.iter()) {
        seen.entry(slug.clone()).or_default().push(name.clone());
    }
    for (slug, names) in &seen {
        if names.len() > 1 {
            warnings.push(ConflictWarning {
                severity: Severity::Critical,
                message: format!(
                    "Duplicate mod detected ('{}'): {}",
                    slug,
                    names.join(", ")
                ),
            });
        }
    }

    // ── 2. Known incompatible pairs ───────────────────────────────────────────
    for (a, b, reason) in KNOWN_CONFLICTS {
        let has_a = jar_names.iter().any(|n| n.contains(a));
        let has_b = jar_names.iter().any(|n| n.contains(b));
        if has_a && has_b {
            warnings.push(ConflictWarning {
                severity: Severity::Critical,
                message: reason.to_string(),
            });
        }
    }

    warnings
}

/// Strip version numbers and common suffixes from a jar filename to produce a
/// stable mod slug. E.g. "sodium-fabric-0.5.3+mc1.21.jar" → "sodium-fabric".
fn slugify(name: &str) -> String {
    // Remove .jar extension
    let base = name.trim_end_matches(".jar");
    // Split on common version separators and keep only the leading non-version part.
    // Segments that start with a digit or contain '+', '-SNAPSHOT', 'mc', 'forge', 'fabric'
    // after the first alphabetic run are stripped.
    let mut parts: Vec<&str> = Vec::new();
    for segment in base.split(['-', '_', '+'].as_ref()) {
        // Stop accumulating once we hit a purely numeric or version-looking segment.
        if segment.is_empty() { continue; }
        let first_char = segment.chars().next().unwrap_or('0');
        if first_char.is_ascii_digit() {
            break;
        }
        // Also stop at well-known loader/version tags that follow the mod name.
        let lower = segment.to_lowercase();
        if lower == "fabric" || lower == "forge" || lower == "mc"
            || lower == "neoforge" || lower == "quilt" || lower.starts_with("1") {
            break;
        }
        parts.push(segment);
    }
    if parts.is_empty() { base.to_string() } else { parts.join("-") }
}
