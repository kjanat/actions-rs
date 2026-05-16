//! Step outputs, saved state, exported variables and `PATH` additions.
//!
//! `set_output` / `save_state` keep a deprecated stdout fallback for older
//! runners. `export_var` / `add_path` do not: GitHub retired `::set-env::` and
//! `::add-path::`, so these operations require the corresponding environment
//! file path from the runner.
//!
//! Because mutating the process environment is `unsafe` in edition 2024 and
//! this crate forbids `unsafe`, same-process parity is provided through a safe
//! overlay: use [`overlay_var`], [`overlay_path`] or [`apply_overlay`] when you
//! need child processes to observe `export_var` / `add_path` changes.
//!
//! [dep]: https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/

use std::collections::BTreeMap;
use std::fmt::Display;
use std::io::{self, Write};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::command::WorkflowCommand;
use crate::error::Result;
use crate::file_command::{issue_file_command, key_value_message};

#[derive(Debug, Default)]
struct EnvOverlay {
    vars: BTreeMap<String, String>,
    path_prefixes: Vec<String>,
}

fn overlay() -> &'static Mutex<EnvOverlay> {
    static OVERLAY: OnceLock<Mutex<EnvOverlay>> = OnceLock::new();
    OVERLAY.get_or_init(|| Mutex::new(EnvOverlay::default()))
}

fn lock_overlay() -> MutexGuard<'static, EnvOverlay> {
    overlay()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn path_delimiter() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

fn effective_path(overlay: &EnvOverlay) -> Option<String> {
    let base = overlay
        .vars
        .get("PATH")
        .cloned()
        .or_else(|| std::env::var("PATH").ok());
    if overlay.path_prefixes.is_empty() {
        return base;
    }

    let mut path = overlay.path_prefixes.join(&path_delimiter().to_string());
    if let Some(base) = base.filter(|value| !value.is_empty()) {
        path.push(path_delimiter());
        path.push_str(&base);
    }
    Some(path)
}

fn record_exported_var(name: &str, value: String) {
    let mut overlay = lock_overlay();
    overlay.vars.insert(name.to_owned(), value);
}

fn record_path(dir: String) {
    let mut overlay = lock_overlay();
    overlay.path_prefixes.insert(0, dir);
}

/// True for variables the runner forbids re-defining via `GITHUB_ENV`.
fn is_reserved(name: &str) -> bool {
    name.starts_with("GITHUB_") || name.starts_with("RUNNER_") || name == "NODE_OPTIONS"
}

/// Return the effective same-process value for `name`, including any overlay
/// created by [`export_var`] and [`add_path`].
#[must_use]
pub fn overlay_var(name: &str) -> Option<String> {
    let overlay = lock_overlay();
    if name == "PATH" {
        effective_path(&overlay)
    } else {
        overlay
            .vars
            .get(name)
            .cloned()
            .or_else(|| std::env::var(name).ok())
    }
}

/// Return the effective same-process `PATH`, including any pending
/// [`add_path`] prefixes and `PATH` exported through [`export_var`].
#[must_use]
pub fn overlay_path() -> Option<String> {
    let overlay = lock_overlay();
    effective_path(&overlay)
}

/// Apply the safe same-process overlay to a child command.
pub fn apply_overlay(command: &mut Command) -> &mut Command {
    let overlay = lock_overlay();
    for (name, value) in &overlay.vars {
        command.env(name, value);
    }
    if let Some(path) = effective_path(&overlay) {
        command.env("PATH", path);
    }
    command
}

/// Set a step output (`GITHUB_OUTPUT`, falling back to `::set-output::`).
///
/// # Errors
/// [`crate::Error`] on a file-command write failure or delimiter collision.
pub fn set_output(name: &str, value: impl Display) -> Result<()> {
    let value = value.to_string();
    let msg = key_value_message(name, &value)?;
    if !issue_file_command("GITHUB_OUTPUT", &msg)? {
        // Deprecated by GitHub (warned, not yet disabled); only reached when
        // GITHUB_OUTPUT is unavailable:
        // https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/
        let _ = writeln!(io::stdout().lock());
        WorkflowCommand::new("set-output")
            .property("name", name.to_owned())
            .message(value)
            .issue();
    }
    Ok(())
}

/// Persist state for the action's `post` step (`GITHUB_STATE`, falling back to
/// `::save-state::`). Read it back with [`get_state`].
///
/// # Errors
/// [`crate::Error`] on a file-command write failure or delimiter collision.
pub fn save_state(name: &str, value: impl Display) -> Result<()> {
    let value = value.to_string();
    let msg = key_value_message(name, &value)?;
    if !issue_file_command("GITHUB_STATE", &msg)? {
        // Deprecated by GitHub (warned, not yet disabled); only reached when
        // GITHUB_STATE is unavailable:
        // https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/
        WorkflowCommand::new("save-state")
            .property("name", name.to_owned())
            .message(value)
            .issue();
    }
    Ok(())
}

/// Read state saved by a previous phase via [`save_state`] (the runner exposes
/// it as `STATE_<name>`). `None` when unset.
#[must_use]
pub fn get_state(name: &str) -> Option<String> {
    std::env::var(format!("STATE_{name}")).ok()
}

/// Export an environment variable to subsequent steps via `GITHUB_ENV`.
///
/// Does **not** mutate the current process environment — subsequent steps run
/// in fresh processes and read the env file; mutating `std::env` here would be
/// `unsafe` in edition 2024. Use [`overlay_var`] / [`apply_overlay`] when the
/// current process needs to observe the change safely.
///
/// # Errors
/// [`crate::Error::ReservedName`] for `GITHUB_*` / `RUNNER_*` / `NODE_OPTIONS`;
/// [`crate::Error::UnavailableFileCommand`] when `GITHUB_ENV` is unset;
/// otherwise on a file-command write failure or delimiter collision.
pub fn export_var(name: &str, value: impl Display) -> Result<()> {
    if is_reserved(name) {
        return Err(crate::Error::ReservedName(name.to_owned()));
    }
    let value = value.to_string();
    let msg = key_value_message(name, &value)?;
    if !issue_file_command("GITHUB_ENV", &msg)? {
        return Err(crate::Error::UnavailableFileCommand {
            var: "GITHUB_ENV",
            operation: "export_var",
        });
    }
    record_exported_var(name, value);
    Ok(())
}

/// Prepend a directory to `PATH` for subsequent steps via `GITHUB_PATH`. The
/// file format is a bare directory per line — not a heredoc key/value pair.
///
/// # Errors
/// [`crate::Error::UnavailableFileCommand`] when `GITHUB_PATH` is unset;
/// otherwise on a file-command write failure.
pub fn add_path(dir: impl Display) -> Result<()> {
    let dir = dir.to_string();
    // GITHUB_PATH is one directory per line; a `\r`/`\n` would inject extra
    // PATH entries.
    if dir.contains(['\r', '\n']) {
        return Err(crate::Error::InvalidName {
            name: dir,
            reason: "path contains a carriage return or line feed",
        });
    }
    if !issue_file_command("GITHUB_PATH", &dir)? {
        return Err(crate::Error::UnavailableFileCommand {
            var: "GITHUB_PATH",
            operation: "add_path",
        });
    }
    record_path(dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn with_clean_overlay(f: impl FnOnce()) {
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        {
            let mut overlay = lock_overlay();
            overlay.vars.clear();
            overlay.path_prefixes.clear();
        }
        f();
        let mut overlay = lock_overlay();
        overlay.vars.clear();
        overlay.path_prefixes.clear();
    }

    #[test]
    fn reserved_names() {
        assert!(is_reserved("GITHUB_SHA"));
        assert!(is_reserved("RUNNER_OS"));
        assert!(is_reserved("NODE_OPTIONS"));
        assert!(!is_reserved("CI"));
        assert!(!is_reserved("MY_VAR"));
    }

    #[test]
    fn export_reserved_errs_without_touching_env() {
        let e = export_var("GITHUB_TOKEN", "x").unwrap_err();
        assert!(matches!(e, crate::Error::ReservedName(_)));
    }

    #[test]
    fn add_path_rejects_line_breaks_before_touching_env() {
        // Validation happens before any file command, so this needs no env.
        for bad in ["/a\n/b", "/a\r/b"] {
            let e = add_path(bad).unwrap_err();
            assert!(matches!(e, crate::Error::InvalidName { .. }), "{bad:?}");
        }
    }

    // `unavailable_file_commands_error` lives in tests/env_files.rs: it must
    // unset GITHUB_ENV/GITHUB_PATH (the CI runner sets them), which needs
    // `unsafe` env mutation — impossible here under crate `forbid(unsafe_code)`.

    #[test]
    fn overlay_tracks_exported_path_changes() {
        with_clean_overlay(|| {
            record_exported_var("PATH", "/base".to_owned());
            record_path("/a".to_owned());
            record_path("/b".to_owned());

            let delim = path_delimiter();
            assert_eq!(overlay_path(), Some(format!("/b{delim}/a{delim}/base")));
            assert_eq!(
                overlay_var("PATH"),
                Some(format!("/b{delim}/a{delim}/base"))
            );
        });
    }
}
