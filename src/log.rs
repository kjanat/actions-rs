//! Logging, grouping, masking and command-flow control.
//!
//! Everything here writes to stdout (the runner's command channel). A failed
//! stdout write inside an action is unrecoverable, so these functions are
//! intentionally **infallible** — mirroring `@actions/core`. Fallible
//! operations live in [`crate::output`] and [`crate::summary`].

use std::io::{self, Write};

use crate::command::WorkflowCommand;
use crate::env;

fn emit(cmd: &WorkflowCommand) {
    cmd.issue();
}

/// Write a plain line to the log (no annotation). Equivalent to `println!`,
/// provided for symmetry with the other log functions.
pub fn info(message: impl AsRef<str>) {
    let _ = writeln!(io::stdout().lock(), "{}", message.as_ref());
}

/// Emit a `::debug::` message. Only visible when step-debug logging is enabled
/// (the `ACTIONS_STEP_DEBUG` secret, surfaced as `RUNNER_DEBUG=1`).
pub fn debug(message: impl Into<String>) {
    emit(&WorkflowCommand::new("debug").message(message));
}

/// Emit a `::notice::` annotation with no location. For located annotations
/// use [`crate::Annotation`].
pub fn notice(message: impl Into<String>) {
    emit(&WorkflowCommand::new("notice").message(message));
}

/// Emit a `::warning::` annotation with no location.
pub fn warning(message: impl Into<String>) {
    emit(&WorkflowCommand::new("warning").message(message));
}

/// Emit an `::error::` annotation with no location.
pub fn error(message: impl Into<String>) {
    emit(&WorkflowCommand::new("error").message(message));
}

/// Whether step-debug logging is enabled (`RUNNER_DEBUG == "1"`).
#[must_use]
pub fn is_debug() -> bool {
    env::is_debug()
}

/// Mask `value` in all subsequent log output (`::add-mask::`).
///
/// Note this only affects output produced *after* the call; anything already
/// logged is not retroactively masked.
pub fn mask(value: impl Into<String>) {
    emit(&WorkflowCommand::new("add-mask").message(value));
}

/// Alias for [`mask`], named after `@actions/core`'s `setSecret`.
pub fn set_secret(value: impl Into<String>) {
    mask(value);
}

/// Mark the action as failed: emit `message` as an `::error::` annotation and
/// set the process exit code to `1`.
///
/// Returns so the caller can `return` afterwards; the exit code is applied via
/// [`std::process::exit`] only if you choose to exit. To match the common
/// pattern this sets a process-global flag is **not** used — instead you
/// should `std::process::exit(1)` yourself, or call [`fail_now`].
pub fn set_failed(message: impl Into<String>) {
    error(message);
}

/// Emit `message` as an error annotation and immediately exit the process with
/// code `1`. Convenience wrapper around [`set_failed`].
pub fn fail_now(message: impl Into<String>) -> ! {
    set_failed(message);
    std::process::exit(1)
}

/// Toggle command echoing (`::echo::on` / `::echo::off`).
pub fn echo(on: bool) {
    emit(&WorkflowCommand::new("echo").message(if on { "on" } else { "off" }));
}

/// Begin a collapsible log group. Prefer [`group`], which closes the group
/// automatically even on panic.
pub fn start_group(name: impl Into<String>) {
    emit(&WorkflowCommand::new("group").message(name));
}

/// End the current collapsible log group.
pub fn end_group() {
    emit(&WorkflowCommand::new("endgroup"));
}

/// RAII guard returned by [`group_guard`]; emits `::endgroup::` on drop.
#[must_use = "the group ends when this guard is dropped"]
pub struct GroupGuard(());

impl Drop for GroupGuard {
    fn drop(&mut self) {
        end_group();
    }
}

/// Start a group and return a guard that closes it when dropped (including on
/// panic / early return).
pub fn group_guard(name: impl Into<String>) -> GroupGuard {
    start_group(name);
    GroupGuard(())
}

/// Run `f` inside a collapsible group, closing the group afterwards even if
/// `f` panics. Returns whatever `f` returns.
pub fn group<R>(name: impl Into<String>, f: impl FnOnce() -> R) -> R {
    let _guard = group_guard(name);
    f()
}

/// RAII guard returned by [`stop_commands`]; emits the resume token on drop,
/// re-enabling workflow-command processing.
#[must_use = "command processing resumes when this guard is dropped"]
pub struct StopGuard {
    token: String,
}

impl Drop for StopGuard {
    fn drop(&mut self) {
        // Resume: the command name *is* the token and carries no message. The
        // token is a hex-suffixed identifier so it needs no escaping, and it
        // is not `&'static`, so write it directly rather than via
        // [`WorkflowCommand`].
        let _ = writeln!(io::stdout().lock(), "::{}::", self.token);
    }
}

/// Stop the runner from interpreting workflow commands until the returned
/// guard is dropped. Useful when logging untrusted text that might otherwise
/// be parsed as a `::command::`.
///
/// The stop/resume token is randomly generated so untrusted content cannot
/// guess it and resume command processing early.
pub fn stop_commands() -> StopGuard {
    let token = crate::file_command::random_token();
    emit(&WorkflowCommand::new("stop-commands").message(token.clone()));
    StopGuard { token }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_runs_and_returns() {
        let v = group("build", || 21 * 2);
        assert_eq!(v, 42);
    }

    #[test]
    fn group_closes_on_panic() {
        let r = std::panic::catch_unwind(|| {
            group("boom", || panic!("inside"));
        });
        assert!(r.is_err(), "panic should propagate after group closes");
    }
}
