#![feature(rustc_private)]
#![feature(let_chains)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

dylint_linting::dylint_library!();

use clippy_utils::{def_path_res_with_base, find_crates, path_def_id, sym};

use rustc_ast::{LitKind, StrStyle};
use rustc_hir::{def_id::DefIdSet, Expr, ExprKind};
use rustc_lint::{LateLintPass, LintContext as _, LintStore};
use rustc_session::{declare_lint, impl_lint_pass, Session};
use rustc_span::{BytePos, Pos, Span, SyntaxContext};
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
        let sqlx_crates = find_crates(cx.tcx, sym!(sqlx));
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
            let names = def_path_res_with_base(cx.tcx, sqlx_crates.clone(), &[path]);
            for name in names {
                if let Some(def_id) = name.opt_def_id() {
                    self.definitions.insert(def_id);
                }
            }
        }
    }

    fn check_expr(&mut self, cx: &rustc_lint::LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Call(fun, args) = expr.kind
            && let Some(def_id) = path_def_id(cx, fun)
            && self.definitions.contains(&def_id)
            && let Some(arg) = args.first()
            && let ExprKind::Lit(lit) = arg.kind
            && let LitKind::Str(ref r, style) = lit.node
        {
            let sql = r.as_str();
            let lit_lo = arg.span.lo().to_u32();
            let prefix_len = match style {
                StrStyle::Raw(n) => 2 + n as u32,
                StrStyle::Cooked => 1,
            };
            let content_start = lit_lo + prefix_len;

            for violation in self
                .linter
                .lint_string(sql, None, false)
                .get_violations(None)
            {
                let rel = &violation.source_slice;
                let abs_start = BytePos::from_u32(content_start + rel.start as u32);
                let abs_end = BytePos::from_u32(content_start + rel.end as u32);
                let abs_span = Span::new(abs_start, abs_end, SyntaxContext::root(), None);

                cx.lint(CARGO_SQRUFF, |diag| {
                    let code = violation.rule.unwrap().code;
                    let description = violation.description;

                    diag.span(abs_span);
                    diag.primary_message(format!("[{code}] {description}"));
                    diag.span_label(abs_span, description);
                });
            }
        }
    }
}
