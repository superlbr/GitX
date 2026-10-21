//! （拆分自 main.rs；见 PROCESS.md「拆分 main.rs」。）

use gitx::models::{
    BlameLine, Branch, Commit, DiffEntry, FileLogEntry, Remote, RemoteRef, Repo, RepoStatus, Stash, Tag,
};
use gitx::rusqlite::Connection;
use std::sync::mpsc::{Receiver, Sender};

use crate::ui::graph::*;
use crate::ui::diffmodel::*;

/// 日志里选中的行。虚拟行「Uncommitted changes」就是工作区页——SourceTree 只有一页。
#[derive(Clone, PartialEq)]
pub(crate) enum Sel {
    Uncommitted,
    Commit(usize),
}

/// 一次差异请求。带 `path` 是单文件，不带是整份。
#[derive(Clone, PartialEq)]
pub(crate) enum Target {
    Working { staged: bool, path: Option<String>, untracked: bool },
    Commit { oid: String, path: Option<String> },
}

/// 文件溯源面板的两个页签。
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum HistTab {
    /// 这个文件被哪些提交改过；右侧看选中那条的改动或全文。
    History,
    /// 逐行归属，`git blame` 的直读。
    Blame,
}

/// 历史页签右侧看什么：选中那条提交的差异，或文件在那个版本的全文。
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum HistView {
    Diff,
    Full,
}

/// 「文件历史」浮层。**打开那一刻的路径和序号钉在这里**：切仓库、重开面板之后，
/// 路上回来的旧回复对不上 `seq`/`path` 就丢掉，不能落到新面板上。
pub(crate) struct FileHistory {
    pub(crate) path: String,
    pub(crate) seq: u64,
    pub(crate) entries: Vec<FileLogEntry>,
    pub(crate) sel: usize,
    pub(crate) diff: Vec<DiffLine>,
    pub(crate) blame: Vec<BlameLine>,
    pub(crate) full: Vec<String>,
    /// `full` 是选中第几条的全文；换行之后旧全文不认。
    pub(crate) full_for: Option<usize>,
    pub(crate) tab: HistTab,
    pub(crate) view: HistView,
    pub(crate) log_loading: bool,
    pub(crate) patch_loading: bool,
    pub(crate) blame_loading: bool,
    pub(crate) full_loading: bool,
}

impl FileHistory {
    pub(crate) fn new(path: String, seq: u64) -> Self {
        Self {
            path,
            seq,
            entries: Vec::new(),
            sel: 0,
            diff: Vec::new(),
            blame: Vec::new(),
            full: Vec::new(),
            full_for: None,
            tab: HistTab::History,
            view: HistView::Diff,
            log_loading: true,
            patch_loading: false,
            blame_loading: false,
            full_loading: false,
        }
    }
}

pub(crate) enum Action {
    Stage(Vec<String>),
    Unstage(Vec<String>),
    Discard(Vec<String>),
    StageAll,
    UnstageAll,
    /// 只动一块。`path` 从当前 `target` 里带过来（按钮只在单文件差异上出现），
    /// `hunk` 就是差异区那句「块 N」的 N。
    StageHunk { path: String, hunk: usize },
    UnstageHunk { path: String, hunk: usize },
    DiscardHunk { path: String, hunk: usize },
    /// 提交**暂存区**（不带路径 = `git commit -m` 的默认行为），和旧 React 版 `store.commit`
    /// 传的空 `paths` 一致。
    Commit(String),
    Checkout(String),
    Fetch,
    /// 只问一个远端：侧栏「远端」那一栏右键用的，`Fetch` 是全抓。
    FetchRemote(String),
    Pull,
    Push,
    /// 新建分支。`start` 是起点（`None` = 从当前 HEAD 长出来），`checkout` = 建完立刻切过去。
    CreateBranch { name: String, start: Option<String>, checkout: bool },
    /// `-d`（不是 `-D`）：没合并过的分支让 git 自己拒掉，错误原样落到 `note` 里。
    /// 不做「静默强删」——那正是用户点错一枚菜单项时会丢工作的地方。
    DeleteBranch(String),
    Merge(String),
    CreateTag { name: String, target: String },
    DeleteTag(String),
    StashSave { message: String, include_untracked: bool },
    StashApply(u32),
    StashPop(u32),
    StashDrop(u32),
    Ignore(String),
    /// 把历史里的某一版拉回工作区。`git checkout <oid> -- <path>` 会连索引一起改，
    /// 所以这条会覆盖工作区和暂存区里这个文件的现状——`land` 里要过确认。
    RestoreFile { oid: String, path: String },
    /// 冲突文件用「我的」那一侧解决（`git checkout --ours` + `git add`）。
    ResolveOurs(String),
    /// 冲突文件用「他们的」那一侧解决。
    ResolveTheirs(String),
    /// 手工改完之后说「这个文件解决了」：把工作区现状直接暂存。
    MarkResolved(String),
    /// 把冲突标记放回去（`git checkout -m`），文件重新回到未解决那一组。
    MarkUnresolved(String),
    /// 继续合并 / 遴选 / 变基。`state` 是打开这一刻的快照状态，只用来选文案。
    ContinueOp { state: String },
    /// 终止合并 / 遴选 / 变基。会丢东西，`land` 里要过确认。
    AbortOp { state: String },
    /// 在浏览器里打开这个远端的仓库主页。`String` 是远端名（工具条那枚固定 `origin`，
    /// 侧栏「远端」的菜单带的是被右键那一条的名字）。
    BrowseRemote(String),
    /// 克隆。**唯一一条不需要「先有一个仓库」的动作**，所以它在 `act` 里走在 `repo`
    /// 那道守卫之前，落地也不走 `Msg::Loaded` 而是自己那条 `Msg::Cloned`。
    Clone { url: String, dest: String },
}

/// 一张模态卡：凡是要「先给个名字 / 先挑一个」的动作都走它（建分支、贮藏、打标签、合并选分支）。
///
/// 为什么不用 `egui::Window`：它自带标题栏、阴影和拖动条，跟参考图那种 macOS 表单对不上；
/// 而且它会跟面板各占一层，卡片后面的行照样点得着。这里画的是「铺满整屏的遮罩 + 居中的一张卡」，
/// 遮罩自己占一格可点矩形，所以点卡片外面只会把卡关掉。
pub(crate) struct Modal {
    pub(crate) title: &'static str,
    /// 标题下面那行说明（「打在：修掉 xxx」）。没有就 `None`。
    pub(crate) detail: Option<String>,
    /// 文本框里正在打的字。`None` = 这张卡没有输入框，只有一排现成的选择。
    pub(crate) text: Option<String>,
    pub(crate) form: Form,
}

/// 卡要办的事。**里面带的数据是打开那一刻从快照算好的**：卡开着的时候后台快照可能刷新，
/// 动作不该跟着漂（「在这条提交上打标签」不能因为日志翻页就换了一条提交）。
pub(crate) enum Form {
    /// 从某条提交长一条分支出来。`start = None` = 当前 HEAD（工具条那枚「分支」就是这个）。
    NewBranch { start: Option<String> },
    /// 把当前改动贮藏起来，旁边一枚「包含未跟踪文件」。
    StashSave { include_untracked: bool },
    /// 在选中的那条提交上打一个轻量标签。
    NewTag { oid: String },
    /// 从本地分支里挑一条合并进当前分支。
    Merge { branches: Vec<String> },
    /// 克隆。源地址走 `Modal.text`（第一格），目标目录放在这里（第二格）：
    /// 卡上只有一格文本框是 `Modal.text` 的既有约定，第二格由表单自己带。
    Clone { dest: String },
    /// 会丢东西的动作先问一句：`then` 已经算好，按下「确定」才取出来发出去。
    Confirm { then: Option<Action> },
}

impl Modal {
    pub(crate) fn new_branch() -> Self {
        Self::new_branch_at(None)
    }

    /// `start` = 从哪条提交长出去；从分支行/日志行的菜单里进来时带的是那条的 oid。
    pub(crate) fn new_branch_at(start: Option<String>) -> Self {
        Modal {
            title: "新建分支",
            detail: start.as_ref().map(|s| format!("从 {s} 长出去")),
            text: Some(String::new()),
            form: Form::NewBranch { start },
        }
    }

    pub(crate) fn stash_save() -> Self {
        Modal {
            title: "贮藏改动",
            detail: Some("贮藏会把工作区和暂存区都收起来，之后回到当前分支".to_owned()),
            text: Some(String::new()),
            form: Form::StashSave { include_untracked: false },
        }
    }

    pub(crate) fn new_tag(oid: String, subject: String) -> Self {
        Modal {
            title: "新建标签",
            detail: Some(format!("打在：{subject}")),
            text: Some(String::new()),
            form: Form::NewTag { oid },
        }
    }

    pub(crate) fn merge(branches: Vec<String>) -> Self {
        Modal { title: "合并到当前分支", detail: None, text: None, form: Form::Merge { branches } }
    }

    /// 克隆那张卡：第一格是源地址（`text`），第二格是目标目录（表单自己带）。
    /// 目标目录留空 = 主按钮按着不让你点，而不是替你猜一个 `~/xxx`。
    pub(crate) fn new_clone() -> Self {
        Modal {
            title: "克隆仓库",
            detail: Some("填相对路径时按主目录算".to_owned()),
            text: Some(String::new()),
            form: Form::Clone { dest: String::new() },
        }
    }

    pub(crate) fn confirm(title: &'static str, detail: String, then: Action) -> Self {
        Modal { title, detail: Some(detail), text: None, form: Form::Confirm { then: Some(then) } }
    }

    /// 文本框的占位文案，也是「这一格该填什么」的唯一说法。
    pub(crate) fn hint(&self) -> &'static str {
        match self.form {
            Form::NewBranch { .. } => "分支名",
            Form::StashSave { .. } => "贮藏说明（可留空）",
            Form::NewTag { .. } => "标签名",
            Form::Clone { .. } => "源仓库地址（https://… 或 /路径）",
            Form::Merge { .. } | Form::Confirm { .. } => "",
        }
    }

    /// 卡中间有几行正文。卡片高度由它算 —— 居中要在布局之前定位置，问 egui 要「自然高度」
    /// 得等布局跑完，所以这几行的账我们自己记。
    pub(crate) fn rows(&self) -> usize {
        let mut n = usize::from(self.detail.is_some());
        match &self.form {
            Form::Merge { branches } => n += branches.len(),
            Form::StashSave { .. } | Form::Clone { .. } => n += 1,
            _ => {}
        }
        if self.text.is_some() {
            n += 1;
        }
        n
    }

    /// 这一张卡的「主按钮」文案（Enter 走的就是它）。
    pub(crate) fn primary(&self) -> &'static str {
        match self.form {
            Form::NewBranch { .. } => "创建并切换",
            Form::StashSave { .. } => "贮藏",
            Form::NewTag { .. } => "创建",
            Form::Clone { .. } => "克隆",
            Form::Merge { .. } => "",
            Form::Confirm { .. } => "确定",
        }
    }
}

/// 描述列前面那枚药丸画的是什么东西。分支（本地与远程跟踪）在参考图里是**同一种**浅蓝药丸，
/// 只有名字不同；「超前 N 个版本」是紧贴在分支药丸右边、连成一颗胶囊的深藏青药丸。
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum RefKind {
    Branch,
    Remote,
    Tag,
    Ahead,
}

/// 一枚药丸：`label` 是**不含前缀图标**的文字（图标由 `kind` 决定，见 `pill_glyph`）。
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct RefBadge {
    pub(crate) label: String,
    pub(crate) kind: RefKind,
}

pub(crate) struct Snapshot {
    pub(crate) status: RepoStatus,
    pub(crate) commits: Vec<Commit>,
    pub(crate) branches: Vec<Branch>,
    pub(crate) tags: Vec<Tag>,
    pub(crate) remotes: Vec<Remote>,
    /// 远程跟踪指针（`origin/main` 那一类）。**留着**是因为下一页接上来时要把整列药丸重算，
    /// 而 `ref_badges` 没有它就认不出「超前/落后」。一份仓库也就几条，不值一次重取。
    pub(crate) remote_refs: Vec<RemoteRef>,
    pub(crate) stashes: Vec<Stash>,
    /// 与 `commits` 一一对应的图序列行。**在 worker 线程上排好**，帧里只管画。
    pub(crate) graph: Vec<GraphRow>,
    /// 与 `commits` 一一对应的药丸列表。同样在 worker 上算：oid 相等判断要跑
    /// 「提交数 × 引用数」遍，帧里每帧重算是白给的开销（判据：`the_frame_never_matches_refs`）。
    pub(crate) badges: Vec<Vec<RefBadge>>,
    /// 与 `commits` 一一对应的日期文案（「今天 12:27」这种）。参考图的日期列不是 ISO 串，
    /// 而且**当天**的提交只给时间；换算要拿「现在」当参照，那份「现在」就是 `taken_at`。
    pub(crate) dates: Vec<String>,
    /// 这份快照拍下的时刻。日期文案与虚拟行那个「今天 HH:MM」都以它为基准，
    /// 所以同一份快照里刷多少帧，日期都不会自己跳。
    pub(crate) taken_at: chrono::DateTime<chrono::Local>,
}

pub(crate) enum Msg {
    Loaded { repo: String, snap: Box<Snapshot> },
    /// 日志的下一页。带 `seq`：切仓库/换排序之后旧页作废，不能往新列表尾巴上接。
    More { repo: String, seq: u64, commits: Vec<Commit> },
    CommitFiles { repo: String, index: usize, files: Vec<DiffEntry> },
    Patch { repo: String, seq: u64, label: String, lines: Vec<DiffLine> },
    /// 文件历史列表（`git log --follow`）。空列表是合法结果：未跟踪文件就是没有历史。
    FileLog { repo: String, seq: u64, path: String, entries: Vec<FileLogEntry> },
    /// 历史面板里选中那条提交对这个文件的补丁。
    HistoryPatch { repo: String, seq: u64, path: String, index: usize, lines: Vec<DiffLine> },
    HistoryText { repo: String, seq: u64, path: String, index: usize, text: String },
    HistoryBlame { repo: String, seq: u64, path: String, lines: Vec<BlameLine> },
    Failed(String),
    /// 克隆回来的只有「成了在哪」或者「为什么没成」，没有快照——成了之后由 `add_repo`
    /// 自己去开那个仓库（它会再发一次 `Msg::Loaded`）。
    Cloned(std::result::Result<String, String>),
}

pub struct Gitx {
    pub(crate) conn: Option<Connection>,
    pub(crate) repos: Vec<Repo>,
    /// 当前仓库路径，取自 `repos` 里的 `path`。
    pub(crate) repo: Option<String>,
    pub(crate) snap: Option<Snapshot>,
    pub(crate) sel: Sel,
    pub(crate) commit_files: Vec<DiffEntry>,
    pub(crate) file: Option<String>,
    /// 差异已经在工作线程排好版（行号、类型），帧里只管画。
    pub(crate) diff: Vec<DiffLine>,
    pub(crate) patch_label: String,
    /// 差异请求序号：快速连点两个文件时，只有最新那次的回复能落地。
    pub(crate) seq: u64,
    pub(crate) busy: bool,
    pub(crate) note: Option<String>,
    pub(crate) log_filter: String,
    pub(crate) side_filter: String,
    /// 日志排序方式，即筛选条上那枚「层级顺序」：`false` = 日期序，`true` = 拓扑序。
    pub(crate) topo: bool,
    /// 日志范围：`None` = 不给 git 传 rev（就走 HEAD 那条线），`Some(分支名)` = 只看那条分支。
    /// 「所有分支」那枚弹出按钮改的就是它。
    pub(crate) log_rev: Option<String>,
    /// 「显示远程分支」：只管日志行上**远程药丸画不画**，不动取列表的参数。
    pub(crate) show_remote: bool,
    /// 日志还有下一页吗。上一次取回不满一页就是到底了，别再问。
    pub(crate) log_more: bool,
    /// 下一页在路上。不立这个牌子，滚到底的那几帧会连着发好几遍同一个请求。
    pub(crate) log_more_busy: bool,
    /// 分页的序号：`open` 一次加一，回复对不上号就丢掉（切了仓库/换了排序，旧页作废）。
    pub(crate) log_seq: u64,
    /// 文件表下方那格提交信息。放在壳里而不是控件里有两个理由：提交成功后要由回复来清空
    /// （见 `clear_msg_on_load`），以及无头门禁要能直接读它。
    /// SourceTree 那格是**两个框**：摘要 + 描述。落进 git 是一条 `-m`，中间隔一个空行。
    pub(crate) commit_msg: String,
    pub(crate) commit_desc: String,
    /// 下一次 `Msg::Loaded` 落地时要不要清空 `commit_msg`——只有提交那一个动作会置上。
    pub(crate) clear_msg_on_load: bool,
    /// 顶部那格「添加仓库」的路径输入。和 `commit_msg` 一样放在壳里而不是控件里：
    /// 无头门禁要能直接读它，而且清空/收起的时机由动作决定。
    pub(crate) add_open: bool,
    pub(crate) add_path: String,
    /// 当前差异区显示的是哪一份补丁。hunk 那两枚按钮要问的就是这个：同一块在第 1 格
    /// 和第 2 格里意思不同（未暂存 → 「暂存区块」，已暂存 → 「取消暂存区块」，
    /// 某条提交 → 根本没有区块可暂存，按钮不出现）。
    pub(crate) target: Option<Target>,
    /// 「放弃区块」按了第一次，正在等第二下确认。放弃会**真的丢掉工作区那几行**，
    /// 而 SourceTree 的那层确认是一个我还没画过、也没目视过的弹窗，所以先不照抄它：
    /// 第一次只把按钮换成「确认放弃」，仓库一个字节都不动（判据见
    /// `a_discard_needs_two_clicks`）。换差异、换仓库、按 Esc 都会取消。
    pub(crate) pending_discard: Option<usize>,
    /// 「文件历史」浮层（见 `FileHistory`）。和模态卡一样铺满整屏，但它不是一次性动作，
    /// 所以单独一层、单独的状态。
    pub(crate) history: Option<FileHistory>,
    /// 历史面板的序号：每开一次加一，路上回来的旧回复对不上号就丢掉。
    pub(crate) hist_seq: u64,
    /// 那张模态卡（见 `Modal`）。`None` = 没有卡，界面层照常可点。
    pub(crate) modal: Option<Modal>,
    /// 卡刚打开的那一帧要把光标送进文本框；之后再抢焦点就该由用户自己决定。
    pub(crate) modal_focus: bool,
    pub(crate) tx: Sender<Msg>,
    pub(crate) rx: Receiver<Msg>,
}
