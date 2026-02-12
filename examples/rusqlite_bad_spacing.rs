fn main() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let _ = conn.execute("SELECT  1;", []);
}
