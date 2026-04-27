#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

use std::sync::{Arc, atomic::AtomicBool};

dylint_linting::dylint_library!();

mod config;
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
pub fn register_lints(sess: &Session, lint_store: &mut LintStore) {
    lint_store.register_lints(&[CARGO_SQRUFF]);

    let config = config::sqruff_config(sess);
    let emitted_config_error = Arc::new(AtomicBool::new(false));

    let macro_config = config.clone();
    let macro_emitted_config_error = Arc::clone(&emitted_config_error);
    lint_store.register_pre_expansion_pass(move || {
        Box::new(passes::SqlMacros::new(
            macro_config.clone(),
            Arc::clone(&macro_emitted_config_error),
        ))
    });

    lint_store.register_late_pass(move |_| {
        Box::new(passes::Sql::new(
            config.clone(),
            Arc::clone(&emitted_config_error),
        ))
    });
}
