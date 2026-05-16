//! Error type for fallible operations (environment-file writes, typed input
//! parsing, oversized job summaries).
//!
//! Pure stdout workflow commands (annotations, groups, masking) never return
//! an error — see the [`crate::log`] module.

use std::fmt;
use std::path::PathBuf;

/// Errors produced by fallible `actions-rs` operations.
///
/// `#[non_exhaustive]` so new variants can be added without a breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An underlying I/O error while reading or appending an environment file.
    Io(std::io::Error),
    /// The runner did not provide the required environment-file path for an
    /// operation whose stdout command fallback has been retired.
    UnavailableFileCommand {
        /// The environment variable that should point at the file.
        var: &'static str,
        /// The attempted operation (for diagnostics).
        operation: &'static str,
    },
    /// The environment-file variable pointed at a path that does not exist.
    ///
    /// GitHub sets these (`GITHUB_ENV`, `GITHUB_OUTPUT`, ...) to a real file;
    /// if the variable is present but the file is missing the runner state is
    /// broken and we surface it rather than silently dropping the write.
    MissingEnvFile {
        /// The environment variable name (e.g. `GITHUB_OUTPUT`).
        var: &'static str,
        /// The path the variable pointed at.
        path: PathBuf,
    },
    /// The randomly generated heredoc delimiter collided with the key or value
    /// being written. Astronomically unlikely; retrying will pick a fresh
    /// delimiter. Mirrors `@actions/core`, which also errors in this case.
    DelimiterCollision,
    /// Attempted to export a reserved variable via [`crate::output::export_var`]
    /// (`GITHUB_*`, `RUNNER_*`, or `NODE_OPTIONS`). The runner forbids this.
    ReservedName(String),
    /// A boolean input did not match the strict YAML 1.2 core schema
    /// (`true|True|TRUE|false|False|FALSE`).
    InvalidBool {
        /// The input name that was queried.
        name: String,
        /// The offending raw value.
        value: String,
    },
    /// A required input was absent or empty.
    MissingRequiredInput(String),
    /// A typed input could not be parsed into the requested type.
    ParseInput {
        /// The input name that was queried.
        name: String,
        /// A human-readable reason from the type's `FromStr` implementation.
        reason: String,
    },
    /// The job summary buffer exceeded GitHub's 1 MiB per-step limit.
    SummaryTooLarge {
        /// The size of the buffer that was rejected, in bytes.
        bytes: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::UnavailableFileCommand { var, operation } => write!(
                f,
                "`{operation}` requires `{var}`; GitHub retired the stdout fallback for this operation"
            ),
            Error::MissingEnvFile { var, path } => {
                write!(f, "{var} points at missing file: {}", path.display())
            }
            Error::DelimiterCollision => {
                f.write_str("generated heredoc delimiter collided with content")
            }
            Error::ReservedName(name) => {
                write!(f, "`{name}` is a reserved variable and cannot be exported")
            }
            Error::InvalidBool { name, value } => write!(
                f,
                "input `{name}` is not a valid boolean (got {value:?}); \
                 expected one of true|True|TRUE|false|False|FALSE"
            ),
            Error::MissingRequiredInput(name) => {
                write!(f, "required input `{name}` was not supplied")
            }
            Error::ParseInput { name, reason } => {
                write!(f, "could not parse input `{name}`: {reason}")
            }
            Error::SummaryTooLarge { bytes } => write!(
                f,
                "job summary is {bytes} bytes, exceeding the 1 MiB per-step limit"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Convenience alias for results returned by fallible `actions-rs` operations.
pub type Result<T> = std::result::Result<T, Error>;
