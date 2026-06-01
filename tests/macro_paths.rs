//! Public macro path regression tests.

#[test]
fn log_group_macro_is_reachable_from_external_path() {
    let answer = actions_rs::log::group!("compute", { 6 * 7 });

    assert_eq!(answer, 42);
}

#[test]
fn prelude_group_macro_remains_available() {
    use actions_rs::prelude::*;

    let answer = group!("compute", { 6 * 7 });

    assert_eq!(answer, 42);
}
