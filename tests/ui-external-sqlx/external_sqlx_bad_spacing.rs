// aux-build: sqlx.rs

extern crate sqlx;

fn main() {
    sqlx::query("SELECT  1;");
}
