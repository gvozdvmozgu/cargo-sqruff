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

use rustc_ast::{
    LitKind, StrStyle, ast,
    token::{Token, TokenKind},
    tokenstream::{TokenStream, TokenTree},
};
use rustc_hir::{Expr, ExprKind, def_id::DefIdSet};
use rustc_lint::{EarlyContext, EarlyLintPass, LateLintPass, LintContext, LintStore};
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

const SQLX_INLINE_MACROS: &[&str] = &[
    "query",
    "query_unchecked",
    "query_as",
    "query_as_unchecked",
    "query_scalar",
    "query_scalar_unchecked",
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
    lint_store.register_pre_expansion_pass(|| {
        Box::new(SqlMacros {
            linter: sqruff_linter(),
        })
    });
    lint_store.register_late_pass(|_| {
        Box::new(Sql {
            linter: sqruff_linter(),
            definitions: DefIdSet::default(),
        })
    });
}

fn sqruff_linter() -> Linter {
    let config = FluffConfig::from_root(None, false, None).unwrap();
    Linter::new(config, None, None, true).unwrap()
}

struct Sql {
    linter: Linter,
    definitions: DefIdSet,
}

struct SqlMacros {
    linter: Linter,
}

struct SqlLiteral {
    sql: String,
    full_span: Span,
    content_start: BytePos,
    content_end: BytePos,
}

impl Sql {
    fn string_literal(raw: &str, style: StrStyle, span: Span) -> SqlLiteral {
        let prefix_len = BytePos(match style {
            StrStyle::Raw(n) => 2 + n as u32,
            StrStyle::Cooked => 1,
        });

        SqlLiteral {
            sql: raw.to_owned(),
            full_span: span,
            content_start: span.lo() + prefix_len,
            content_end: span.hi() - prefix_len,
        }
    }

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

    fn string_token(token: &Token) -> Option<SqlLiteral> {
        if let TokenKind::Literal(lit) = token.kind
            && lit.suffix.is_none()
            && let Ok(LitKind::Str(ref raw, style)) = LitKind::from_token_lit(lit)
        {
            return Some(Self::string_literal(raw.as_str(), style, token.span));
        }

        None
    }

    fn string_token_tree(tt: &TokenTree) -> Option<SqlLiteral> {
        if let TokenTree::Token(token, _) = tt {
            Self::string_token(token)
        } else {
            None
        }
    }

    fn string_literal_arg(tokens: &TokenStream, target_arg_index: usize) -> Option<SqlLiteral> {
        let mut arg_index = 0;
        let mut candidate = None;
        let mut saw_other_token = false;

        for tt in tokens.iter() {
            if Self::token_is_comma(tt) {
                if arg_index == target_arg_index {
                    return if !saw_other_token { candidate } else { None };
                }

                arg_index += 1;
                candidate = None;
                saw_other_token = false;
            } else if candidate.is_none()
                && !saw_other_token
                && let Some(literal) = Self::string_token_tree(tt)
            {
                candidate = Some(literal);
            } else {
                saw_other_token = true;
            }
        }

        if arg_index == target_arg_index && !saw_other_token {
            candidate
        } else {
            None
        }
    }

    fn token_is_comma(tt: &TokenTree) -> bool {
        matches!(
            tt,
            TokenTree::Token(
                Token {
                    kind: TokenKind::Comma,
                    ..
                },
                _
            )
        )
    }

    fn sqlx_macro_name(mac: &ast::MacCall) -> Option<&str> {
        let [krate, name] = mac.path.segments.as_slice() else {
            return None;
        };

        if krate.ident.name.as_str() != "sqlx" {
            return None;
        }

        let name = name.ident.name.as_str();
        SQLX_INLINE_MACROS.contains(&name).then_some(name)
    }

    fn macro_sql_literal(mac: &ast::MacCall) -> Option<SqlLiteral> {
        let name = Self::sqlx_macro_name(mac)?;
        let query_arg_index = match name {
            "query_as" | "query_as_unchecked" => 1,
            _ => 0,
        };

        Self::string_literal_arg(&mac.args.tokens, query_arg_index)
    }

    fn sql_literal<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<SqlLiteral> {
        let arg = Self::first_arg(expr)?;

        if let ExprKind::Lit(lit) = arg.kind
            && let LitKind::Str(ref raw, style) = lit.node
        {
            return Some(Self::string_literal(raw.as_str(), style, arg.span));
        }

        None
    }

    fn emit_sqruff_error(cx: &impl LintContext, span: Span, message: impl Into<String>) {
        cx.lint(CARGO_SQRUFF, |diag| {
            diag.span(span);
            diag.primary_message("failed to lint SQL query");
            diag.note(message.into());
        });
    }

    fn emit_violation(cx: &impl LintContext, span: Span, code: &str, description: String) {
        cx.lint(CARGO_SQRUFF, |diag| {
            diag.span(span);
            diag.primary_message(format!("[{code}]: {description}"));
            diag.span_label(span, description);
        });
    }

    fn emit_fix_suggestion(cx: &impl LintContext, span: Span, suggestion: String) {
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

    fn lint_literal(cx: &impl LintContext, linter: &mut Linter, literal: SqlLiteral) {
        let result = match linter.lint_string(&literal.sql, None, true) {
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
impl_lint_pass!(SqlMacros => [CARGO_SQRUFF]);

impl EarlyLintPass for SqlMacros {
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expr: &ast::Expr) {
        if let ast::ExprKind::MacCall(mac) = &expr.kind
            && let Some(literal) = Sql::macro_sql_literal(mac)
        {
            Sql::lint_literal(cx, &mut self.linter, literal);
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Sql {
    fn check_crate(&mut self, cx: &rustc_lint::LateContext<'tcx>) {
        self.register_paths(cx, SQLX_PATHS);
        self.register_paths(cx, RUSQLITE_PATHS);
    }

    fn check_expr(&mut self, cx: &rustc_lint::LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        if !self.tracked_call(cx, expr) {
            return;
        }

        if let Some(literal) = Self::sql_literal(expr) {
            Self::lint_literal(cx, &mut self.linter, literal);
        }
    }
}
