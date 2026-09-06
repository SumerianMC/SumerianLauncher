/// Playtime Goals — set daily and weekly playtime targets and check progress.
///
/// Goals are stored at `<data_dir>/config/playtime_goals.json`.
/// Progress is computed on-demand from `HistoryManager`.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::launcher::history::HistoryManager;
use crate::launcher::playtime::fmt_duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaytimeGoals {
    /// Target seconds per day (0 = disabled).
    #[serde(default)]
    pub daily_secs: u64,
    /// Target seconds per week (0 = disabled).
    #[serde(default)]
    pub weekly_secs: u64,
}

pub struct PlaytimeGoalManager {
    path: PathBuf,
}

impl PlaytimeGoalManager {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("config").join("playtime_goals.json") }
    }

    pub async fn load(&self) -> PlaytimeGoals {
        match fs::read_to_string(&self.path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => PlaytimeGoals::default(),
        }
    }

    pub async fn save(&self, goals: &PlaytimeGoals) -> Result<()> {
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p).await?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(goals)?).await?;
        Ok(())
    }

    /// Compute today's played seconds from history.
    pub async fn today_secs(&self, history: &HistoryManager) -> Result<u64> {
        let records = history.load().await?;
        let today = chrono::Utc::now().date_naive();
        let total = records.iter()
            .filter(|r| r.started_at.date_naive() == today)
            .map(|r| r.duration_secs)
            .sum();
        Ok(total)
    }

    /// Compute this ISO-week's played seconds from history.
    pub async fn this_week_secs(&self, history: &HistoryManager) -> Result<u64> {
        use chrono::Datelike;
        let records = history.load().await?;
        let now = chrono::Utc::now();
        let this_week = now.iso_week();
        let total = records.iter()
            .filter(|r| {
                let w = r.started_at.iso_week();
                w.year() == this_week.year() && w.week() == this_week.week()
            })
            .map(|r| r.duration_secs)
            .sum();
        Ok(total)
    }

    /// Print a progress report to stdout.
    pub async fn print_progress(&self, history: &HistoryManager) -> Result<()> {
        use console::style;
        let goals = self.load().await;
        let today = self.today_secs(history).await?;
        let week  = self.this_week_secs(history).await?;

        println!("  {} Playtime Goals", style("◆").cyan().bold());
        println!();

        if goals.daily_secs == 0 && goals.weekly_secs == 0 {
            println!("  No goals set. Use 'Set goals' to configure targets.");
            return Ok(());
        }

        if goals.daily_secs > 0 {
            let pct = (today as f64 / goals.daily_secs as f64 * 100.0).min(100.0) as u64;
            let bar = progress_bar(pct, 30);
            let hit = today >= goals.daily_secs;
            println!(
                "  Daily   {} / {}  {} {}",
                style(fmt_duration(today)).cyan(),
                fmt_duration(goals.daily_secs),
                bar,
                if hit { style("✓ Goal reached!").green().bold().to_string() } else { format!("{}% — {} to go", pct, fmt_duration(goals.daily_secs.saturating_sub(today))) },
            );
        }

        if goals.weekly_secs > 0 {
            let pct = (week as f64 / goals.weekly_secs as f64 * 100.0).min(100.0) as u64;
            let bar = progress_bar(pct, 30);
            let hit = week >= goals.weekly_secs;
            println!(
                "  Weekly  {} / {}  {} {}",
                style(fmt_duration(week)).cyan(),
                fmt_duration(goals.weekly_secs),
                bar,
                if hit { style("✓ Goal reached!").green().bold().to_string() } else { format!("{}% — {} to go", pct, fmt_duration(goals.weekly_secs.saturating_sub(week))) },
            );
        }

        println!();
        Ok(())
    }

    /// Check goals after a session and print a notification if a goal was just crossed.
    pub async fn check_after_session(&self, history: &HistoryManager) {
        use console::style;
        let goals = self.load().await;
        if goals.daily_secs == 0 && goals.weekly_secs == 0 { return; }

        let today = self.today_secs(history).await.unwrap_or(0);
        let week  = self.this_week_secs(history).await.unwrap_or(0);

        if goals.daily_secs > 0 && today >= goals.daily_secs {
            println!(
                "\n  {} Daily playtime goal reached! ({} played today)\n",
                style("🎯").bold(),
                fmt_duration(today)
            );
        }
        if goals.weekly_secs > 0 && week >= goals.weekly_secs {
            println!(
                "\n  {} Weekly playtime goal reached! ({} played this week)\n",
                style("🎯").bold(),
                fmt_duration(week)
            );
        }
    }
}

fn progress_bar(pct: u64, width: usize) -> String {
    let filled = (pct as usize * width / 100).min(width);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}
