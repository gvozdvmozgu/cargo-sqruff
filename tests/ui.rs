#[test]
fn ui_examples() {
    dylint_testing::ui_test_examples(env!("CARGO_PKG_NAME"));
}

#[test]
fn ui_cargo_metadata() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "tests/ui-cargo-metadata");
}

#[test]
fn ui_external_sqlx() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "tests/ui-external-sqlx");
}
