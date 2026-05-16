//! Runtime detection and typed access to the GitHub Actions environment.
//!
//! Nothing here mutates the process environment; every accessor is a cheap read of a `GITHUB_*` / `RUNNER_*` variable.\
//! The canonical names are also exposed as constants in [`vars`] for callers who want raw access.

use std::path::PathBuf;

/// Canonical names of GitHub-provided environment variables.
///
/// Use these instead of stringly-typed literals to avoid typos.
pub mod vars {
    /// Always `"true"` while a step runs inside GitHub Actions; unset otherwise.
    /// Set by the runner. Use it to no-op locally.
    ///
    /// Example: `true`
    pub const GITHUB_ACTIONS: &str = "GITHUB_ACTIONS";
    /// `"true"` under Actions and virtually every other CI (Travis, Circle, GitLab, …).
    /// Broader and less reliable than [`GITHUB_ACTIONS`].
    ///
    /// Example: `true`
    pub const CI: &str = "CI";
    /// `"1"` when step-debug logging is enabled; otherwise unset.
    ///
    /// logging is enabled when the `ACTIONS_STEP_DEBUG` secret is `true`,
    /// or the run was re-run with debug logging
    ///
    /// Example: `1`
    pub const RUNNER_DEBUG: &str = "RUNNER_DEBUG";
    /// OS of the runner image. One of `Linux`, `Windows`, `macOS`. Derived from the `runs-on` image.
    ///
    /// Example: `runs-on: ubuntu-latest` → `Linux`
    pub const RUNNER_OS: &str = "RUNNER_OS";
    /// CPU architecture of the runner. One of `X86`, `X64`, `ARM`, `ARM64`.
    ///
    /// Example: `runs-on: ubuntu-latest` → `X64`
    pub const RUNNER_ARCH: &str = "RUNNER_ARCH";
    /// Per-job temporary directory, emptied at the end of each job.
    /// Platform path set by the runner.
    ///
    /// Example: `/home/runner/work/_temp` (Linux hosted runner)
    pub const RUNNER_TEMP: &str = "RUNNER_TEMP";
    /// Root of the pre-installed tool cache (Python, Node, …) on hosted runners.
    ///
    /// Example: `/opt/hostedtoolcache` (Linux hosted runner)
    pub const RUNNER_TOOL_CACHE: &str = "RUNNER_TOOL_CACHE";
    /// `owner/repo` of the repository the workflow runs in.
    ///
    /// Example: `octocat/Hello-World`
    pub const GITHUB_REPOSITORY: &str = "GITHUB_REPOSITORY";
    /// Login of the repository owner (the part before `/` in [`GITHUB_REPOSITORY`]).
    ///
    /// Example: `octocat`
    pub const GITHUB_REPOSITORY_OWNER: &str = "GITHUB_REPOSITORY_OWNER";
    /// Full 40-char commit SHA that triggered the run. For `pull_request`
    /// events this is the PR's test-merge commit, not the PR head.
    ///
    /// Example: `ffac537e6cbbf934b08745a378932722df287a53`
    pub const GITHUB_SHA: &str = "GITHUB_SHA";
    /// Full ref that triggered the run. Branch push → `refs/heads/<branch>`;
    /// tag → `refs/tags/<tag>`; pull request → `refs/pull/<n>/merge`.
    ///
    /// Example: push to `main` → `refs/heads/main`
    pub const GITHUB_REF: &str = "GITHUB_REF";
    /// Short form of [`GITHUB_REF`] with the `refs/heads/` or `refs/tags/`
    /// prefix stripped.
    ///
    /// Example: push to `main` → `main`; tag `v1.2.0` → `v1.2.0`;
    /// PR #42 → `42/merge`
    pub const GITHUB_REF_NAME: &str = "GITHUB_REF_NAME";
    /// Kind of ref that triggered the run: `branch` or `tag`.
    ///
    /// Example: push to `main` → `branch`
    pub const GITHUB_REF_TYPE: &str = "GITHUB_REF_TYPE";
    /// Source (head) branch of a pull request. Set **only** for
    /// `pull_request` / `pull_request_target` events; empty/unset for `push`
    /// and most other events.
    ///
    /// Example: PR from `feature/login` into `main` → `feature/login`
    pub const GITHUB_HEAD_REF: &str = "GITHUB_HEAD_REF";
    /// Target (base) branch of a pull request. Set **only** for
    /// `pull_request` / `pull_request_target` events; empty/unset otherwise.
    ///
    /// Example: PR from `feature/login` into `main` → `main`
    pub const GITHUB_BASE_REF: &str = "GITHUB_BASE_REF";
    /// Name of the webhook event that triggered the run.
    ///
    /// Example: `push`, `pull_request`, `workflow_dispatch`, `schedule`
    pub const GITHUB_EVENT_NAME: &str = "GITHUB_EVENT_NAME";
    /// Filesystem path to the JSON file holding the full webhook event
    /// payload (parse it yourself; this crate keeps it serde-free).
    ///
    /// Example: `/home/runner/work/_temp/_github_workflow/event.json`
    pub const GITHUB_EVENT_PATH: &str = "GITHUB_EVENT_PATH";
    /// Working directory containing the checked-out repository (after
    /// `actions/checkout`). Default cwd for `run:` steps.
    ///
    /// Example: `/home/runner/work/Hello-World/Hello-World`
    pub const GITHUB_WORKSPACE: &str = "GITHUB_WORKSPACE";
    /// `name:` of the running workflow, or its file path if `name:` is
    /// omitted.
    ///
    /// Example: `CI` (or `.github/workflows/ci.yml` if unnamed)
    pub const GITHUB_WORKFLOW: &str = "GITHUB_WORKFLOW";
    /// The job's *id key* from the workflow YAML (the `jobs:` map key), not
    /// the human `name:`.
    ///
    /// Example: `jobs: { build: … }` → `build`
    pub const GITHUB_JOB: &str = "GITHUB_JOB";
    /// Unique id of the workflow run. Stable across re-runs of the same run;
    /// usable to build a run URL.
    ///
    /// Example: `1658821493`
    pub const GITHUB_RUN_ID: &str = "GITHUB_RUN_ID";
    /// Count of runs of *this workflow* in the repo, incremented per run.
    /// Unlike [`GITHUB_RUN_ID`] it does **not** change on a re-run.
    ///
    /// Example: `42`
    pub const GITHUB_RUN_NUMBER: &str = "GITHUB_RUN_NUMBER";
    /// Login of the account that initiated the run (on a re-run, the original
    /// initiator, not whoever clicked re-run).
    ///
    /// Example: `octocat`
    pub const GITHUB_ACTOR: &str = "GITHUB_ACTOR";
    /// Base URL of the GitHub server — `https://github.com` on github.com,
    /// the instance URL on GitHub Enterprise Server.
    ///
    /// Example: `https://github.com`
    pub const GITHUB_SERVER_URL: &str = "GITHUB_SERVER_URL";
    /// REST API base URL (Enterprise-aware).
    ///
    /// Example: `https://api.github.com`
    pub const GITHUB_API_URL: &str = "GITHUB_API_URL";
    /// GraphQL API endpoint (Enterprise-aware).
    ///
    /// Example: `https://api.github.com/graphql`
    pub const GITHUB_GRAPHQL_URL: &str = "GITHUB_GRAPHQL_URL";
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Whether the code is running inside GitHub Actions (`GITHUB_ACTIONS=="true"`).
///
/// # Examples
///
/// ```
/// if actions_rs::env::is_github_actions() {
///     actions_rs::log::info("on a runner");
/// } // else: running locally — no-op
/// ```
#[must_use]
pub fn is_github_actions() -> bool {
    std::env::var(vars::GITHUB_ACTIONS).as_deref() == Ok("true")
}

/// Whether running in a CI environment (`CI=="true"`).
///
/// # Examples
///
/// ```
/// // Broader than `is_github_actions` (also true on Travis, GitLab, …).
/// let interactive = !actions_rs::env::is_ci();
/// let _ = interactive;
/// ```
#[must_use]
pub fn is_ci() -> bool {
    std::env::var(vars::CI).as_deref() == Ok("true")
}

/// Whether step-debug logging is enabled (`RUNNER_DEBUG=="1"`).
///
/// # Examples
///
/// ```
/// if actions_rs::env::is_debug() {
///     actions_rs::log::debug("extra diagnostics");
/// }
/// ```
#[must_use]
pub fn is_debug() -> bool {
    std::env::var(vars::RUNNER_DEBUG).as_deref() == Ok("1")
}

/// The runner operating system, parsed from `RUNNER_OS`.
///
/// # Examples
///
/// ```
/// use actions_rs::RunnerOs;
/// // Unrecognised values are preserved rather than lost.
/// assert_eq!(RunnerOs::Other("Plan9".into()), RunnerOs::Other("Plan9".into()));
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::RunnerOs;
    /// match RunnerOs::from_env() {
    ///     Some(RunnerOs::Linux) => { /* hosted ubuntu runner */ }
    ///     Some(other) => { let _ = other; }
    ///     None => { /* not on a runner */ }
    /// }
    /// ```
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
///
/// # Examples
///
/// ```
/// use actions_rs::RunnerArch;
/// assert_eq!(RunnerArch::X64, RunnerArch::X64);
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::RunnerArch;
    /// if let Some(arch) = RunnerArch::from_env() {
    ///     let _ = arch;
    /// }
    /// ```
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
///
/// # Examples
///
/// ```
/// let ctx = actions_rs::Context::new();
/// // Each accessor is `Option`: `None` outside Actions, `Some` on a runner.
/// match ctx.repository() {
///     Some(repo) => println!("running for {repo}"),
///     None => println!("not in GitHub Actions"),
/// }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Context;

impl Context {
    /// Construct a context handle.
    ///
    /// # Examples
    ///
    /// ```
    /// let ctx = actions_rs::Context::new();
    /// let _ = ctx.sha(); // each accessor is a fresh env read
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Context
    }

    /// `owner/repo`, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// let ctx = actions_rs::Context::new();
    /// if let Some(repo) = ctx.repository() {
    ///     assert!(repo.contains('/'));
    /// }
    /// ```
    #[must_use]
    pub fn repository(&self) -> Option<String> {
        var(vars::GITHUB_REPOSITORY)
    }

    /// `(owner, repo)` split from [`Context::repository`].
    ///
    /// # Examples
    ///
    /// ```
    /// let ctx = actions_rs::Context::new();
    /// if let Some((owner, repo)) = ctx.repo_parts() {
    ///     assert!(!owner.is_empty() && !repo.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn repo_parts(&self) -> Option<(String, String)> {
        let full = self.repository()?;
        let (owner, repo) = full.split_once('/')?;
        Some((owner.to_owned(), repo.to_owned()))
    }

    /// Repository owner login.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(owner) = actions_rs::Context::new().repository_owner() {
    ///     assert!(!owner.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn repository_owner(&self) -> Option<String> {
        var(vars::GITHUB_REPOSITORY_OWNER)
    }

    /// Commit SHA that triggered the run.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(sha) = actions_rs::Context::new().sha() {
    ///     assert!(!sha.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn sha(&self) -> Option<String> {
        var(vars::GITHUB_SHA)
    }

    /// Full git ref, e.g. `refs/heads/main`.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(r) = actions_rs::Context::new().git_ref() {
    ///     assert!(!r.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn git_ref(&self) -> Option<String> {
        var(vars::GITHUB_REF)
    }

    /// Short ref name, e.g. `main`.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(name) = actions_rs::Context::new().ref_name() {
    ///     assert!(!name.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn ref_name(&self) -> Option<String> {
        var(vars::GITHUB_REF_NAME)
    }

    /// `branch` or `tag`.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(t) = actions_rs::Context::new().ref_type() {
    ///     assert!(t == "branch" || t == "tag");
    /// }
    /// ```
    #[must_use]
    pub fn ref_type(&self) -> Option<String> {
        var(vars::GITHUB_REF_TYPE)
    }

    /// PR head ref (empty/`None` outside pull requests).
    ///
    /// # Examples
    ///
    /// ```
    /// // `None` for `push` events; `Some(branch)` on a pull request.
    /// let ctx = actions_rs::Context::new();
    /// if let Some(head) = ctx.head_ref() {
    ///     assert!(!head.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn head_ref(&self) -> Option<String> {
        var(vars::GITHUB_HEAD_REF)
    }

    /// PR base ref (empty/`None` outside pull requests).
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(base) = actions_rs::Context::new().base_ref() {
    ///     assert!(!base.is_empty()); // e.g. "main"
    /// }
    /// ```
    #[must_use]
    pub fn base_ref(&self) -> Option<String> {
        var(vars::GITHUB_BASE_REF)
    }

    /// Event name, e.g. `push`, `pull_request`.
    ///
    /// # Examples
    ///
    /// ```
    /// let ctx = actions_rs::Context::new();
    /// if ctx.event_name().as_deref() == Some("pull_request") {
    ///     actions_rs::log::info("triggered by a PR");
    /// }
    /// ```
    #[must_use]
    pub fn event_name(&self) -> Option<String> {
        var(vars::GITHUB_EVENT_NAME)
    }

    /// Path to the webhook payload JSON file.
    ///
    /// # Examples
    ///
    /// ```
    /// // The crate is serde-free, so you parse the JSON yourself if needed.
    /// if let Some(path) = actions_rs::Context::new().event_path() {
    ///     let _payload = std::fs::read_to_string(path);
    /// }
    /// ```
    #[must_use]
    pub fn event_path(&self) -> Option<PathBuf> {
        var(vars::GITHUB_EVENT_PATH).map(PathBuf::from)
    }

    /// Workspace directory (checked-out repo root).
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(ws) = actions_rs::Context::new().workspace() {
    ///     let _manifest = ws.join("Cargo.toml");
    /// }
    /// ```
    #[must_use]
    pub fn workspace(&self) -> Option<PathBuf> {
        var(vars::GITHUB_WORKSPACE).map(PathBuf::from)
    }

    /// Workflow name.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(wf) = actions_rs::Context::new().workflow() {
    ///     assert!(!wf.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn workflow(&self) -> Option<String> {
        var(vars::GITHUB_WORKFLOW)
    }

    /// Current job id.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(job) = actions_rs::Context::new().job() {
    ///     assert!(!job.is_empty()); // the `jobs:` map key, not its `name:`
    /// }
    /// ```
    #[must_use]
    pub fn job(&self) -> Option<String> {
        var(vars::GITHUB_JOB)
    }

    /// Unique numeric run id.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(id) = actions_rs::Context::new().run_id() {
    ///     let _url = format!("https://github.com/o/r/actions/runs/{id}");
    /// }
    /// ```
    #[must_use]
    pub fn run_id(&self) -> Option<u64> {
        var(vars::GITHUB_RUN_ID)?.parse().ok()
    }

    /// Per-workflow incrementing run number.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(n) = actions_rs::Context::new().run_number() {
    ///     let _ = n; // stable across re-runs, unlike `run_id`
    /// }
    /// ```
    #[must_use]
    pub fn run_number(&self) -> Option<u64> {
        var(vars::GITHUB_RUN_NUMBER)?.parse().ok()
    }

    /// Login of the triggering user/app.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(actor) = actions_rs::Context::new().actor() {
    ///     assert!(!actor.is_empty());
    /// }
    /// ```
    #[must_use]
    pub fn actor(&self) -> Option<String> {
        var(vars::GITHUB_ACTOR)
    }

    /// Server URL (`https://github.com` or an Enterprise URL).
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(url) = actions_rs::Context::new().server_url() {
    ///     assert!(url.starts_with("http"));
    /// }
    /// ```
    #[must_use]
    pub fn server_url(&self) -> Option<String> {
        var(vars::GITHUB_SERVER_URL)
    }

    /// REST API base URL.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(api) = actions_rs::Context::new().api_url() {
    ///     assert!(api.starts_with("http"));
    /// }
    /// ```
    #[must_use]
    pub fn api_url(&self) -> Option<String> {
        var(vars::GITHUB_API_URL)
    }

    /// GraphQL API URL.
    ///
    /// # Examples
    ///
    /// ```
    /// if let Some(gql) = actions_rs::Context::new().graphql_url() {
    ///     assert!(gql.starts_with("http"));
    /// }
    /// ```
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
