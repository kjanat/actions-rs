//! Internal: environment-file ("file command") plumbing.
//!
//! Modern GitHub runners expose `GITHUB_ENV`, `GITHUB_OUTPUT`, `GITHUB_STATE`
//! and `GITHUB_PATH` as paths to append-only files. We always serialise
//! key/value pairs using the heredoc form `KEY<<DELIM\nVALUE\nDELIM` (the same
//! choice `@actions/core` makes) because it is the only form that safely
//! survives newlines in the value.
//!
//! The delimiter must not appear in the key or value, otherwise a crafted
//! value could inject arbitrary variables (the class of bug behind
//! CVE-2020-15228). We generate a per-call random delimiter with **zero
//! dependencies** and still validate, returning [`Error::DelimiterCollision`]
//! on the (astronomically unlikely) clash.

use std::collections::hash_map::RandomState;
use std::fs::OpenOptions;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

/// Produce a random, per-call heredoc delimiter without any external crate.
///
/// Entropy sources: a process-wide [`RandomState`] (std seeds it from the OS
/// CSPRNG once per process), mixed with a monotonic counter, the PID and the
/// wall-clock nanoseconds. Far more than enough to make a same-process
/// collision negligible while remaining `#![forbid(unsafe_code)]`-clean.
fn random_hex() -> String {
    static SEED: OnceLock<RandomState> = OnceLock::new();
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let seed = SEED.get_or_init(RandomState::new);
    let mut hasher = seed.build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u64(u64::from(std::process::id()));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    hasher.write_u64(nanos);
    format!("{:016x}", hasher.finish())
}

fn delimiter() -> String {
    format!("ghadelimiter_{}", random_hex())
}

/// A random, unguessable single-word token for `stop-commands` / resume.
///
/// Unguessable so untrusted log content cannot emit the resume command itself
/// and re-enable command processing early.
pub(crate) fn random_token() -> String {
    format!("stopcommands_{}", random_hex())
}

/// Build a heredoc key/value message for a fixed delimiter.
///
/// Split out from [`key_value_message`] so the formatting and the
/// collision-detection logic can be unit-tested deterministically.
fn key_value_message_with(key: &str, value: &str, delim: &str) -> Result<String> {
    if key.contains(delim) || value.contains(delim) {
        return Err(Error::DelimiterCollision);
    }
    Ok(format!("{key}<<{delim}\n{value}\n{delim}"))
}

/// Build a heredoc key/value message with a fresh random delimiter.
pub(crate) fn key_value_message(key: &str, value: &str) -> Result<String> {
    key_value_message_with(key, value, &delimiter())
}

/// Append `line` (plus a trailing newline) to the file pointed at by env
/// variable `var`.
///
/// Returns `Ok(false)` when `var` is unset, signalling the caller to fall back
/// to the deprecated stdout command. Returns [`Error::MissingEnvFile`] when
/// `var` is set but the file does not exist (a broken runner state we refuse
/// to paper over).
pub(crate) fn issue_file_command(var: &'static str, line: &str) -> Result<bool> {
    let Some(path) = std::env::var_os(var) else {
        return Ok(false);
    };
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(Error::MissingEnvFile { var, path });
    }
    let mut file = OpenOptions::new().append(true).open(&path)?;
    writeln!(file, "{line}")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heredoc_shape() {
        let msg = key_value_message_with("NAME", "multi\nline", "D").unwrap();
        assert_eq!(msg, "NAME<<D\nmulti\nline\nD");
    }

    #[test]
    fn collision_in_value_errors() {
        let err = key_value_message_with("k", "has D inside", "D").unwrap_err();
        assert!(matches!(err, Error::DelimiterCollision));
    }

    #[test]
    fn collision_in_key_errors() {
        let err = key_value_message_with("kD", "v", "D").unwrap_err();
        assert!(matches!(err, Error::DelimiterCollision));
    }

    #[test]
    fn generated_delimiter_is_prefixed_and_unique() {
        let a = delimiter();
        let b = delimiter();
        assert!(a.starts_with("ghadelimiter_"));
        assert_ne!(a, b, "counter must vary the delimiter per call");
    }

    #[test]
    fn unset_var_signals_fallback() {
        // A name that is essentially guaranteed not to exist.
        let ok = issue_file_command("GHACTIONS_TEST_DEFINITELY_UNSET", "x").unwrap();
        assert!(!ok);
    }
}
