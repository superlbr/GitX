//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use gitx::db;
use gitx::git;
use gitx::models::{
    BlameLine, Branch, Commit, DiffEntry, FileLogEntry, Remote, RemoteRef, Repo, RepoStatus, Stash, Tag,
};
use gitx::rusqlite::Connection;
use gitx::tr;
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
    /// **搜索结果**列表里的第几格（`Gitx::search_hits`）。不并进 `Commit` 那一档：
    /// 同一个下标在两份列表里指的是不同的提交，并起来之后「上一本列表的回复盖到
    /// 这一本的选中行上」这种脏数据就没有闸能拦了（见 `Msg::CommitFiles` 那个 `search`）。
    Search(usize),
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
    /// 「代码托管」那一页：每家认得出的托管填一枚访问令牌，看板取数用的就是它。
    Hosting,
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
    /// 弹出按钮上那五项，顺序和 nib 里的菜单一字不差。文案跟着语言档位走，
    /// 所以这是一**函数**：常量表会把两种语言钉死成一份。
    pub(crate) fn labels() -> [&'static str; 5] {
        [
            tr!("无", "None"),
            tr!("匹配", "Matching"),
            tr!("上流", "Upstream"),
            tr!("简单", "Simple"),
            tr!("当前", "Current"),
        ]
    }

    pub(crate) fn label(self) -> &'static str {
        Self::labels()[self.index()]
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
    /// 界面语言。默认中文（「跟随系统」没做：从 Finder 起来的 GUI 进程读不到可靠的
    /// locale，猜错档位比不给这一档更坏）。
    pub(crate) lang: gitx::lang::Lang,
    /// 代码托管的访问令牌，按 provider 的 id 存（键名 `provider_token_<id>`，见
    /// `provider::Host::id` 那句注释）。只收**声明了能力**的那几家：Codeup 认得出
    /// 但 PR/CI 都做不了，给它留一格令牌就是一格按下去没反应的输入框。
    /// 顺序跟着 `provider::HOSTS`，所以两份 Prefs 比得动。
    pub(crate) tokens: Vec<(String, String)>,
}

impl Prefs {
    pub(crate) const KEY_RESTORE: &'static str = "gitx_prefs_restore_last";
    pub(crate) const KEY_TOPO: &'static str = "gitx_prefs_topo";
    pub(crate) const KEY_REMOTE: &'static str = "gitx_prefs_show_remote";
    pub(crate) const KEY_PUSH_BEHAVIOR: &'static str = "gitx_prefs_push_behavior";
    pub(crate) const KEY_DEFAULT_REMOTE: &'static str = "gitx_prefs_default_remote";
    pub(crate) const KEY_PUSH_TAGS: &'static str = "gitx_prefs_push_tags";
    pub(crate) const KEY_ALLOW_FORCE: &'static str = "gitx_prefs_allow_force";
    /// 语言存 `"zh"` / `"en"` 两个明文值（不是布尔，所以走 `text()` 那一格）。
    pub(crate) const KEY_LANG: &'static str = "gitx_prefs_lang";
    /// 令牌的键前缀：完整键是 `provider_token_<id>`（`id` 取自 `provider::Host::id`）。
    pub(crate) const KEY_TOKEN_PREFIX: &'static str = "provider_token_";

    /// 这一家 provider 当前存的令牌；没存过就是空串。
    pub(crate) fn token(&self, provider: &str) -> &str {
        self.tokens
            .iter()
            .find(|(id, _)| id == provider)
            .map(|(_, t)| t.as_str())
            .unwrap_or("")
    }

    /// 取出（必要时创建）那一家那一格，供「代码托管」页的输入框直接绑。
    /// provider 的 id 必须来自 `provider::HOSTS`，否则这一格根本不该存在。
    pub(crate) fn token_mut(&mut self, provider: &str) -> &mut String {
        if !self.tokens.iter().any(|(id, _)| id == provider) {
            self.tokens.push((provider.to_owned(), String::new()));
        }
        &mut self
            .tokens
            .iter_mut()
            .find(|(id, _)| id == provider)
            .unwrap()
            .1
    }

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
            lang: gitx::lang::Lang::from_key(&text(Self::KEY_LANG, gitx::lang::Lang::Zh.key())),
            tokens: {
                let mut t = Vec::new();
                for h in gitx::provider::HOSTS
                    .iter()
                    .filter(|h| h.capabilities().contains(&gitx::provider::Capability::PullRequests))
                {
                    let v = text(&format!("{}{}", Self::KEY_TOKEN_PREFIX, h.id()), "");
                    if !v.is_empty() {
                        t.push((h.id().to_owned(), v));
                    }
                }
                t
            },
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
        let _ = db::set_setting(c, Self::KEY_LANG, self.lang.key());
        // 令牌整排重写：清空的要落回空串，否则「删掉绑定」在库里其实还在。
        for h in gitx::provider::HOSTS
            .iter()
            .filter(|h| h.capabilities().contains(&gitx::provider::Capability::PullRequests))
        {
            let _ = db::set_setting(
                c,
                &format!("{}{}", Self::KEY_TOKEN_PREFIX, h.id()),
                self.token(h.id()),
            );
        }
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
            title: tr!("新建分支", "New branch"),
            detail: start
                .as_ref()
                .map(|s| tr!(format!("从 {s} 长出去"), format!("branched from {s}"))),
            text: Some(String::new()),
            form: Form::NewBranch { start },
        }
    }

    pub(crate) fn stash_save() -> Self {
        Modal {
            title: tr!("贮藏改动", "Stash changes"),
            detail: Some(tr!(
                "贮藏会把工作区和暂存区都收起来，之后回到当前分支",
                "Stashing takes the working tree and the index away, then returns you to this branch"
            ).to_owned()),
            text: Some(String::new()),
            form: Form::StashSave { include_untracked: false },
        }
    }

    pub(crate) fn new_tag(oid: String, subject: String) -> Self {
        Modal {
            title: tr!("新建标签", "New tag"),
            detail: Some(tr!(format!("打在：{subject}"), format!("on: {subject}"))),
            text: Some(String::new()),
            form: Form::NewTag { oid },
        }
    }

    /// 重命名分支。旧名字放在说明里，输入框空着等新名字——预填旧名字会让用户
    /// 在末尾接着打，得到 `main-new` 这种串。
    pub(crate) fn rename_branch(from: String) -> Self {
        Modal {
            title: tr!("重命名分支", "Rename branch"),
            detail: Some(tr!(format!("{from} 改名为"), format!("Rename {from} to"))),
            text: Some(String::new()),
            form: Form::RenameBranch { from },
        }
    }

    pub(crate) fn merge(branches: Vec<String>) -> Self {
        Modal { title: tr!("合并到当前分支", "Merge into current branch"), detail: None, text: None, form: Form::Merge { branches } }
    }

    /// 「把当前分支重置到这条提交」那张卡。标题里的「当前分支」不是顺手写的：`git reset`
    /// 挪的是**你现在站着的那条**，不是右键那条提交所在的分支——这一句说不清，用户会以为
    /// 自己在改别条分支。`subject` 是打开那一刻那条提交的话，卡开着时日志翻页不该让它换内容。
    pub(crate) fn reset(oid: String, subject: String) -> Self {
        Modal {
            title: tr!("重置当前分支", "Reset current branch"),
            detail: Some(tr!(format!("把当前分支挪到：{subject}"), format!("Move this branch to: {subject}"))),
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
            title: tr!("添加仓库", "Add repository"),
            detail: None,
            text: Some(String::new()),
            form: Form::AddRepo { tab: AddTab::Open, dest: String::new() },
        }
    }

    /// 提交那张卡。`branch`/`staged` 是打开那一刻从快照读出来的数（只进说明行，不参与判断——
    /// 能不能提交由落地时那条 git 命令说了算）。
    pub(crate) fn commit(branch: String, staged: usize) -> Self {
        Modal {
            title: tr!("提交", "Commit"),
            detail: Some(tr!(
                format!("提交到 {branch} · 已暂存 {staged} 个文件"),
                format!("to {branch} · {staged} staged file(s)")
            )),
            text: Some(String::new()),
            form: Form::Commit { desc: String::new(), amend: false, push_after: false },
        }
    }

    /// 偏好那张卡：把当下那三枚开关原样端上来，改一格当场落库一格。
    pub(crate) fn settings(prefs: Prefs, repos: usize, database: Option<String>) -> Self {
        Modal {
            title: tr!("设置", "Settings"),
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
            title: tr!("推送", "Push"),
            detail: None,
            text: None,
            form: Form::Push { remote, rows, force: false, tags: prefs.push_tags },
        }
    }

    /// 拉取那张卡。标题「拉取」取自 `GitPullSheet.nib`（那张里没有带问号的写法）。
    pub(crate) fn pull(remote: String, branch: String, into: String) -> Self {
        Modal {
            title: tr!("拉取", "Pull"),
            detail: None,
            text: None,
            form: Form::Pull { remote, branch, into, opts: git::PullOpts::default() },
        }
    }

    /// 文本框的占位文案，也是「这一格该填什么」的唯一说法。
    pub(crate) fn hint(&self) -> &'static str {
        match &self.form {
            Form::NewBranch { .. } => tr!("分支名", "Branch name"),
            Form::StashSave { .. } => tr!("贮藏说明（可留空）", "Stash description (optional)"),
            Form::NewTag { .. } => tr!("标签名", "Tag name"),
            Form::RenameBranch { .. } => tr!("新分支名", "New branch name"),
            Form::AddRepo { tab, .. } => match tab {
                AddTab::Open => tr!("已有仓库的路径", "Path to an existing repository"),
                AddTab::Clone => tr!("源仓库地址（https://… 或 /路径）", "Source URL (https://… or a path)"),
                AddTab::New => tr!("新仓库的目录", "Directory for the new repository"),
            },
            Form::Commit { .. } => tr!("提交摘要", "Commit summary"),
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
            Form::NewBranch { .. } => tr!("创建并切换", "Create and check out"),
            Form::StashSave { .. } => tr!("贮藏", "Stash"),
            Form::NewTag { .. } => tr!("创建", "Create"),
            Form::RenameBranch { .. } => tr!("重命名", "Rename"),
            Form::AddRepo { tab, .. } => match tab {
                AddTab::Open => tr!("添加", "Add"),
                AddTab::Clone => tr!("克隆", "Clone"),
                // 不叫「新建仓库」：那一串已经是上面那档页签的名字，同一张卡上出现两次，
                // 判据（和无头夹具里的点击）就分不清点的是哪一格。
                AddTab::New => tr!("创建", "Create"),
            },
            Form::Commit { amend, .. } => if *amend { tr!("修正并提交", "Amend and commit") } else { tr!("提交", "Commit") },
            Form::Settings { .. } => tr!("完成", "Done"),
            // 两张卡的主按钮写动词不写「确定」：nib 里那两枚按钮的文案确实是「确定 / 取消」，
            // 但那两张卡是工作表不是确认框，标题就叫「推送」「拉取」——按钮再写「确定」，
            // 同一张卡上两个词指同一件事，还得多读一次。「推送」「拉取」这两个词同样在那两张 nib 里。
            Form::Push { .. } => tr!("推送", "Push"),
            Form::Pull { .. } => tr!("拉取", "Pull"),
            Form::Merge { .. } => "",
            Form::Confirm { .. } => tr!("确定", "OK"),
            // 动词 + 对象：这一按改的是分支指针，光写「重置」和卡上那排页签读起来
            // 像「重置这个选择」。
            Form::Reset { .. } => tr!("重置分支", "Reset branch"),
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

/// 历史页那一格搜索的一条命中，连同**落地那一刻**算好的药丸与日期文案。
/// 单独一个结构体而不是三份平行数组：`commit`/`badges`/`date` 必须一一对应，
/// 拆成三个 `Vec` 的话「多算一行少推一个」这类错位没人拦（`Snapshot` 那三本账
/// 是 worker 一次算完的，所以它敢拆）。
pub(crate) struct SearchHit {
    pub(crate) commit: Commit,
    pub(crate) badges: Vec<RefBadge>,
    pub(crate) date: String,
}

/// 一行上那两个补充请求（按号取详情、按 sha 取检查）的落地结果。
///
/// 任何一项是 `None` = **还没回来**。「压根没去取」不由这里表示——那是 `Board::facts`
/// 里根本没有这个键；两件事挤进同一个 `Option`，迟早会把「取失败」读成「没取」。
#[derive(Debug, Default, Clone)]
pub(crate) struct RowFacts {
    /// `mergeable_state` 的**原始值**（GitHub：`clean` / `blocked` / `unknown` …）。
    pub(crate) detail: Option<String>,
    pub(crate) checks: Option<gitx::models::Ci>,
    /// 详情回过几次 `unknown`（GitHub 那一格是后台算的，第一次常常还没有）。
    /// 有上限，不然一本 CI 永远在跑的仓库会把这格变成无休止的重发。
    pub(crate) detail_tries: u8,
    /// 到点重取详情的那个时刻（egui 的秒）。`None` = 不排。
    pub(crate) retry_at: Option<f64>,
}

/// `Board::asked` 里的那一格：同一个键的两个请求各自记一次。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Ask {
    Detail,
    Checks,
}

/// 看板那一栏的全部状态：当前仓库对应的 provider、取回来的 PR、以及「这条我看过没有」。
///
/// 为什么这些数不住在 `Snapshot` 里：快照是**一次 git 调用**的产物，而看板的数据来自
/// HTTP，取回来的时机和 git 完全不同步。住在同一份结构里会让「git 落地」顺手清掉
/// 刚取回的列表，或者反过来。
#[derive(Default)]
pub(crate) struct Board {
    /// 当前这一栏对着哪家 provider（`provider::Host::id`）；`None` = 这本仓库的远端
    /// 认不出来，看板画「没有可看的托管」而不是空列表。
    pub(crate) host: Option<&'static dyn gitx::provider::Host>,
    /// 认出来的那个 `owner/repo`（连同 host），拼 API 与网页地址都用它。
    pub(crate) repo_ref: Option<gitx::provider::RepoRef>,
    pub(crate) prs: Vec<gitx::models::PullRequest>,
    /// **还没取过**和**取过、结果是空**是两回事：前者画「还没有取」，后者画「没有开放的 PR」。
    pub(crate) loaded: bool,
    /// 正在飞的取数。和 `busy` 分开：那一面旗子是 git 的（见 PROCESS「分离 HEAD」那批
    /// 记的 `Msg::Patch` 抢旗教训），HTTP 在飞不该让 git 动作看起来在跑。
    /// 这一位只管**列表**；每一行的那两个补充请求另有 `asked` / `facts` 两本账。
    pub(crate) fetching: bool,
    /// 上一次取数的失败原因；成功取回时清掉。
    pub(crate) error: Option<String>,
    /// 「上次看过」的**签名**（见 `PullRequest::signature`），按 PR 号存；键见 `seen_key`。
    pub(crate) seen: std::collections::HashMap<u32, String>,
    /// 补充请求的结果，按 `(号, head sha)` 存。sha 是键的一半不是装饰：
    /// 作者 force-push 之后号还是那个号，旧的那份绿必须作废，不然新提交挂着上一版的结论。
    /// 它在「按刷新」和换列表之间**留着**——同一版提交不必问第二次。
    pub(crate) facts: std::collections::HashMap<(u32, String), RowFacts>,
    /// 为哪个键发过哪种请求。每帧都在重画，不去重的话就是每帧一轮 HTTP。
    pub(crate) asked: std::collections::HashSet<((u32, String), Ask)>,
    /// **这本账第一次建**：库里读回来是空的（或压根没有这一条）。
    /// 这一趟里落下来的结论只进账、不点亮蓝点 —— 用户正盯着这一栏，
    /// 「补充的两格到齐」不是他离开之后发生的动静，给它一个点是骗他。
    /// 关掉这一栏（或换仓库）之后就不再有这一档：从第二次打开起，蓝点比的
    /// 是上一次真正看过的那一版。
    pub(crate) seeding: bool,
}

impl Board {
    /// 这一本的「看过」记录在库里的那个键。provider id + `owner/repo` 都在键里，
    /// 所以 fork 之间、同名仓库之间不会互相顶掉。
    ///
    /// `pr_seen2_` 的那个 2：签名换过格式（旧账本存的是 `updated_at` 串）。不换代的话，
    /// 旧值对不上任何新签名 → 一进来满屏蓝点，那正是「切走再回来不该满屏蓝点」要防的事。
    /// 换档 = 旧账本整本作废，而不是逐条被误判成「动过」。
    pub(crate) fn seen_key(&self) -> Option<String> {
        let id = self.host.map(|h| h.id())?;
        self.repo_ref
            .as_ref()
            .map(|r| format!("pr_seen2_{}_{}", id, r.path.replace('/', "_")))
    }

    /// 这条 PR 自上次查看之后有没有新动静（看板那一枚蓝点）。
    ///
    /// 比的是**签名**不是 `updated_at`：检查跑完不会把 `updated_at` 顶走（实测），
    /// 只看它的话「CI 从在跑到全绿」这一眼看不见的那次变化永远不亮。
    /// 首次建账那趟（`seeding`）一律不亮：见那个字段的说明。
    pub(crate) fn is_new(&self, pr: &gitx::models::PullRequest) -> bool {
        if self.seeding {
            return false;
        }
        self.seen
            .get(&pr.number)
            .map(|s| *s != pr.signature())
            .unwrap_or(true)
    }

    /// 发过、但还没结出结果的补充请求有几笔。两处用同一个数：`enrich_rows` 的并发上限，
    /// 和「首次建账这一趟走完了没有」。所以它住在这儿而不是写两遍。
    pub(crate) fn outstanding(&self) -> usize {
        self.asked
            .iter()
            .filter(|((number, sha), ask)| {
                let f = self.facts.get(&(number.clone(), sha.clone()));
                match ask {
                    Ask::Detail => f.map(|f| f.detail.is_none()).unwrap_or(true),
                    Ask::Checks => f.map(|f| f.checks.is_none()).unwrap_or(true),
                }
            })
            .count()
    }

    /// 首次建账：把每一行**现在**的签名当作「看过了」记下来，返回账本有没有变。
    /// 只在 `seeding` 那趟里被反复调用（列表落地一次、补充结论每落一次一次），
    /// 所以它必须自己幂等 —— 变了才落库，不然每帧写一次数据库。
    pub(crate) fn seed_seen(&mut self) -> bool {
        let mut changed = false;
        for pr in self.prs.iter() {
            let sig = pr.signature();
            if self.seen.get(&pr.number) != Some(&sig) {
                self.seen.insert(pr.number, sig);
                changed = true;
            }
        }
        changed
    }

    /// 这一行还有没有在飞的补充请求。**决定「正在看它卡在哪」那句话能不能说**：
    /// 压根不取的那一家（没声明 capability）永远等不到答案，对它说「正在看」是骗人。
    pub(crate) fn row_fetching(&self, pr: &gitx::models::PullRequest) -> bool {
        let key = (pr.number, pr.head_sha.clone());
        let facts = self.facts.get(&key);
        let outstanding = |ask: Ask, got: bool| self.asked.contains(&(key.clone(), ask)) && !got;
        let detail_got = facts.map(|f| f.detail.is_some()).unwrap_or(false);
        let checks_got = facts.map(|f| f.checks.is_some()).unwrap_or(false);
        outstanding(Ask::Detail, detail_got) || outstanding(Ask::Checks, checks_got)
    }

    /// 旁证：这一行挂着的那个 job，在同一栏的**别行**上也挂着。
    /// 只做正向证据——「别行没挂着」不等于「别行是绿的」（可能压根还没取到），
    /// 所以没证据时那句「大概率无关」不许出口。
    pub(crate) fn failing_elsewhere(&self, number: u32) -> bool {
        use gitx::models::Ci;
        let mine: Vec<&String> = match self.prs.iter().find(|p| p.number == number).map(|p| &p.ci) {
            Some(Ci::Failed { failed }) => failed.iter().collect(),
            _ => return false,
        };
        self.prs.iter().any(|other| {
            other.number != number
                && matches!(&other.ci, Ci::Failed { failed } if failed.iter().any(|n| mine.contains(&n)))
        })
    }

    /// 把缓存涂回列表：按刷新之后列表是新的（那两格全空），但按 `(号, sha)` 的缓存还有效，
    /// 同一版提交不该再问第二次。
    pub(crate) fn apply_facts(&mut self) {
        for pr in self.prs.iter_mut() {
            if let Some(f) = self.facts.get(&(pr.number, pr.head_sha.clone())) {
                if let Some(ms) = &f.detail {
                    pr.mergeable_state = ms.clone();
                }
                if let Some(ci) = &f.checks {
                    pr.ci = ci.clone();
                }
            }
        }
    }
}

/// 状态栏左下角那格推送状态的阶段。平时 `None`，发推送时 `Pushing`，落地后转 `Done`/`Failed`
/// 并带上 egui 时间（秒）当作计时起点，超过几秒由 `status_bar` 自动清除。
pub(crate) enum PushPhase {
    Pushing,
    Done(f64),
    Failed(f64),
}

pub(crate) enum Msg {
    Loaded { repo: String, snap: Box<Snapshot> },    /// 日志的下一页。带 `seq`：切仓库/换排序之后旧页作废，不能往新列表尾巴上接。
    More { repo: String, seq: u64, commits: Vec<Commit> },
    CommitFiles { repo: String, index: usize, files: Vec<DiffEntry>, search: bool },
    /// 历史页那一格搜索的回复。`seq` 是发出去那一次的号：切仓库、换范围、又敲一次都会
    /// 让它变，对不上号的回复直接丢——不然人会看到「搜 A 出 B」。
    /// `query` 带回发出去那个词，落地时对一眼：词已经改了的回复同样作废（`search_q != query`）。
    Search { repo: String, seq: u64, query: String, commits: Vec<Commit> },
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
    /// 推送这一路**单独**回的字：成功 `Ok(())`、失败 `Err(原因)`。状态栏靠它报
    /// 「正在推送 / 推送完成 / 推送失败」，而不是只弹一条 toast。
    PushDone(std::result::Result<(), String>),
    /// 克隆回来的只有「成了在哪」或者「为什么没成」，没有快照——成了之后由 `add_repo`
    /// 自己去开那个仓库（它会再发一次 `Msg::Loaded`）。
    Cloned(std::result::Result<String, String>),
    /// 归档成了，`String` 是落地的那个路径。失败那一路共用 `Msg::Failed`（它已经把
    /// `busy` 落回来、也把原文进了 toast），这里只管成功那一条。
    Archived(String),
    /// 看板取数的回复。`seq` 对不上 `Gitx::board_seq` 的一律丢：切仓库、按刷新、
    /// 再点进看板都会重新编号，旧回复落地就会把 A 仓库的列表挂在 B 仓库名下。
    /// 成功那一路带回来的已经是解析好的列表——解析归 provider（有 fixture 判据），
    /// 这里只管搬运。
    PullRequests { seq: u64, prs: std::result::Result<Vec<gitx::models::PullRequest>, String> },
    /// 单条 PR 详情落回的那一格 `mergeable_state`（列表端点**不给**这一格，实测）。
    /// 带 `sha`：作者在这一趟在飞时推了新提交，回来那一版已经不是人现在看的那一版，
    /// 把它涂进行会挂出一个假的「可以合并」。
    PrDetail { seq: u64, number: u32, sha: String, r: std::result::Result<String, String> },
    /// head 那一版提交的检查汇总（已经在后台线程 `aggregate_ci` 过了）。
    PrChecks { seq: u64, number: u32, sha: String, r: std::result::Result<gitx::models::Ci, String> },
}

pub struct Gitx {
    pub(crate) conn: Option<Connection>,
    pub(crate) repos: Vec<Repo>,
    /// 当前仓库路径，取自 `repos` 里的 `path`。
    pub(crate) repo: Option<String>,
    /// 顶栏那个**固定在最左**的「首页」tab 选中了。选中时主区换成仓库列表，
    /// 侧栏、日志、暂存带整块让位（见 `Gitx::home_page`）。
    /// 产品默认停在首页；点任一仓库 tab / `open()` 就离开，再点「首页」回来。
    pub(crate) home: bool,
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
    /// 状态栏左下角的推送状态（见 `PushPhase`）。平时 `None`，发推送期间 `Pushing`，
    /// 落地后 `Done`/`Failed` 并带时间戳，几秒后由 `status_bar` 清回 `None`。
    pub(crate) push_state: Option<PushPhase>,
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
    /// 历史页那一格搜索**开着吗**。单独一个布尔而不是「有命中就算开着」：搜了但零命中时
    /// 那一格必须还留着，不然人会以为搜索自己关掉了，再敲一遍同样的词。
    pub(crate) search_open: bool,
    /// 搜索框里的词。和 `log_filter`（日志条那格「转到Commit的SHA-1」，只在**已加载**的
    /// 那几页里筛）是两件事：这一格回车之后打到 git，找的是整本历史 + 文件名。
    pub(crate) search_q: String,
    /// 上一次搜索的命中（两路合并、新的在上）。空列表 = 没搜到，也 = 还没搜过。
    /// 药丸和日期文案**在落地那一刻算好**：那本账是「命中数 × 引用数」遍字符串比较，
    /// 和快照里 `badges` / `dates` 同一个理由（见 `ref_badges` 上面那段注释）——每帧现算
    /// 就是拿它换一次滚动，而这一档命中上限是 400 条。
    pub(crate) search_hits: Vec<SearchHit>,
    /// 搜索回复在路上。不立这块牌子，回车之后到落地之间那几帧会连着发好几遍同一个查询。
    pub(crate) search_busy: bool,
    /// 搜索的序号：一次回车加一，`open()` 换仓库也加一（旧回复作废，理由同 `log_seq`）。
    pub(crate) search_seq: u64,
    /// 「这一帧要不要把光标送进搜索框」。和 `modal_focus` 同一套：那一格是在面板闭包里
    /// 才存在的，而请求焦点要在它存在之后——所以「刚点开搜索」这件事得先记一笔，下一帧消费掉。
    pub(crate) search_focus: bool,
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
    /// 界面语言。`shell()` 每帧开头把它刷进线程局部（见 `gitx::lang`），
    /// 于是 400 多处 `tr!` 不用把档位一路当参数传；后台线程由 `lang::spawn` 带走同一份。
    pub(crate) lang: gitx::lang::Lang,
    /// 代码托管的访问令牌，按 provider id 存（同 `Prefs::tokens`）。看板的 PR 列表、
    /// 以及这一批还不做的 CI 状态都从这儿取钥匙。
    pub(crate) provider_tokens: Vec<(String, String)>,
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
    /// 已经点过「还有 N 个」、整组摊开的远端名。默认空 = 每个远端只露前 3 条分支。
    pub(crate) remote_all: Vec<String>,
    /// 左栏当前画的是仓库大纲还是**看板**（provider 的 PR 列表）。看板整栏替掉大纲
    /// （DESIGN §2.2），切过去那一刻起一次取数。
    pub(crate) board_open: bool,
    pub(crate) board: Board,
    /// 看板的取数号：切仓库、按「刷新」、再点进看板都会 +1。落地的回复对不上号就丢——
    /// 否则人会看到「A 仓库的列表挂在 B 仓库名下」（同 `search_seq` 那一招）。
    pub(crate) board_seq: u64,
    /// 看板那一击要在浏览器里打开的地址。攒在字段里、由 `poll` 推给 egui 的命令队列：
    /// `land` 手上没有 `Context`，而起进程 `open <url>` 会让跑测试真的弹出浏览器
    /// （同差异区复制走 `CopyText` 那一条路）。
    pub(crate) board_open_url: Option<String>,
    pub(crate) tx: Sender<Msg>,
    pub(crate) rx: Receiver<Msg>,
}
