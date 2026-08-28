/// Background benchmark sampler.
///
/// Spawns a reader thread that tails `<game_dir>/logs/latest.log` while the
/// game process is running. It parses lines like:
///
///   [Client thread/INFO]: [CHAT] 45 fps, ...
///   [Client thread/INFO]: fps: 45 ...
///
/// and JVM GC log lines for heap usage.  The caller receives a `BenchmarkHandle`
/// that can be joined once the game exits to retrieve summary statistics.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct BenchmarkStats {
    pub fps_samples: Vec<u32>,
    pub heap_samples_mb: Vec<u64>,
}

impl BenchmarkStats {
    pub fn fps_avg(&self) -> Option<u32> {
        if self.fps_samples.is_empty() { return None; }
        Some(self.fps_samples.iter().sum::<u32>() / self.fps_samples.len() as u32)
    }

    pub fn fps_min(&self) -> Option<u32> {
        self.fps_samples.iter().copied().reduce(u32::min)
    }

    pub fn fps_max(&self) -> Option<u32> {
        self.fps_samples.iter().copied().reduce(u32::max)
    }

    pub fn peak_heap_mb(&self) -> Option<u64> {
        self.heap_samples_mb.iter().copied().reduce(u64::max)
    }
}

pub struct BenchmarkHandle {
    stop: Arc<Mutex<bool>>,
    thread: Option<std::thread::JoinHandle<BenchmarkStats>>,
}

impl BenchmarkHandle {
    /// Signal the sampler to stop and wait for it to finish. Returns the
    /// collected stats.
    pub fn finish(mut self) -> BenchmarkStats {
        {
            let mut s = self.stop.lock().unwrap();
            *s = true;
        }
        self.thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default()
    }
}

/// Start the benchmark sampler.  Call `handle.finish()` after the game exits.
pub fn start(game_dir: &Path) -> BenchmarkHandle {
    let log_path: PathBuf = game_dir.join("logs").join("latest.log");
    let stop = Arc::new(Mutex::new(false));
    let stop_clone = Arc::clone(&stop);

    let thread = std::thread::spawn(move || {
        sample_log(log_path, stop_clone)
    });

    BenchmarkHandle { stop, thread: Some(thread) }
}

fn sample_log(log_path: PathBuf, stop: Arc<Mutex<bool>>) -> BenchmarkStats {
    use std::io::{BufRead, Seek, SeekFrom};

    let mut stats = BenchmarkStats::default();

    // Wait up to 10 s for the log file to appear (game may not have started yet).
    let mut waited = 0u32;
    while !log_path.exists() && waited < 100 {
        std::thread::sleep(Duration::from_millis(100));
        waited += 1;
    }

    let mut file = match std::fs::File::open(&log_path) {
        Ok(f) => f,
        Err(_) => return stats,
    };

    // Seek to end so we only process new lines.
    let _ = file.seek(SeekFrom::End(0));
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();

    loop {
        {
            if *stop.lock().unwrap() {
                break;
            }
        }

        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data; sleep a bit and poll again.
                std::thread::sleep(Duration::from_millis(500));
            }
            Ok(_) => {
                parse_fps_line(&line, &mut stats);
                parse_heap_line(&line, &mut stats);
            }
            Err(_) => break,
        }
    }

    stats
}

/// Parse Minecraft's F3 debug output written to the log, e.g.:
///   "[16:00:00] [Render thread/INFO]: 87 fps ..."
///   "[16:00:00] [Client thread/INFO]: fps: 87 ..."
fn parse_fps_line(line: &str, stats: &mut BenchmarkStats) {
    let lower = line.to_lowercase();
    // Modern MC format: "N fps"
    if let Some(idx) = lower.find(" fps") {
        // Walk backwards to find the number.
        let before = &lower[..idx];
        let num_str: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Ok(fps) = num_str.trim().parse::<u32>() {
            if fps > 0 && fps < 10_000 {
                stats.fps_samples.push(fps);
            }
        }
        return;
    }
    // Legacy format: "fps: N"
    if let Some(idx) = lower.find("fps: ") {
        let after = &lower[idx + 5..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(fps) = num_str.trim().parse::<u32>() {
            if fps > 0 && fps < 10_000 {
                stats.fps_samples.push(fps);
            }
        }
    }
}

/// Parse heap usage from GC log lines written to latest.log, e.g.:
///   "[GC (Allocation Failure) ...  used: 512M ...]"
///   "Heap after GC invocations=N ...: used=512M ..."
fn parse_heap_line(line: &str, stats: &mut BenchmarkStats) {
    let lower = line.to_lowercase();
    for marker in &["used: ", "used="] {
        if let Some(idx) = lower.find(marker) {
            let after = &lower[idx + marker.len()..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(mb) = num_str.trim().parse::<u64>() {
                // Values without a suffix are assumed to be in MB already.
                // Ignore implausibly large values (> 32 GB).
                if mb > 0 && mb < 32_768 {
                    stats.heap_samples_mb.push(mb);
                }
            }
        }
    }
}
