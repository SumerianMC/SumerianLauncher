/// Instance diff — compare two instances side-by-side.
///
/// Compares the `mods/` and `config/` directories of two instances and
/// reports mods that are unique to each, shared but on different versions,
/// and config files present in one but not the other.
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ModDiff {
    pub name: String,
    /// Present in instance A only
    pub only_in_a: bool,
    /// Present in instance B only
    pub only_in_b: bool,
    /// Present in both but the filename differs (different version)
    pub version_mismatch: bool,
    pub version_a: Option<String>,
    pub version_b: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigDiff {
    pub filename: String,
    pub only_in_a: bool,
    pub only_in_b: bool,
}

#[derive(Debug)]
pub struct InstanceDiffReport {
    pub mod_diffs: Vec<ModDiff>,
    pub config_diffs: Vec<ConfigDiff>,
    pub shared_mod_count: usize,
}

/// Compare two instance directories.
pub fn diff(dir_a: &Path, dir_b: &Path) -> InstanceDiffReport {
    let mods_a = collect_jars(&dir_a.join("mods"));
    let mods_b = collect_jars(&dir_b.join("mods"));
    let configs_a = collect_filenames(&dir_a.join("config"));
    let configs_b = collect_filenames(&dir_b.join("config"));

    let mut mod_diffs: Vec<ModDiff> = Vec::new();
    let mut shared_mod_count = 0usize;

    // Build slug → filename maps for each side
    let slugs_a: HashMap<String, String> = mods_a.iter().map(|f| (slugify(f), f.clone())).collect();
    let slugs_b: HashMap<String, String> = mods_b.iter().map(|f| (slugify(f), f.clone())).collect();

    let mut all_slugs: Vec<String> = slugs_a.keys().chain(slugs_b.keys()).cloned().collect();
    all_slugs.sort();
    all_slugs.dedup();

    for slug in all_slugs {
        let fa = slugs_a.get(&slug);
        let fb = slugs_b.get(&slug);
        match (fa, fb) {
            (Some(a), Some(b)) => {
                if a != b {
                    mod_diffs.push(ModDiff {
                        name: slug,
                        only_in_a: false,
                        only_in_b: false,
                        version_mismatch: true,
                        version_a: Some(a.clone()),
                        version_b: Some(b.clone()),
                    });
                } else {
                    shared_mod_count += 1;
                }
            }
            (Some(a), None) => mod_diffs.push(ModDiff {
                name: slug,
                only_in_a: true,
                only_in_b: false,
                version_mismatch: false,
                version_a: Some(a.clone()),
                version_b: None,
            }),
            (None, Some(b)) => mod_diffs.push(ModDiff {
                name: slug,
                only_in_a: false,
                only_in_b: true,
                version_mismatch: false,
                version_a: None,
                version_b: Some(b.clone()),
            }),
            (None, None) => {}
        }
    }

    // Config diffs
    let mut all_configs: Vec<String> = configs_a.iter().chain(configs_b.iter()).cloned().collect();
    all_configs.sort();
    all_configs.dedup();
    let config_diffs: Vec<ConfigDiff> = all_configs
        .into_iter()
        .filter_map(|f| {
            let in_a = configs_a.contains(&f);
            let in_b = configs_b.contains(&f);
            if in_a && in_b { return None; } // present in both — no diff
            Some(ConfigDiff { filename: f, only_in_a: in_a, only_in_b: !in_a })
        })
        .collect();

    InstanceDiffReport { mod_diffs, config_diffs, shared_mod_count }
}

fn collect_jars(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".jar"))
        .collect()
}

fn collect_filenames(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

/// Strip version suffix from a jar filename to get a comparable slug.
fn slugify(name: &str) -> String {
    let base = name.trim_end_matches(".jar").to_lowercase();
    let mut parts: Vec<&str> = Vec::new();
    for seg in base.split(['-', '_', '+'].as_ref()) {
        if seg.is_empty() { continue; }
        let first = seg.chars().next().unwrap_or('0');
        if first.is_ascii_digit() { break; }
        let low = seg;
        if low == "fabric" || low == "forge" || low == "mc"
            || low == "neoforge" || low == "quilt" { break; }
        parts.push(seg);
    }
    if parts.is_empty() { base } else { parts.join("-") }
}
