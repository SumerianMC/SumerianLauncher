/// Log Tail — stream `logs/latest.log` from an instance or the default game
/// directory to the terminal in real time, similar to `tail -f`.
///
/// Pressing Ctrl-C or sending SIGINT will stop the tail.  The function returns
/// when the log file has not produced new data for `idle_secs` seconds (default
/// 10) or when the stop signal is received.
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use console::style;

/// Tail `<game_dir>/logs/latest.log`, printing each new line to stdout.
///
/// `idle_timeout` — stop after this many seconds with no new log output.
/// Returns the number of lines printed.
pub fn tail(game_dir: &Path, idle_timeout: Duration) -> anyhow::Result<usize> {
    let log_path: PathBuf = game_dir.join("logs").join("latest.log");

    if !log_path.exists() {
        println!(
            "  {} Log file not found: {}",
            style("✗").red(),
            log_path.display()
        );
        println!("  Start the game at least once to generate a log.");
        return Ok(0);
    }

    // Set up a Ctrl-C flag so the user can exit cleanly.
    let running = Arc::new(AtomicBool::new(true));
    {
        let r = Arc::clone(&running);
        let _ = ctrlc_handler(move || {
            r.store(false, Ordering::SeqCst);
        });
    }

    println!(
        "  {} Tailing {} (Ctrl-C to stop, idle timeout {}s)",
        style("→").cyan(),
        style(log_path.display().to_string()).dim(),
        idle_timeout.as_secs()
    );
    println!();

    let file = std::fs::File::open(&log_path)?;
    let mut reader = std::io::BufReader::new(file);
    // Start from the end — only show new lines.
    reader.seek(SeekFrom::End(0))?;

    let mut count = 0usize;
    let mut last_new_data = Instant::now();

    loop {
        if !running.load(Ordering::SeqCst) {
            break;
        }

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data yet.
                if last_new_data.elapsed() >= idle_timeout {
                    println!(
                        "  {} No new log output for {}s — stopping tail.",
                        style("ℹ").cyan(),
                        idle_timeout.as_secs()
                    );
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Ok(_) => {
                last_new_data = Instant::now();
                let trimmed = line.trim_end();
                // Colour-code severity levels.
                let formatted = colourize_log_line(trimmed);
                println!("{}", formatted);
                count += 1;
            }
            Err(e) => {
                println!("  {} Read error: {}", style("✗").red(), e);
                break;
            }
        }
    }

    println!();
    println!("  {} Tail stopped ({} lines shown).", style("✓").green(), count);
    Ok(count)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn colourize_log_line(line: &str) -> String {
    let lower = line.to_lowercase();
    if lower.contains("] [error]") || lower.contains("/error]") || lower.contains("exception") || lower.contains("error:") {
        style(line).red().to_string()
    } else if lower.contains("] [warn]") || lower.contains("/warn]") || lower.contains("warning") {
        style(line).yellow().to_string()
    } else if lower.contains("] [debug]") || lower.contains("/debug]") {
        style(line).dim().to_string()
    } else {
        line.to_string()
    }
}

/// Register a one-shot Ctrl-C handler, silently ignoring failures (e.g. in
/// contexts where a handler is already registered).
fn ctrlc_handler<F: Fn() + Send + 'static>(f: F) -> Result<(), ()> {
    // Use the ctrlc crate if available; otherwise fall back to a no-op.
    // We gate on cfg so the dependency is optional.
    #[cfg(feature = "ctrlc")]
    {
        ctrlc::set_handler(f).map_err(|_| ())
    }
    #[cfg(not(feature = "ctrlc"))]
    {
        let _ = f;
        Ok(())
    }
}
