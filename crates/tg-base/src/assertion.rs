//! Port of `base/assertion.h`: always-on assertions.
//!
//! Translation of the original macros:
//! - `Expects(cond)` -> `expects!(cond)`
//! - `Ensures(cond)` -> `ensures!(cond)`
//! - `Assert(cond)` -> `tg_assert!(cond)`
//! - `Unexpected("text")` -> `unexpected!("text")` (diverges, type `!`)
//!
//! Unlike `debug_assert!` they are checked in every build, like in the
//! original. A failure is reported to the log integration
//! ([`crate::debug_log::set_integration`]) and then panics (the original
//! crashes with a null write to get a crash report).

/// Returns the part of `path` after the last `/` or `\`.
pub fn extract_basename(path: &str) -> &str {
    match path.rfind(['/', '\\']) {
        Some(at) => &path[at + 1..],
        None => path,
    }
}

/// `base::assertion::log`: reports the violation to the integration.
pub fn log(message: &str, file: &str, line: u32) {
    if let Some(integration) = crate::debug_log::integration() {
        integration.log_assertion_violation(&format!("{message} {file}:{line}"));
    }
}

/// `base::assertion::fail`: logs and aborts the current operation.
#[cold]
#[track_caller]
pub fn fail(message: &str, file: &str, line: u32) -> ! {
    log(message, file, line);
    panic!("Assertion failed: {message} {file}:{line}");
}

#[doc(hidden)]
#[macro_export]
macro_rules! __assert_condition {
    ($cond:expr, $message:expr) => {
        if !($cond) {
            $crate::assertion::fail(
                $message,
                $crate::assertion::extract_basename(file!()),
                line!(),
            );
        }
    };
}

/// `Assert(condition)`.
#[macro_export]
macro_rules! tg_assert {
    ($cond:expr $(,)?) => {
        $crate::__assert_condition!($cond, concat!("\"", stringify!($cond), "\""))
    };
}

/// `Expects(condition)`: precondition.
#[macro_export]
macro_rules! expects {
    ($cond:expr $(,)?) => {
        $crate::__assert_condition!($cond, concat!("\"", stringify!($cond), "\""))
    };
}

/// `Ensures(condition)`: postcondition.
#[macro_export]
macro_rules! ensures {
    ($cond:expr $(,)?) => {
        $crate::__assert_condition!($cond, concat!("\"", stringify!($cond), "\""))
    };
}

/// `Unexpected(message)`: unreachable state; the message may use `format!`
/// arguments.
#[macro_export]
macro_rules! unexpected {
    ($message:literal $(, $arg:expr)* $(,)?) => {
        $crate::assertion::fail(
            &format!(concat!("Unexpected: ", $message) $(, $arg)*),
            $crate::assertion::extract_basename(file!()),
            line!(),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::extract_basename;

    #[test]
    fn basename() {
        assert_eq!(extract_basename("a/b/c.rs"), "c.rs");
        assert_eq!(extract_basename("a\\b\\c.rs"), "c.rs");
        assert_eq!(extract_basename("c.rs"), "c.rs");
        assert_eq!(extract_basename("a/"), "");
    }

    #[allow(clippy::nonminimal_bool)]
    #[test]
    fn passing_conditions_do_nothing() {
        expects!(1 + 1 == 2);
        ensures!(true);
        tg_assert!(!false,);
    }

    #[test]
    #[should_panic(expected = "\"1 + 1 == 3\"")]
    fn failing_expects_panics_with_condition() {
        expects!(1 + 1 == 3);
    }

    #[allow(clippy::diverging_sub_expression)]
    #[test]
    #[should_panic(expected = "Unexpected: bad 5")]
    fn unexpected_diverges() {
        let _: i32 = unexpected!("bad {}", 5);
    }

    #[allow(clippy::diverging_sub_expression)]
    #[test]
    #[should_panic(expected = "Unexpected: plain")]
    fn unexpected_plain() {
        unexpected!("plain");
    }
}
