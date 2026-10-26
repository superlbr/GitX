use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Unmerged,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: Option<i64>,
    pub path: String,
    pub name: String,
    pub added_at: Option<String>,
    /// When a fetch last completed, so the list can say how current it is.
    pub last_fetched_at: Option<String>,
    /// 侧栏首页「分组」那一层用。**组只有一层，不嵌套**（DESIGN.md §2.1）。
    /// 空串读作「未分组」——spec 选的是「列总在有值，缺省是空串」而不是 `NULL`，
    /// 这样 `ORDER BY group_name` 和 `WHERE group_name <> ''` 都不用处理三值逻辑。
    /// 改名/移出组都是对这一列的一条 UPDATE，没有单独的组表。
    pub group_name: String,
}

/// 首屏「最近打开」的一行。`opened_at` 是 Unix 毫秒而不是 `added_at` 那种 RFC3339 串：
/// 这一列要 `ORDER BY`，带本地偏移的串走的是文本序，跨时区或过夏令时就排错。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentOpen {
    pub path: String,
    pub opened_at: i64,
}

/// 进行中的合并 / 遴选 / 变基。
///
/// `step`/`total` 只有变基给得出（读 git-dir 里的 `msgnum`/`end`），合并与遴选是
/// `None` —— 界面必须能把「第 3 / 12 步」和「正在合并」分开说，把 `None` 显示成
/// 「0 / 0」是谎报进度。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    pub kind: String,
    pub step: Option<u32>,
    pub total: Option<u32>,
}

/// 首屏「最近」面板里一个仓库的那份摘要。字段按「角标要画什么」选，不按「git 能给什么」：
/// `ahead`/`behind` 直接来自 `status --branch` 的 `# branch.ab`，`dirty`/`stash` 是两枚
/// 计数角标，`mine` 是活动流的那几行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub repo: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: u32,
    pub stash: u32,
    pub op: Option<Op>,
    /// 时间窗内属于本机身份的提交，新的在前。
    pub mine: Vec<Commit>,
}

/// 首屏聚合的一个待查仓库。`emails` 由调用方备好并缓存，聚合自己**不为身份多 fork**。
#[derive(Debug, Clone)]
pub struct RepoQuery {
    pub repo: String,
    pub emails: Vec<String>,
}

/// 聚合回来的一行：仓库名一定在，成没成看 `result` —— 一个仓库读不了（被删、在移动硬盘上、
/// 权限不对）不能让整块首屏一起消失。
#[derive(Debug)]
pub struct RepoOutcome {
    pub repo: String,
    pub result: crate::Result<Activity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub old_path: Option<String>,
    pub index_status: ChangeStatus,
    pub worktree_status: ChangeStatus,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub subject: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub parents: Vec<String>,
}

/// A provider-side pull request, normalised across GitHub / Gitee field names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: String,
    pub head: String,
    pub base: String,
    pub draft: bool,
    pub url: String,
    pub updated: String,
}

/// The newest published release of the app's own repo, as read from the code
/// host. `url` is the page a human downloads from, `notes` the release body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub date: String,
    pub assets: Vec<Asset>,
}

/// One downloadable artifact attached to a release.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// Git LFS as seen from one repository. `files` is empty when git-lfs is not
/// installed, which is exactly the case the UI has to spell out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfsInfo {
    pub installed: bool,
    pub version: String,
    pub files: Vec<String>,
}

/// One commit in a single file's history, tagged with what that commit did to
/// the file — this is what lets the file-log list read as a story of the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLogEntry {
    pub commit: Commit,
    pub change: ChangeStatus,
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub target: String,
    pub annotated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
    pub push_url: Option<String>,
}

/// A remote-tracking ref, e.g. `origin/main` — where the repository on the other
/// end actually is. Distinct from `Branch`/`Tag` because the history page paints it
/// as its own kind of chip: "my branch" and "what the world has" are two facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRef {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stash {
    pub index: u32,
    /// 这条贮藏自己的提交号。选中它看内容时按 oid 取，不按 `stash@{n}`——列表随时会
    /// 因为新存/弹出而重排，下标只在这一份快照里有效。
    pub oid: String,
    /// 用户写的那句说明。git 的 `On <分支>: ` / `WIP on <分支>: ` 包装在核心层就剥掉了，
    /// 侧栏不再显示 `stash@{n}:`。
    pub message: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub branch: String,
    /// HEAD 不挂在任何分支上时 = 那条提交的完整号；正常在分支上 = `None`。
    /// `git status --porcelain=v2 --branch` 在分离时把 `# branch.head` 写成字面量
    /// `(detached)`、把 `# branch.oid` 写成那条提交的号。于是「分离」有两处要说清：
    /// `branch` 这时候**不是分支名**（所以留空，界面不许把它当名字显示），
    /// 而那条号在 `detached_at` 里。
    ///
    /// `detached_at` 只收**完整号**（≥40 位十六进制）。一条提交都没有的仓库那里写的是
    /// 占位符（本机 git 2.54 实测 `(initial)`，旧版写 `(none)`），形状这一挡把
    /// 「还没提交」和「HEAD 脱离了分支」分开——不然新仓库一打开就顶着一行
    /// 「分离 HEAD · (initial)」。
    pub detached_at: Option<String>,
    /// What this branch tracks, if anything. Without it the two counts below
    /// are not "in sync" — they are "never compared to a remote at all",
    /// which the interface has to be able to tell apart.
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<FileStatus>,
    pub stash_count: u32,
    pub repo_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: ChangeStatus,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub commit: String,
    pub short_id: String,
    pub author: String,
    pub date: String,
    pub line_no: u32,
    pub content: String,
}
