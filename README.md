# cargo-sqruff

Lint your SQL queries directly within your Rust code. A cargo-sqruff library powered by sqruff.

Currently supports string literal queries passed to `sqlx` query functions, inline `sqlx` query macros, and common `rusqlite` query methods.

## Supported SQL call sites

cargo-sqruff lints string literal SQL passed to these APIs:

- `sqlx` functions: `query`, `query_as`, `query_as_with`, `query_scalar`, `query_scalar_with`, `query_with`, and `raw_sql`
- `sqlx` macros: `query!`, `query_unchecked!`, `query_as!`, `query_as_unchecked!`, `query_scalar!`, and `query_scalar_unchecked!`
- `rusqlite::Connection` methods: `execute`, `execute_batch`, `prepare`, `prepare_cached`, `query_row`, and `query_row_and_then`
- `rusqlite::Transaction` methods: `execute`, `execute_batch`, `prepare`, `prepare_cached`, `query_row`, and `query_row_and_then`

The lint resolves calls through the actual external crates, so these APIs are linted even when they come from your project's `sqlx` or `rusqlite` dependency rather than from this repository's examples.

## Configuration

By default, cargo-sqruff uses sqruff's default configuration. You can override it from your package's `Cargo.toml` with `[package.metadata.sqruff]`.

```toml
[package.metadata.sqruff]
dialect = "postgres"
exclude_rules = ["LT01", "LT12"]
```

Top-level values under `[package.metadata.sqruff]` are applied to sqruff's core config. For example, the configuration above disables `LT01` and `LT12` for SQL literals linted by cargo-sqruff.

Nested TOML tables map to sqruff config sections:

```toml
[package.metadata.sqruff.indentation]
tab_space_size = 2
indented_joins = true

[package.metadata.sqruff.layout.type.comma]
spacing_before = "touch"
line_position = "trailing"
```

Supported metadata value types are strings, integers, floats, booleans, and arrays of those scalar values. Arrays are passed to sqruff as comma-separated values, so `exclude_rules = ["LT01", "LT12"]` is equivalent to `exclude_rules = "LT01,LT12"`. Datetimes, nested arrays, and tables inside arrays are rejected.

## Quick start

1. Install cargo-dylint and dylint-link:

    ```sh
    cargo install cargo-dylint dylint-link
    ```

2. Run `cargo-dylint`:

    ```sh
    $ DYLINT_LIBRARY_PATH=cargo-sqruff/target/release/ cargo dylint --all
    Checking with toolchain `nightly-2025-02-20-x86_64-unknown-linux-gnu`
    warning: [LT01]: Expected only single space before "1". Found "  ".
    --> src/main.rs:2:48
    |
    2 |     let _ = sqlx::query::<sqlx::Sqlite>("SELECT  1;");
    |                                                  ^^ Expected only single space before "1". Found "  ".
    |
    = note: `#[warn(cargo_sqruff)]` on by default

    warning: SQL query contains violations
    |
    help: consider using `sqruff` to fix this
    |
    2 -     let _= sqlx::query::<sqlx::Sqlite>("SELECT  1;");
    2 +     let_ = sqlx::query::<sqlx::Sqlite>("SELECT 1;");
    |

    warning: `sql` (bin "sql") generated 2 warnings (run `cargo fix --bin "sql"` to apply 1 suggestion)
        Finished `release` profile target(s) in 0.05s

    ```

3. VS Code integration

    ```json
    {
        "rust-analyzer.check.overrideCommand": [
            "cargo",
            "dylint",
            "--all",
            "--",
            "--all-targets",
            "--message-format=json"
        ],
        "rust-analyzer.cargo.extraEnv": {
            "DYLINT_LIBRARY_PATH": "cargo-sqruff/target/release/"
        }
    }
    ```
