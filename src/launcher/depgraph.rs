/// Dependency Graph — build and display a mod dependency tree for installed
/// mods in an instance using Modrinth's version metadata.
///
/// For each `.jar` in the mods directory, the module looks up the project
/// on Modrinth by SHA-1 hash (using the `/version_file` endpoint), then
/// fetches the version's dependency list and recursively resolves it.
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MODRINTH_VERSION_FILE: &str =
    "https://api.modrinth.com/v2/version_file/{hash}";
const MODRINTH_PROJECT: &str = "https://api.modrinth.com/v2/project/{id}";

#[derive(Debug, Deserialize)]
struct VersionFileMeta {
    project_id: String,
    id: String,
    name: String,
    #[serde(default)]
    dependencies: Vec<DepEntry>,
}

#[derive(Debug, Deserialize)]
struct DepEntry {
    #[serde(default)]
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Deserialize)]
struct ProjectMeta {
    title: String,
    slug: String,
}

/// A node in the dependency tree.
#[derive(Debug, Clone)]
pub struct DepNode {
    /// Human-readable name (from Modrinth or filename fallback).
    pub name: String,
    /// Whether this is a required or optional dependency.
    pub required: bool,
    /// Child dependencies.
    pub children: Vec<DepNode>,
}

/// Build the dependency tree for all `.jar` files in `mods_dir`.
/// Returns one root `DepNode` per installed mod.
/// Mods not found on Modrinth are included as leaf nodes with their filename.
pub async fn build(
    http: &reqwest::Client,
    mods_dir: &Path,
    game_version: &str,
) -> Result<Vec<DepNode>> {
    let mut roots = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();

    let entries: Vec<_> = match std::fs::read_dir(mods_dir) {
        Ok(rd) => rd.flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jar"))
            .collect(),
        Err(_) => return Ok(roots),
    };

    for path in &entries {
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let hash = sha1_file(path);

        let node = match resolve_node(http, &hash, &filename, true, game_version, &mut visited).await {
            Ok(n) => n,
            Err(_) => DepNode { name: filename, required: true, children: vec![] },
        };
        roots.push(node);
    }

    Ok(roots)
}

/// Recursively resolve a single mod version into a `DepNode`.
async fn resolve_node(
    http: &reqwest::Client,
    hash: &str,
    fallback_name: &str,
    required: bool,
    game_version: &str,
    visited: &mut HashSet<String>,
) -> Result<DepNode> {
    if visited.contains(hash) {
        return Ok(DepNode { name: fallback_name.to_string(), required, children: vec![] });
    }
    visited.insert(hash.to_string());

    let url = MODRINTH_VERSION_FILE.replace("{hash}", hash);
    let meta: VersionFileMeta = http
        .get(&url)
        .query(&[("algorithm", "sha1")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut children = Vec::new();
    for dep in &meta.dependencies {
        if let Some(ref pid) = dep.project_id {
            let proj_url = MODRINTH_PROJECT.replace("{id}", pid);
            let proj: ProjectMeta = match http.get(&proj_url).send().await?.json().await {
                Ok(p) => p,
                Err(_) => continue,
            };
            // We don't have a hash for dependencies, so just record a leaf.
            let child = DepNode {
                name: proj.title,
                required: dep.dependency_type == "required",
                children: vec![],
            };
            if !visited.contains(pid) {
                visited.insert(pid.clone());
                children.push(child);
            }
        }
    }

    Ok(DepNode { name: meta.name, required, children })
}

/// Print a dependency tree to stdout, indented with ASCII art.
pub fn print_tree(nodes: &[DepNode]) {
    use console::style;
    for node in nodes {
        print_node(node, "", true);
        println!();
    }
}

fn print_node(node: &DepNode, prefix: &str, is_last: bool) {
    use console::style;
    let connector = if is_last { "└─ " } else { "├─ " };
    let req_marker = if node.required {
        style("●").green().to_string()
    } else {
        style("○").yellow().to_string()
    };
    println!("{}{}{} {}", prefix, connector, req_marker, style(&node.name).cyan());

    let child_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
    for (i, child) in node.children.iter().enumerate() {
        let last = i == node.children.len() - 1;
        print_node(child, &child_prefix, last);
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn sha1_file(path: &std::path::Path) -> String {
    use sha1::{Digest, Sha1};
    let data = std::fs::read(path).unwrap_or_default();
    let mut hasher = Sha1::new();
    hasher.update(&data);
    hex::encode(hasher.finalize())
}
