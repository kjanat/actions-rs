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

    let prev = std::env::var_os(var);
    // SAFETY: serialised by ENV_LOCK; no other thread reads/writes env here.
    unsafe { std::env::set_var(var, &path) };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&path)));

    // SAFETY: serialised by ENV_LOCK. Restore the runner's prior value rather
    // than blindly removing it, so later tests/steps are not perturbed.
    unsafe {
        match prev {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }
    let _ = std::fs::remove_file(&path);

    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn with_env_var(var: &str, value: &str, f: impl FnOnce()) {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let prev = std::env::var_os(var);
    unsafe { std::env::set_var(var, value) };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

    // SAFETY: serialised by ENV_LOCK. Restore prior value, don't just remove.
    unsafe {
        match prev {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }

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
    // the deprecated `::set-output::` stdout command. That goes to fd 1, which
    // libtest does NOT capture, so under `cargo test` on a real runner GitHub
    // would parse it and emit the set-output deprecation warning. Fence it
    // with stop-commands so the runner ignores the emitted command.
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: serialised by ENV_LOCK.
    unsafe { std::env::remove_var("GITHUB_OUTPUT") };
    let _stop = actions_rs::log::stop_commands();
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

#[test]
fn export_var_and_add_path_error_without_env_files() {
    // The CI runner sets GITHUB_ENV/GITHUB_PATH for every step, so this must
    // explicitly unset them to exercise the no-fallback error path.
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let prev_env = std::env::var_os("GITHUB_ENV");
    let prev_path = std::env::var_os("GITHUB_PATH");
    // SAFETY: serialised by ENV_LOCK.
    unsafe {
        std::env::remove_var("GITHUB_ENV");
        std::env::remove_var("GITHUB_PATH");
    }

    let env_result = actions_rs::output::export_var("MY_FLAG", true);
    let path_result = actions_rs::output::add_path("/tmp/bin");

    // Restore before asserting so a failed assertion can't leak state.
    // SAFETY: serialised by ENV_LOCK.
    unsafe {
        match prev_env {
            Some(v) => std::env::set_var("GITHUB_ENV", v),
            None => std::env::remove_var("GITHUB_ENV"),
        }
        match prev_path {
            Some(v) => std::env::set_var("GITHUB_PATH", v),
            None => std::env::remove_var("GITHUB_PATH"),
        }
    }

    assert!(matches!(
        env_result.unwrap_err(),
        actions_rs::Error::UnavailableFileCommand {
            var: "GITHUB_ENV",
            ..
        }
    ));
    assert!(matches!(
        path_result.unwrap_err(),
        actions_rs::Error::UnavailableFileCommand {
            var: "GITHUB_PATH",
            ..
        }
    ));
}

#[test]
fn required_input_accepts_whitespace_only_then_trims() {
    with_env_var("INPUT_FLAG", "   ", || {
        let value = actions_rs::input::input_required("flag").unwrap();
        assert_eq!(value, "");
    });
}

#[test]
fn multiline_input_keeps_whitespace_only_lines() {
    with_env_var("INPUT_ITEMS", "a\n   \n\n b\n", || {
        assert_eq!(
            actions_rs::input::multiline_input("items"),
            vec!["a".to_owned(), "".to_owned(), "b".to_owned()]
        );
    });
}

#[test]
fn summary_write_drains_buffer() {
    with_env_file("GITHUB_STEP_SUMMARY", |path| {
        let mut summary = actions_rs::Summary::new();
        summary.heading("One", 2);
        summary.write().unwrap();
        assert!(summary.is_empty());

        summary.write().unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "<h2>One</h2>\n");
    });
}

#[test]
fn summary_append_limit_counts_existing_file_bytes() {
    with_env_file("GITHUB_STEP_SUMMARY", |path| {
        std::fs::write(path, "x".repeat(700 * 1024)).unwrap();

        let mut summary = actions_rs::Summary::new();
        summary.raw("y".repeat(400 * 1024), false);
        let err = summary.write().unwrap_err();
        assert!(matches!(err, actions_rs::Error::SummaryTooLarge { .. }));
    });
}
