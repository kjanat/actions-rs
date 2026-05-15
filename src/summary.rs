//! Fluent builder for the job summary (`GITHUB_STEP_SUMMARY`).
//!
//! The summary is GitHub-Flavored Markdown with embedded HTML. This builder
//! emits the same HTML fragments as `@actions/core`'s `summary` API. Buffer
//! construction is pure (testable via [`Summary::stringify`]); only
//! [`Summary::write`] / [`Summary::write_overwrite`] touch the filesystem and
//! can fail.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;

use crate::error::{Error, Result};

/// GitHub's documented per-step summary size limit (1 MiB).
const MAX_BYTES: usize = 1024 * 1024;

/// Escape text destined for HTML element content. Without this, content like
/// `DEMO_FLAG<<delim` is parsed by the browser as a bogus tag and truncated.
fn esc_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape text destined for a double-quoted HTML attribute value.
fn esc_attr(s: &str) -> String {
    esc_text(s).replace('"', "&quot;")
}

/// A table cell. Use [`Cell::header`] for `<th>`; `colspan`/`rowspan` map to
/// the matching HTML attributes.
#[derive(Debug, Clone)]
pub struct Cell {
    data: String,
    header: bool,
    colspan: u32,
    rowspan: u32,
}

impl Cell {
    /// A `<td>` cell.
    #[must_use]
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            header: false,
            colspan: 1,
            rowspan: 1,
        }
    }

    /// A `<th>` header cell.
    #[must_use]
    pub fn header(data: impl Into<String>) -> Self {
        Self {
            header: true,
            ..Self::new(data)
        }
    }

    /// Set the column span.
    #[must_use]
    pub fn colspan(mut self, n: u32) -> Self {
        self.colspan = n;
        self
    }

    /// Set the row span.
    #[must_use]
    pub fn rowspan(mut self, n: u32) -> Self {
        self.rowspan = n;
        self
    }
}

impl From<&str> for Cell {
    fn from(s: &str) -> Self {
        Cell::new(s)
    }
}

impl From<String> for Cell {
    fn from(s: String) -> Self {
        Cell::new(s)
    }
}

/// Accumulating job-summary builder. Chain `add_*` calls, then `write`.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    buf: String,
}

impl Summary {
    /// Create an empty summary buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append raw text. When `eol` is true a newline is appended after it.
    pub fn raw(&mut self, text: impl AsRef<str>, eol: bool) -> &mut Self {
        self.buf.push_str(text.as_ref());
        if eol {
            self.buf.push('\n');
        }
        self
    }

    /// Append a newline.
    pub fn eol(&mut self) -> &mut Self {
        self.buf.push('\n');
        self
    }

    /// Append an `<h1>`–`<h6>` heading (`level` clamped to 1..=6).
    pub fn heading(&mut self, text: impl AsRef<str>, level: u8) -> &mut Self {
        let l = level.clamp(1, 6);
        let _ = writeln!(self.buf, "<h{l}>{}</h{l}>", esc_text(text.as_ref()));
        self
    }

    /// Append a fenced `<pre><code>` block with an optional language hint.
    pub fn code_block(&mut self, code: impl AsRef<str>, lang: Option<&str>) -> &mut Self {
        let code = esc_text(code.as_ref());
        match lang {
            Some(l) => {
                let _ = writeln!(
                    self.buf,
                    "<pre lang=\"{}\"><code>{code}</code></pre>",
                    esc_attr(l)
                );
            }
            None => {
                let _ = writeln!(self.buf, "<pre><code>{code}</code></pre>");
            }
        }
        self
    }

    /// Append a `<ul>` (or `<ol>` when `ordered`) of `items`.
    pub fn list<I, S>(&mut self, items: I, ordered: bool) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let tag = if ordered { "ol" } else { "ul" };
        self.buf.push('<');
        self.buf.push_str(tag);
        self.buf.push('>');
        for item in items {
            let _ = write!(self.buf, "<li>{}</li>", esc_text(item.as_ref()));
        }
        let _ = writeln!(self.buf, "</{tag}>");
        self
    }

    /// Append a `<table>`. Each row is a list of [`Cell`]s.
    pub fn table(&mut self, rows: impl IntoIterator<Item = Vec<Cell>>) -> &mut Self {
        self.buf.push_str("<table>");
        for row in rows {
            self.buf.push_str("<tr>");
            for cell in row {
                let tag = if cell.header { "th" } else { "td" };
                let _ = write!(
                    self.buf,
                    "<{tag} colspan=\"{}\" rowspan=\"{}\">{}</{tag}>",
                    cell.colspan,
                    cell.rowspan,
                    esc_text(&cell.data)
                );
            }
            self.buf.push_str("</tr>");
        }
        self.buf.push_str("</table>\n");
        self
    }

    /// Append a `<details>` block with a `<summary>` label.
    pub fn details(&mut self, label: impl AsRef<str>, content: impl AsRef<str>) -> &mut Self {
        let _ = writeln!(
            self.buf,
            "<details><summary>{}</summary>{}</details>",
            esc_text(label.as_ref()),
            esc_text(content.as_ref())
        );
        self
    }

    /// Append an `<img>`. `size` is an optional `(width, height)` in pixels.
    pub fn image(
        &mut self,
        src: impl AsRef<str>,
        alt: impl AsRef<str>,
        size: Option<(u32, u32)>,
    ) -> &mut Self {
        self.buf.push_str("<img src=\"");
        self.buf.push_str(&esc_attr(src.as_ref()));
        self.buf.push_str("\" alt=\"");
        self.buf.push_str(&esc_attr(alt.as_ref()));
        self.buf.push('"');
        if let Some((w, h)) = size {
            let _ = write!(self.buf, " width=\"{w}\" height=\"{h}\"");
        }
        self.buf.push_str(">\n");
        self
    }

    /// Append an `<a>` link.
    pub fn link(&mut self, text: impl AsRef<str>, href: impl AsRef<str>) -> &mut Self {
        let _ = writeln!(
            self.buf,
            "<a href=\"{}\">{}</a>",
            esc_attr(href.as_ref()),
            esc_text(text.as_ref())
        );
        self
    }

    /// Append a `<blockquote>` with an optional `cite` URL.
    pub fn quote(&mut self, text: impl AsRef<str>, cite: Option<&str>) -> &mut Self {
        match cite {
            Some(c) => {
                let _ = writeln!(
                    self.buf,
                    "<blockquote cite=\"{}\">{}</blockquote>",
                    esc_attr(c),
                    esc_text(text.as_ref())
                );
            }
            None => {
                let _ = writeln!(
                    self.buf,
                    "<blockquote>{}</blockquote>",
                    esc_text(text.as_ref())
                );
            }
        }
        self
    }

    /// Append an `<hr>`.
    pub fn separator(&mut self) -> &mut Self {
        self.buf.push_str("<hr>\n");
        self
    }

    /// Append a `<br>`.
    pub fn break_(&mut self) -> &mut Self {
        self.buf.push_str("<br>\n");
        self
    }

    /// The buffered summary content.
    #[must_use]
    pub fn stringify(&self) -> &str {
        &self.buf
    }

    /// Whether nothing has been buffered yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Clear the buffer (does not touch the file).
    pub fn clear(&mut self) -> &mut Self {
        self.buf.clear();
        self
    }

    fn write_inner(&self, append: bool) -> Result<()> {
        if self.buf.len() > MAX_BYTES {
            return Err(Error::SummaryTooLarge {
                bytes: self.buf.len(),
            });
        }
        let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
            // Not in Actions / summaries disabled: nothing to write to. This
            // is a normal local-run condition, not an error.
            return Ok(());
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(append)
            .write(true)
            .truncate(!append)
            .open(path)?;
        file.write_all(self.buf.as_bytes())?;
        Ok(())
    }

    /// Append the buffer to the job summary file.
    ///
    /// # Errors
    /// [`Error::SummaryTooLarge`] if the buffer exceeds 1 MiB, or an I/O error.
    pub fn write(&self) -> Result<()> {
        self.write_inner(true)
    }

    /// Overwrite the job summary file with the buffer.
    ///
    /// # Errors
    /// [`Error::SummaryTooLarge`] if the buffer exceeds 1 MiB, or an I/O error.
    pub fn write_overwrite(&self) -> Result<()> {
        self.write_inner(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_clamps_level() {
        let mut s = Summary::new();
        s.heading("Top", 9);
        assert_eq!(s.stringify(), "<h6>Top</h6>\n");
    }

    #[test]
    fn html_metachars_are_escaped() {
        let mut s = Summary::new();
        // The exact bug: `DEMO_FLAG<<delim` was eaten by the HTML parser.
        s.code_block("DEMO_FLAG<<d & a>b", None);
        assert_eq!(
            s.stringify(),
            "<pre><code>DEMO_FLAG&lt;&lt;d &amp; a&gt;b</code></pre>\n"
        );

        let mut h = Summary::new();
        h.heading("a < b & c", 2);
        assert_eq!(h.stringify(), "<h2>a &lt; b &amp; c</h2>\n");

        // Attribute values also escape the double quote.
        let mut l = Summary::new();
        l.link("x", "https://e.com/?a=1\"&b=2");
        assert_eq!(
            l.stringify(),
            "<a href=\"https://e.com/?a=1&quot;&amp;b=2\">x</a>\n"
        );

        // raw() stays raw by contract.
        let mut r = Summary::new();
        r.raw("<b>kept</b>", false);
        assert_eq!(r.stringify(), "<b>kept</b>");
    }

    #[test]
    fn chaining_builds_expected_html() {
        let mut s = Summary::new();
        s.heading("Report", 2)
            .list(["a", "b"], false)
            .code_block("cargo test", Some("sh"))
            .separator();
        assert_eq!(
            s.stringify(),
            "<h2>Report</h2>\n<ul><li>a</li><li>b</li></ul>\n\
             <pre lang=\"sh\"><code>cargo test</code></pre>\n<hr>\n"
        );
    }

    #[test]
    fn table_with_header_and_spans() {
        let mut s = Summary::new();
        s.table([
            vec![Cell::header("H1"), Cell::header("H2")],
            vec![Cell::new("a").colspan(2)],
        ]);
        assert_eq!(
            s.stringify(),
            "<table><tr><th colspan=\"1\" rowspan=\"1\">H1</th>\
             <th colspan=\"1\" rowspan=\"1\">H2</th></tr>\
             <tr><td colspan=\"2\" rowspan=\"1\">a</td></tr></table>\n"
        );
    }

    #[test]
    fn oversized_buffer_rejected() {
        let mut s = Summary::new();
        s.raw("x".repeat(MAX_BYTES + 1), false);
        let e = s.write_overwrite().unwrap_err();
        assert!(matches!(e, Error::SummaryTooLarge { bytes } if bytes == MAX_BYTES + 1));
    }

    #[test]
    fn empty_and_clear() {
        let mut s = Summary::new();
        assert!(s.is_empty());
        s.raw("hi", true);
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }
}
