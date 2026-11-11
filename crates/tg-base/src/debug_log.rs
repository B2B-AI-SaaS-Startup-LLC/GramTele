//! Port of `base/debug_log.h` / `debug_log.cpp`.
//!
//! Translation of the original macros (`%1`-style `.arg()` chains become
//! `format!` arguments):
//! - `LOG(("a %1").arg(x))` -> `log!("a {}", x)`
//! - `DEBUG_LOG((..))` -> `debug_log!(..)`
//! - `PROFILE_LOG((..))` -> `profile_log!(..)`
//!
//! The sink is the `base::Integration` of the original; it lives in another
//! file, so here it is the [`Integration`] trait installed with
//! [`set_integration`].

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

/// The logging part of `base::Integration`.
pub trait Integration: Send + Sync {
    fn log_skip_debug(&self) -> bool;
    fn log_message_debug(&self, message: &str);
    fn log_message(&self, message: &str);
    /// Default of the original: `"Assertion Failed! " + info`.
    fn log_assertion_violation(&self, info: &str) {
        self.log_message(&format!("Assertion Failed! {info}"));
    }
}

fn slot() -> &'static RwLock<Option<Arc<dyn Integration>>> {
    static SLOT: OnceLock<RwLock<Option<Arc<dyn Integration>>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

/// Installs (or removes with `None`) the integration.
pub fn set_integration(value: Option<Arc<dyn Integration>>) {
    *slot().write().unwrap_or_else(|e| e.into_inner()) = value;
}

/// `Integration::Exists() ? &Integration::Instance() : nullptr`.
pub fn integration() -> Option<Arc<dyn Integration>> {
    slot().read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `base::LogWriteMain`.
pub fn log_write_main(message: &str) {
    if let Some(integration) = integration() {
        integration.log_message(message);
    }
}

/// `base::LogSkipDebug`.
pub fn log_skip_debug() -> bool {
    integration().is_none_or(|i| i.log_skip_debug())
}

/// `base::LogWriteDebug`.
pub fn log_write_debug(message: &str, file: &str, _line: u32) {
    crate::expects!(!log_skip_debug());
    if let Some(integration) = integration() {
        // The original formats `__LINE__` of debug_log.cpp instead of the
        // `line` argument; the bug is kept so that logs stay comparable.
        integration.log_message_debug(&format!("{message} ({file} : 25)"));
    }
}

/// Microseconds since the first call (`crl::profile()` stand-in; the real
/// clock belongs to `tg-runtime`).
fn profile_micros() -> i64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_micros() as i64
}

/// `base::LogProfilePrefix`: `"[12.345] "` (milliseconds).
pub fn log_profile_prefix() -> String {
    format!("[{:.3}] ", profile_micros() as f64 / 1000.)
}

/// `LOG((...))`.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::debug_log::log_write_main(&format!($($arg)*))
    };
}

/// `PROFILE_LOG((...))`: skipped when debug logging is off.
#[macro_export]
macro_rules! profile_log {
    ($($arg:tt)*) => {{
        if !$crate::debug_log::log_skip_debug() {
            $crate::debug_log::log_write_main(&format!(
                "{}{}",
                $crate::debug_log::log_profile_prefix(),
                format!($($arg)*)
            ));
        }
    }};
}

/// `DEBUG_LOG((...))`: skipped when debug logging is off.
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        if !$crate::debug_log::log_skip_debug() {
            $crate::debug_log::log_write_debug(
                &format!($($arg)*),
                $crate::assertion::extract_basename(file!()),
                line!(),
            );
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The integration is process-global; serialise the tests using it.
    static LOCK: Mutex<()> = Mutex::new(());

    #[derive(Default)]
    struct Sink {
        skip_debug: bool,
        lines: Mutex<Vec<String>>,
    }
    impl Integration for Sink {
        fn log_skip_debug(&self) -> bool {
            self.skip_debug
        }
        fn log_message_debug(&self, message: &str) {
            self.lines.lock().unwrap().push(format!("D:{message}"));
        }
        fn log_message(&self, message: &str) {
            self.lines.lock().unwrap().push(format!("M:{message}"));
        }
    }

    #[test]
    fn routes_messages() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let sink = Arc::new(Sink::default());
        set_integration(Some(sink.clone()));
        crate::log!("a {} {}", 1, "b");
        crate::debug_log!("dbg {}", 2);
        crate::profile_log!("step {}", 3);
        let lines = sink.lines.lock().unwrap().clone();
        assert_eq!(lines[0], "M:a 1 b");
        assert!(lines[1].starts_with("D:dbg 2 (debug_log.rs : "));
        assert!(lines[2].starts_with("M:[") && lines[2].ends_with("] step 3"));
        crate::assertion::log("\"x\"", "f.rs", 7);
        let lines = sink.lines.lock().unwrap().clone();
        assert_eq!(lines[3], "M:Assertion Failed! \"x\" f.rs:7");
        set_integration(None);
    }

    #[test]
    fn debug_is_skipped_when_asked_or_without_integration() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_integration(None);
        assert!(log_skip_debug());
        crate::debug_log!("nobody listens");
        let sink = Arc::new(Sink {
            skip_debug: true,
            ..Default::default()
        });
        set_integration(Some(sink.clone()));
        crate::debug_log!("skipped");
        crate::profile_log!("skipped");
        crate::log!("kept");
        assert_eq!(sink.lines.lock().unwrap().len(), 1);
        set_integration(None);
    }

    #[test]
    fn profile_prefix_format() {
        let prefix = log_profile_prefix();
        assert!(prefix.starts_with('[') && prefix.ends_with("] "));
        let number = &prefix[1..prefix.len() - 2];
        assert_eq!(number.split('.').nth(1).map(str::len), Some(3));
    }
}
