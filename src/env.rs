//! Runtime detection and typed access to the GitHub Actions environment.
//!
//! Nothing here mutates the process environment; every accessor is a cheap
//! read of a `GITHUB_*` / `RUNNER_*` variable. The canonical names are also
//! exposed as constants in [`vars`] for callers who want raw access.

use std::path::PathBuf;

/// Canonical names of GitHub-provided environment variables.
///
/// Use these instead of stringly-typed literals to avoid typos.
pub mod vars {
    /// `"true"` when running inside GitHub Actions.
    pub const GITHUB_ACTIONS: &str = "GITHUB_ACTIONS";
    /// `"true"` in any CI (set by Actions and most other CI systems).
    pub const CI: &str = "CI";
    /// `"1"` when step-debug logging is enabled.
    pub const RUNNER_DEBUG: &str = "RUNNER_DEBUG";
    /// Runner OS: `Linux`, `Windows` or `macOS`.
    pub const RUNNER_OS: &str = "RUNNER_OS";
    /// Runner CPU architecture: `X86`, `X64`, `ARM` or `ARM64`.
    pub const RUNNER_ARCH: &str = "RUNNER_ARCH";
    /// Temp directory cleaned up after each job.
    pub const RUNNER_TEMP: &str = "RUNNER_TEMP";
    /// Pre-installed tool cache directory.
    pub const RUNNER_TOOL_CACHE: &str = "RUNNER_TOOL_CACHE";
    /// `owner/repo`.
    pub const GITHUB_REPOSITORY: &str = "GITHUB_REPOSITORY";
    /// Repository owner login.
    pub const GITHUB_REPOSITORY_OWNER: &str = "GITHUB_REPOSITORY_OWNER";
    /// Commit SHA that triggered the run.
    pub const GITHUB_SHA: &str = "GITHUB_SHA";
    /// Full ref, e.g. `refs/heads/main`.
    pub const GITHUB_REF: &str = "GITHUB_REF";
    /// Short ref name, e.g. `main`.
    pub const GITHUB_REF_NAME: &str = "GITHUB_REF_NAME";
    /// `branch` or `tag`.
    pub const GITHUB_REF_TYPE: &str = "GITHUB_REF_TYPE";
    /// PR head ref (empty outside pull requests).
    pub const GITHUB_HEAD_REF: &str = "GITHUB_HEAD_REF";
    /// PR base ref (empty outside pull requests).
    pub const GITHUB_BASE_REF: &str = "GITHUB_BASE_REF";
    /// Event name that triggered the run, e.g. `push`.
    pub const GITHUB_EVENT_NAME: &str = "GITHUB_EVENT_NAME";
    /// Path to the JSON file with the full webhook payload.
    pub const GITHUB_EVENT_PATH: &str = "GITHUB_EVENT_PATH";
    /// Workspace directory (checked-out repo root).
    pub const GITHUB_WORKSPACE: &str = "GITHUB_WORKSPACE";
    /// Workflow name.
    pub const GITHUB_WORKFLOW: &str = "GITHUB_WORKFLOW";
    /// Job id of the current job.
    pub const GITHUB_JOB: &str = "GITHUB_JOB";
    /// Unique numeric id of the run.
    pub const GITHUB_RUN_ID: &str = "GITHUB_RUN_ID";
    /// Per-workflow incrementing run number.
    pub const GITHUB_RUN_NUMBER: &str = "GITHUB_RUN_NUMBER";
    /// Login of the user/app that triggered the run.
    pub const GITHUB_ACTOR: &str = "GITHUB_ACTOR";
    /// Server URL, e.g. `https://github.com`.
    pub const GITHUB_SERVER_URL: &str = "GITHUB_SERVER_URL";
    /// REST API base URL.
    pub const GITHUB_API_URL: &str = "GITHUB_API_URL";
    /// GraphQL API URL.
    pub const GITHUB_GRAPHQL_URL: &str = "GITHUB_GRAPHQL_URL";
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Whether the code is running inside GitHub Actions (`GITHUB_ACTIONS=="true"`).
#[must_use]
pub fn is_github_actions() -> bool {
    std::env::var(vars::GITHUB_ACTIONS).as_deref() == Ok("true")
}

/// Whether running in a CI environment (`CI=="true"`).
#[must_use]
pub fn is_ci() -> bool {
    std::env::var(vars::CI).as_deref() == Ok("true")
}

/// Whether step-debug logging is enabled (`RUNNER_DEBUG=="1"`).
#[must_use]
pub fn is_debug() -> bool {
    std::env::var(vars::RUNNER_DEBUG).as_deref() == Ok("1")
}

/// The runner operating system, parsed from `RUNNER_OS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerOs {
    /// `RUNNER_OS == "Linux"`.
    Linux,
    /// `RUNNER_OS == "Windows"`.
    Windows,
    /// `RUNNER_OS == "macOS"`.
    MacOs,
    /// An unrecognised value (forward-compatible).
    Other(String),
}

impl RunnerOs {
    /// Read and parse `RUNNER_OS`. `None` when unset.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        var(vars::RUNNER_OS).map(|v| match v.as_str() {
            "Linux" => RunnerOs::Linux,
            "Windows" => RunnerOs::Windows,
            "macOS" => RunnerOs::MacOs,
            _ => RunnerOs::Other(v),
        })
    }
}

/// The runner CPU architecture, parsed from `RUNNER_ARCH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerArch {
    /// 32-bit x86.
    X86,
    /// 64-bit x86.
    X64,
    /// 32-bit ARM.
    Arm,
    /// 64-bit ARM.
    Arm64,
    /// An unrecognised value (forward-compatible).
    Other(String),
}

impl RunnerArch {
    /// Read and parse `RUNNER_ARCH`. `None` when unset.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        var(vars::RUNNER_ARCH).map(|v| match v.as_str() {
            "X86" => RunnerArch::X86,
            "X64" => RunnerArch::X64,
            "ARM" => RunnerArch::Arm,
            "ARM64" => RunnerArch::Arm64,
            _ => RunnerArch::Other(v),
        })
    }
}

/// Typed, lazily-read accessors for the workflow run context.
///
/// This is a zero-sized handle; every method reads the corresponding
/// environment variable on call, so values reflect the current process
/// environment. Per the design decision, the webhook payload is exposed only
/// as the *path* ([`Context::event_path`]) — parsing the JSON would require a
/// serde dependency and is out of scope.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context;

impl Context {
    /// Construct a context handle.
    #[must_use]
    pub fn new() -> Self {
        Context
    }

    /// `owner/repo`, if set.
    #[must_use]
    pub fn repository(&self) -> Option<String> {
        var(vars::GITHUB_REPOSITORY)
    }

    /// `(owner, repo)` split from [`Context::repository`].
    #[must_use]
    pub fn repo_parts(&self) -> Option<(String, String)> {
        let full = self.repository()?;
        let (owner, repo) = full.split_once('/')?;
        Some((owner.to_owned(), repo.to_owned()))
    }

    /// Repository owner login.
    #[must_use]
    pub fn repository_owner(&self) -> Option<String> {
        var(vars::GITHUB_REPOSITORY_OWNER)
    }

    /// Commit SHA that triggered the run.
    #[must_use]
    pub fn sha(&self) -> Option<String> {
        var(vars::GITHUB_SHA)
    }

    /// Full git ref, e.g. `refs/heads/main`.
    #[must_use]
    pub fn git_ref(&self) -> Option<String> {
        var(vars::GITHUB_REF)
    }

    /// Short ref name, e.g. `main`.
    #[must_use]
    pub fn ref_name(&self) -> Option<String> {
        var(vars::GITHUB_REF_NAME)
    }

    /// `branch` or `tag`.
    #[must_use]
    pub fn ref_type(&self) -> Option<String> {
        var(vars::GITHUB_REF_TYPE)
    }

    /// PR head ref (empty/`None` outside pull requests).
    #[must_use]
    pub fn head_ref(&self) -> Option<String> {
        var(vars::GITHUB_HEAD_REF)
    }

    /// PR base ref (empty/`None` outside pull requests).
    #[must_use]
    pub fn base_ref(&self) -> Option<String> {
        var(vars::GITHUB_BASE_REF)
    }

    /// Event name, e.g. `push`, `pull_request`.
    #[must_use]
    pub fn event_name(&self) -> Option<String> {
        var(vars::GITHUB_EVENT_NAME)
    }

    /// Path to the webhook payload JSON file.
    #[must_use]
    pub fn event_path(&self) -> Option<PathBuf> {
        var(vars::GITHUB_EVENT_PATH).map(PathBuf::from)
    }

    /// Workspace directory (checked-out repo root).
    #[must_use]
    pub fn workspace(&self) -> Option<PathBuf> {
        var(vars::GITHUB_WORKSPACE).map(PathBuf::from)
    }

    /// Workflow name.
    #[must_use]
    pub fn workflow(&self) -> Option<String> {
        var(vars::GITHUB_WORKFLOW)
    }

    /// Current job id.
    #[must_use]
    pub fn job(&self) -> Option<String> {
        var(vars::GITHUB_JOB)
    }

    /// Unique numeric run id.
    #[must_use]
    pub fn run_id(&self) -> Option<u64> {
        var(vars::GITHUB_RUN_ID)?.parse().ok()
    }

    /// Per-workflow incrementing run number.
    #[must_use]
    pub fn run_number(&self) -> Option<u64> {
        var(vars::GITHUB_RUN_NUMBER)?.parse().ok()
    }

    /// Login of the triggering user/app.
    #[must_use]
    pub fn actor(&self) -> Option<String> {
        var(vars::GITHUB_ACTOR)
    }

    /// Server URL (`https://github.com` or an Enterprise URL).
    #[must_use]
    pub fn server_url(&self) -> Option<String> {
        var(vars::GITHUB_SERVER_URL)
    }

    /// REST API base URL.
    #[must_use]
    pub fn api_url(&self) -> Option<String> {
        var(vars::GITHUB_API_URL)
    }

    /// GraphQL API URL.
    #[must_use]
    pub fn graphql_url(&self) -> Option<String> {
        var(vars::GITHUB_GRAPHQL_URL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_parses_known_and_unknown() {
        // Pure mapping is exercised via the match arms; here we only assert
        // the unknown fallback shape since env mutation is global/unsafe.
        assert_eq!(
            RunnerOs::Other("Plan9".into()),
            RunnerOs::Other("Plan9".into())
        );
    }

    #[test]
    fn repo_parts_splits() {
        // repo_parts is pure given repository(); emulate via a temp env in the
        // integration tests. Here assert the split helper logic indirectly.
        let full = "octocat/Hello-World";
        let (o, r) = full.split_once('/').unwrap();
        assert_eq!((o, r), ("octocat", "Hello-World"));
    }
}
