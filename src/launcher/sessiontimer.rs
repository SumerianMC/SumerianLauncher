/// Session Timer Overlay — prints elapsed session time to the terminal
/// in-place (using ANSI cursor control) while the game process is running.
///
/// A background thread wakes every 60 seconds and rewrites a single status
/// line so the user can glance at the terminal and see how long they've been
/// playing without any extra tools.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Handle returned by `start()`.  Call `stop()` when the game exits.
pub struct TimerHandle {
    stop: Arc<Mutex<bool>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TimerHandle {
    /// Signal the timer thread to stop and wait for it to exit.
    pub fn stop(mut self) {
        {
            let mut s = self.stop.lock().unwrap();
            *s = true;
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        // Clear the timer line when the game exits.
        eprint!("\r{}\r", " ".repeat(72));
    }
}

/// Start the background timer.  Prints an updating line to *stderr* every
/// `interval` so it doesn't pollute stdout (which may be captured by tests).
pub fn start(interval: Duration) -> TimerHandle {
    let stop = Arc::new(Mutex::new(false));
    let stop_clone = Arc::clone(&stop);
    let began = Instant::now();

    let thread = std::thread::spawn(move || {
        loop {
            std::thread::sleep(interval);
            if *stop_clone.lock().unwrap() {
                break;
            }
            let elapsed = began.elapsed();
            let h = elapsed.as_secs() / 3600;
            let m = (elapsed.as_secs() % 3600) / 60;
            let s = elapsed.as_secs() % 60;

            // \r moves to column 0; trailing spaces erase any leftover text.
            if h > 0 {
                eprint!("\r  ⏱  Session: {:02}h {:02}m {:02}s    ", h, m, s);
            } else {
                eprint!("\r  ⏱  Session: {:02}m {:02}s    ", m, s);
            }
        }
    });

    TimerHandle { stop, thread: Some(thread) }
}
