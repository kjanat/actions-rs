//! Typed access to action inputs.
//!
//! An action input named `foo-bar` is passed to the process as the environment
//! variable `INPUT_FOO-BAR`: the rule is `INPUT_` + uppercased name with
//! spaces replaced by underscores (hyphens are **kept**). This matches
//! `@actions/core`'s `getInput`.
//!
//! The name→key transform and the strict boolean parser are pure functions so
//! they are unit-tested without mutating the global environment.

use std::fmt::Display;
use std::str::FromStr;

use crate::error::{Error, Result};
use crate::log;

/// Options controlling how an input is read.
#[derive(Debug, Clone, Copy)]
pub struct InputOptions {
    /// Error with [`Error::MissingRequiredInput`] if the input is absent/empty.
    pub required: bool,
    /// Trim leading/trailing whitespace (default `true`, as in `@actions/core`).
    pub trim: bool,
}

impl Default for InputOptions {
    fn default() -> Self {
        Self {
            required: false,
            trim: true,
        }
    }
}

/// Compute the environment-variable key for an input name.
///
/// `INPUT_` + `name.to_uppercase()` with ASCII spaces → `_`.
#[must_use]
pub fn input_env_key(name: &str) -> String {
    format!("INPUT_{}", name.replace(' ', "_").to_uppercase())
}

fn raw(name: &str) -> Option<String> {
    std::env::var(input_env_key(name)).ok()
}

/// Read an input with explicit [`InputOptions`].
///
/// # Errors
/// [`Error::MissingRequiredInput`] when `options.required` and the input is
/// absent or (after optional trimming) empty.
pub fn input_with(name: &str, options: InputOptions) -> Result<String> {
    let value = raw(name).unwrap_or_default();
    let value = if options.trim {
        value.trim().to_owned()
    } else {
        value
    };
    if options.required && value.is_empty() {
        return Err(Error::MissingRequiredInput(name.to_owned()));
    }
    Ok(value)
}

/// Read an optional input, trimmed. Returns `""` when unset.
#[must_use]
pub fn input(name: &str) -> String {
    // Infallible: required is false, so `input_with` cannot error here.
    input_with(name, InputOptions::default()).unwrap_or_default()
}

/// Read a required input, trimmed.
///
/// # Errors
/// [`Error::MissingRequiredInput`] when absent or empty.
pub fn input_required(name: &str) -> Result<String> {
    input_with(
        name,
        InputOptions {
            required: true,
            trim: true,
        },
    )
}

/// Strict YAML 1.2 core-schema boolean parse of `value` for input `name`.
fn parse_bool(name: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "True" | "TRUE" => Ok(true),
        "false" | "False" | "FALSE" => Ok(false),
        _ => Err(Error::InvalidBool {
            name: name.to_owned(),
            value: value.to_owned(),
        }),
    }
}

/// Read a boolean input using the strict YAML 1.2 core schema
/// (`true|True|TRUE|false|False|FALSE`).
///
/// # Errors
/// [`Error::InvalidBool`] for any other value, including absent/empty
/// (matching `@actions/core`'s `getBooleanInput`).
pub fn bool_input(name: &str) -> Result<bool> {
    let value = input_with(
        name,
        InputOptions {
            required: false,
            trim: true,
        },
    )?;
    parse_bool(name, &value)
}

/// Split a multiline input on `\n`, dropping empty lines. Each retained line
/// is trimmed.
#[must_use]
pub fn multiline_input(name: &str) -> Vec<String> {
    split_multiline(&input(name))
}

fn split_multiline(value: &str) -> Vec<String> {
    value
        .split('\n')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Read an input and parse it via [`FromStr`].
///
/// # Errors
/// [`Error::ParseInput`] if parsing fails (the type's `FromStr::Err` is
/// rendered via [`Display`]).
pub fn input_as<T>(name: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    let value = input_required(name)?;
    value.parse::<T>().map_err(|e| Error::ParseInput {
        name: name.to_owned(),
        reason: e.to_string(),
    })
}

/// Mask the (untrimmed) raw value of input `name` in subsequent logs.
///
/// No-op when the input is unset.
pub fn mask_input(name: &str) {
    if let Some(value) = raw(name).filter(|v| !v.is_empty()) {
        log::mask(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_transform() {
        assert_eq!(input_env_key("my input"), "INPUT_MY_INPUT");
        assert_eq!(input_env_key("my-input"), "INPUT_MY-INPUT");
        assert_eq!(input_env_key("myInput"), "INPUT_MYINPUT");
        assert_eq!(input_env_key("a b-c d"), "INPUT_A_B-C_D");
    }

    #[test]
    fn strict_bool_accepts_canonical() {
        for v in ["true", "True", "TRUE"] {
            assert!(parse_bool("x", v).unwrap());
        }
        for v in ["false", "False", "FALSE"] {
            assert!(!parse_bool("x", v).unwrap());
        }
    }

    #[test]
    fn strict_bool_rejects_others() {
        for v in ["yes", "1", "TrUe", "", " true", "0"] {
            let e = parse_bool("flag", v).unwrap_err();
            assert!(
                matches!(e, Error::InvalidBool { .. }),
                "{v:?} should be invalid"
            );
        }
    }

    #[test]
    fn multiline_splits_and_trims_and_drops_empty() {
        assert_eq!(
            split_multiline("a\n  b  \n\n c\n"),
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
        assert!(split_multiline("").is_empty());
    }
}
