//! Writer-aware scope contracts, including streaming, unwinding and I/O failures.

use std::cell::Cell;
use std::io::{self, Write};
use std::rc::Rc;

use actions_rs::log::{group_guard_to, group_to, stop_commands_to};

fn stop_token(output: &[u8]) -> &str {
    let opening = output.split(|&byte| byte == b'\n').next().unwrap();
    let token = std::str::from_utf8(opening)
        .unwrap()
        .strip_prefix("::stop-commands::")
        .unwrap();
    let suffix = token.strip_prefix("stopcommands_").unwrap();
    assert_eq!(suffix.len(), 16);
    assert!(suffix.bytes().all(|byte| byte.is_ascii_hexdigit()));
    token
}

#[test]
fn custom_writer_isolation() {
    // Run this test in a child so accidental stdout/stderr markers cannot hide
    // behind the test harness's normal output capture.
    const CHILD: &str = "ACTIONS_RS_WRITER_ISOLATION_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mut output = Vec::new();
        group_to("isolated", &mut output, |group| {
            let mut stopped = stop_commands_to(group)?;
            stopped.write_all(b"::error::literal replay\n")?;
            stopped.finish()
        })
        .unwrap();
        let prefix = b"::group::isolated\n";
        assert!(output.starts_with(prefix));
        let token = stop_token(&output[prefix.len()..]);
        assert_eq!(
            output,
            format!(
                "::group::isolated\n::stop-commands::{token}\n\
                 ::error::literal replay\n::{token}::\n::endgroup::\n"
            )
            .as_bytes()
        );
        return;
    }

    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "custom_writer_isolation", "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(child.status.success(), "{child:?}");
    assert!(child.stderr.is_empty(), "{child:?}");
    let stdout = String::from_utf8(child.stdout).unwrap();
    assert!(!stdout.contains("::"), "{stdout}");
    assert!(!stdout.contains("literal replay"), "{stdout}");
}

#[test]
fn group_escapes_name_and_returns_value() {
    let mut output = Vec::new();
    let answer = group_to("build%\r\nnext", &mut output, |group| {
        group.write_all(b"working\n")?;
        Ok(42)
    })
    .unwrap();
    assert_eq!(answer, 42);
    assert_eq!(
        output,
        b"::group::build%25%0D%0Anext\nworking\n::endgroup::\n"
    );
}

#[test]
fn stop_streams_binary_bytes_and_uses_unique_matching_tokens() {
    let payload = b"::error::spoofed\n::endgroup::\r\n100%\0\xff\n";
    let mut tokens = Vec::new();
    for _ in 0..2 {
        let mut output = Vec::new();
        let mut stopped = stop_commands_to(&mut output).unwrap();
        io::copy(&mut &payload[..], &mut stopped).unwrap();
        stopped.finish().unwrap();
        let token = stop_token(&output).to_owned();
        let mut expected = format!("::stop-commands::{token}\n").into_bytes();
        expected.extend_from_slice(payload);
        expected.extend_from_slice(format!("::{token}::\n").as_bytes());
        assert_eq!(output, expected);
        tokens.push(token);
    }
    assert_ne!(tokens[0], tokens[1]);
}

#[test]
fn closing_markers_have_line_boundaries() {
    for payload in [b"".as_slice(), b"tail", b"tail\n", b"tail\r"] {
        let mut output = Vec::new();
        let mut group = group_guard_to("replay", &mut output).unwrap();
        let mut stopped = stop_commands_to(&mut group).unwrap();
        stopped.write_all(payload).unwrap();
        // An empty write must not forget a previously incomplete line.
        assert_eq!(stopped.write(b"").unwrap(), 0);
        stopped.finish().unwrap();
        group.finish().unwrap();

        let prefix = b"::group::replay\n";
        let token = stop_token(&output[prefix.len()..]);
        let mut expected = prefix.to_vec();
        expected.extend_from_slice(format!("::stop-commands::{token}\n").as_bytes());
        expected.extend_from_slice(payload);
        if !payload.is_empty() && !payload.ends_with(b"\n") {
            expected.push(b'\n');
        }
        expected.extend_from_slice(format!("::{token}::\n::endgroup::\n").as_bytes());
        assert_eq!(output, expected);
    }

    let mut output = Vec::new();
    let mut group = group_guard_to("build", &mut output).unwrap();
    group.write_all(b"tail").unwrap();
    group.finish().unwrap();
    assert_eq!(output, b"::group::build\ntail\n::endgroup::\n");
}

fn early_return(output: &mut Vec<u8>) -> io::Result<()> {
    let mut group = group_guard_to("early", output)?;
    let mut stopped = stop_commands_to(&mut group)?;
    stopped.write_all(b"::warning::unfinished")?;
    Err(io::Error::other("body failed"))
}

#[test]
fn nested_scopes_cleanup_on_early_return_and_panic() {
    for panic in [false, true] {
        let mut output = Vec::new();
        if panic {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                group_to("early", &mut output, |group| -> io::Result<()> {
                    let mut stopped = stop_commands_to(group)?;
                    stopped.write_all(b"::warning::unfinished")?;
                    panic!("body panicked");
                })
                .unwrap();
            }));
            assert!(result.is_err());
        } else {
            assert_eq!(
                early_return(&mut output).unwrap_err().to_string(),
                "body failed"
            );
        }
        let prefix = b"::group::early\n";
        let token = stop_token(&output[prefix.len()..]);
        assert_eq!(
            output,
            format!(
                "::group::early\n::stop-commands::{token}\n\
                 ::warning::unfinished\n::{token}::\n::endgroup::\n"
            )
            .as_bytes()
        );
    }
}

#[test]
fn group_closes_when_body_returns_error() {
    let mut output = Vec::new();
    let error = group_to("build", &mut output, |group| -> io::Result<()> {
        group.write_all(b"tail")?;
        Err(io::Error::other("body failed"))
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "body failed");
    assert_eq!(output, b"::group::build\ntail\n::endgroup::\n");
}

struct TestWriter {
    output: Vec<u8>,
    budget: Rc<Cell<usize>>,
    writes: Rc<Cell<usize>>,
    chunk: usize,
    flushes: usize,
    fail_flush: bool,
}

impl TestWriter {
    fn new() -> Self {
        Self {
            output: Vec::new(),
            budget: Rc::new(Cell::new(usize::MAX)),
            writes: Rc::new(Cell::new(0)),
            chunk: usize::MAX,
            flushes: 0,
            fail_flush: false,
        }
    }
}

impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes.set(self.writes.get() + 1);
        if self.budget.get() == 0 {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"));
        }
        let written = buf.len().min(self.chunk).min(self.budget.get());
        self.output.extend_from_slice(&buf[..written]);
        self.budget.set(self.budget.get() - written);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush {
            Err(io::Error::other("flush failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn accepts_borrowed_trait_object_and_short_writes() {
    let mut writer = TestWriter::new();
    writer.chunk = 1;
    let destination: &mut dyn Write = &mut writer;
    let mut group = group_guard_to("short", destination).unwrap();
    let mut stopped = stop_commands_to(&mut group).unwrap();
    stopped.write_all(b"first\nlast\xff").unwrap();
    stopped.finish().unwrap();
    group.finish().unwrap();
    let prefix = b"::group::short\n";
    let token = stop_token(&writer.output[prefix.len()..]);
    let mut expected = format!("::group::short\n::stop-commands::{token}\n").into_bytes();
    expected.extend_from_slice(b"first\nlast\xff\n");
    expected.extend_from_slice(format!("::{token}::\n::endgroup::\n").as_bytes());
    assert_eq!(writer.output, expected);
}

#[test]
fn partial_body_failure_still_separates_cleanup_marker() {
    let mut writer = TestWriter::new();
    let budget = writer.budget.clone();
    let mut stopped = stop_commands_to(&mut writer).unwrap();
    budget.set(2);
    assert_eq!(
        stopped.write_all(b"abc\n").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    budget.set(usize::MAX);
    drop(stopped);
    let token = stop_token(&writer.output);
    assert_eq!(
        writer.output,
        format!("::stop-commands::{token}\nab\n::{token}::\n").as_bytes()
    );
}

#[test]
fn opening_errors_propagate_without_calling_body_or_cleanup() {
    for limit in [0, 4] {
        let mut writer = TestWriter::new();
        writer.budget.set(limit);
        let result = group_to("build", &mut writer, |_| -> io::Result<()> {
            panic!("body must not run after an opening failure")
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(writer.output, &b"::group::build\n"[..limit]);

        let mut writer = TestWriter::new();
        writer.budget.set(limit);
        let error = stop_commands_to(&mut writer).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(writer.output, &b"::stop-commands::"[..limit]);
    }
}

#[test]
fn finish_errors_propagate_without_retrying_partial_markers() {
    for stop in [false, true] {
        for limit in [0, 4] {
            let mut writer = TestWriter::new();
            let budget = writer.budget.clone();
            let writes = writer.writes.clone();
            let finish = if stop {
                let stopped = stop_commands_to(&mut writer).unwrap();
                budget.set(limit);
                let before = writes.get();
                let error = stopped.finish().unwrap_err();
                (error, writes.get() - before)
            } else {
                let group = group_guard_to("build", &mut writer).unwrap();
                budget.set(limit);
                let before = writes.get();
                let error = group.finish().unwrap_err();
                (error, writes.get() - before)
            };
            assert_eq!(finish.0.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(finish.1, if limit == 0 { 1 } else { 2 });
        }
    }
}

#[test]
fn separator_errors_propagate_and_drop_ignores_io_errors() {
    for explicit in [false, true] {
        let mut writer = TestWriter::new();
        let budget = writer.budget.clone();
        let mut stopped = stop_commands_to(&mut writer).unwrap();
        stopped.write_all(b"tail").unwrap();
        budget.set(0);
        if explicit {
            assert_eq!(
                stopped.finish().unwrap_err().kind(),
                io::ErrorKind::BrokenPipe
            );
        } else {
            drop(stopped);
        }
        let token = stop_token(&writer.output);
        assert_eq!(
            writer.output,
            format!("::stop-commands::{token}\ntail").as_bytes()
        );
    }
}

#[test]
fn group_to_reports_closing_failure_and_preserves_body_error() {
    for body_fails in [false, true] {
        let mut writer = TestWriter::new();
        let budget = writer.budget.clone();
        let result = group_to("build", &mut writer, |_| {
            budget.set(0);
            if body_fails {
                Err(io::Error::other("body failed"))
            } else {
                Ok(())
            }
        });
        let error = result.unwrap_err();
        if body_fails {
            assert_eq!(error.to_string(), "body failed");
        } else {
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        }
        assert_eq!(writer.output, b"::group::build\n");
    }
}

#[test]
fn flushing_is_explicit_and_flush_errors_propagate() {
    let mut writer = TestWriter::new();
    let mut group = group_guard_to("buffered", &mut writer).unwrap();
    let mut stopped = stop_commands_to(&mut group).unwrap();
    stopped.write_all(b"live\n").unwrap();
    stopped.flush().unwrap();
    stopped.finish().unwrap();
    group.finish().unwrap();
    assert_eq!(writer.flushes, 1);
    writer.flush().unwrap();
    assert_eq!(writer.flushes, 2);

    writer.fail_flush = true;
    let mut stopped = stop_commands_to(&mut writer).unwrap();
    assert_eq!(stopped.flush().unwrap_err().to_string(), "flush failed");
    stopped.finish().unwrap();
}
