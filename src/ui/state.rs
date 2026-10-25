//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use gitx::db;
use gitx::git;
use gitx::models::{
    BlameLine, Branch, Commit, DiffEntry, FileLogEntry, Remote, RemoteRef, Repo, RepoStatus, Stash, Tag,
};
use gitx::rusqlite::Connection;
use std::sync::mpsc::{Receiver, Sender};

use crate::ui::graph::*;
use crate::ui::theme::DESC_ROWS;
use crate::ui::diffmodel::*;

/// 主区域选中的行。虚拟行「Uncommitted changes」就是工作区页——SourceTree 只有一页。
/// 侧栏的贮藏不在日志里，所以它自带一档：点开就在同一块「文件 | 差异」里看那条贮藏。
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Sel {
    Uncommitted,
    Commit(usize),
    /// 侧栏「已贮藏」那条的下标（`snapshot.stashes` 里的位置）。
    Stash(usize),
}

/// 一次差异请求。带 `path` 是单文件，不带是整份。
#[derive(Clone, PartialEq)]
pub(crate) enum Target {
    Working { staged: bool, path: Option<String>, untracked: bool },
    Commit { oid: String, path: Option<String> },
    /// 一条贮藏。`oid` 是那条贮藏自己的提交号（`stash@{n}` 会随列表重排而失效）。
    Stash { oid: String, path: Option<String> },
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
    /// 修正上一条提交（`git commit --amend`）。单独一条动作而不是给 `Commit` 加布尔字段：
    /// 它要过确认卡、落地后还要把「修正上一条」那枚勾去掉，和普通提交不是同一条路。
    Amend(String),
    Checkout(String),
    /// 侧栏「远端」点开之后那一条：**切到它对应的本地分支**。本地没有同名分支就建一条
    /// 跟踪它的（见 `git::checkout_remote_branch`）。带的是**整条远程 ref 名**
    /// （`origin/main`）而不是尾巴 `main`——两条远端各有一条同名分支时，只有全名说得清
    /// 用户点的是哪一条。
    CheckoutRemote(String),
    Fetch,
    /// 只问一个远端：侧栏「远端」那一栏右键用的，`Fetch` 是全抓。
    FetchRemote(String),
    /// 拉取卡按下去那一条。`branch` 是**远端**上那条分支的名字。git 的 `pull <远端> <分支>`
    /// 只会拉进**当前**分支，没有第二条路，所以 `into` 不是当前分支时先切过去再拉。
    PullFrom { remote: String, branch: String, into: Option<String>, opts: git::PullOpts },
    /// 推送卡按下去那一条：一个远端 + 若干行「本地分支 → 远端上叫什么 + 要不要记上游」，
    /// 外加「强制推送」「推送所有标签」那两枚。多条分支就是**依次**跑几条 `git push`：
    /// 中途某条失败，前面那几条已经落地了（git 自己也是这样，不会回滚已推上去的 ref）。
    PushRefs { remote: String, refs: Vec<PushRef>, force: bool, tags: bool },
    /// 新建分支。`start` 是起点（`None` = 从当前 HEAD 长出来），`checkout` = 建完立刻切过去。
    CreateBranch { name: String, start: Option<String>, checkout: bool },
    /// `-d`（不是 `-D`）：没合并过的分支让 git 自己拒掉，错误原样落到 `note` 里。
    /// 不做「静默强删」——那正是用户点错一枚菜单项时会丢工作的地方。
    DeleteBranch(String),
    /// 改本地分支名。`from` 是菜单打开那一刻的名字。
    RenameBranch { from: String, to: String },
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
    /// 把一份已有的本地仓库登记成书签并打开。和克隆同一条理由走在 `repo` 守卫之前：
    /// 手上还没有仓库时这一条照样要点得动。
    AddExisting { path: String },
    /// `git init` 一个新仓库。产物和克隆一样是「磁盘上多了一本仓库」，所以回复共用
    /// `Msg::Cloned`——由 `add_repo` 去登记并打开。
    Init { path: String },
    /// 提交完立刻推送。会先落地提交再推：推送失败时报错，但提交**已经在**，不回滚
    /// （回滚一条已经落地的提交比留着它危险得多）。
    CommitAndPush { msg: String, amend: bool },
    /// 把这条提交的树导成一份 zip。落哪儿由**原生存盘面板**问，所以这条动作不直接带路径：
    /// 它在 `act` 里只把 `pending_dialog` 立起来，面板在下一帧的 `ui()` 里跑
    /// （见 `Dialog`）。`git archive` 只读对象库，不动工作区，所以不过确认卡。
    Archive(String),
    /// 把**当前分支**挪到历史里那条提交上。哪一档由卡上那排页签定（见 `Form::Reset`）。
    /// 它不进 `needs_confirm`：开它的那张卡本身就是那一句「先问一下」——三档摆在你面前、
    /// 每一档写明丢什么，再叠一层确认只会让人以为第一下已经落了。
    Reset { oid: String, mode: git::ResetMode },
}

/// 等在一帧之外跑的**原生面板**。为什么不当场就弹：`land()` 是在 `Panel::show` 的闭包跑完
/// 之后调的，但 `act()` 还会从卡片的按钮那条路进来，而 `rfd` 的面板是**嵌套事件循环**——
/// 在 egui 正画一帧的途中拉起它，等于让同一个窗口在两个循环里跑。所以点一下只立个牌子，
/// 面板统一在 `eframe::App::ui` 的最前面、`shell()` 之前消费掉。
/// 判据也正因为这条才测得到：无头帧直接调 `shell()`，一辈子碰不到 `rfd`。
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Dialog {
    /// 导出 `oid` 那一版。`name` 是**建议文件名**（`仓库名-短号.zip`），面板里用户可以改。
    Archive { repo: String, oid: String, name: String },
    /// 选一个目录，填进「添加仓库」那张卡的某一格。
    PickFolder { field: FolderField },
}

/// 「浏览…」填的是哪一格。两格分开列是因为克隆那一档有**两个**文本框：
/// 主输入是源地址，目标目录在第二格——合成一个枚举值就会填错格子。
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum FolderField {
    /// 「打开已有」/「新建仓库」那两档的唯一主输入。
    Main,
    /// 「克隆到本地」那档的「目标目录」。
    CloneDest,
}

/// 推送卡表格里被勾中的那一条，落地时变成一次 `git push <远端> <本地>:<远端那一侧>`。
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct PushRef {
    pub(crate) local: String,
    /// 推出去在远端叫什么名字。和 `local` 不同就带 refspec，相同就不带（让 git 走自己那套
    /// `push.default`，其中一条分支的推送参数写错会把它推到别人的分支上去）。
    pub(crate) remote_name: String,
    /// 这一条要不要 `-u`。
    pub(crate) track: bool,
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
    /// 把一条本地分支改名。`from` 是右键那一刻的名字，输入框留空等新名字。
    RenameBranch { from: String },
    /// 从本地分支里挑一条合并进当前分支。
    Merge { branches: Vec<String> },
    /// 会丢东西的动作先问一句：`then` 已经算好，按下「确定」才取出来发出去。
    Confirm { then: Option<Action> },
    /// 「添加仓库」那张卡。第一格按档换意思（路径 / 地址 / 新目录），克隆另有第二格目标目录。
    AddRepo { tab: AddTab, dest: String },
    /// 提交那张卡。摘要走 `Modal.text`，描述、修正、提交后推送由表单自己带。
    /// 分支名和暂存计数是**打开那一刻**从快照钉进来的数字（说明行），不是活的。
    Commit { desc: String, amend: bool, push_after: bool },
    /// 偏好那张卡：开关直接写进 `Prefs`，卡片关掉之前就已经生效并落库（macOS 那套没有「应用」）。
    /// `repos`/`database` 是「关于」那一页要念的两条事实，打开那一刻读好端进来。
    Settings { prefs: Prefs, page: PrefPage, repos: usize, database: Option<String> },
    /// 推送那张卡（照 `GitPushSheet`）：一个远端 + 一张本地分支表 + 两枚勾。
    /// 表里的行是**打开那一刻**从快照钉进来的：卡开着的时候后台刷新不该让表换人，
    /// 否则勾了一半，「main」可能已经不是刚才那条 main。
    Push { remote: String, rows: Vec<PushRow>, force: bool, tags: bool },
    /// 拉取那张卡（照 `GitPullSheet`）：远端 / 远程分支 / 拉取到哪条本地分支，加「选项」那三句。
    Pull { remote: String, branch: String, into: String, opts: git::PullOpts },
    /// 把当前分支重置到某条提交。`mode` 是卡上那排页签**当前选中**的那一档，
    /// 打开时给的是 git 自己的默认（混合）——不动工作区那一档，猜错也不会丢东西。
    Reset { oid: String, mode: git::ResetMode },
}

/// 推送卡表格的一行。
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct PushRow {
    pub(crate) local: String,
    /// 远端那一侧叫什么。默认和 `local` 同名（SourceTree 那一格也是预先填好同名的）。
    pub(crate) remote_name: String,
    /// 这条分支现在有没有上游——只用来给「跟踪？」那格一个**初值**（没上游的才勾上）。
    pub(crate) has_upstream: bool,
    /// 这一条推不推。
    pub(crate) picked: bool,
    /// 「跟踪？」那一格。
    pub(crate) track: bool,
}

/// 偏好卡的页签。SourceTree 那张 `PreferencesWindow` 有 通用 / Git / Mercurial / 账户 /
/// 差异视图 / 更新 / 自定义操作 / 高级 八页（nib 里的 `Prefs - *` 那组标识），
/// 这里只放**开关真接得通**的两页：其余页一张空壳比没有更糟。
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PrefPage {
    General,
    /// 「推送与拉取」那一页：默认远端 + `push.default` 那五档 + 两枚勾。
    Push,
    About,
}

/// 打开推送卡时**默认勾哪几条分支**。这一档就是 git 的 `push.default`，五项文案逐字取自
/// 本机 SourceTree 4.2.19 `zh_CN.lproj/PreferencesWindow.nib` 里「Push 分支：」那枚弹出按钮
/// 的菜单（`无 / 匹配 / 上流 / 简单 / 当前`；同一条 nib 的英文版是
/// `nothing / matching / upstream / simple / current`，键名和 git 的取值一一对上）。
/// 测量方法见 docs/PROCESS.md「逆向本机 SourceTree.app」——nib 用 `NSNib` 实例化之后直接读
/// frame 与 menu，数是从那一份 nib 里读出来的，不是拍的。
///
/// 它改的是**卡上表格的预选**，不是我们发出去的实参：卡上的推送一律带明确的 refspec
/// （`git push <远端> <本地>:<远端那一侧>`），而带 refspec 的 `git push` 会**完全无视**
/// `push.default`——把这一档当成 `-c push.default=…` 传下去等于传了个不起作用的参数，
/// 还要谎称「选了就有区别」。
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum PushBehavior {
    /// 「无」→ git 的 `nothing`：一条都不勾，主按钮因此按不动。
    Nothing,
    /// 「匹配」→ git 的 `matching`：所有本地分支都对着远端同名那一条推。
    /// SourceTree 的默认那一档（nib 里那枚按钮 selected 的就是 `matching`）。
    Matching,
    /// 「上流」→ git 的 `upstream`：只推有上游的那些。
    Upstream,
    /// 「简单」→ git 的 `simple`：只推上游且远端名字和本地一样的那些。
    Simple,
    /// 「当前」→ git 的 `current`：只推当前分支，推到远端同名。
    Current,
}

impl PushBehavior {
    /// 弹出按钮上那五项，顺序和 nib 里的菜单一字不差。
    pub(crate) const LABELS: [&'static str; 5] = ["无", "匹配", "上流", "简单", "当前"];

    pub(crate) fn label(self) -> &'static str {
        Self::LABELS[self.index()]
    }

    pub(crate) fn index(self) -> usize {
        match self {
            PushBehavior::Nothing => 0,
            PushBehavior::Matching => 1,
            PushBehavior::Upstream => 2,
            PushBehavior::Simple => 3,
            PushBehavior::Current => 4,
        }
    }

    /// 偏好卡那个弹出按钮**按位置**选档用的反向函数：菜单里那五项的顺序就是 nib 里的顺序，
    /// 所以「点到第几项」比「点到哪项的文案」更直接，也不用把中文文案当键来比。
    pub(crate) fn from_index(i: usize) -> PushBehavior {
        match i {
            0 => PushBehavior::Nothing,
            1 => PushBehavior::Matching,
            2 => PushBehavior::Upstream,
            3 => PushBehavior::Simple,
            _ => PushBehavior::Current,
        }
    }

    /// 存的是 git 那个取值（nib 的英文版就是它）；读回不认识的串回到默认那一档。
    pub(crate) fn key(self) -> &'static str {
        match self {
            PushBehavior::Nothing => "nothing",
            PushBehavior::Matching => "matching",
            PushBehavior::Upstream => "upstream",
            PushBehavior::Simple => "simple",
            PushBehavior::Current => "current",
        }
    }

    pub(crate) fn from_key(value: &str) -> PushBehavior {
        match value {
            "nothing" => PushBehavior::Nothing,
            "upstream" => PushBehavior::Upstream,
            "simple" => PushBehavior::Simple,
            "current" => PushBehavior::Current,
            _ => PushBehavior::Matching,
        }
    }

    /// 表格里这一条该不该预先勾上——**五档的差别只在这儿**（不在 git 的实参上，见上面那段）。
    pub(crate) fn picks(self, branch: &Branch) -> bool {
        match self {
            PushBehavior::Nothing => false,
            PushBehavior::Matching => true,
            // 「当前」这一档字面上就是**只有当前那条**：SourceTree 的说明写的是
            // 「将当前分支推送到远程同名分支」，勾上别的分支等于替用户多推了几条。
            PushBehavior::Current => branch.is_current,
            PushBehavior::Upstream => branch.upstream.is_some(),
            PushBehavior::Simple => branch
                .upstream
                .as_ref()
                .map(|up| up.rsplit('/').next().unwrap_or("") == branch.name)
                .unwrap_or(false),
        }
    }
}

/// 「添加仓库」卡上那三档，和 SourceTree 那张表里的三件事一一对上：
/// 打开一份已有的本地工作区 / 从别处克隆 / 新建一个空仓库。
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum AddTab {
    Open,
    Clone,
    New,
}

/// 偏好卡上那几枚**真的接得通**的开关，存进 core 那张 `settings` 表（和 `gitx_last_repo` 同一张）。
///
/// 单独成一个结构而不是散在 `Gitx` 的字段上：装载发生在 `Gitx::new`，而无头夹具是自己填字段、
/// 不走构造函数（见 `default_scope` 那段注释）——「读配置」这件事必须由一个判据调得到的函数承担。
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Prefs {
    /// 启动时自动打开上一次那本仓库。
    pub(crate) restore_last: bool,
    /// 日志默认排序：`true` = 层级顺序（拓扑），`false` = 按日期。
    pub(crate) topo: bool,
    /// 描述列要不要画远程分支那枚药丸。
    pub(crate) show_remote: bool,
    /// 打开推送卡时表格预先勾哪几条（见 `PushBehavior`）。
    pub(crate) push_behavior: PushBehavior,
    /// 当前分支**没有上游**时，推送卡拿哪一条远端当目标。空串 = 没有默认，卡上远端那格
    /// 留空、主按钮按不动（不替你猜 `origin`：一本仓库可以先推到 fork 再推到 upstream）。
    pub(crate) default_remote: String,
    /// 「推送所有标签到远程仓库」：推送卡上那枚勾的初值。
    pub(crate) push_tags: bool,
    /// 「允许强制推送」：关掉它，推送卡上「强制推送」那枚勾**摆都不摆**——
    /// SourceTree 这一档就是拿它当安全闸的，勾着不给按等于没闸。
    pub(crate) allow_force: bool,
}

impl Prefs {
    pub(crate) const KEY_RESTORE: &'static str = "gitx_prefs_restore_last";
    pub(crate) const KEY_TOPO: &'static str = "gitx_prefs_topo";
    pub(crate) const KEY_REMOTE: &'static str = "gitx_prefs_show_remote";
    pub(crate) const KEY_PUSH_BEHAVIOR: &'static str = "gitx_prefs_push_behavior";
    pub(crate) const KEY_DEFAULT_REMOTE: &'static str = "gitx_prefs_default_remote";
    pub(crate) const KEY_PUSH_TAGS: &'static str = "gitx_prefs_push_tags";
    pub(crate) const KEY_ALLOW_FORCE: &'static str = "gitx_prefs_allow_force";

    /// 没有数据库、或某个键没写过 → 用这层原本的初值。存的是 `"true"` / `"false"`
    /// （和 core 那张表里别的值一样是明文），读回一个不像布尔的串就按默认走，
    /// 不让一次写坏把开关永久钉死。
    pub(crate) fn load(conn: Option<&Connection>) -> Prefs {
        let read = |key: &str, default: bool| -> bool {
            let Some(c) = conn else { return default };
            db::get_setting(c, key)
                .ok()
                .flatten()
                .map(|v| flag(&v, default))
                .unwrap_or(default)
        };
        let text = |key: &str, default: &str| -> String {
            let Some(c) = conn else { return default.to_owned() };
            db::get_setting(c, key).ok().flatten().unwrap_or_else(|| default.to_owned())
        };
        Prefs {
            restore_last: read(Self::KEY_RESTORE, true),
            topo: read(Self::KEY_TOPO, false),
            show_remote: read(Self::KEY_REMOTE, true),
            push_behavior: PushBehavior::from_key(&text(
                Self::KEY_PUSH_BEHAVIOR,
                PushBehavior::Matching.key(),
            )),
            default_remote: text(Self::KEY_DEFAULT_REMOTE, ""),
            push_tags: read(Self::KEY_PUSH_TAGS, false),
            allow_force: read(Self::KEY_ALLOW_FORCE, true),
        }
    }

    pub(crate) fn save(&self, conn: Option<&Connection>) {
        let Some(c) = conn else { return };
        let _ = db::set_setting(c, Self::KEY_RESTORE, bit(self.restore_last));
        let _ = db::set_setting(c, Self::KEY_TOPO, bit(self.topo));
        let _ = db::set_setting(c, Self::KEY_REMOTE, bit(self.show_remote));
        let _ = db::set_setting(c, Self::KEY_PUSH_BEHAVIOR, self.push_behavior.key());
        let _ = db::set_setting(c, Self::KEY_DEFAULT_REMOTE, &self.default_remote);
        let _ = db::set_setting(c, Self::KEY_PUSH_TAGS, bit(self.push_tags));
        let _ = db::set_setting(c, Self::KEY_ALLOW_FORCE, bit(self.allow_force));
    }
}

fn bit(on: bool) -> &'static str {
    if on {
        "true"
    } else {
        "false"
    }
}

/// 只认字面 `"true"`，其余一律 `default`：读不懂的值不该悄悄变成「开」。
fn flag(value: &str, default: bool) -> bool {
    if value == "true" {
        true
    } else if value == "false" {
        false
    } else {
        default
    }
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

    /// 重命名分支。旧名字放在说明里，输入框空着等新名字——预填旧名字会让用户
    /// 在末尾接着打，得到 `main-new` 这种串。
    pub(crate) fn rename_branch(from: String) -> Self {
        Modal {
            title: "重命名分支",
            detail: Some(format!("{from} 改名为")),
            text: Some(String::new()),
            form: Form::RenameBranch { from },
        }
    }

    pub(crate) fn merge(branches: Vec<String>) -> Self {
        Modal { title: "合并到当前分支", detail: None, text: None, form: Form::Merge { branches } }
    }

    /// 「把当前分支重置到这条提交」那张卡。标题里的「当前分支」不是顺手写的：`git reset`
    /// 挪的是**你现在站着的那条**，不是右键那条提交所在的分支——这一句说不清，用户会以为
    /// 自己在改别条分支。`subject` 是打开那一刻那条提交的话，卡开着时日志翻页不该让它换内容。
    pub(crate) fn reset(oid: String, subject: String) -> Self {
        Modal {
            title: "重置当前分支",
            detail: Some(format!("把当前分支挪到：{subject}")),
            text: None,
            form: Form::Reset { oid, mode: git::ResetMode::Mixed },
        }
    }

    pub(crate) fn confirm(title: &'static str, detail: String, then: Action) -> Self {
        Modal { title, detail: Some(detail), text: None, form: Form::Confirm { then: Some(then) } }
    }

    /// 「添加仓库」那张卡。三档共用一格主输入：已有仓库的路径 / 克隆的源地址 / 新仓库目录。
    pub(crate) fn add_repo() -> Self {
        Modal {
            title: "添加仓库",
            detail: None,
            text: Some(String::new()),
            form: Form::AddRepo { tab: AddTab::Open, dest: String::new() },
        }
    }

    /// 提交那张卡。`branch`/`staged` 是打开那一刻从快照读出来的数（只进说明行，不参与判断——
    /// 能不能提交由落地时那条 git 命令说了算）。
    pub(crate) fn commit(branch: String, staged: usize) -> Self {
        Modal {
            title: "提交",
            detail: Some(format!("提交到 {branch} · 已暂存 {staged} 个文件")),
            text: Some(String::new()),
            form: Form::Commit { desc: String::new(), amend: false, push_after: false },
        }
    }

    /// 偏好那张卡：把当下那三枚开关原样端上来，改一格当场落库一格。
    pub(crate) fn settings(prefs: Prefs, repos: usize, database: Option<String>) -> Self {
        Modal {
            title: "设置",
            detail: None,
            text: None,
            form: Form::Settings { prefs, page: PrefPage::General, repos, database },
        }
    }

    /// 推送那张卡。标题从 `GitPushSheet.nib` 那句「推送？」改成「推送」：这张卡不是确认框，
    /// 它是「选要推哪几条」的工作表，底下那枚主按钮写的就是同一个动词，带问号读起来像在执行。
    /// 表头那一列仍叫「推送？」——那一格问的确实是「这条要不要推」。
    /// `rows` 与 `remote` 都由调用方从**打开那一刻**的快照与偏好算好：卡开着的时候后台
    /// 刷新不该让表换人，也不该让远端那一格自己漂走。
    pub(crate) fn push(rows: Vec<PushRow>, remote: String, prefs: &Prefs) -> Self {
        Modal {
            title: "推送",
            detail: None,
            text: None,
            form: Form::Push { remote, rows, force: false, tags: prefs.push_tags },
        }
    }

    /// 拉取那张卡。标题「拉取」取自 `GitPullSheet.nib`（那张里没有带问号的写法）。
    pub(crate) fn pull(remote: String, branch: String, into: String) -> Self {
        Modal {
            title: "拉取",
            detail: None,
            text: None,
            form: Form::Pull { remote, branch, into, opts: git::PullOpts::default() },
        }
    }

    /// 文本框的占位文案，也是「这一格该填什么」的唯一说法。
    pub(crate) fn hint(&self) -> &'static str {
        match &self.form {
            Form::NewBranch { .. } => "分支名",
            Form::StashSave { .. } => "贮藏说明（可留空）",
            Form::NewTag { .. } => "标签名",
            Form::RenameBranch { .. } => "新分支名",
            Form::AddRepo { tab, .. } => match tab {
                AddTab::Open => "已有仓库的路径",
                AddTab::Clone => "源仓库地址（https://… 或 /路径）",
                AddTab::New => "新仓库的目录",
            },
            Form::Commit { .. } => "提交摘要",
            Form::Merge { .. } | Form::Confirm { .. } | Form::Settings { .. } => "",
            // 两张卡没有「主输入」那一格：远端和分支都是弹出按钮，「远程分支」那一列的
            // 改名框在表格里面自己带提示，不占这张卡的 `hint`。
            Form::Push { .. } | Form::Pull { .. } => "",
            // 重置卡没有文本框：要挑的只有三档页签，那一句「会丢什么」是卡自己写的。
            Form::Reset { .. } => "",
        }
    }

    /// 卡中间有几行正文。卡片高度由它算 —— 居中要在布局之前定位置，问 egui 要「自然高度」
    /// 得等布局跑完，所以这几行的账我们自己记。
    pub(crate) fn rows(&self) -> usize {
        let mut n = usize::from(self.detail.is_some());
        match &self.form {
            Form::Merge { branches } => n += branches.len(),
            Form::StashSave { .. } => n += 1,
            // 三档按钮占一行，克隆那一档再多一格目标目录。
            Form::AddRepo { tab, .. } => n += 1 + usize::from(*tab == AddTab::Clone),
            // 描述那格按三行数（和内联那一格一样高），再加「修正」「提交后推送」两枚勾。
            Form::Commit { .. } => n += DESC_ROWS + 2,
            // 重置卡：页签一行 + 说明一行 + 「丢什么」一行，加 `detail` 那行共四行。
            // 说明和「丢什么」两行**固定占位**，不随档位长短增减——换一档卡片跳一下，
            // 看着就像按下去了。
            Form::Reset { .. } => n += 3,
            // 偏好卡自己按量来的尺寸摆，不数行；`rows()` 对它只管标题和脚部那两条。
            Form::Settings { .. } => return 0,
            // 推送/拉取两张卡同一条理由：它们的框是从 nib 量出来的（650×271 / 614×299），
            // 里面每一格摆在哪也是量出来的，不跟行数走。
            Form::Push { .. } | Form::Pull { .. } => return 0,
            _ => {}
        }
        if self.text.is_some() {
            n += 1;
        }
        n
    }

    /// 这一张卡的「主按钮」文案（Enter 走的就是它）。
    pub(crate) fn primary(&self) -> &'static str {
        match &self.form {
            Form::NewBranch { .. } => "创建并切换",
            Form::StashSave { .. } => "贮藏",
            Form::NewTag { .. } => "创建",
            Form::RenameBranch { .. } => "重命名",
            Form::AddRepo { tab, .. } => match tab {
                AddTab::Open => "添加",
                AddTab::Clone => "克隆",
                // 不叫「新建仓库」：那一串已经是上面那档页签的名字，同一张卡上出现两次，
                // 判据（和无头夹具里的点击）就分不清点的是哪一格。
                AddTab::New => "创建",
            },
            Form::Commit { amend, .. } => if *amend { "修正并提交" } else { "提交" },
            Form::Settings { .. } => "完成",
            // 两张卡的主按钮写动词不写「确定」：nib 里那两枚按钮的文案确实是「确定 / 取消」，
            // 但那两张卡是工作表不是确认框，标题就叫「推送」「拉取」——按钮再写「确定」，
            // 同一张卡上两个词指同一件事，还得多读一次。「推送」「拉取」这两个词同样在那两张 nib 里。
            Form::Push { .. } => "推送",
            Form::Pull { .. } => "拉取",
            Form::Merge { .. } => "",
            Form::Confirm { .. } => "确定",
            // 动词 + 对象：这一按改的是分支指针，光写「重置」和卡上那排页签读起来
            // 像「重置这个选择」。
            Form::Reset { .. } => "重置分支",
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
    /// 侧栏点开的那条贮藏改了哪些文件。和 `CommitFiles` 分开：选中项不是同一种行。
    StashFiles { repo: String, index: usize, files: Vec<DiffEntry> },
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
    /// 归档成了，`String` 是落地的那个路径。失败那一路共用 `Msg::Failed`（它已经把
    /// `busy` 落回来、也把原文进了 toast），这里只管成功那一条。
    Archived(String),
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
    /// 当前**可见**的差异行下标：折叠某个文件后，它名下的行从这里筛掉。
    /// 只在补丁落地与点折叠时重建——每帧重算就是拿 2.8 万行去换一次点击的手感。
    pub(crate) diff_rows: Vec<usize>,
    /// 被折叠的文件序号（`DiffLine::file`，从 1 数）。文件头本身不折叠，点了会重新展开。
    pub(crate) collapsed: std::collections::HashSet<usize>,
    /// 差异区右上角那格搜索框里的字。空串 = 不过滤；有字时只留命中的内容行、
    /// 它们所属的文件头，以及引入命中行的那一块 hunk 头（行号才有上下文）。
    pub(crate) diff_search: String,
    /// 这份差异最长那行大约占多少个等宽字格（ASCII 算 1、其余算 2、制表符算 4）。
    /// 差异区要能往右滚，内容宽度不能只看窗口宽度——只在 `Msg::Patch` 落地时算一次。
    pub(crate) diff_cols: usize,
    pub(crate) patch_label: String,
    /// 差异请求序号：快速连点两个文件时，只有最新那次的回复能落地。
    pub(crate) seq: u64,
    pub(crate) busy: bool,
    pub(crate) note: Option<String>,
    /// `note` 第一次出现的时刻（egui 时间，秒）。toast 靠它结算 8 秒超时；
    /// `None` 表示这一帧还没画过。无头判据直接往 `RawInput::time` 塞值来造「过了 8 秒」。
    pub(crate) note_at: Option<f64>,
    pub(crate) log_filter: String,
    pub(crate) side_filter: String,
    /// 日志排序方式，即筛选条上那枚「层级顺序」：`false` = 日期序，`true` = 拓扑序。
    pub(crate) topo: bool,
    /// 日志范围，即筛选条最左那枚弹出按钮：三档**互斥**，所以用核心层那个枚举而不是
    /// 「rev + 布尔」两个字段——`All` 和一条具体分支同时成立的状态建不出来。
    /// 默认 `Current`：不给 git 传任何范围参数，走 HEAD 那条线。
    pub(crate) scope: git::Scope,
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
    /// 提交框左边那枚「修正上一条」：勾上之后提交按钮落地的是 `git commit --amend`，
    /// 不是新的一条。成功落地后连同信息一起清掉。
    pub(crate) amend: bool,
    /// 下一次 `Msg::Loaded` 落地时要不要清空 `commit_msg`——只有提交/修正那两条动作会置上。
    pub(crate) clear_msg_on_load: bool,
    /// 启动时要不要自动打开上一次那本仓库（偏好卡「通用」那一页的第一枚开关）。
    /// 它只在 `Gitx::new` 里被读一次，但和另两条一样存在同一张 `settings` 表里。
    pub(crate) restore_last: bool,
    /// 下面四条只服务推送那两张卡，不进渲染路径；和上面那条一样存在 core 那张 `settings`
    /// 表里，改完当场落库（见 `apply_prefs`）。为什么要镜像进 `Gitx` 而不是现读：无头夹具
    /// 是自己填字段、不走 `Gitx::new`，而「打开推送卡要用的初值」必须由判据直接摆得上来。
    /// 推送卡表格里**预先勾哪几条**（五档的差别只在这儿，见 `PushBehavior`）。
    pub(crate) push_behavior: PushBehavior,
    /// 当前分支没有上游时，推送卡远端那一格拿哪一条当初值。空串 = 留空，让用户自己挑。
    pub(crate) default_remote: String,
    /// 推送卡「推送所有标签」那枚勾的初值。
    pub(crate) push_tags: bool,
    /// 推送卡「强制推送」那枚勾**摆不摆**的闸（关掉就整个不出现，见 `push_card`）。
    pub(crate) allow_force: bool,
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
    /// 下一帧的 `ui()` 要拉起的原生面板。`take()` 消费，所以一次点击最多弹一个面板；
    /// 判据 `a_headless_frame_never_consumes_the_pending_dialog` 钉的就是「立起来但**不在绘制帧里被消费**」
    /// ——那条边界同时保证了无头测试一辈子碰不到 `rfd`，不会在 CI 里弹出真面板。
    pub(crate) pending_dialog: Option<Dialog>,
    /// 最后一次**在分支上**时看到的那条分支名。分离 HEAD 之后 `status.branch` 是空的，
    /// 而用户要的那句「回去」恰恰只有得出去才知道：状态栏靠它画「回到 main」那一格
    /// （判据 `going_back_from_a_detached_head_reattaches_to_the_branch`）。
    pub(crate) last_branch: String,
    /// 侧栏「远端」里**展开着**的那几条的名字。只记名字、不记下面的分支：分支列表归快照，
    /// 抓取之后新快照落地，展开着的那条自己就把新的分支摆出来。
    /// 存在 `Gitx` 而不是数据库：这是这一次看仓库时的临时状态，重启又铺开一堆分支不是用户要的。
    pub(crate) remote_open: Vec<String>,
    pub(crate) tx: Sender<Msg>,
    pub(crate) rx: Receiver<Msg>,
}
