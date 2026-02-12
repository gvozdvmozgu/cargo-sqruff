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

const SQLX_PATHS: &[&str] = &[
    "sqlx::query",
    "sqlx::query_as",
    "sqlx::query_as_with",
    "sqlx::query_scalar",
    "sqlx::query_scalar_with",
    "sqlx::query_with",
    "sqlx::raw_sql",
];

const RUSQLITE_PATHS: &[&str] = &[
    "rusqlite::Connection::execute",
    "rusqlite::Connection::execute_batch",
    "rusqlite::Connection::prepare",
    "rusqlite::Connection::prepare_cached",
    "rusqlite::Connection::query_row",
    "rusqlite::Connection::query_row_and_then",
    "rusqlite::Transaction::execute",
    "rusqlite::Transaction::execute_batch",
    "rusqlite::Transaction::prepare",
    "rusqlite::Transaction::prepare_cached",
    "rusqlite::Transaction::query_row",
    "rusqlite::Transaction::query_row_and_then",
];

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

struct SqlLiteral {
    sql: String,
    full_span: Span,
    content_start: BytePos,
    content_end: BytePos,
}

impl Sql {
    fn register_paths<'tcx>(&mut self, cx: &rustc_lint::LateContext<'tcx>, paths: &[&str]) {
        for path in paths {
            for def_id in lookup_path_str(cx.tcx, PathNS::Value, path) {
                self.definitions.insert(def_id);
            }
        }
    }

    fn tracked_call<'tcx>(
        &self,
        cx: &rustc_lint::LateContext<'tcx>,
        expr: &'tcx Expr<'tcx>,
    ) -> bool {
        fn_def_id(cx, expr).is_some_and(|def_id| self.definitions.contains(&def_id))
    }

    fn first_arg<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
        match expr.kind {
            ExprKind::Call(_, args) => args.first(),
            ExprKind::MethodCall(_, _, args, _) => args.first(),
            _ => None,
        }
    }

    fn sql_literal<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<SqlLiteral> {
        let arg = Self::first_arg(expr)?;

        if let ExprKind::Lit(lit) = arg.kind
            && let LitKind::Str(ref raw, style) = lit.node
        {
            let prefix_len = BytePos(match style {
                StrStyle::Raw(n) => 2 + n as u32,
                StrStyle::Cooked => 1,
            });

            return Some(SqlLiteral {
                sql: raw.as_str().to_owned(),
                full_span: arg.span,
                content_start: arg.span.lo() + prefix_len,
                content_end: arg.span.hi() - prefix_len,
            });
        }

        None
    }

    fn emit_sqruff_error<'tcx>(
        cx: &rustc_lint::LateContext<'tcx>,
        span: Span,
        message: impl Into<String>,
    ) {
        cx.lint(CARGO_SQRUFF, |diag| {
            diag.span(span);
            diag.primary_message("failed to lint SQL query");
            diag.note(message.into());
        });
    }

    fn emit_violation<'tcx>(
        cx: &rustc_lint::LateContext<'tcx>,
        span: Span,
        code: &str,
        description: String,
    ) {
        cx.lint(CARGO_SQRUFF, |diag| {
            diag.span(span);
            diag.primary_message(format!("[{code}]: {description}"));
            diag.span_label(span, description);
        });
    }

    fn emit_fix_suggestion<'tcx>(
        cx: &rustc_lint::LateContext<'tcx>,
        span: Span,
        suggestion: String,
    ) {
        cx.lint(CARGO_SQRUFF, |diag| {
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

    fn lint_literal<'tcx>(&mut self, cx: &rustc_lint::LateContext<'tcx>, literal: SqlLiteral) {
        let result = match self.linter.lint_string(&literal.sql, None, true) {
            Ok(result) => result,
            Err(err) => {
                Self::emit_sqruff_error(cx, literal.full_span, err.to_string());
                return;
            }
        };
        let has_violations = result.has_violations();

        for violation in result.violations() {
            let abs_start = literal.content_start + BytePos(violation.source_slice.start as u32);
            let abs_end = literal.content_start + BytePos(violation.source_slice.end as u32);
            let abs_span = Span::new(abs_start, abs_end, SyntaxContext::root(), None);
            let code = violation.rule.as_ref().map_or("????", |rule| rule.code);
            let description = violation.description.to_string();

            Self::emit_violation(cx, abs_span, code, description);
        }

        let suggestion = result.fix_string();
        if !has_violations || literal.sql == suggestion {
            return;
        }

        let span = Span::new(
            literal.content_start,
            literal.content_end,
            SyntaxContext::root(),
            None,
        );
        Self::emit_fix_suggestion(cx, span, suggestion);
    }
}

impl_lint_pass!(Sql => [CARGO_SQRUFF]);

impl<'tcx> LateLintPass<'tcx> for Sql {
    fn check_crate(&mut self, cx: &rustc_lint::LateContext<'tcx>) {
        self.register_paths(cx, SQLX_PATHS);
        self.register_paths(cx, RUSQLITE_PATHS);
    }

    fn check_expr(&mut self, cx: &rustc_lint::LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if !self.tracked_call(cx, expr) {
            return;
        }

        if let Some(literal) = Self::sql_literal(expr) {
            self.lint_literal(cx, literal);
        }
    }
}
