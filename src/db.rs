use chrono::Local;
use rusqlite::{params, Connection};

use crate::error::Result;
use crate::models::Repo;

/// Open (or create) the local SQLite database and run migrations.
pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS repos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            added_at TEXT NOT NULL,
            last_fetched_at TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;
    // Databases created before freshness was tracked have the old four-column
    // shape and `CREATE TABLE IF NOT EXISTS` leaves them alone, so the missing
    // column is added here rather than by demanding a rebuild.
    if !has_column(conn, "repos", "last_fetched_at")? {
        conn.execute("ALTER TABLE repos ADD COLUMN last_fetched_at TEXT", [])?;
    }
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let found = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .flatten()
        .any(|name| name == column);
    Ok(found)
}

pub fn add_repo(conn: &Connection, path: &str, name: &str) -> Result<i64> {
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO repos (path, name, added_at) VALUES (?1, ?2, ?3)",
        params![path, name, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Remember that this repo's remote refs were just refreshed. Only a fetch that
/// actually completed should call this — a failed one leaves the age honest.
pub fn mark_fetched(conn: &Connection, path: &str) -> Result<()> {
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE repos SET last_fetched_at = ?2 WHERE path = ?1",
        params![path, now],
    )?;
    Ok(())
}

pub fn list_repos(conn: &Connection) -> Result<Vec<Repo>> {
    let mut stmt =
        conn.prepare("SELECT id, path, name, added_at, last_fetched_at FROM repos ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok(Repo {
            id: r.get(0)?,
            path: r.get(1)?,
            name: r.get(2)?,
            added_at: r.get(3)?,
            last_fetched_at: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn remove_repo(conn: &Connection, path: &str) -> Result<()> {
    conn.execute("DELETE FROM repos WHERE path = ?1", params![path])?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let r = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    );
    match r {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gitx.db");
        let conn = open(path.to_str().unwrap()).unwrap();
        (dir, conn)
    }

    #[test]
    fn repo_crud() {
        let (_d, conn) = tmp_db();
        add_repo(&conn, "/repo/a", "a").unwrap();
        add_repo(&conn, "/repo/b", "b").unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "a");

        remove_repo(&conn, "/repo/a").unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "b");
    }

    #[test]
    fn settings_roundtrip() {
        let (_d, conn) = tmp_db();
        set_setting(&conn, "theme", "dark").unwrap();
        assert_eq!(get_setting(&conn, "theme").unwrap(), Some("dark".into()));
        assert_eq!(get_setting(&conn, "missing").unwrap(), None);
    }

    /// A database written before freshness existed must still open, and its old
    /// rows must read as "never fetched" rather than failing the whole list.
    #[test]
    fn an_old_database_gains_the_freshness_column() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("gitx.db")).unwrap();
        conn.execute(
            "CREATE TABLE repos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                added_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO repos (path, name, added_at) VALUES ('/r', 'r', '2026-01-01T00:00:00+08:00')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert_eq!(list_repos(&conn).unwrap()[0].last_fetched_at, None);

        mark_fetched(&conn, "/r").unwrap();
        assert!(
            list_repos(&conn).unwrap()[0]
                .last_fetched_at
                .as_deref()
                .is_some_and(|s| !s.is_empty()),
            "fetch 成功后要留下时间戳"
        );
        // A path that is not in the list is a no-op, not an error.
        mark_fetched(&conn, "/nowhere").unwrap();
    }
}
