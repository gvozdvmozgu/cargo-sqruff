#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

dylint_linting::dylint_library!();

mod literal;
mod passes;
mod registry;
mod sqruff;

use rustc_lint::LintStore;
use rustc_session::{Session, declare_lint};

declare_lint! {
    pub CARGO_SQRUFF,
    Warn,
    "description goes here"
}

#[allow(clippy::no_mangle_with_rust_abi)]
#[unsafe(no_mangle)]
pub fn register_lints(_sess: &Session, lint_store: &mut LintStore) {
    lint_store.register_lints(&[CARGO_SQRUFF]);
    lint_store.register_pre_expansion_pass(|| Box::new(passes::SqlMacros::new()));
    lint_store.register_late_pass(|_| Box::new(passes::Sql::new()));
}
