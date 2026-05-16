//! Ergonomic `format!`-style macros for the most common log calls.
//!
//! These are thin wrappers over [`crate::log`]; the functions remain available
//! for composition and testing. Exported at the crate root, so call them as
//! `actions_rs::warning!(...)`.

/// `debug!("x = {x}")` → [`crate::log::debug`] with `format!` arguments.
///
/// # Examples
///
/// ```
/// let key = "v2-linux";
/// actions_rs::debug!("cache key = {key}");
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => { $crate::log::debug(::std::format!($($arg)*)) };
}

/// `info!("...")` → [`crate::log::info`] with `format!` arguments.
///
/// # Examples
///
/// ```
/// let n = 3;
/// actions_rs::info!("processed {n} files");
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::log::info(::std::format!($($arg)*)) };
}

/// `notice!("...")` → [`crate::log::notice`] with `format!` arguments.
///
/// # Examples
///
/// ```
/// actions_rs::notice!("released v{}.{}", 1, 2);
/// ```
#[macro_export]
macro_rules! notice {
    ($($arg:tt)*) => { $crate::log::notice(::std::format!($($arg)*)) };
}

/// `warning!("...")` → [`crate::log::warning`] with `format!` arguments.
///
/// # Examples
///
/// ```
/// let pct = 92;
/// actions_rs::warning!("disk {pct}% full");
/// ```
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => { $crate::log::warning(::std::format!($($arg)*)) };
}

/// `error!("...")` → [`crate::log::error`] with `format!` arguments.
///
/// # Examples
///
/// ```
/// let path = "Cargo.toml";
/// actions_rs::error!("{path}: missing `version` field");
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::log::error(::std::format!($($arg)*)) };
}

/// `group!("name", { ... })` runs the block inside a collapsible group that is
/// closed even on panic. Evaluates to the block's value.
///
/// # Examples
///
/// ```
/// let answer = actions_rs::group!("compute", { 6 * 7 });
/// assert_eq!(answer, 42);
/// ```
#[macro_export]
macro_rules! group {
    ($name:expr, $body:block) => {
        $crate::log::group($name, || $body)
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn group_macro_returns_value() {
        let n = group!("compute", { 6 * 7 });
        assert_eq!(n, 42);
    }

    #[test]
    fn log_macros_format_without_panicking() {
        let x = 3;
        debug!("debug {x}");
        info!("info {}", x);
        notice!("notice");
        warning!("warn {x}");
        error!("err {x}");
    }
}
