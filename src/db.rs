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
            last_fetched_at TEXT,
            group_name TEXT NOT NULL DEFAULT ''
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
    // 同一个手法补侧栏首页那层分组（DESIGN.md §2.1）。`DEFAULT ''` 而不是 `NULL`：
    // ALTER 给旧行填的就是这个空串，也就是「未分组」，旧库里已经在列的仓库不会变成一团 NULL。
    if !has_column(conn, "repos", "group_name")? {
        conn.execute("ALTER TABLE repos ADD COLUMN group_name TEXT NOT NULL DEFAULT ''", [])?;
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
    // **不是 `INSERT OR REPLACE`**：REPLACE 是删了重插，那一整行没写到的列全被打回默认值——
    // 重新添加一个已在列表里的仓库（同一目录再指一次是常事）会**悄悄把它踢出分组**（`group_name`
    // 清空）、并把 `last_fetched_at` 抹成「从没取过」。那两列是用户的组织信息和新鲜度，
    // 不属于「添加」这件事，所以这里只覆盖 `name`/`added_at`，别的列一个字不碰。
    conn.execute(
        "INSERT OR IGNORE INTO repos (path, name, added_at) VALUES (?1, ?2, ?3)",
        params![path, name, now],
    )?;
    conn.execute(
        "UPDATE repos SET name = ?2, added_at = ?3 WHERE path = ?1",
        params![path, name, now],
    )?;
    let id = conn.query_row("SELECT id FROM repos WHERE path = ?1", params![path], |r| r.get::<_, i64>(0))?;
    Ok(id)
}

/// 把一条仓库放进某个组；`group` 给空串就是「移出组、回未分组」。
/// 没有单独的组表：组的存续完全由「有没有仓库挂着这个名字」决定，
/// 最后一成员移走，这个名字在 `list_groups` 里自然就消失了。
pub fn set_repo_group(conn: &Connection, path: &str, group: &str) -> Result<()> {
    conn.execute("UPDATE repos SET group_name = ?2 WHERE path = ?1", params![path, group])?;
    Ok(())
}

/// 整组改名：一条 UPDATE 挪走全部成员，不需要逐条 `set_repo_group`。
/// 目标名撞上另一个已有组时，这就是一次**合并**（成员并过去），spec 没禁止，行为也不意外。
pub fn rename_group(conn: &Connection, from: &str, to: &str) -> Result<()> {
    conn.execute("UPDATE repos SET group_name = ?2 WHERE group_name = ?1", params![from, to])?;
    Ok(())
}

/// 现有过的组名，按名排序，**不含**空串（未分组不是组）。
pub fn list_groups(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT group_name FROM repos WHERE group_name <> '' ORDER BY group_name",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
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
    let mut stmt = conn.prepare(
        "SELECT id, path, name, added_at, last_fetched_at, group_name FROM repos ORDER BY name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Repo {
            id: r.get(0)?,
            path: r.get(1)?,
            name: r.get(2)?,
            added_at: r.get(3)?,
            last_fetched_at: r.get(4)?,
            group_name: r.get(5)?,
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

    /// 侧栏首页那层分组（§2.1）：**组只有一层、没有组表**，名字完全靠仓库自己挂着。
    /// 一次问全四件事：列出来的是不是排好序的去重名（空串不算组）、整组改名是不是把**全部**
    /// 成员一起挪走、末位成员离组后这个名字是不是自己消失、以及别组的成员一动不许动。
    ///
    /// 反向变异实测：`rename_group` 的 `WHERE group_name = ?1` 换成 `WHERE path = ?1`
    /// → 红在「整组改名漏了 /r/web」那一句（只有第一条挪了，`/r/api` 还在旧组里）；
    /// `list_groups` 去掉 `<> ''` → 红在第一次列名，`an_old_database_gains_the_group_column`
    /// 的最后一句同时红（「未分组」被当成一个组，两处都认得）；
    /// 去掉 `ORDER BY` → 红在第一次列名那一句，实测 `left` 是 `["前端", "back"]`——SQLite 的自然序按插入走，
    /// 所以「排好序」这一半只有那一条断言杀得掉。
    #[test]
    fn groups_are_one_flat_level_named_by_their_members() {
        let (_d, conn) = tmp_db();
        for p in ["/r/web", "/r/api", "/r/other", "/r/seed"] {
            add_repo(&conn, p, p.rsplit('/').next().unwrap()).unwrap();
        }
        set_repo_group(&conn, "/r/web", "前端").unwrap();
        set_repo_group(&conn, "/r/api", "前端").unwrap();
        set_repo_group(&conn, "/r/seed", "back").unwrap();

        assert_eq!(list_groups(&conn).unwrap(), vec!["back", "前端"], "组名该去重、排序，未分组不算一组");
        assert_eq!(list_repos(&conn).unwrap().iter().find(|r| r.path == "/r/other").unwrap().group_name, "");

        rename_group(&conn, "前端", "客户端").unwrap();
        let after: Vec<(String, String)> = list_repos(&conn)
            .unwrap()
            .into_iter()
            .map(|r| (r.path, r.group_name))
            .collect();
        assert!(after.contains(&("/r/web".to_owned(), "客户端".to_owned())), "整组改名漏了 /r/web：{after:?}");
        assert!(after.contains(&("/r/api".to_owned(), "客户端".to_owned())), "整组改名漏了 /r/api：{after:?}");
        assert!(after.contains(&("/r/seed".to_owned(), "back".to_owned())), "别组的成员被挪动了：{after:?}");
        assert!(after.contains(&("/r/other".to_owned(), String::new())), "未分组的成员被卷进来了：{after:?}");

        // 末位成员离组，组名自己消失——没有组表，所以没有残留的空组。
        set_repo_group(&conn, "/r/seed", "").unwrap();
        assert_eq!(list_groups(&conn).unwrap(), vec!["客户端"], "空组该跟着最后一个成员一起走");
    }

    /// 「重新添加同一个目录」是常事（从别处再指一次、或改名后重加），它**不许**是一条重置：
    /// `INSERT OR REPLACE` 会删行重插，`group_name` 被打回空串、`last_fetched_at` 被打回 NULL，
    /// 用户看到的是「仓库自己跳出了分组，新鲜度变成从没取过」。
    ///
    /// 反向变异实测：把 `add_repo` 换回 `INSERT OR REPLACE` → 红在本条第一句（组名丢了）；
    /// 只删掉那句 `UPDATE ... SET name` → 红在「名字要跟着新添加时给的走」。
    #[test]
    fn re_adding_a_repo_keeps_its_group_and_freshness() {
        let (_d, conn) = tmp_db();
        add_repo(&conn, "/r/a", "a").unwrap();
        set_repo_group(&conn, "/r/a", "工作").unwrap();
        mark_fetched(&conn, "/r/a").unwrap();

        add_repo(&conn, "/r/a", "A 改名了").unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 1, "同一个目录不该插出两行");
        assert_eq!(repos[0].group_name, "工作", "重新添加把仓库悄悄踢出了分组");
        assert!(repos[0].last_fetched_at.is_some(), "重新添加把「上次取过」抹了");
        assert_eq!(repos[0].name, "A 改名了", "名字该跟着这次添加给的走");
        assert_eq!(repos[0].id, Some(repos[0].id.unwrap()), "重复添加还得问得出主键（移出组要靠它）");
    }

    /// 换列之前的老库（四列那版）开起来要能读、旧行读作「未分组」，别要求重建数据库。
    /// 和 `an_old_database_gains_the_freshness_column` 同一个手法，只是这一列有 `DEFAULT ''`，
    /// 所以旧行不是 NULL 而是空串——判据分的正是这两者。
    ///
    /// 反向变异实测：删掉 `migrate` 里那句 `ALTER ... group_name` → 红在**第一句** `has_column`
    /// （不是 `list_repos`：列没补上，`SELECT ... group_name` 那条语句根本 prepare 不了，
    /// 但先撞上的是我们自己写的那句断言）；
    /// 把 `DEFAULT ''` 去掉 → 红在 `migrate(...).unwrap()` 那一步，SQLite 直接拒收
    /// `Cannot add a NOT NULL column with default value NULL`——**旧库连开都开不了**，
    /// 比「旧行读成 NULL」严重，这一档 `DEFAULT ''` 不是可选的写法。
    #[test]
    fn an_old_database_gains_the_group_column() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("gitx.db")).unwrap();
        conn.execute(
            "CREATE TABLE repos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                added_at TEXT NOT NULL,
                last_fetched_at TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO repos (path, name, added_at) VALUES ('/r/old', 'old', '2026-01-01T00:00:00+08:00')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert!(has_column(&conn, "repos", "group_name").unwrap(), "老库该被补上这一列");
        let old = list_repos(&conn).unwrap();
        assert_eq!(old[0].path, "/r/old");
        assert_eq!(old[0].group_name, "", "旧行读作「未分组」，不是 NULL、也不是报错");
        assert!(list_groups(&conn).unwrap().is_empty(), "「未分组」不该被列成一个组");
    }
}
