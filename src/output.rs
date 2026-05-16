//! Step outputs, saved state, exported variables and `PATH` additions.
//!
//! Each function prefers the modern environment-file mechanism and falls back
//! to the deprecated stdout command when the corresponding `GITHUB_*` variable
//! is unset (older runners, or running outside Actions). All values are taken
//! as `impl Display`, so `&str`, `String`, integers and booleans all work
//! without a serde dependency.
//!
//! The `::set-output::` / `::save-state::` fallbacks are deprecated by GitHub;
//! see [Deprecating `save-state` and `set-output` commands][dep]. They are
//! warned-but-not-yet-disabled and only emitted here when the environment
//! file is unavailable.
//!
//! [dep]: https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/

use std::fmt::Display;

use crate::command::WorkflowCommand;
use crate::error::Result;
use crate::file_command::{issue_file_command, key_value_message};

/// True for variables the runner forbids re-defining via `GITHUB_ENV`.
fn is_reserved(name: &str) -> bool {
    name.starts_with("GITHUB_") || name.starts_with("RUNNER_") || name == "NODE_OPTIONS"
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

/// Export an environment variable to subsequent steps (`GITHUB_ENV`, falling
/// back to `::set-env::`).
///
/// Does **not** mutate the current process environment — subsequent steps run
/// in fresh processes and read the env file; mutating `std::env` here would be
/// `unsafe` in edition 2024 and serve no purpose.
///
/// # Errors
/// [`crate::Error::ReservedName`] for `GITHUB_*` / `RUNNER_*` / `NODE_OPTIONS`;
/// otherwise on a file-command write failure or delimiter collision.
pub fn export_var(name: &str, value: impl Display) -> Result<()> {
    if is_reserved(name) {
        return Err(crate::Error::ReservedName(name.to_owned()));
    }
    let value = value.to_string();
    let msg = key_value_message(name, &value)?;
    if !issue_file_command("GITHUB_ENV", &msg)? {
        WorkflowCommand::new("set-env")
            .property("name", name.to_owned())
            .message(value)
            .issue();
    }
    Ok(())
}

/// Prepend a directory to `PATH` for subsequent steps (`GITHUB_PATH`, falling
/// back to `::add-path::`). The `GITHUB_PATH` file format is a bare directory
/// per line — not a heredoc key/value pair.
///
/// # Errors
/// [`crate::Error`] on a file-command write failure.
pub fn add_path(dir: impl Display) -> Result<()> {
    let dir = dir.to_string();
    if !issue_file_command("GITHUB_PATH", &dir)? {
        WorkflowCommand::new("add-path").message(dir).issue();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
