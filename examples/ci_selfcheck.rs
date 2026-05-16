//! `cargo run --example ci_selfcheck`
//!
//! Build the report once, write it BOTH to stdout (the job log) AND to a
//! tmpfile on the runner, then read that tmpfile back and drop it into the
//! job summary as a single code block. No shell, no prose. Exits 1 only on a
//! real round-trip mismatch.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use actions_rs::{Annotation, AnnotationKind, AnnotationSpan, Summary, output};

fn read_env_file(var: &str) -> Option<String> {
    std::fs::read_to_string(PathBuf::from(std::env::var_os(var)?)).ok()
}

fn main() -> ExitCode {
    let notice = Annotation::new()
        .file("examples/ci_selfcheck.rs")
        .span(AnnotationSpan::Line {
            start: 18,
            end: Some(24),
        })
        .title("ci_selfcheck")
        .command(
            AnnotationKind::Notice,
            "actions-rs self-check ran in this job",
        );
    let warning = Annotation::new()
        .file("src/summary.rs")
        .span(AnnotationSpan::Column {
            line: 112,
            start: 5,
            end: None,
        })
        .title("example warning")
        .command(
            AnnotationKind::Warning,
            "ranged warning annotation covering Summary::code_block",
        );
    notice.issue();
    warning.issue();

    if let Err(e) = output::set_output("answer", "42\nwith newline") {
        eprintln!("::error::set_output: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = output::export_var("DEMO_FLAG", true)
        && !matches!(
            e,
            actions_rs::Error::UnavailableFileCommand {
                var: "GITHUB_ENV",
                ..
            }
        )
    {
        eprintln!("::error::export_var: {e}");
        return ExitCode::FAILURE;
    }

    let gh_output = read_env_file("GITHUB_OUTPUT").unwrap_or_else(|| "<local: unset>".into());
    let gh_env = read_env_file("GITHUB_ENV").unwrap_or_else(|| "<local: unset>".into());

    // Build the whole report once.
    let mut report = String::new();
    for (label, body) in [
        ("workflow commands (stdout)", format!("{notice}\n{warning}")),
        ("GITHUB_OUTPUT", gh_output.clone()),
        ("GITHUB_ENV", gh_env.clone()),
    ] {
        let _ = write!(report, "===== {label} =====\n{body}\n");
    }

    // 1. normal out (the job log).
    print!("{report}");

    // 2. a tmpfile on the runner.
    let tmp = std::env::var_os("RUNNER_TEMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ci_selfcheck.report.txt");
    if let Err(e) = std::fs::write(&tmp, &report) {
        eprintln!("::error::tmpfile write: {e}");
        return ExitCode::FAILURE;
    }

    // 3. read the tmpfile back, drop it into the summary as one code block.
    let captured = match std::fs::read_to_string(&tmp) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("::error::tmpfile read: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut summary = Summary::new();
    summary
        .heading("actions-rs ci_selfcheck", 2)
        .code_block(&captured, None);
    if let Err(e) = summary.write_overwrite() {
        eprintln!("::error::summary.write_overwrite: {e}");
        return ExitCode::FAILURE;
    }

    // round-trip assertions (only meaningful when the runner set the files).
    let mut exit = ExitCode::SUCCESS;
    if std::env::var_os("GITHUB_OUTPUT").is_some() {
        for (var, needle, hay) in [
            ("GITHUB_OUTPUT", "answer<<", &gh_output),
            ("GITHUB_ENV", "DEMO_FLAG<<", &gh_env),
        ] {
            if !hay.contains(needle) {
                eprintln!("::error::{var} missing {needle:?}");
                exit = ExitCode::FAILURE;
            }
        }
    }
    exit
}
