//! `cargo run --example ci_selfcheck`
//!
//! Prints the actual artifacts the crate emits (raw, verbatim), then writes a
//! job summary that is those same artifacts formatted as code blocks. No
//! prose, no "OK", no "verified" — just the real bytes, twice: raw in the log
//! and rendered in the summary. Exits 1 only on a real mismatch.

use std::path::PathBuf;
use std::process::ExitCode;

use actions_rs::{Annotation, AnnotationKind, Summary, output};

fn read_env_file(var: &str) -> Option<String> {
    std::fs::read_to_string(PathBuf::from(std::env::var_os(var)?)).ok()
}

fn main() -> ExitCode {
    let notice = Annotation::new()
        .file("examples/ci_selfcheck.rs")
        .line(18)
        .end_line(24)
        .col(5)
        .title("ci_selfcheck")
        .command(
            AnnotationKind::Notice,
            "actions-rs self-check ran in this job",
        );
    let warning = Annotation::new()
        .file("src/summary.rs")
        .line(112)
        .end_line(126)
        .col(5)
        .title("example warning")
        .command(
            AnnotationKind::Warning,
            "this is a ranged warning annotation covering Summary::code_block",
        );
    let commands = format!("{notice}\n{warning}");

    notice.issue();
    warning.issue();

    let mut exit = ExitCode::SUCCESS;
    if let Err(e) = output::set_output("answer", "42\nwith newline") {
        eprintln!("::error::set_output: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = output::export_var("DEMO_FLAG", true) {
        eprintln!("::error::export_var: {e}");
        return ExitCode::FAILURE;
    }

    let gh_output = read_env_file("GITHUB_OUTPUT").unwrap_or_else(|| "<local: unset>".into());
    let gh_env = read_env_file("GITHUB_ENV").unwrap_or_else(|| "<local: unset>".into());

    let artifacts = [
        ("workflow commands (stdout)", commands.as_str()),
        ("GITHUB_OUTPUT", gh_output.as_str()),
        ("GITHUB_ENV", gh_env.as_str()),
    ];

    // raw, verbatim, in the log
    for (label, body) in artifacts {
        println!("---8<--- {label} ---8<---");
        println!("{body}");
        println!("---8<--- /{label} ---8<---");
    }

    // same artifacts, formatted in the job summary
    let mut summary = Summary::new();
    summary.heading("actions-rs — emitted artifacts", 2);
    for (label, body) in artifacts {
        summary.heading(label, 3).code_block(body, None);
    }
    if let Err(e) = summary.write_overwrite() {
        eprintln!("::error::summary.write_overwrite: {e}");
        return ExitCode::FAILURE;
    }

    if let Some(s) = read_env_file("GITHUB_STEP_SUMMARY") {
        println!("---8<--- GITHUB_STEP_SUMMARY ---8<---");
        println!("{s}");
        println!("---8<--- /GITHUB_STEP_SUMMARY ---8<---");
        for (var, needle) in [("GITHUB_OUTPUT", "answer<<"), ("GITHUB_ENV", "DEMO_FLAG<<")] {
            if !read_env_file(var).is_some_and(|c| c.contains(needle)) {
                eprintln!("::error::{var} missing {needle:?}");
                exit = ExitCode::FAILURE;
            }
        }
        if !s.contains("<h2>actions-rs — emitted artifacts</h2>") {
            eprintln!("::error::GITHUB_STEP_SUMMARY missing heading");
            exit = ExitCode::FAILURE;
        }
    }
    exit
}
