fn main() {
    let _ = sqlx::query_unchecked!("SELECT  1 AS id;");
}
