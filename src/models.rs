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
