use chrono::{Local, Utc};
use rusqlite::{params, Connection};

use crate::error::Result;
use crate::models::{RecentOpen, Repo};

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
    // 首屏「最近打开」。路径直接当主键：`repos.path` 是 UNIQUE 的，同一仓库重复打开
    // 只会覆盖这一行，不会攒出一串历史。
    conn.execute(
        "CREATE TABLE IF NOT EXISTS recent_open (
            path TEXT PRIMARY KEY,
            opened_at INTEGER NOT NULL
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
    // 摘掉书签就要同时退出「最近打开」，否则首屏会留一行点开必定失败的死路径。
    conn.execute("DELETE FROM recent_open WHERE path = ?1", params![path])?;
    Ok(())
}

/// 记下「这个仓库刚刚被打开过」。首屏的骨架全靠这一条，所以它必须便宜到可以在
/// `Gitx::open()` 里每次都写一次。
pub fn touch_open(conn: &Connection, path: &str) -> Result<()> {
    touch_open_at(conn, path, Utc::now().timestamp_millis())
}

/// `touch_open` 的时间可注入版。判据要钉的是**排序**，而 `Utc::now()` 在同一毫秒内
/// 连开两个仓库会给出一样的值，`ORDER BY opened_at DESC` 就成了并列——那样测出来的
/// 顺序是巧合不是规则。
fn touch_open_at(conn: &Connection, path: &str, opened_at: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO recent_open (path, opened_at) VALUES (?1, ?2)",
        params![path, opened_at],
    )?;
    Ok(())
}

/// 最近打开的仓库，最新在前。`limit` 只是首屏的显示上限——读得再多也用不上，
/// 而每多一行就多一次要发力的聚合。
pub fn list_recent(conn: &Connection, limit: u32) -> Result<Vec<RecentOpen>> {
    let mut stmt = conn.prepare(
        "SELECT path, opened_at FROM recent_open ORDER BY opened_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(RecentOpen {
            path: r.get(0)?,
            opened_at: r.get(1)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
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

    /// 首屏骨架的唯一来源：最近打开，最新在前，超出上限的丢掉。
    #[test]
    fn recent_open_is_newest_first_and_capped() {
        let (_d, conn) = tmp_db();
        touch_open_at(&conn, "/r/old", 1000).unwrap();
        touch_open_at(&conn, "/r/mid", 2000).unwrap();
        touch_open_at(&conn, "/r/new", 3000).unwrap();

        let all = list_recent(&conn, 10).unwrap();
        assert_eq!(
            all.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["/r/new", "/r/mid", "/r/old"],
            "最新一次打开要排在最前"
        );
        assert_eq!(all[0].opened_at, 3000);

        // 同一个仓库再开一次只挪时间，不新增行。
        touch_open_at(&conn, "/r/old", 4000).unwrap();
        let again = list_recent(&conn, 10).unwrap();
        assert_eq!(again.len(), 3, "重复打开不能攒出一串历史");
        assert_eq!(again[0].path, "/r/old");

        assert_eq!(list_recent(&conn, 2).unwrap().len(), 2, "上限要生效");
        assert_eq!(list_recent(&conn, 0).unwrap().len(), 0);
    }

    /// 摘掉书签的仓库必须同时退出首屏 —— 留着就是一行点开必定失败的死路径。
    #[test]
    fn removing_a_repo_drops_it_from_recents() {
        let (_d, conn) = tmp_db();
        add_repo(&conn, "/r/a", "a").unwrap();
        add_repo(&conn, "/repo/中文名 带空格", "中文名 带空格").unwrap();
        touch_open(&conn, "/r/a").unwrap();
        touch_open(&conn, "/repo/中文名 带空格").unwrap();

        remove_repo(&conn, "/r/a").unwrap();
        let left = list_recent(&conn, 10).unwrap();
        assert_eq!(left.len(), 1, "被摘掉的仓库不能还留在最近打开里");
        // 中文与带空格的路径要原样往返（`core.quotepath` 那条老事故的同源风险）。
        assert_eq!(left[0].path, "/repo/中文名 带空格");
    }

    /// 老库没有 `recent_open` 这张表，也要能开、能问出「一条都没有」而不是报错。
    #[test]
    fn an_old_database_gets_the_recents_table() {
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
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert!(list_recent(&conn, 10).unwrap().is_empty());

        touch_open(&conn, "/r/a").unwrap();
        assert_eq!(list_recent(&conn, 10).unwrap()[0].path, "/r/a");
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
