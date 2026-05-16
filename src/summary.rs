//! Fluent builder for the job summary (`GITHUB_STEP_SUMMARY`).
//!
//! The summary is GitHub-Flavored Markdown with embedded HTML.\
//! Buffer construction is pure (testable via [`Summary::stringify`]);
//! only [`Summary::write`] / [`Summary::write_overwrite`] touch the filesystem and can fail.
//!
//! Text node content is escaped by default.\
//! Use [`SummaryText::html`] or [`Summary::raw`] when you intentionally want raw HTML parity with `@actions/core`.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::Write as _;

use crate::error::{Error, Result};

/// GitHub's documented per-step summary size limit (1 MiB).
const MAX_BYTES: usize = 1024 * 1024;

/// Escape text destined for HTML element content.
/// Without this, content like `DEMO_FLAG<<delim` is parsed by the browser as a bogus tag and truncated.
fn esc_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape text destined for a double-quoted HTML attribute value.
fn esc_attr(s: &str) -> String {
    esc_text(s).replace('"', "&quot;")
}

/// Text destined for a summary element.
///
/// # Examples
///
/// ```
/// use actions_rs::{Summary, SummaryText};
///
/// let mut s = Summary::new();
/// s.heading(SummaryText::escaped("a<b"), 2)         // escaped
///  .heading(SummaryText::html("<em>raw</em>"), 3);  // verbatim
/// assert_eq!(s.stringify(), "<h2>a&lt;b</h2>\n<h3><em>raw</em></h3>\n");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryText {
    /// Escape HTML metacharacters before rendering.
    Escaped(String),
    /// Insert trusted HTML verbatim.
    Html(String),
}

impl SummaryText {
    /// Escape `text` before rendering it into an element body.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Summary, SummaryText};
    /// let mut s = Summary::new();
    /// s.heading(SummaryText::escaped("1 < 2 & ok"), 1);
    /// assert_eq!(s.stringify(), "<h1>1 &lt; 2 &amp; ok</h1>\n");
    /// ```
    #[must_use]
    pub fn escaped(text: impl Into<String>) -> Self {
        Self::Escaped(text.into())
    }

    /// Insert trusted HTML verbatim into an element body.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Summary, SummaryText};
    /// let mut s = Summary::new();
    /// s.heading(SummaryText::html("<strong>done</strong>"), 2);
    /// assert_eq!(s.stringify(), "<h2><strong>done</strong></h2>\n");
    /// ```
    #[must_use]
    pub fn html(html: impl Into<String>) -> Self {
        Self::Html(html.into())
    }

    fn into_html(self) -> String {
        match self {
            SummaryText::Escaped(text) => esc_text(&text),
            SummaryText::Html(html) => html,
        }
    }
}

impl From<&str> for SummaryText {
    fn from(text: &str) -> Self {
        SummaryText::escaped(text)
    }
}

impl From<&String> for SummaryText {
    fn from(text: &String) -> Self {
        SummaryText::escaped(text.clone())
    }
}

impl From<String> for SummaryText {
    fn from(text: String) -> Self {
        SummaryText::escaped(text)
    }
}

/// A table cell.
/// Use [`Cell::header`] for `<th>`; `colspan`/`rowspan` map to the matching HTML attributes.
/// Cell content is escaped unless you pass [`SummaryText::html`].
///
/// # Examples
///
/// ```
/// use actions_rs::{Cell, Summary};
/// let mut s = Summary::new();
/// s.table([vec![Cell::header("Name"), Cell::new("value")]]);
/// assert!(s.stringify().contains(r#"<th colspan="1" rowspan="1">Name</th>"#));
/// ```
#[derive(Debug, Clone)]
pub struct Cell {
    data: SummaryText,
    header: bool,
    colspan: u32,
    rowspan: u32,
}

impl Cell {
    /// A `<td>` cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Cell, Summary};
    /// let mut s = Summary::new();
    /// s.table([vec![Cell::new("x")]]);
    /// assert_eq!(s.stringify(), "<table><tr><td colspan=\"1\" rowspan=\"1\">x</td></tr></table>\n");
    /// ```
    #[must_use]
    pub fn new(data: impl Into<SummaryText>) -> Self {
        Self {
            data: data.into(),
            header: false,
            colspan: 1,
            rowspan: 1,
        }
    }

    /// A `<th>` header cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Cell, Summary};
    /// let mut s = Summary::new();
    /// s.table([vec![Cell::header("Col")]]);
    /// assert_eq!(s.stringify(), "<table><tr><th colspan=\"1\" rowspan=\"1\">Col</th></tr></table>\n");
    /// ```
    #[must_use]
    pub fn header(data: impl Into<SummaryText>) -> Self {
        Self {
            header: true,
            ..Self::new(data)
        }
    }

    /// Set the column span (clamped to ≥ 1; the HTML spec forbids 0).
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Cell, Summary};
    /// let mut s = Summary::new();
    /// s.table([vec![Cell::new("wide").colspan(0)]]); // 0 clamps to 1
    /// assert!(s.stringify().contains(r#"colspan="1""#));
    /// ```
    #[must_use]
    pub fn colspan(mut self, n: u32) -> Self {
        self.colspan = n.max(1);
        self
    }

    /// Set the row span (clamped to ≥ 1; the HTML spec forbids 0).
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Cell, Summary};
    /// let mut s = Summary::new();
    /// s.table([vec![Cell::new("tall").rowspan(3)]]);
    /// assert!(s.stringify().contains(r#"rowspan="3""#));
    /// ```
    #[must_use]
    pub fn rowspan(mut self, n: u32) -> Self {
        self.rowspan = n.max(1);
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

impl From<SummaryText> for Cell {
    fn from(text: SummaryText) -> Self {
        Cell::new(text)
    }
}

/// Accumulating job-summary builder.
/// Chain the builder methods, then [`write`](Summary::write) (append) or [`write_overwrite`](Summary::write_overwrite).
/// Building is pure and inspectable via [`stringify`](Summary::stringify);
/// only the `write*` methods touch `GITHUB_STEP_SUMMARY`.
///
/// # Examples
///
/// ```
/// use actions_rs::Summary;
///
/// let mut s = Summary::new();
/// s.heading("Build", 2)
///     .code_block("cargo test", Some("sh"));
///
/// assert_eq!(
///     s.stringify(),
///     "<h2>Build</h2>\n<pre lang=\"sh\"><code>cargo test</code></pre>\n"
/// );
///
/// // In a real action you would then persist it:
/// // s.write()?;  // appends to $GITHUB_STEP_SUMMARY
/// ```
#[derive(Debug, Clone, Default)]
pub struct Summary {
    buf: String,
}

impl Summary {
    /// Create an empty summary buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// let s = actions_rs::Summary::new();
    /// assert!(s.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append raw text **without HTML escaping**.
    /// When `eol` is true a newline is appended after it.
    ///
    /// # Safety
    /// This is the one builder method that does not escape `& < > "`.
    /// Passing untrusted input here is an HTML-injection vector.
    /// Use it only for trusted or already-escaped markup;
    /// for arbitrary text prefer [`Summary::code_block`] / [`Summary::heading`] etc., which escape.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.raw("<div class=\"trusted\">ok</div>", true);
    /// assert_eq!(s.stringify(), "<div class=\"trusted\">ok</div>\n");
    /// ```
    pub fn raw(&mut self, text: impl AsRef<str>, eol: bool) -> &mut Self {
        self.buf.push_str(text.as_ref());
        if eol {
            self.buf.push('\n');
        }
        self
    }

    /// Append a newline.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.raw("a", false).eol().raw("b", false);
    /// assert_eq!(s.stringify(), "a\nb");
    /// ```
    pub fn eol(&mut self) -> &mut Self {
        self.buf.push('\n');
        self
    }

    /// Append an `<h1>`–`<h6>` heading (`level` clamped to `1..=6`).
    /// Text is escaped unless you pass [`SummaryText::html`].
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.heading("Result", 9); // level clamps to 6
    /// assert_eq!(s.stringify(), "<h6>Result</h6>\n");
    /// ```
    pub fn heading(&mut self, text: impl Into<SummaryText>, level: u8) -> &mut Self {
        let l = level.clamp(1, 6);
        let text = text.into().into_html();
        let _ = writeln!(self.buf, "<h{l}>{text}</h{l}>");
        self
    }

    /// Append a fenced `<pre><code>` block with an optional language hint.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.code_block("cargo test", Some("sh"));
    /// assert_eq!(s.stringify(), "<pre lang=\"sh\"><code>cargo test</code></pre>\n");
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.list(["a", "b"], false);
    /// assert_eq!(s.stringify(), "<ul><li>a</li><li>b</li></ul>\n");
    /// ```
    pub fn list<I, S>(&mut self, items: I, ordered: bool) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SummaryText>,
    {
        let tag = if ordered { "ol" } else { "ul" };
        self.buf.push('<');
        self.buf.push_str(tag);
        self.buf.push('>');
        for item in items {
            let _ = write!(self.buf, "<li>{}</li>", item.into().into_html());
        }
        let _ = writeln!(self.buf, "</{tag}>");
        self
    }

    /// Append a `<table>`.
    /// Each row is a list of [`Cell`]s.
    ///
    /// # Examples
    ///
    /// ```
    /// use actions_rs::{Cell, Summary};
    /// let mut s = Summary::new();
    /// s.table([vec![Cell::header("k"), Cell::new("v")]]);
    /// assert_eq!(
    ///     s.stringify(),
    ///     "<table><tr><th colspan=\"1\" rowspan=\"1\">k</th>\
    ///      <td colspan=\"1\" rowspan=\"1\">v</td></tr></table>\n"
    /// );
    /// ```
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
                    cell.data.into_html()
                );
            }
            self.buf.push_str("</tr>");
        }
        self.buf.push_str("</table>\n");
        self
    }

    /// Append a `<details>` block with a `<summary>` label.
    /// Both text nodes are escaped unless you pass [`SummaryText::html`].
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.details("logs", "all green");
    /// assert_eq!(s.stringify(), "<details><summary>logs</summary>all green</details>\n");
    /// ```
    pub fn details(
        &mut self,
        label: impl Into<SummaryText>,
        content: impl Into<SummaryText>,
    ) -> &mut Self {
        let label = label.into().into_html();
        let content = content.into().into_html();
        let _ = writeln!(
            self.buf,
            "<details><summary>{}</summary>{}</details>",
            label, content
        );
        self
    }

    /// Append an `<img>`.
    /// `size` is an optional `(width, height)` in pixels.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.image("cov.svg", "coverage", Some((120, 20)));
    /// assert_eq!(
    ///     s.stringify(),
    ///     "<img src=\"cov.svg\" alt=\"coverage\" width=\"120\" height=\"20\">\n"
    /// );
    /// ```
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
    /// The link text is escaped unless you pass [`SummaryText::html`];
    /// `href` is always attribute-escaped.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.link("run", "https://example.com/run/1");
    /// assert_eq!(s.stringify(), "<a href=\"https://example.com/run/1\">run</a>\n");
    /// ```
    pub fn link(&mut self, text: impl Into<SummaryText>, href: impl AsRef<str>) -> &mut Self {
        let text = text.into().into_html();
        let _ = writeln!(
            self.buf,
            "<a href=\"{}\">{}</a>",
            esc_attr(href.as_ref()),
            text
        );
        self
    }

    /// Append a `<blockquote>` with an optional `cite` URL.
    /// Quote text is escaped unless you pass [`SummaryText::html`].
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.quote("ship it", None);
    /// assert_eq!(s.stringify(), "<blockquote>ship it</blockquote>\n");
    /// ```
    pub fn quote(&mut self, text: impl Into<SummaryText>, cite: Option<&str>) -> &mut Self {
        let text = text.into().into_html();
        match cite {
            Some(c) => {
                let _ = writeln!(
                    self.buf,
                    "<blockquote cite=\"{}\">{}</blockquote>",
                    esc_attr(c),
                    text
                );
            }
            None => {
                let _ = writeln!(self.buf, "<blockquote>{text}</blockquote>");
            }
        }
        self
    }

    /// Append an `<hr>`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.separator();
    /// assert_eq!(s.stringify(), "<hr>\n");
    /// ```
    pub fn separator(&mut self) -> &mut Self {
        self.buf.push_str("<hr>\n");
        self
    }

    /// Append a `<br>`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.break_();
    /// assert_eq!(s.stringify(), "<br>\n");
    /// ```
    pub fn break_(&mut self) -> &mut Self {
        self.buf.push_str("<br>\n");
        self
    }

    /// The buffered summary content.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.heading("Hi", 1);
    /// assert_eq!(s.stringify(), "<h1>Hi</h1>\n");
    /// ```
    #[must_use]
    pub fn stringify(&self) -> &str {
        &self.buf
    }

    /// Whether nothing has been buffered yet.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// assert!(s.is_empty());
    /// s.eol();
    /// assert!(!s.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Clear the buffer (does not touch the file).
    ///
    /// # Examples
    ///
    /// ```
    /// let mut s = actions_rs::Summary::new();
    /// s.heading("x", 1);
    /// s.clear();
    /// assert!(s.is_empty());
    /// ```
    pub fn clear(&mut self) -> &mut Self {
        self.buf.clear();
        self
    }

    fn write_inner(&mut self, append: bool) -> Result<()> {
        let write_bytes = self.buf.len() as u64;
        if write_bytes > MAX_BYTES as u64 {
            return Err(Error::SummaryTooLarge {
                bytes: self.buf.len(),
            });
        }
        let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
            // Not in Actions / summaries disabled: nothing to write to. This
            // is a normal local-run condition, not an error — but the buffer
            // is *kept* (no write happened, so draining it would lose data
            // silently). The caller can still `stringify()` or retry.
            return Ok(());
        };
        let existing_bytes = if append {
            match std::fs::metadata(&path) {
                Ok(meta) => meta.len(),
                Err(err) if err.kind() == ErrorKind::NotFound => 0,
                Err(err) => return Err(err.into()),
            }
        } else {
            0
        };
        let total_bytes = existing_bytes.saturating_add(write_bytes);
        if total_bytes > MAX_BYTES as u64 {
            return Err(Error::SummaryTooLarge {
                bytes: usize::try_from(total_bytes).unwrap_or(usize::MAX),
            });
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(append)
            .write(true)
            .truncate(!append)
            .open(path)?;
        file.write_all(self.buf.as_bytes())?;
        self.clear();
        Ok(())
    }

    /// Append the buffer to the job summary file.
    ///
    /// # Errors
    /// [`Error::SummaryTooLarge`] if the buffer exceeds 1 MiB, or an I/O error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mut s = actions_rs::Summary::new();
    /// s.heading("Build passed", 2);
    /// s.write()?; // appends to $GITHUB_STEP_SUMMARY
    /// # Ok::<(), actions_rs::Error>(())
    /// ```
    pub fn write(&mut self) -> Result<()> {
        self.write_inner(true)
    }

    /// Overwrite the job summary file with the buffer.
    ///
    /// # Errors
    /// [`Error::SummaryTooLarge`] if the buffer exceeds 1 MiB, or an I/O error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mut s = actions_rs::Summary::new();
    /// s.heading("Replaces any prior summary", 2);
    /// s.write_overwrite()?;
    /// # Ok::<(), actions_rs::Error>(())
    /// ```
    pub fn write_overwrite(&mut self) -> Result<()> {
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
    fn raw_html_is_opt_in() {
        let mut s = Summary::new();
        s.details(
            SummaryText::html("<b>open</b>"),
            SummaryText::html("<p>surprise</p>"),
        );
        assert_eq!(
            s.stringify(),
            "<details><summary><b>open</b></summary><p>surprise</p></details>\n"
        );
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
    fn span_zero_is_clamped_to_one() {
        let mut s = Summary::new();
        s.table([vec![Cell::new("x").colspan(0).rowspan(0)]]);
        assert_eq!(
            s.stringify(),
            "<table><tr><td colspan=\"1\" rowspan=\"1\">x</td></tr></table>\n"
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
