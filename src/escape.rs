//! Percent-encoding for GitHub Actions workflow commands.
//!
//! The runner parses lines of the form `::cmd key=val,key=val::data`.
//! To keep that grammar unambiguous certain characters must be percent-encoded.
//! The rules below are taken verbatim from `@actions/core` (`packages/core/src/command.ts`) and the
//! official workflow-commands documentation.\
//! **Order matters**: `%` must be encoded first so the escape sequences introduced by later replacements are not double-encoded.

/// Encode a command *data* segment (the message after `::`).
///
/// Encodes `%` → `%25`, `\r` → `%0D`, `\n` → `%0A`.
pub(crate) fn escape_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Encode a command *property* value (e.g. `file=`, `title=`).
///
/// Everything [`escape_data`] does, plus `:` → `%3A` and `,` → `%2C` so the `key=value` / comma-separated
/// property grammar stays parseable.
pub(crate) fn escape_property(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_encodes_percent_first() {
        // If `%` were not encoded first, `\n` → `%0A` would then have its `%`
        // re-encoded to `%250A`. This asserts the ordering is correct.
        assert_eq!(escape_data("%\n"), "%25%0A");
        assert_eq!(escape_data("a%b"), "a%25b");
        assert_eq!(escape_data("l1\r\nl2"), "l1%0D%0Al2");
    }

    #[test]
    fn data_leaves_colon_and_comma() {
        assert_eq!(escape_data("a:b,c"), "a:b,c");
    }

    #[test]
    fn property_encodes_colon_and_comma_too() {
        assert_eq!(escape_property("a:b,c"), "a%3Ab%2Cc");
        assert_eq!(escape_property("%\r\n:,"), "%25%0D%0A%3A%2C");
    }

    #[test]
    fn empty_is_empty() {
        assert_eq!(escape_data(""), "");
        assert_eq!(escape_property(""), "");
    }
}
