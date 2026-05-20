//! Session-scoped debug logging (`debug.log`). Never writes to the TUI transcript.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init_from_env_and_args(args: &[String]) {
    if std::env::var("ZERO_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        ENABLED.store(true, Ordering::Relaxed);
    }
    for arg in args {
        if arg == "--debug" || arg == "--debug=on" || arg == "--debug=true" {
            ENABLED.store(true, Ordering::Relaxed);
        }
    }
}

pub fn toggle() -> bool {
    let next = !is_enabled();
    set_enabled(next);
    next
}

pub fn set_enabled(on: bool) {
    if is_enabled() && !on {
        log("debug", "disabled");
    }
    ENABLED.store(on, Ordering::Relaxed);
    if on {
        log("debug", "enabled");
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_log_path(path: PathBuf) {
    if let Ok(mut guard) = LOG_PATH.lock() {
        *guard = Some(path);
    }
}

pub fn log_path_display() -> Option<String> {
    LOG_PATH
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.display().to_string()))
}

pub fn log(category: &str, message: &str) {
    if !is_enabled() {
        return;
    }
    let path = match LOG_PATH.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{ts}] [{category}] {message}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn debug_log_writes_when_enabled() {
        let tmp = std::env::temp_dir().join(format!("zero-debug-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let log_file = tmp.join("debug.log");

        set_log_path(log_file.clone());
        set_enabled(true);
        log("test", "hello");

        let content = fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("[test] hello"));

        set_enabled(false);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn toggle_flips_enabled_state() {
        set_enabled(false);
        assert!(!is_enabled());
        assert!(toggle());
        assert!(is_enabled());
        assert!(!toggle());
        assert!(!is_enabled());
    }
}
