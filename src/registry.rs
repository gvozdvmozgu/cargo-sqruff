use std::collections::HashMap;

use clippy_utils::fn_def_id;
use clippy_utils::paths::{PathNS, lookup_path_str};
use rustc_ast::ast;
use rustc_hir::{Expr, def_id::DefId};
use rustc_lint::LateContext;

pub(crate) struct LibrarySpec {
    pub(crate) calls: &'static [CallSpec],
    pub(crate) macros: &'static [MacroSpec],
}

pub(crate) struct CallSpec {
    pub(crate) path: &'static str,
    pub(crate) sql_arg_index: usize,
}

pub(crate) struct MacroSpec {
    pub(crate) path_segments: &'static [&'static str],
    pub(crate) sql_arg_index: usize,
}

pub(crate) struct ResolvedCallRegistry {
    calls: HashMap<DefId, usize>,
}

impl ResolvedCallRegistry {
    pub(crate) fn new() -> Self {
        Self {
            calls: HashMap::new(),
        }
    }

    pub(crate) fn resolve<'tcx>(&mut self, cx: &LateContext<'tcx>, specs: &[LibrarySpec]) {
        for spec in specs {
            for call in spec.calls {
                for def_id in lookup_path_str(cx.tcx, PathNS::Value, call.path) {
                    self.calls.insert(def_id, call.sql_arg_index);
                }
            }
        }
    }

    pub(crate) fn sql_arg_index<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        expr: &'tcx Expr<'tcx>,
    ) -> Option<usize> {
        fn_def_id(cx, expr).and_then(|def_id| self.calls.get(&def_id).copied())
    }
}

pub(crate) fn macro_sql_arg_index(mac: &ast::MacCall, specs: &[LibrarySpec]) -> Option<usize> {
    specs
        .iter()
        .flat_map(|spec| spec.macros)
        .find(|spec| macro_path_matches(mac, spec.path_segments))
        .map(|spec| spec.sql_arg_index)
}

pub(crate) fn builtin_library_specs() -> &'static [LibrarySpec] {
    &BUILTIN_LIBRARY_SPECS
}

// To support another SQL library, add its call and macro specs here, then
// include the library in BUILTIN_LIBRARY_SPECS.
fn macro_path_matches(mac: &ast::MacCall, segments: &[&str]) -> bool {
    mac.path.segments.len() == segments.len()
        && mac
            .path
            .segments
            .iter()
            .zip(segments)
            .all(|(actual, expected)| actual.ident.name.as_str() == *expected)
}

static BUILTIN_LIBRARY_SPECS: [LibrarySpec; 2] = [
    LibrarySpec {
        calls: SQLX_CALLS,
        macros: SQLX_MACROS,
    },
    LibrarySpec {
        calls: RUSQLITE_CALLS,
        macros: &[],
    },
];

static SQLX_CALLS: &[CallSpec] = &[
    CallSpec {
        path: "sqlx::query",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "sqlx::query_as",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "sqlx::query_as_with",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "sqlx::query_scalar",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "sqlx::query_scalar_with",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "sqlx::query_with",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "sqlx::raw_sql",
        sql_arg_index: 0,
    },
];

static SQLX_MACROS: &[MacroSpec] = &[
    MacroSpec {
        path_segments: &["sqlx", "query"],
        sql_arg_index: 0,
    },
    MacroSpec {
        path_segments: &["sqlx", "query_unchecked"],
        sql_arg_index: 0,
    },
    MacroSpec {
        path_segments: &["sqlx", "query_as"],
        sql_arg_index: 1,
    },
    MacroSpec {
        path_segments: &["sqlx", "query_as_unchecked"],
        sql_arg_index: 1,
    },
    MacroSpec {
        path_segments: &["sqlx", "query_scalar"],
        sql_arg_index: 0,
    },
    MacroSpec {
        path_segments: &["sqlx", "query_scalar_unchecked"],
        sql_arg_index: 0,
    },
];

static RUSQLITE_CALLS: &[CallSpec] = &[
    CallSpec {
        path: "rusqlite::Connection::execute",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Connection::execute_batch",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Connection::prepare",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Connection::prepare_cached",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Connection::query_row",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Connection::query_row_and_then",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Transaction::execute",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Transaction::execute_batch",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Transaction::prepare",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Transaction::prepare_cached",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Transaction::query_row",
        sql_arg_index: 0,
    },
    CallSpec {
        path: "rusqlite::Transaction::query_row_and_then",
        sql_arg_index: 0,
    },
];
