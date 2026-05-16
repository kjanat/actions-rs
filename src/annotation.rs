//! Annotation builder for `notice` / `warning` / `error` commands.
//!
//! Annotations may carry a source location (file + line/column range) and a title;
//! GitHub renders them inline in the diff and in the run summary.\
//! The property names emitted here match `@actions/core`'s mapping: its public `startLine`/`startColumn`
//! become the wire properties `line`/`col`.

use crate::command::WorkflowCommand;

/// Which annotation channel to emit on.
///
/// # Examples
///
/// ```
/// use actions_rs::{Annotation, AnnotationKind};
/// let c = Annotation::new().command(AnnotationKind::Warning, "heads up");
/// assert_eq!(c.to_string(), "::warning::heads up");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    /// A neutral `::notice::` annotation.
    Notice,
    /// A `::warning::` annotation (does not fail the job).
    Warning,
    /// An `::error::` annotation (does not by itself fail the job;
    /// pair with a non-zero exit code or [`crate::log::set_failed`]).
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

/// A valid annotation span.
///
/// # Examples
///
/// ```
/// use actions_rs::{Annotation, AnnotationKind, AnnotationSpan};
/// let c = Annotation::new()
///     .span(AnnotationSpan::Column { line: 7, start: 3, end: Some(9) })
///     .command(AnnotationKind::Error, "bad token");
/// assert_eq!(c.to_string(), "::error line=7,col=3,endColumn=9::bad token");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationSpan {
    /// A whole-line span.
    Line {
        /// The 1-based start line.
        start: u32,
        /// The optional 1-based end line.
        end: Option<u32>,
    },
    /// A same-line column span.
    /// When `end` is omitted GitHub treats the span as a single column.
    Column {
        /// The 1-based line.
        line: u32,
        /// The 1-based start column.
        start: u32,
        /// The optional 1-based end column.
        end: Option<u32>,
    },
}

/// Fluent builder for a located annotation.
///
/// All fields are optional — an empty `Annotation` simply produces a plain annotation with no location.
/// Build it, then emit with [`Annotation::notice`], [`Annotation::warning`] or [`Annotation::error`].
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
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().command(AnnotationKind::Notice, "hi");
    /// assert_eq!(c.to_string(), "::notice::hi");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the annotation title shown in the GitHub UI.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().title("clippy").command(AnnotationKind::Warning, "w");
    /// assert_eq!(c.to_string(), "::warning title=clippy::w");
    /// ```
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the file path the annotation refers to (relative to the workspace).
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().file("src/lib.rs").command(AnnotationKind::Error, "e");
    /// assert_eq!(c.to_string(), "::error file=src/lib.rs::e");
    /// ```
    #[must_use]
    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set the (1-based) start line.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().file("x").line(42).command(AnnotationKind::Warning, "w");
    /// assert_eq!(c.to_string(), "::warning file=x,line=42::w");
    /// ```
    #[must_use]
    pub fn line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the (1-based) end line of a multi-line span.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().file("x").line(10).end_line(12)
    ///     .command(AnnotationKind::Warning, "w");
    /// assert_eq!(c.to_string(), "::warning file=x,line=10,endLine=12::w");
    /// ```
    #[must_use]
    pub fn end_line(mut self, end_line: u32) -> Self {
        self.end_line = Some(end_line);
        self
    }

    /// Set the (1-based) start column.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().file("x").line(7).col(3)
    ///     .command(AnnotationKind::Warning, "w");
    /// assert_eq!(c.to_string(), "::warning file=x,line=7,col=3,endColumn=3::w");
    /// ```
    #[must_use]
    pub fn col(mut self, col: u32) -> Self {
        self.col = Some(col);
        self
    }

    /// Set the (1-based) end column.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// let c = Annotation::new().file("x").line(7).col(3).end_column(9)
    ///     .command(AnnotationKind::Warning, "w");
    /// assert_eq!(c.to_string(), "::warning file=x,line=7,col=3,endColumn=9::w");
    /// ```
    #[must_use]
    pub fn end_column(mut self, end_column: u32) -> Self {
        self.end_column = Some(end_column);
        self
    }

    /// Replace the current location fields with a span that is valid by construction.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind, AnnotationSpan};
    /// let c = Annotation::new()
    ///     .span(AnnotationSpan::Line { start: 4, end: Some(6) })
    ///     .command(AnnotationKind::Notice, "block");
    /// assert_eq!(c.to_string(), "::notice line=4,endLine=6::block");
    /// ```
    #[must_use]
    pub fn span(mut self, span: AnnotationSpan) -> Self {
        match span {
            AnnotationSpan::Line { start, end } => {
                self.line = Some(start);
                self.end_line = end;
                self.col = None;
                self.end_column = None;
            }
            AnnotationSpan::Column { line, start, end } => {
                self.line = Some(line);
                self.end_line = None;
                self.col = Some(start);
                self.end_column = end;
            }
        }
        self
    }

    /// Build the [`WorkflowCommand`] for this annotation and `message` without emitting it.
    /// Useful for testing or custom sinks.
    ///
    /// Property order matches `@actions/core`: `title, file, line, endLine, col, endColumn`.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Annotation, AnnotationKind};
    /// // Inspect the wire form without writing to stdout.
    /// let cmd = Annotation::new().file("a.rs").line(1).command(AnnotationKind::Error, "e");
    /// assert_eq!(cmd.to_string(), "::error file=a.rs,line=1::e");
    /// ```
    #[must_use]
    pub fn command(&self, kind: AnnotationKind, message: impl Into<String>) -> WorkflowCommand {
        let line = self.line;
        let end_line = self.end_line.filter(|_| line.is_some());
        let same_line = match (line, end_line) {
            (Some(_), None) => true,
            (Some(start), Some(end)) => start == end,
            _ => false,
        };
        let col = if same_line { self.col } else { None };
        let end_column = if same_line {
            col.map(|start| self.end_column.unwrap_or(start))
        } else {
            None
        };

        WorkflowCommand::new(kind.command_name())
            .property_opt("title", self.title.clone())
            .property_opt("file", self.file.clone())
            .property_opt("line", line.map(|n| n.to_string()))
            .property_opt("endLine", end_line.map(|n| n.to_string()))
            .property_opt("col", col.map(|n| n.to_string()))
            .property_opt("endColumn", end_column.map(|n| n.to_string()))
            .message(message)
    }

    /// Emit a `::notice::` annotation to stdout.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::Annotation;
    /// Annotation::new().file("README.md").line(1).notice("looks good");
    /// ```
    pub fn notice(&self, message: impl Into<String>) {
        self.command(AnnotationKind::Notice, message).issue();
    }

    /// Emit a `::warning::` annotation to stdout.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::Annotation;
    /// Annotation::new().file("src/lib.rs").line(42).title("lint").warning("unused import");
    /// ```
    pub fn warning(&self, message: impl Into<String>) {
        self.command(AnnotationKind::Warning, message).issue();
    }

    /// Emit an `::error::` annotation to stdout.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::Annotation;
    /// Annotation::new().file("src/main.rs").line(7).error("type mismatch");
    /// ```
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
            "::notice title=t,file=f.rs,line=1,endLine=2::msg"
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

    #[test]
    fn multiline_range_drops_columns() {
        let c = Annotation::new()
            .file("x")
            .line(7)
            .end_line(8)
            .col(3)
            .end_column(9)
            .command(AnnotationKind::Warning, "w");
        assert_eq!(c.to_string(), "::warning file=x,line=7,endLine=8::w");
    }

    #[test]
    fn column_span_defaults_end_column() {
        let c = Annotation::new()
            .span(AnnotationSpan::Column {
                line: 7,
                start: 3,
                end: None,
            })
            .command(AnnotationKind::Warning, "w");
        assert_eq!(c.to_string(), "::warning line=7,col=3,endColumn=3::w");
    }
}
