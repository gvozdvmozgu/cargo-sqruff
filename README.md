# cargo-sqruff

Lint your SQL queries directly within your Rust code. A cargo-sqruff library powered by sqruff.

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
