use rustc_lint::LintContext;
use rustc_span::{BytePos, Span, SyntaxContext};
use sqruff_lib::core::{config::FluffConfig, linter::core::Linter};

use crate::{CARGO_SQRUFF, literal::SqlLiteral};

pub(crate) fn linter(config: FluffConfig) -> Result<Linter, String> {
    Linter::new(config, None, None, true).map_err(|err| err.to_string())
}

pub(crate) fn emit_config_error(cx: &impl LintContext, span: Span, message: &str) {
    cx.lint(CARGO_SQRUFF, |diag| {
        diag.span(span);
        diag.primary_message("failed to load sqruff config");
        diag.note(message.to_owned());
    });
}

pub(crate) fn lint_literal(cx: &impl LintContext, linter: &mut Linter, literal: SqlLiteral) {
    let result = match linter.lint_string(&literal.sql, None, true) {
        Ok(result) => result,
        Err(err) => {
            emit_sqruff_error(cx, literal.full_span, err.to_string());
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

        emit_violation(cx, abs_span, code, description);
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
    emit_fix_suggestion(cx, span, suggestion);
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
