#[allow(dead_code)]
struct Row {
    id: i64,
}

fn main() {
    let _ = sqlx::query_as!(Row, r#"SELECT  1 AS "id: _";"#);
}
