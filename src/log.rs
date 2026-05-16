//! Logging, grouping, masking and command-flow control.
//!
//! Everything here writes to stdout (the runner's command channel).\
//! A failed stdout write inside an action is unrecoverable, so these functions are intentionally
//! **infallible** — mirroring `@actions/core`.\
//! Fallible operations live in [`crate::output`] and [`crate::summary`].

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::command::WorkflowCommand;
use crate::env;

/// Process-global failure flag, the Rust analogue of `@actions/core`'s `process.exitCode = ExitCode.Failure`.
/// Set by [`set_failed`], read by [`exit_code`] / [`is_failed`].
static FAILED: AtomicBool = AtomicBool::new(false);

fn emit(cmd: &WorkflowCommand) {
    cmd.issue();
}

/// Write a plain line to the log (no annotation).
/// Equivalent to `println!`, provided for symmetry with the other log functions.
///
/// # Examples
///
/// ```
/// actions_rs::log::info("starting build");
/// ```
pub fn info(message: impl AsRef<str>) {
    let _ = writeln!(io::stdout().lock(), "{}", message.as_ref());
}

/// Emit a `::debug::` message.
/// Only visible when step-debug logging is enabled (the `ACTIONS_STEP_DEBUG` secret, surfaced as `RUNNER_DEBUG=1`).
///
/// # Examples
///
/// ```
/// actions_rs::log::debug("cache key = v2-linux");
/// ```
pub fn debug(message: impl Into<String>) {
    emit(&WorkflowCommand::new("debug").message(message));
}

/// Emit a `::notice::` annotation with no location.
/// For located annotations use [`crate::Annotation`].
///
/// # Examples
///
/// ```
/// actions_rs::log::notice("published 3 artifacts");
/// ```
pub fn notice(message: impl Into<String>) {
    emit(&WorkflowCommand::new("notice").message(message));
}

/// Emit a `::warning::` annotation with no location.
///
/// # Examples
///
/// ```
/// actions_rs::log::warning("deprecated input `path`; use `dir`");
/// ```
pub fn warning(message: impl Into<String>) {
    emit(&WorkflowCommand::new("warning").message(message));
}

/// Emit an `::error::` annotation with no location.
///
/// # Examples
///
/// ```
/// actions_rs::log::error("manifest checksum mismatch");
/// ```
pub fn error(message: impl Into<String>) {
    emit(&WorkflowCommand::new("error").message(message));
}

/// Whether step-debug logging is enabled (`RUNNER_DEBUG == "1"`).
///
/// # Examples
///
/// ```
/// if actions_rs::log::is_debug() {
///     actions_rs::log::debug("verbose diagnostics enabled");
/// }
/// ```
#[must_use]
pub fn is_debug() -> bool {
    env::is_debug()
}

/// Mask `value` in all subsequent log output (`::add-mask::`).
///
/// Note this only affects output produced *after* the call;
/// anything already logged is not retroactively masked.
///
/// # Examples
///
/// ```
/// let token = "ghp_example";
/// actions_rs::log::mask(token);
/// // Any later log line containing `ghp_example` is shown as `***`.
/// ```
pub fn mask(value: impl Into<String>) {
    emit(&WorkflowCommand::new("add-mask").message(value));
}

/// Alias for [`mask`], named after `@actions/core`'s `setSecret`.
///
/// # Examples
///
/// ```
/// actions_rs::log::set_secret(std::env::var("API_KEY").unwrap_or_default());
/// ```
pub fn set_secret(value: impl Into<String>) {
    mask(value);
}

/// Mark the action as failed: emit `message` as an `::error::` annotation and set the process-global failure flag.
///
/// This mirrors `@actions/core`'s `setFailed`, which sets `process.exitCode = 1` *without* exiting — the step runs to completion (allowing cleanup) and then fails.
/// Rust has no settable deferred process exit code, so the deferred part is realised by returning [`exit_code`] from `main`:
///
/// ```no_run
/// use std::process::ExitCode;
/// fn main() -> ExitCode {
///     ghactions_doctest();
///     actions_rs::log::exit_code() // Failure iff set_failed was called
/// }
/// # fn ghactions_doctest() {}
/// ```
///
/// For immediate termination instead, use [`fail_now`].
pub fn set_failed(message: impl Into<String>) {
    error(message);
    FAILED.store(true, Ordering::SeqCst);
}

/// Whether [`set_failed`] has been called in this process.
///
/// # Examples
///
/// ```
/// assert!(!actions_rs::log::is_failed());
/// actions_rs::log::set_failed("step failed");
/// assert!(actions_rs::log::is_failed());
/// ```
#[must_use]
pub fn is_failed() -> bool {
    FAILED.load(Ordering::SeqCst)
}

/// The process exit code to return from `main`: [`ExitCode::FAILURE`] if [`set_failed`] was called, otherwise [`ExitCode::SUCCESS`].
/// This is the faithful analogue of `@actions/core`'s deferred `process.exitCode`.
///
/// [`ExitCode::FAILURE`]: std::process::ExitCode::FAILURE
/// [`ExitCode::SUCCESS`]: std::process::ExitCode::SUCCESS
///
/// # Examples
///
/// ```no_run
/// use std::process::ExitCode;
/// fn main() -> ExitCode {
///     // ... action body; call `set_failed` on any recoverable failure ...
///     actions_rs::log::exit_code()
/// }
/// ```
#[must_use]
pub fn exit_code() -> std::process::ExitCode {
    if is_failed() {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

/// Emit `message` as an error annotation and immediately exit the process with code `1`.
/// Convenience wrapper around [`set_failed`] that does not wait for `main` to return [`exit_code`].
///
/// # Examples
///
/// ```no_run
/// let Some(input) = std::env::var_os("INPUT_TARGET") else {
///     actions_rs::log::fail_now("required input `target` missing");
/// };
/// ```
pub fn fail_now(message: impl Into<String>) -> ! {
    set_failed(message);
    std::process::exit(1)
}

/// Toggle command echoing (`::echo::on` / `::echo::off`).
///
/// # Examples
///
/// ```
/// actions_rs::log::echo(true);  // runner echoes subsequent workflow commands
/// actions_rs::log::echo(false);
/// ```
pub fn echo(on: bool) {
    emit(&WorkflowCommand::new("echo").message(if on { "on" } else { "off" }));
}

/// Begin a collapsible log group.
/// Prefer [`group`], which closes the group automatically even on panic.
///
/// # Examples
///
/// ```
/// actions_rs::log::start_group("install");
/// actions_rs::log::info("downloading toolchain");
/// actions_rs::log::end_group();
/// ```
pub fn start_group(name: impl Into<String>) {
    emit(&WorkflowCommand::new("group").message(name));
}

/// End the current collapsible log group.
///
/// # Examples
///
/// ```
/// actions_rs::log::start_group("tests");
/// actions_rs::log::info("running");
/// actions_rs::log::end_group();
/// ```
pub fn end_group() {
    emit(&WorkflowCommand::new("endgroup"));
}

/// RAII guard returned by [`group_guard`]; emits `::endgroup::` on drop.
///
/// # Examples
///
/// ```
/// {
///     let _g = actions_rs::log::group_guard("lint");
///     actions_rs::log::info("clippy clean");
/// } // `::endgroup::` emitted here
/// ```
#[must_use = "the group ends when this guard is dropped"]
pub struct GroupGuard(());

impl Drop for GroupGuard {
    fn drop(&mut self) {
        end_group();
    }
}

/// Start a group and return a guard that closes it when dropped (including on panic / early return).
///
/// # Examples
///
/// ```
/// fn step() -> Result<(), &'static str> {
///     let _g = actions_rs::log::group_guard("deploy");
///     // early return still closes the group via the guard's Drop
///     Err("boom")
/// }
/// assert!(step().is_err());
/// ```
pub fn group_guard(name: impl Into<String>) -> GroupGuard {
    start_group(name);
    GroupGuard(())
}

/// Run `f` inside a collapsible group, closing the group afterwards even if `f` panics.
/// Returns whatever `f` returns.
///
/// # Examples
///
/// ```no_run
/// let built = actions_rs::log::group("build", || {
///     actions_rs::log::info("compiling...");
///     6 * 7
/// });
/// assert_eq!(built, 42);
/// ```
pub fn group<R>(name: impl Into<String>, f: impl FnOnce() -> R) -> R {
    let _guard = group_guard(name);
    f()
}

/// RAII guard returned by [`stop_commands`];
/// emits the resume token on drop, re-enabling workflow-command processing.
///
/// # Examples
///
/// ```
/// {
///     let _g = actions_rs::log::stop_commands();
///     println!("::not-a-command:: this line is not interpreted");
/// } // command processing resumes here
/// ```
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

/// Stop the runner from interpreting workflow commands until the returned guard is dropped.
/// Useful when logging untrusted text that might otherwise be parsed as a `::command::`.
///
/// The stop/resume token is randomly generated so untrusted content cannot guess it and resume command processing early.
///
/// # Examples
///
/// ```
/// let untrusted = "::error::spoofed";
/// {
///     let _g = actions_rs::log::stop_commands();
///     actions_rs::log::info(untrusted); // logged literally, not interpreted
/// }
/// ```
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
