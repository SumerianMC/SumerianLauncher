/// Memory leak detector.
///
/// Analyses heap samples collected by the benchmark module.  A session is
/// flagged as a probable leak when the heap trend is monotonically increasing
/// over the last N samples with no significant GC recovery trough.
use crate::launcher::benchmark::BenchmarkStats;

#[derive(Debug, Clone)]
pub struct LeakReport {
    pub suspected: bool,
    /// Slope of heap growth in MB per minute (positive = growing).
    pub slope_mb_per_min: f64,
    /// Largest GC recovery drop observed (MB). Low value → GC not collecting.
    pub max_gc_recovery_mb: u64,
    pub message: String,
}

/// Analyse benchmark stats collected from a session.
/// `duration_secs` is the total session length used to normalise the slope.
pub fn analyse(stats: &BenchmarkStats, duration_secs: u64) -> Option<LeakReport> {
    let samples = &stats.heap_samples_mb;
    if samples.len() < 6 {
        return None; // not enough data
    }

    // Compute the maximum single-step drop (GC recovery).
    let max_gc_recovery_mb = samples
        .windows(2)
        .filter_map(|w| if w[0] > w[1] { Some(w[0] - w[1]) } else { None })
        .max()
        .unwrap_or(0);

    // Linear regression slope over all samples.
    let n = samples.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = samples.iter().sum::<u64>() as f64 / n;
    let mut num = 0.0_f64;
    let mut den = 0.0_f64;
    for (i, &y) in samples.iter().enumerate() {
        let x = i as f64 - x_mean;
        num += x * (y as f64 - y_mean);
        den += x * x;
    }
    let raw_slope = if den == 0.0 { 0.0 } else { num / den };

    // Convert slope from MB/sample to MB/minute.
    // Assume samples arrive roughly every 500 ms (as in benchmark.rs).
    let sample_interval_mins = 0.5 / 60.0;
    let slope_mb_per_min = raw_slope / sample_interval_mins;

    // Leak heuristic:
    //  - heap growing faster than 5 MB/min on average, AND
    //  - GC never recovered more than 20 MB in one step (GC not helping)
    let suspected = slope_mb_per_min > 5.0 && max_gc_recovery_mb < 20;

    let message = if suspected {
        format!(
            "Heap grew ~{:.0} MB/min over {}m{}s with little GC recovery ({} MB max drop). \
             A mod may be leaking objects. Try disabling mods one by one.",
            slope_mb_per_min,
            duration_secs / 60,
            duration_secs % 60,
            max_gc_recovery_mb
        )
    } else {
        format!(
            "No significant leak detected (slope {:.1} MB/min, max GC recovery {} MB).",
            slope_mb_per_min, max_gc_recovery_mb
        )
    };

    Some(LeakReport { suspected, slope_mb_per_min, max_gc_recovery_mb, message })
}
