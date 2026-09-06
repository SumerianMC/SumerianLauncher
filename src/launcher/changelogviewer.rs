/// Changelog Viewer — fetch and display the changelog for any installed
/// Minecraft version from Mojang's launcher content API.
///
/// Mojang exposes patch-note articles at:
///   `https://launchercontent.mojang.com/v2/javaPatchNotes.json`
/// Each entry has an `id` matching the version string and a `contentPath`
/// pointing to a JSON article.
use anyhow::{Context, Result};
use serde::Deserialize;

const PATCH_NOTES_INDEX: &str =
    "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const CONTENT_BASE: &str = "https://launchercontent.mojang.com";

#[derive(Debug, Deserialize)]
struct PatchNotesIndex {
    entries: Vec<PatchNoteEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PatchNoteEntry {
    pub version: String,
    #[serde(rename = "contentPath")]
    pub content_path: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub date: String,
}

#[derive(Debug, Deserialize)]
struct ArticleBody {
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub title: String,
}

/// Fetch the index and return entries for all versions whose `version` field
/// starts with `version_prefix` (e.g. "1.21" matches "1.21", "1.21.1", etc.).
pub async fn find_entries(
    http: &reqwest::Client,
    version_prefix: &str,
) -> Result<Vec<PatchNoteEntry>> {
    let index: PatchNotesIndex = http
        .get(PATCH_NOTES_INDEX)
        .send()
        .await
        .context("Failed to fetch patch notes index")?
        .json()
        .await
        .context("Failed to parse patch notes index")?;

    let matches: Vec<_> = index
        .entries
        .into_iter()
        .filter(|e| e.version.starts_with(version_prefix))
        .collect();

    Ok(matches)
}

/// Fetch the full article body for a `PatchNoteEntry` and return plain-text
/// content (strips simple HTML tags for terminal display).
pub async fn fetch_body(http: &reqwest::Client, entry: &PatchNoteEntry) -> Result<String> {
    let url = format!("{}{}", CONTENT_BASE, entry.content_path);
    let article: ArticleBody = http
        .get(&url)
        .send()
        .await
        .context("Failed to fetch changelog article")?
        .json()
        .await
        .context("Failed to parse changelog article")?;

    Ok(strip_html(&article.body))
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal HTML tag stripper for terminal display — replaces common block
/// tags with newlines and removes all remaining `<...>` sequences.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                in_tag = true;
                // Emit a newline for block-level closing tags.
                let rest: String = chars.clone().take(4).collect();
                if rest.starts_with("/p>") || rest.starts_with("/h") || rest.starts_with("br") {
                    out.push('\n');
                }
            }
            '>' => { in_tag = false; }
            '&' => {
                // Very basic HTML entity decoding.
                let entity: String = std::iter::once('&')
                    .chain(chars.by_ref().take_while(|&c| c != ';'))
                    .collect();
                match entity.as_str() {
                    "&amp"  => out.push('&'),
                    "&lt"   => out.push('<'),
                    "&gt"   => out.push('>'),
                    "&nbsp" => out.push(' '),
                    other   => out.push_str(other),
                }
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // Collapse runs of 3+ newlines into 2.
    let mut result = String::new();
    let mut nl_run = 0u8;
    for c in out.chars() {
        if c == '\n' {
            nl_run += 1;
            if nl_run <= 2 { result.push(c); }
        } else {
            nl_run = 0;
            result.push(c);
        }
    }
    result.trim().to_string()
}
