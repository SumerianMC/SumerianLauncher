/// Modrinth trending / featured mods feed.
///
/// Uses the Modrinth v2 search endpoint with the "follows" relevance sort and
/// a configurable game-version facet to show what the community is currently
/// playing with.
use anyhow::{Context, Result};
use serde::Deserialize;

const MODRINTH_SEARCH: &str = "https://api.modrinth.com/v2/search";

#[derive(Debug, Deserialize)]
pub struct TrendingHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
    pub follows: u64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResp {
    hits: Vec<TrendingHit>,
}

/// Fetch trending mods for `game_version`.
///
/// `page` is 0-indexed; each page returns up to `limit` results (max 100).
pub async fn fetch_trending(
    client: &reqwest::Client,
    game_version: &str,
    page: usize,
    limit: usize,
) -> Result<Vec<TrendingHit>> {
    let offset = page * limit;
    let facets = format!("[[\"versions:{game_version}\"],[\"project_type:mod\"]]");

    let resp = client
        .get(MODRINTH_SEARCH)
        .query(&[
            ("facets", facets.as_str()),
            ("index",  "follows"),          // sort by follower count — trending proxy
            ("limit",  &limit.to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await
        .context("Modrinth trending request failed")?
        .json::<SearchResp>()
        .await
        .context("Failed to parse Modrinth trending response")?;

    Ok(resp.hits)
}
