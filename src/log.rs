//! Logging, grouping, masking and command-flow control.
//!
//! The stdout convenience functions are **infallible**, mirroring `@actions/core`.
//! [`group_to`], [`group_guard_to`] and [`stop_commands_to`] instead accept any [`Write`]
//! destination and propagate I/O errors. Their guards implement [`Write`], so a borrowed
//! writer remains usable through the guard, including for nested scopes and binary data.
//!
//! Commands must begin on a new line. Start a writer-aware scope at a line boundary;
//! [`GroupGuardTo::stop_commands`] ensures this boundary before suspending a group.
//! Finishing or dropping its guard adds a separating newline if the last body byte was
//! not `\n`. Body bytes are otherwise written unchanged. Guards do not flush implicitly:
//! call [`Write::flush`] through a guard for live output, or on the writer after finishing
//! to deliver buffered closing markers. `finish()` returns the writer, including owned
//! buffers; [`group_to`] flushes after closing. Drop cleanup is best-effort; use `finish()`
//! to observe closing errors.
//!
//! For raw or custom workflow commands, see [`crate::command::WorkflowCommand`].

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::command::WorkflowCommand;
use crate::env;

// `format!`-style shortcuts for the logging functions in this module, surfaced
// here (and at the crate root) next to the functions they wrap — e.g. `group!`
// wraps `group`, `warning!` wraps `warning`.
#[doc(inline)]
pub use crate::macros::{debug, error, group, info, notice, warning};

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
/// Prefer [`group()`], which closes the group automatically even on panic.
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

// Own the writer (which may itself be a mutable borrow) so all body and closing
// writes share one destination. Track only the last successfully written byte;
// even partial writes and non-UTF-8 data need no buffering.
struct WriterScope<W: Write> {
    writer: Option<W>,
    closing: String,
    line_start: bool,
    finished: bool,
}

impl<W: Write> WriterScope<W> {
    fn new(writer: W, closing: String) -> Self {
        Self {
            writer: Some(writer),
            closing,
            line_start: true,
            finished: false,
        }
    }

    fn writer(&mut self) -> &mut W {
        self.writer.as_mut().expect("scope owns its writer")
    }

    fn close(&mut self) -> io::Result<()> {
        // Do not retry a partially written closing marker from Drop.
        self.finished = true;
        if !self.line_start {
            self.writer().write_all(b"\n")?;
        }
        writeln!(
            self.writer.as_mut().expect("scope owns its writer"),
            "{}",
            self.closing
        )
    }

    fn finish(mut self) -> io::Result<W> {
        self.close()?;
        Ok(self.writer.take().expect("scope owns its writer"))
    }
}

impl<W: Write> Write for WriterScope<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.writer().write(buf)?;
        if written != 0 {
            self.line_start = buf[written - 1] == b'\n';
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer().flush()
    }
}

impl<W: Write> Drop for WriterScope<W> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.close();
        }
    }
}

/// Writer-aware group guard returned by [`group_guard_to`].
///
/// Implements [`Write`]: write body bytes through this guard while it owns or borrows
/// the destination. Drop closes the group best-effort, including on early return or
/// panic. See the [module documentation](self) for line-boundary and flushing rules.
///
/// # Examples
///
/// ```
/// use actions_rs::log;
/// use std::io::Write;
///
/// let mut output = Vec::new();
/// {
///     let mut group: log::GroupGuardTo<_> = log::group_guard_to("build", &mut output)?;
///     group.write_all(b"working")?;
/// } // drop writes a separating newline and closes the group
/// assert_eq!(output, b"::group::build\nworking\n::endgroup::\n");
/// # Ok::<(), std::io::Error>(())
/// ```
#[must_use = "the group ends when this guard is dropped"]
pub struct GroupGuardTo<W: Write>(WriterScope<W>);

impl<W: Write> GroupGuardTo<W> {
    /// Consume the guard, write `::endgroup::`, and return its destination.
    ///
    /// Adds a separating newline when needed, without flushing. The closing write
    /// is attempted once; drop does not retry if it fails partway through.
    /// Flush the returned writer to observe delivery errors from owned buffers.
    ///
    /// # Errors
    /// Propagates any write error from the destination.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::log;
    /// use std::io::Write;
    ///
    /// let mut group = log::group_guard_to("build", Vec::new())?;
    /// group.write_all(b"tail")?;
    /// let mut output = group.finish()?;
    /// output.flush()?;
    /// assert_eq!(output, b"::group::build\ntail\n::endgroup::\n");
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn finish(self) -> io::Result<W> {
        self.0.finish()
    }

    /// Suspend workflow commands in this group, first completing any partial line.
    ///
    /// Write replay bytes through the returned guard, then finish or drop it before
    /// closing this group. Unlike bare [`stop_commands_to`], this method knows the
    /// group's line state and inserts a newline before the stop marker if needed.
    ///
    /// # Errors
    /// Propagates separator and opening marker write errors.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::log;
    /// use std::io::Write;
    ///
    /// let mut group = log::group_guard_to("replay", Vec::new())?;
    /// group.write_all(b"tail")?;
    /// let mut stopped = group.stop_commands()?; // completes "tail" before the marker
    /// stopped.write_all(b"::error::literal\n")?;
    /// stopped.finish()?;
    /// let output = group.finish()?;
    /// assert!(output.starts_with(b"::group::replay\ntail\n::stop-commands::"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn stop_commands(&mut self) -> io::Result<StopGuardTo<&mut Self>> {
        if !self.0.line_start {
            self.write_all(b"\n")?;
        }
        stop_commands_to(self)
    }
}

impl<W: Write> Write for GroupGuardTo<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// Start a group on `writer`, returning a guard that closes it on the same writer.
///
/// Pass `&mut writer` to borrow a buffer or a locked stream, then write through the
/// guard's [`Write`] implementation. The opening command uses normal command-data
/// escaping. See [`group_to`] for a closure helper and [`stop_commands_to`] for a
/// nested stderr replay example.
///
/// # Errors
/// Propagates opening write errors. No guard is returned on failure, so an opening
/// marker partially accepted by the writer cannot be cleaned up automatically.
///
/// # Examples
///
/// ```
/// use std::io::Write;
/// use actions_rs::log;
///
/// let mut output = Vec::new();
/// let mut group = log::group_guard_to("build", &mut output)?;
/// group.write_all(b"compiling\n")?;
/// group.finish()?;
/// assert_eq!(output, b"::group::build\ncompiling\n::endgroup::\n");
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn group_guard_to<W: Write>(
    name: impl Into<String>,
    mut writer: W,
) -> io::Result<GroupGuardTo<W>> {
    WorkflowCommand::new("group")
        .message(name)
        .issue_to(&mut writer)?;
    Ok(GroupGuardTo(WriterScope::new(
        writer,
        "::endgroup::".into(),
    )))
}

/// Run `f` inside a group on `writer`, passing a writable guard to the closure.
///
/// Opening, body and closing output all use `writer`. Closes the group even when
/// the closure returns an error or panics. Flushes the writer after successful closing,
/// even if the body failed, so errors delivering owned buffers remain observable.
/// On panic, cleanup is best-effort.
///
/// # Errors
/// Propagates opening, body, closing or flushing errors. If the body fails, returns
/// its error after attempting to close and flush the group.
///
/// # Examples
///
/// ```
/// use std::io::Write;
/// use actions_rs::log;
///
/// let mut output = Vec::new();
/// let answer = log::group_to("compute", &mut output, |group| {
///     writeln!(group, "working")?;
///     Ok(42)
/// })?;
/// assert_eq!(answer, 42);
/// assert_eq!(output, b"::group::compute\nworking\n::endgroup::\n");
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn group_to<W: Write, R>(
    name: impl Into<String>,
    writer: W,
    f: impl FnOnce(&mut GroupGuardTo<W>) -> io::Result<R>,
) -> io::Result<R> {
    let mut guard = group_guard_to(name, writer)?;
    let result = f(&mut guard);
    let closing = guard.finish().and_then(|mut writer| writer.flush());
    let value = result?;
    closing?;
    Ok(value)
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

/// Writer-aware command-suspension guard returned by [`stop_commands_to`].
///
/// Implements [`Write`] to stream arbitrary body bytes unchanged through the
/// borrowed or owned writer. Drop resumes command processing best-effort, including
/// on early return or panic. See the [module documentation](self) for line boundaries
/// and flushing.
///
/// # Examples
///
/// ```
/// use actions_rs::log;
/// use std::io::Write;
///
/// let mut output = Vec::new();
/// let replay = b"::error::literal\n";
/// {
///     let mut stopped: log::StopGuardTo<_> = log::stop_commands_to(&mut output)?;
///     stopped.write_all(replay)?;
/// } // drop emits the matching resume token
/// assert!(output.windows(replay.len()).any(|bytes| bytes == replay));
/// # Ok::<(), std::io::Error>(())
/// ```
#[must_use = "command processing resumes when this guard is dropped"]
pub struct StopGuardTo<W: Write>(WriterScope<W>);

impl<W: Write> StopGuardTo<W> {
    /// Consume the guard, emit its matching resume token, and return its destination.
    ///
    /// Adds a separating newline when needed, without flushing. The closing write
    /// is attempted once; drop does not retry if it fails partway through.
    /// Flush the returned writer to observe delivery errors from owned buffers.
    ///
    /// # Errors
    /// Propagates any write error from the destination.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::log;
    /// use std::io::Write;
    ///
    /// let mut stopped = log::stop_commands_to(Vec::new())?;
    /// stopped.write_all(b"tail")?;
    /// let mut output = stopped.finish()?;
    /// output.flush()?;
    /// let output = String::from_utf8(output).unwrap();
    /// let token = output.lines().next().unwrap().strip_prefix("::stop-commands::").unwrap();
    /// assert!(output.ends_with(&format!("tail\n::{token}::\n")));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn finish(self) -> io::Result<W> {
        self.0.finish()
    }
}

impl<W: Write> Write for StopGuardTo<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// Suspend workflow-command interpretation, emitting stop/resume markers to `writer`.
///
/// Uses the same random token generation as [`stop_commands`]. Write through the
/// returned guard to keep body bytes and both markers on the chosen destination.
/// This streams binary data without buffering or UTF-8 conversion.
///
/// Open the group before suspending commands, resume before closing the group,
/// and start opening markers at a line boundary. Inside a group, prefer
/// [`GroupGuardTo::stop_commands`], which completes any partial group line before
/// the stop marker. A bare writer's existing line state is unknown to this function.
/// If replay lacks a final newline,
/// `finish()` (or drop) inserts one before the resume marker. No implicit flushing
/// occurs; flush through the guard for live output, and flush the destination after
/// finishing to deliver buffered closing markers. `finish()` returns owned writers
/// as well as borrowed ones, allowing callers to observe their flush errors.
///
/// # Errors
/// Propagates opening write errors. No guard is returned on failure, so an opening
/// marker partially accepted by the writer cannot be cleaned up automatically.
///
/// # Examples
///
/// Replay a failed task on stderr, preserving stdout for pipes:
///
/// ```no_run
/// use std::io::{self, Read, Write};
/// use actions_rs::log;
///
/// fn replay_failed_task(mut replay: impl Read) -> io::Result<()> {
///     let mut stderr = io::stderr().lock();
///     let mut group = log::group_guard_to("failed task", &mut stderr)?;
///     let mut stopped = group.stop_commands()?;
///     io::copy(&mut replay, &mut stopped)?;
///     stopped.finish()?; // resume before emitting ::endgroup::
///     group.finish()?;
///     stderr.flush()
/// }
/// ```
pub fn stop_commands_to<W: Write>(mut writer: W) -> io::Result<StopGuardTo<W>> {
    let token = crate::file_command::random_token();
    WorkflowCommand::new("stop-commands")
        .message(token.clone())
        .issue_to(&mut writer)?;
    Ok(StopGuardTo(WriterScope::new(
        writer,
        format!("::{token}::"),
    )))
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

    #[test]
    fn group_macro_is_reachable_via_log_path() {
        // Regression: the `format!`-style macros are re-exported into `log`, so
        // `crate::log::group!` (i.e. `actions_rs::log::group!`) resolves.
        let answer = crate::log::group!("compute", { 6 * 7 });
        assert_eq!(answer, 42);
    }
}
