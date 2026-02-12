#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

dylint_linting::dylint_library!();

use clippy_utils::fn_def_id;
use clippy_utils::paths::{PathNS, lookup_path_str};

use rustc_ast::{LitKind, StrStyle};
use rustc_hir::{Expr, ExprKind, def_id::DefIdSet};
use rustc_lint::{LateLintPass, LintContext as _, LintStore};
use rustc_session::{Session, declare_lint, impl_lint_pass};
use rustc_span::{BytePos, Span, SyntaxContext};
use sqruff_lib::core::{config::FluffConfig, linter::core::Linter};

declare_lint! {
    pub CARGO_SQRUFF,
    Warn,
    "description goes here"
}

#[allow(clippy::no_mangle_with_rust_abi)]
#[unsafe(no_mangle)]
pub fn register_lints(_sess: &Session, lint_store: &mut LintStore) {
    lint_store.register_lints(&[CARGO_SQRUFF]);
    lint_store.register_late_pass(|_| {
        let config = FluffConfig::from_root(None, false, None).unwrap();
        let linter = Linter::new(config, None, None, true);

        Box::new(Sql {
            linter,
            definitions: DefIdSet::default(),
        })
    });
}

struct Sql {
    linter: Linter,
    definitions: DefIdSet,
}

impl_lint_pass!(Sql => [CARGO_SQRUFF]);

impl<'tcx> LateLintPass<'tcx> for Sql {
    fn check_crate(&mut self, cx: &rustc_lint::LateContext<'tcx>) {
        let paths = [
            "query",
            "query_as",
            "query_as_with",
            "query_scalar",
            "query_scalar_with",
            "query_with",
            "raw_sql",
        ];

        for path in paths {
            let full_path = format!("sqlx::{path}");
            for def_id in lookup_path_str(cx.tcx, PathNS::Value, &full_path) {
                self.definitions.insert(def_id);
            }
        }
    }

    fn check_expr(&mut self, cx: &rustc_lint::LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Call(_, args) = expr.kind
            && let Some(def_id) = fn_def_id(cx, expr)
            && self.definitions.contains(&def_id)
            && let Some(arg) = args.first()
            && let ExprKind::Lit(lit) = arg.kind
            && let LitKind::Str(ref r, style) = lit.node
        {
            let sql = r.as_str();
            let lit_lo = arg.span.lo();
            let prefix_len = BytePos(match style {
                StrStyle::Raw(n) => 2 + n as u32,
                StrStyle::Cooked => 1,
            });

            let content_start = lit_lo + prefix_len;
            let content_end = arg.span.hi() - prefix_len;

            let result = match self.linter.lint_string(sql, None, true) {
                Ok(result) => result,
                Err(err) => {
                    cx.lint(CARGO_SQRUFF, |diag| {
                        diag.span(arg.span);
                        diag.primary_message("failed to lint SQL query");
                        diag.note(err.to_string());
                    });
                    return;
                }
            };
            let has_violations = result.has_violations();

            for violation in result.violations() {
                let rel = &violation.source_slice;
                let abs_start = content_start + BytePos(rel.start as u32);
                let abs_end = content_start + BytePos(rel.end as u32);
                let abs_span = Span::new(abs_start, abs_end, SyntaxContext::root(), None);

                cx.lint(CARGO_SQRUFF, |diag| {
                    let code = violation.rule.as_ref().unwrap().code;
                    let description = violation.description.to_string();

                    diag.span(abs_span);
                    diag.primary_message(format!("[{code}]: {description}"));
                    diag.span_label(abs_span, description);
                });
            }

            let suggestion = result.fix_string();
            if !has_violations || sql == suggestion {
                return;
            }

            cx.lint(CARGO_SQRUFF, |diag| {
                let span = Span::new(content_start, content_end, SyntaxContext::root(), None);

                diag.primary_message("SQL query contains violations");
                diag.span_suggestion_with_style(
                    span,
                    "consider using `sqruff` to fix this",
                    suggestion,
                    rustc_errors::Applicability::MachineApplicable,
                    rustc_errors::SuggestionStyle::ShowAlways,
                );
            });
        }
    }
}
