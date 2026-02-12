fn main() {
    let _ = sqlx::query::<sqlx::Sqlite>("SELECT  1;");
}
