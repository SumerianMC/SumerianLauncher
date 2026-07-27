use anyhow::Result;
use serde::Deserialize;

const NEWS_URL: &str =
    "https://launchercontent.mojang.com/v2/javaPatchNotes.json";

#[derive(Debug, Deserialize)]
pub struct NewsEntry {
    pub title: String,
    pub version: String,
    pub date: Option<String>,
    pub body: Option<String>,
}

#[derive(Deserialize)]
struct Feed {
    entries: Vec<NewsEntry>,
}

pub async fn fetch_news(http: &reqwest::Client) -> Result<Vec<NewsEntry>> {
    let feed: Feed = http.get(NEWS_URL).send().await?.json().await?;
    Ok(feed.entries)
}
