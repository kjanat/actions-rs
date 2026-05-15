//! Integration tests for the environment-file mechanism.
//!
//! These mutate the process environment, which is `unsafe` in edition 2024 and
//! process-global, so every test takes a shared lock to serialise. Each test
//! points a `GITHUB_*` variable at a private temp file and asserts the exact
//! bytes written (the heredoc delimiter is random, so it is parsed back out of
//! the first line and then matched exactly).

use std::path::Path;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_env_file(var: &str, f: impl FnOnce(&Path)) {
    // Recover a poisoned lock: a panicking earlier test must not wedge the
    // rest of the suite.
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let path = std::env::temp_dir().join(format!(
        "actions_rs_it_{}_{var}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, b"").expect("create temp env file");

    // SAFETY: serialised by ENV_LOCK; no other thread reads/writes env here.
    unsafe { std::env::set_var(var, &path) };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&path)));

    unsafe { std::env::remove_var(var) };
    let _ = std::fs::remove_file(&path);

    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn set_output_writes_validated_heredoc() {
    with_env_file("GITHUB_OUTPUT", |path| {
        actions_rs::output::set_output("result", "hello\nworld").unwrap();
        let content = std::fs::read_to_string(path).unwrap();

        let first = content.lines().next().unwrap();
        let (key, delim) = first.split_once("<<").unwrap();
        assert_eq!(key, "result");
        assert!(
            delim.starts_with("ghadelimiter_"),
            "unexpected delimiter: {delim}"
        );
        assert_eq!(content, format!("result<<{delim}\nhello\nworld\n{delim}\n"));
    });
}

#[test]
fn export_var_accepts_non_reserved_and_serialises_display() {
    with_env_file("GITHUB_ENV", |path| {
        actions_rs::output::export_var("MY_FLAG", true).unwrap();
        actions_rs::output::export_var("MY_COUNT", 42_u32).unwrap();
        let content = std::fs::read_to_string(path).unwrap();

        assert!(content.contains("MY_FLAG<<ghadelimiter_"));
        assert!(content.contains("\ntrue\n"));
        assert!(content.contains("MY_COUNT<<ghadelimiter_"));
        assert!(content.contains("\n42\n"));
    });
}

#[test]
fn add_path_appends_bare_line() {
    with_env_file("GITHUB_PATH", |path| {
        actions_rs::output::add_path("/opt/tools/bin").unwrap();
        actions_rs::output::add_path("/home/runner/.local/bin").unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "/opt/tools/bin\n/home/runner/.local/bin\n");
    });
}

#[test]
fn missing_env_file_var_is_a_clean_fallback_not_an_error() {
    // With GITHUB_OUTPUT unset, set_output must not error: it falls back to
    // the deprecated stdout command (printed to this process's stdout, which
    // the harness captures and discards).
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: serialised by ENV_LOCK.
    unsafe { std::env::remove_var("GITHUB_OUTPUT") };
    actions_rs::output::set_output("k", "v").expect("fallback must succeed");
}

#[test]
fn get_state_reads_state_prefixed_var() {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: serialised by ENV_LOCK.
    unsafe { std::env::set_var("STATE_cache_hit", "true") };
    assert_eq!(
        actions_rs::output::get_state("cache_hit"),
        Some("true".to_owned())
    );
    unsafe { std::env::remove_var("STATE_cache_hit") };
}

#[test]
fn export_reserved_name_is_rejected() {
    let err = actions_rs::output::export_var("GITHUB_SHA", "x").unwrap_err();
    assert!(matches!(err, actions_rs::Error::ReservedName(_)));
}
