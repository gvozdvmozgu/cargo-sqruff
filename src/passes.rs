use rustc_ast::ast;
use rustc_hir::Expr;
use rustc_lint::{EarlyContext, EarlyLintPass, LateContext, LateLintPass};
use rustc_session::impl_lint_pass;
use sqruff_lib::core::linter::core::Linter;

use crate::{
    CARGO_SQRUFF, literal, registry,
    registry::{ResolvedCallRegistry, builtin_library_specs},
    sqruff,
};

pub(crate) struct Sql {
    linter: Linter,
    calls: ResolvedCallRegistry,
}

pub(crate) struct SqlMacros {
    linter: Linter,
}

impl Sql {
    pub(crate) fn new() -> Self {
        Self {
            linter: sqruff::linter(),
            calls: ResolvedCallRegistry::new(),
        }
    }
}

impl SqlMacros {
    pub(crate) fn new() -> Self {
        Self {
            linter: sqruff::linter(),
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
            sqruff::lint_literal(cx, &mut self.linter, literal);
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
            sqruff::lint_literal(cx, &mut self.linter, literal);
        }
    }
}
