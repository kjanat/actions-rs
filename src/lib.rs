//! # actions-rs
//!
//! A **zero-dependency** toolkit for writing GitHub Actions in Rust — the Rust analogue of `@actions/core`.
//! It speaks the GitHub Actions *workflow-command* and *environment-file* protocols so your action can:
//!
//! - emit `notice` / `warning` / `error` annotations with file + line/column ranges ([`Annotation`], [`log`]),
//! - group and mask log output, and pause command interpretation ([`log::group`], [`log::mask`], [`log::stop_commands`]),
//! - read typed, validated inputs ([`input`]),
//! - set step outputs, saved state, env vars and `PATH` ([`output`]) — using modern env files,
//!   with deprecated-command fallback only for output/state,
//! - build a rich job summary ([`Summary`]),
//! - detect and inspect the runtime ([`env`](mod@env)).
//!
//! Pure stdout commands are infallible;
//! operations that touch the filesystem or parse input return [`Result`].
//!
//! ## Quick start
//!
//! ```
//! use actions_rs::{Annotation, log};
//!
//! if actions_rs::env::is_github_actions() {
//!     log::info("running inside GitHub Actions");
//! }
//!
//! // A located warning, rendered as a workflow command on stdout.
//! Annotation::new()
//!     .file("src/lib.rs")
//!     .line(1)
//!     .title("example")
//!     .warning("this is just a demo");
//!
//! // `format!`-style macros are exported at the crate root.
//! actions_rs::warning!("disk {}% full", 92);
//!
//! // A group that closes even if the closure panics.
//! let answer = actions_rs::group!("compute", { 6 * 7 });
//! assert_eq!(answer, 42);
//! ```

#![forbid(unsafe_code)]

pub mod annotation;
pub mod command;
pub mod env;
pub mod error;
mod escape;
mod file_command;
pub mod input;
pub mod log;
mod macros;
pub mod output;
pub mod summary;

pub use annotation::{Annotation, AnnotationKind, AnnotationSpan};
pub use command::WorkflowCommand;
pub use env::{Context, RunnerArch, RunnerOs};
pub use error::{Error, Result};
pub use input::InputOptions;
pub use summary::{Cell, Summary, SummaryText};

/// Common imports for action authors: `use actions_rs::prelude::*;`.
pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::input::InputOptions;
    pub use crate::summary::{Cell, Summary, SummaryText};
    pub use crate::{
        Annotation, AnnotationKind, AnnotationSpan, Context, RunnerArch, RunnerOs, WorkflowCommand,
    };
    pub use crate::{debug, error, group, info, notice, warning};
    pub use crate::{env, input, log, output};
}
