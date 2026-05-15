//! Annotation builder for `notice` / `warning` / `error` commands.
//!
//! Annotations may carry a source location (file + line/column range) and a
//! title; GitHub renders them inline in the diff and in the run summary. The
//! property names emitted here match `@actions/core`'s mapping: its public
//! `startLine`/`startColumn` become the wire properties `line`/`col`.

use crate::command::WorkflowCommand;

/// Which annotation channel to emit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    /// A neutral `::notice::` annotation.
    Notice,
    /// A `::warning::` annotation (does not fail the job).
    Warning,
    /// An `::error::` annotation (does not by itself fail the job; pair with a
    /// non-zero exit code or [`crate::log::set_failed`]).
    Error,
}

impl AnnotationKind {
    const fn command_name(self) -> &'static str {
        match self {
            AnnotationKind::Notice => "notice",
            AnnotationKind::Warning => "warning",
            AnnotationKind::Error => "error",
        }
    }
}

/// Fluent builder for a located annotation.
///
/// All fields are optional — an empty `Annotation` simply produces a plain
/// annotation with no location. Build it, then emit with [`Annotation::notice`],
/// [`Annotation::warning`] or [`Annotation::error`].
///
/// ```
/// use actions_rs::Annotation;
/// let cmd = Annotation::new()
///     .file("src/lib.rs")
///     .line(10)
///     .end_line(12)
///     .title("clippy")
///     .command(actions_rs::AnnotationKind::Warning, "unused variable");
/// assert_eq!(
///     cmd.to_string(),
///     "::warning title=clippy,file=src/lib.rs,line=10,endLine=12::unused variable"
/// );
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Annotation {
    title: Option<String>,
    file: Option<String>,
    line: Option<u32>,
    end_line: Option<u32>,
    col: Option<u32>,
    end_column: Option<u32>,
}

impl Annotation {
    /// Create an empty annotation (no location, no title).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the annotation title shown in the GitHub UI.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the file path the annotation refers to (relative to the workspace).
    #[must_use]
    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set the (1-based) start line.
    #[must_use]
    pub fn line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the (1-based) end line of a multi-line span.
    #[must_use]
    pub fn end_line(mut self, end_line: u32) -> Self {
        self.end_line = Some(end_line);
        self
    }

    /// Set the (1-based) start column.
    #[must_use]
    pub fn col(mut self, col: u32) -> Self {
        self.col = Some(col);
        self
    }

    /// Set the (1-based) end column.
    #[must_use]
    pub fn end_column(mut self, end_column: u32) -> Self {
        self.end_column = Some(end_column);
        self
    }

    /// Build the [`WorkflowCommand`] for this annotation and `message` without
    /// emitting it. Useful for testing or custom sinks.
    ///
    /// Property order matches `@actions/core`:
    /// `title, file, line, endLine, col, endColumn`.
    #[must_use]
    pub fn command(&self, kind: AnnotationKind, message: impl Into<String>) -> WorkflowCommand {
        WorkflowCommand::new(kind.command_name())
            .property_opt("title", self.title.clone())
            .property_opt("file", self.file.clone())
            .property_opt("line", self.line.map(|n| n.to_string()))
            .property_opt("endLine", self.end_line.map(|n| n.to_string()))
            .property_opt("col", self.col.map(|n| n.to_string()))
            .property_opt("endColumn", self.end_column.map(|n| n.to_string()))
            .message(message)
    }

    /// Emit a `::notice::` annotation to stdout.
    pub fn notice(&self, message: impl Into<String>) {
        self.command(AnnotationKind::Notice, message).issue();
    }

    /// Emit a `::warning::` annotation to stdout.
    pub fn warning(&self, message: impl Into<String>) {
        self.command(AnnotationKind::Warning, message).issue();
    }

    /// Emit an `::error::` annotation to stdout.
    pub fn error(&self, message: impl Into<String>) {
        self.command(AnnotationKind::Error, message).issue();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_annotation_is_plain() {
        let c = Annotation::new().command(AnnotationKind::Error, "boom");
        assert_eq!(c.to_string(), "::error::boom");
    }

    #[test]
    fn full_property_order() {
        let c = Annotation::new()
            .title("t")
            .file("f.rs")
            .line(1)
            .end_line(2)
            .col(3)
            .end_column(4)
            .command(AnnotationKind::Notice, "msg");
        assert_eq!(
            c.to_string(),
            "::notice title=t,file=f.rs,line=1,endLine=2,col=3,endColumn=4::msg"
        );
    }

    #[test]
    fn partial_skips_unset() {
        let c = Annotation::new()
            .file("x")
            .line(7)
            .command(AnnotationKind::Warning, "w");
        assert_eq!(c.to_string(), "::warning file=x,line=7::w");
    }
}
