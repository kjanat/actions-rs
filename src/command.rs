//! Low-level workflow-command construction and emission.
//!
//! Most users want the ergonomic helpers in [`crate::log`] /
//! [`crate::annotation`]. This module exposes the underlying
//! [`WorkflowCommand`] for power users who need to emit a command the
//! higher-level API does not cover.

use std::fmt;
use std::io::{self, Write};

use crate::escape::{escape_data, escape_property};

/// A single GitHub Actions workflow command: `::name key=val,...::message`.
///
/// Properties are kept in insertion order to produce deterministic output
/// (which the test-suite and `@actions/core` both rely on). The [`Display`]
/// implementation performs all required percent-encoding, so the rendered
/// string is always safe to write to stdout.
///
/// [`Display`]: std::fmt::Display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCommand {
    name: &'static str,
    properties: Vec<(&'static str, String)>,
    message: String,
}

impl WorkflowCommand {
    /// Create a command with the given name and an empty message.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            properties: Vec::new(),
            message: String::new(),
        }
    }

    /// Set the command message (the segment after the final `::`).
    #[must_use]
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Append a property. The value is percent-encoded on render.
    #[must_use]
    pub fn property(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.properties.push((key, value.into()));
        self
    }

    /// Append a property only when `value` is `Some`.
    #[must_use]
    pub fn property_opt(self, key: &'static str, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(v) => self.property(key, v),
            None => self,
        }
    }

    /// Render and write this command followed by a newline to `w`.
    ///
    /// Used by the test-suite to capture output; the convenience helpers use
    /// [`WorkflowCommand::issue`].
    ///
    /// # Errors
    /// Propagates any write error from `w`.
    pub fn issue_to<W: Write>(&self, mut w: W) -> io::Result<()> {
        writeln!(w, "{self}")
    }

    /// Render and write this command to stdout followed by a newline.
    ///
    /// Stdout is the runner's command channel; a failed write here cannot be
    /// meaningfully recovered from inside an action, so the result is dropped
    /// deliberately (matching `@actions/core` behaviour).
    pub fn issue(&self) {
        let _ = self.issue_to(io::stdout().lock());
    }
}

impl fmt::Display for WorkflowCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "::{}", self.name)?;
        for (i, (key, value)) in self.properties.iter().enumerate() {
            let sep = if i == 0 { ' ' } else { ',' };
            write!(f, "{sep}{key}={}", escape_property(value))?;
        }
        write!(f, "::{}", escape_data(&self.message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_properties() {
        let c = WorkflowCommand::new("warning").message("hello");
        assert_eq!(c.to_string(), "::warning::hello");
    }

    #[test]
    fn bare_command() {
        assert_eq!(WorkflowCommand::new("endgroup").to_string(), "::endgroup::");
    }

    #[test]
    fn properties_are_ordered_and_escaped() {
        let c = WorkflowCommand::new("error")
            .property("title", "Type: bad")
            .property("file", "a,b.rs")
            .message("oops\nsecond");
        assert_eq!(
            c.to_string(),
            "::error title=Type%3A bad,file=a%2Cb.rs::oops%0Asecond"
        );
    }

    #[test]
    fn property_opt_skips_none() {
        let c = WorkflowCommand::new("notice")
            .property_opt("file", Option::<String>::None)
            .property_opt("line", Some("10"))
            .message("m");
        assert_eq!(c.to_string(), "::notice line=10::m");
    }

    #[test]
    fn issue_to_appends_newline() {
        let mut buf = Vec::new();
        WorkflowCommand::new("debug")
            .message("d")
            .issue_to(&mut buf)
            .unwrap();
        assert_eq!(buf, b"::debug::d\n");
    }
}
