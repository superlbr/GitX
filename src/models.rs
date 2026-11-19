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
///
/// `rev` 是这次操作**正卡在那条提交上**的那条号，来自同一批哨兵文件的第一行
/// （合并 = `MERGE_HEAD`，遴选 = `CHERRY_PICK_HEAD`，变基 = `REBASE_HEAD`）。
/// 它是历史列表能不能看见这场冲突的**唯一**依据：那几条提交只从 `MERGE_HEAD` 可达，
/// 而 `git log` 只走 HEAD，于是冲突时「正在被合进来的那批」一条都不出现。
/// `None` = 文件读不出或形状不对，界面按「没有正在进行的操作」处理，不许把
/// 一个垃圾串塞进 `git log` 的位置参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    pub kind: String,
    pub step: Option<u32>,
    pub total: Option<u32>,
    pub rev: Option<String>,
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
    /// head 那一版提交的 sha。列表端点**给**这一格，所以「按 sha 取检查」不必先等详情
    /// 回来；它同时是缓存键的一半：作者 force-push 之后 sha 变了，旧结果自动作废。
    pub head_sha: String,
    pub draft: bool,
    pub url: String,
    pub updated: String,
    /// 平台自己算出来的「现在能不能合」，各家字段名不同、取值也不同，这里保留**原始值**
    /// （GitHub：`clean` / `dirty` / `blocked` / `unstable` / `behind` / `unknown` …）。
    /// 看板那一行状态文案由 `provider::pr_status` 从它翻译出来；空串 = 这家没给。
    ///
    /// 2026-10-31 实测：**GitHub 的列表端点没有这一格**，只有单条 PR 的详情端点给。
    /// 所以列表回来时它是空串，要等「按号取详情」那一次补上——在那之前看板不能编状态。
    pub mergeable_state: String,
    /// head 上的检查汇总。和详情一样，列表不给，由「按可见行」那一路补进来。
    pub ci: Ci,
}

impl PullRequest {
    /// 蓝点用的签名：`state | head_sha | ci 档 | mergeable_state`。
    ///
    /// 为什么不只看 `updated`：**检查跑完不会把 `updated_at` 顶走**（那是 PR 本体最后
    /// 被改的时刻），而「CI 从在跑到挂了三个」恰恰是人最想看见的动静；反过来作者
    /// force-push 换了 sha，人却会当成「没动」。这几格里任何一格变都算动。
    /// 它是指纹不是文案，格式一变旧账本自然作废——所以改它要连 `seen_key` 一起换档。
    pub fn signature(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.state,
            self.head_sha,
            self.ci.tag(),
            self.mergeable_state
        )
    }
}

/// 一条归一后的检查。各家字段名不同（GitHub 的 check run、将来的流水线…），这里只留
/// **名字**和**结果**——名字是跨 PR 交叉印证「这个 job 在别处也挂吗」唯一的凭据，
/// 没有名字就什么也印证不了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub outcome: CheckOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    /// 过了，或者被跳过——两者都不挡路。
    Pass,
    Fail,
    /// 还没跑完。
    Pending,
}

/// head 那一版提交上的 CI 汇总。**「没去取」和「取到了但没有」是两件事**，
/// 所以它们是两档而不是同一个空值：前者过一会儿会变，后者不会。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ci {
    /// 还没去取：这一行刚画出来，或者这家 provider 没声明 `CiStatus`。
    NotFetched,
    /// 取过但读不懂 / 取失败。说实话，不猜。
    Unknown,
    /// 取到了，这一版提交上一个检查都没有。
    NoChecks,
    Running { passed: u32, failed: u32, pending: u32 },
    /// 全跑完。`failed` 是挂掉的那些**名字**，条数就是它的长度。
    Failed { failed: Vec<String> },
    Passed { total: u32 },
}

impl Ci {
    /// 签名里那一小截：只到「哪一档 + 挂了几条」为止，不带名字列表——不然检查的
    /// 返回顺序换一次就把蓝点亮一次。
    pub fn tag(&self) -> String {
        match self {
            Ci::NotFetched => "-".to_owned(),
            Ci::Unknown => "?".to_owned(),
            Ci::NoChecks => "none".to_owned(),
            Ci::Running { failed, pending, .. } => format!("run{pending}/fail{failed}"),
            Ci::Failed { failed } => format!("fail{}", failed.len()),
            Ci::Passed { total } => format!("pass{total}"),
        }
    }
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
    /// 进行中那次操作卡住的提交（`Op::rev` 的另一层投影，正常仓库是 `None`）。
    /// 历史列表把它作为**额外一条 rev** 交给 `git log`，否则那场冲突正卡着的提交
    /// 只从 `MERGE_HEAD` 可达、在只走 HEAD 的日志里一条都不出现。
    pub op_rev: Option<String>,
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
