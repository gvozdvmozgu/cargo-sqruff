use rustc_ast::{
    LitKind, StrStyle,
    token::{Token, TokenKind},
    tokenstream::{TokenStream, TokenTree},
};
use rustc_hir::{Expr, ExprKind};
use rustc_span::{BytePos, Span};

pub(crate) struct SqlLiteral {
    pub(crate) sql: String,
    pub(crate) full_span: Span,
    pub(crate) content_start: BytePos,
    pub(crate) content_end: BytePos,
}

pub(crate) fn expr_sql_literal<'tcx>(
    expr: &'tcx Expr<'tcx>,
    sql_arg_index: usize,
) -> Option<SqlLiteral> {
    let arg = expr_arg(expr, sql_arg_index)?;

    if let ExprKind::Lit(lit) = arg.kind
        && let LitKind::Str(ref raw, style) = lit.node
    {
        return Some(string_literal(raw.as_str(), style, arg.span));
    }

    None
}

pub(crate) fn token_stream_sql_literal(
    tokens: &TokenStream,
    sql_arg_index: usize,
) -> Option<SqlLiteral> {
    let mut arg_index = 0;
    let mut candidate = None;
    let mut saw_other_token = false;

    for tt in tokens.iter() {
        if token_is_comma(tt) {
            if arg_index == sql_arg_index {
                return if !saw_other_token { candidate } else { None };
            }

            arg_index += 1;
            candidate = None;
            saw_other_token = false;
        } else if candidate.is_none()
            && !saw_other_token
            && let Some(literal) = string_token_tree(tt)
        {
            candidate = Some(literal);
        } else {
            saw_other_token = true;
        }
    }

    if arg_index == sql_arg_index && !saw_other_token {
        candidate
    } else {
        None
    }
}

fn expr_arg<'tcx>(expr: &'tcx Expr<'tcx>, index: usize) -> Option<&'tcx Expr<'tcx>> {
    match expr.kind {
        ExprKind::Call(_, args) => args.get(index),
        ExprKind::MethodCall(_, _, args, _) => args.get(index),
        _ => None,
    }
}

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

fn string_token(token: &Token) -> Option<SqlLiteral> {
    if let TokenKind::Literal(lit) = token.kind
        && lit.suffix.is_none()
        && let Ok(LitKind::Str(ref raw, style)) = LitKind::from_token_lit(lit)
    {
        return Some(string_literal(raw.as_str(), style, token.span));
    }

    None
}

fn string_token_tree(tt: &TokenTree) -> Option<SqlLiteral> {
    if let TokenTree::Token(token, _) = tt {
        string_token(token)
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
