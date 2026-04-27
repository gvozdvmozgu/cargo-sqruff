use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use rustc_ast::ast;
use rustc_hir::Expr;
use rustc_lint::{EarlyContext, EarlyLintPass, LateContext, LateLintPass};
use rustc_session::impl_lint_pass;
use sqruff_lib::core::linter::core::Linter;

use crate::{
    CARGO_SQRUFF,
    config::{self, SqruffConfig},
    literal, registry,
    registry::{ResolvedCallRegistry, builtin_library_specs},
    sqruff,
};

pub(crate) struct Sql {
    linter: ConfiguredLinter,
    calls: ResolvedCallRegistry,
}

pub(crate) struct SqlMacros {
    linter: ConfiguredLinter,
}

struct ConfiguredLinter {
    state: Result<Linter, String>,
    emitted_config_error: Arc<AtomicBool>,
}

impl ConfiguredLinter {
    fn new(config: SqruffConfig, emitted_config_error: Arc<AtomicBool>) -> Self {
        Self {
            state: config::build_config(&config).and_then(sqruff::linter),
            emitted_config_error,
        }
    }

    fn lint_literal(&mut self, cx: &impl rustc_lint::LintContext, literal: literal::SqlLiteral) {
        match &mut self.state {
            Ok(linter) => sqruff::lint_literal(cx, linter, literal),
            Err(message) if !self.emitted_config_error.swap(true, Ordering::Relaxed) => {
                sqruff::emit_config_error(cx, literal.full_span, message);
            }
            Err(_) => {}
        }
    }
}

impl Sql {
    pub(crate) fn new(config: SqruffConfig, emitted_config_error: Arc<AtomicBool>) -> Self {
        Self {
            linter: ConfiguredLinter::new(config, emitted_config_error),
            calls: ResolvedCallRegistry::new(),
        }
    }
}

impl SqlMacros {
    pub(crate) fn new(config: SqruffConfig, emitted_config_error: Arc<AtomicBool>) -> Self {
        Self {
            linter: ConfiguredLinter::new(config, emitted_config_error),
        }
    }
}

impl_lint_pass!(Sql => [CARGO_SQRUFF]);
impl_lint_pass!(SqlMacros => [CARGO_SQRUFF]);

impl EarlyLintPass for SqlMacros {
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expr: &ast::Expr) {
        if let ast::ExprKind::MacCall(mac) = &expr.kind
            && let Some(sql_arg_index) = registry::macro_sql_arg_index(mac, builtin_library_specs())
            && let Some(literal) =
                literal::token_stream_sql_literal(&mac.args.tokens, sql_arg_index)
        {
            self.linter.lint_literal(cx, literal);
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Sql {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.calls.resolve(cx, builtin_library_specs());
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let Some(sql_arg_index) = self.calls.sql_arg_index(cx, expr) else {
            return;
        };

        if let Some(literal) = literal::expr_sql_literal(expr, sql_arg_index) {
            self.linter.lint_literal(cx, literal);
        }
    }
}
