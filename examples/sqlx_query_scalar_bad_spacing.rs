fn main() {
    let _ = sqlx::query_scalar::<sqlx::Sqlite, i64>("SELECT  1;");
}
