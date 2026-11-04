//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use eframe::egui::{self, Align, Align2, Color32, FontId, Label, Margin, Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, Vec2, pos2, vec2};
use gitx::lang::Lang;
use gitx::models::{FileStatus, Repo};
use gitx::{db, git, provider, tr, GitxError};
use std::sync::mpsc::channel;

use crate::ui::theme::*;
use crate::ui::state::*;
use crate::ui::widgets::*;
use crate::ui::data::*;
use crate::ui::diffmodel::*;

/// 开仓时日志默认走哪一档范围。
///
/// 单独成一个函数是因为**判据读不到产品构造函数**：无头夹具自己填 `Gitx` 的字段
/// （见 `geometry.rs::shell`），把这一位改坏不会让任何一帧变红。和方法论第 11 条
/// 里 `viewport()` 同一招——读配置对象，别读画布。
/// 钉的是「别把默认偷偷换成 `--all`」：那样每个仓库一开就把别的分支的提交混进用户
/// 第一眼看到的列表，而他没点过任何筛选。
pub(crate) fn default_scope() -> git::Scope {
    git::Scope::Current
}

/// 侧栏「远端」开局展开哪一条。用户 2026-10-26 的要求是「origin 默认展示分支」，
/// 所以这里给 `origin`；其余远端仍旧收着，一下铺开整列会把「标签 / 已贮藏」挤下去。
///
/// 和 `default_scope()` 同一个理由单独成函数：无头夹具自己填 `Gitx` 的字段，
/// 把 `Gitx::new` 里的初值改坏不会有任何一帧变红。判据通过这个函数摆初值，
/// 产品也走同一个函数——两边同源，改坏才会真的红。
pub(crate) fn default_remote_open() -> Vec<String> {
    vec!["origin".to_owned()]
}

/// 看板上「看得见的行」补取详情/检查时，同时在飞的请求上限。
/// 每条 PR 两个请求，一次刷屏八九行就是十几个；不限并发会把速率预算一次打空
/// （GitHub 认证请求 5000/小时，见 PROCESS 同日实测）。
pub(crate) const ENRICH_IN_FLIGHT: usize = 6;

/// `mergeable_state = unknown` 的那一格自愈：等多久、最多试几次。
/// GitHub 那一格是后台算的，第一次打经常还没有；不重试的话看板会一直挂着
/// 「状态还没算出来」，而那恰恰是它**已经能回答**之后的样子。
pub(crate) const DETAIL_RETRY_SECS: f64 = 4.0;
pub(crate) const DETAIL_RETRY_MAX: u8 = 2;

/// 顶栏右边那一组会占多宽：11 枚 28 的按钮 + 它们之间的 2 + 组间距 14 + 分支名 · `↑↓`。
/// 它是**有界**的一串（`TOOLS.len()` 定死），所以收纳只发生在左边那串无界的仓库 tab 上。
/// 每一项都按「宽 + 一段间距」计：宁可多算 2pt，也不要出现「算下来放得下、画出来压住」。
pub(crate) fn header_right_group_width(ui: &egui::Ui, head: &Option<(String, Option<String>)>) -> f32 {
    let mut w = TOOLS.len() as f32 * (ICON_BTN + ICON_BTN_GAP) + HEADER_GROUP_GAP;
    if let Some((branch, counts)) = head {
        w += text_w(ui, branch, FontId::proportional(BRANCH_FONT)) + BRANCH_GAP;
        if let Some(c) = counts {
            w += text_w(ui, c, counts_font()) + BRANCH_GAP;
        }
    }
    w
}

/// §2 那句「窗口收窄时溢出收进 `»` 菜单」的算法：**按顺序**能放几枚放几枚，剩下的一律收进去。
/// 只收前缀是硬的——书签列表的顺序就是用户的读法，不能因为收纳抽走第 3 枚却留下第 4 枚。
/// `»` 自己也要占一格，所以「有东西被收走」不是免费的：先当没有 `»` 排一遍，只要少了一枚，
/// 就把那一格从预算里再扣掉重排——宁可少画一枚，也不要两串撞在一起。
/// 返回 `(留下的枚数, 要不要画 »)`。「首页」和「添加仓库」不在 `widths` 里：那两枚永远画得出。
pub(crate) fn fit_header_tabs(widths: &[f32], budget: f32, more_w: f32) -> (usize, bool) {
    let take = |cap: f32| -> usize {
        let mut used = 0.0;
        let mut n = 0;
        for w in widths {
            if used + w + TAB_GAP > cap {
                break;
            }
            used += w + TAB_GAP;
            n += 1;
        }
        n
    };
    let n = take(budget);
    if n == widths.len() {
        (n, false)
    } else {
        (take(budget - more_w - TAB_GAP).min(n), true)
    }
}

/// 看板那一行状态的颜色：**只给挡路的那件事上色**（§原则①「颜色只给状态和层级，不装饰」）。
/// 认不出的状态一律次要灰——「还没算出来」不该看起来像一件要办的事。
///
/// 吃的是 `provider::PrStatus` 而不是 `mergeable_state` 那个原始取值：同一句话的文案
/// 与颜色必须有**一个**主人，否则加了新档位时改了一处、另一处还在按旧表匹配。
fn board_status_color(status: provider::PrStatus) -> Color32 {
    use provider::PrStatus as S;
    match status {
        S::Conflicts => DANGER,
        S::MergeReady => ADD_FG,
        // 等评审与落后于目标分支都是「事情在别人手里 / 要先动一步」，同一档被拦色。
        S::AwaitingReview | S::Behind => ACCENT_INK,
        // 真挂了的检查才值得黄色；「在别的 PR 上也挂」和「不挡合并」都是**别报警**的意思，
        // 报成黄色就等于把一件不必办的事涂成要办的事（§原则①）。
        S::CiFailed { .. } => WARN_FG,
        S::CiFailedShared { .. } | S::CiWarned { .. } | S::Draft => TEXT_MUTED,
        S::CiRunning { .. } | S::CiNone | S::Closed | S::Merged | S::Unknown | S::Fetching => {
            TEXT_SECONDARY
        }
    }
}

/// 存盘面板里那格建议文件名的前半段：路径最后一段当仓库名。
/// `Path::file_name` 自己会忽略尾斜杠（`/a/b/` 也给出 `b`），所以不先 trim。
/// 归档的 zip 落到用户自己选的位置，这个名字只是**默认值**，面板里可以改。
fn repo_name(repo: &str) -> String {
    std::path::Path::new(repo)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| repo.to_owned())
}

/// 首页「代码托管」那一格要显示什么：只在**当前仓库的快照**里认远端，不额外跑 git。
/// 返回（平台名，`owner/repo`，打开主页要用到的远端名）。
/// 认不出（不是 GitHub/Gitee/Codeup，或地址不是 `主机/所有者/仓库`）就整块不画——
/// 摆一个「未知平台」的空壳比不摆更吵。
fn home_provider(snap: &Snapshot) -> Option<(&'static str, String, String)> {
    let urls: Vec<String> = snap.remotes.iter().map(|r| r.url.clone()).collect();
    let host = provider::detect(&urls)?;
    let repo = provider::repo_ref_for(host, &urls)?;
    let remote = snap.remotes.iter().find(|r| host.handles(&r.url))?.name.clone();
    Some((host.label(), repo.path, remote))
}

impl Gitx {
    pub(crate) fn new() -> Self {
        let (tx, rx) = channel();
        let mut conn = db_path().and_then(|p| match db::open(&p) {
            Ok(c) => {
                let _ = db::migrate(&c);
                Some(c)
            }
            Err(e) => {
                eprintln!("gitx-app: 打不开数据库 {p}: {e}");
                None
            }
        });
        let repos = conn.as_mut().and_then(|c| db::list_repos(c).ok()).unwrap_or_default();
        // 三枚开关在**构造之前**读出来：`topo`/`show_remote` 直接进字段，`restore_last`
        // 决定下面要不要恢复上次那本。走不到 `Gitx::new` 的判据读不到这些初值，
        // 所以「读配置」这件事由 `Prefs::load` 自己承担（见 `state.rs` 上那段注释）。
        let prefs = Prefs::load(conn.as_ref());
        let mut app = Gitx {
            conn,
            repos,
            repo: None,
            // 首页是落点：有仓库且开了「恢复上次」时，下面 `open()` 会把它翻成 false。
            home: true,
            snap: None,
            sel: Sel::Uncommitted,
            commit_files: Vec::new(),
            file: None,
            diff: Vec::new(),
            diff_rows: Vec::new(),
            collapsed: std::collections::HashSet::new(),
            diff_search: String::new(),
            diff_cols: 0,
            patch_label: String::new(),
            seq: 0,
            busy: false,
            push_state: None,
            note: None,
            note_at: None,
            log_filter: String::new(),
            side_filter: String::new(),
            topo: prefs.topo,
            scope: default_scope(),
            show_remote: prefs.show_remote,
            // 真壳一上来就允许翻页：第一页取回不满一页时，`Msg::More` 会把这块牌子拧死。
            log_more: true,
            log_more_busy: false,
            log_seq: 0,
            // 搜索一上来是关着的：那一格连日志条都不在（点开日志条右端的「搜索」才换上去）。
            search_open: false,
            search_q: String::new(),
            search_hits: Vec::new(),
            search_busy: false,
            search_seq: 0,
            search_focus: false,
            commit_msg: String::new(),
            commit_desc: String::new(),
            amend: false,
            clear_msg_on_load: false,
            restore_last: prefs.restore_last,
            push_behavior: prefs.push_behavior,
            default_remote: prefs.default_remote,
            push_tags: prefs.push_tags,
            allow_force: prefs.allow_force,
            lang: prefs.lang,
            provider_tokens: prefs.tokens,
            // 看板开局是关着的：左栏画仓库大纲。取数只在用户切过去那一刻起，
            // 开仓库不连带问一次网络（国内网络下那一次会挂 15 秒）。
            board_open: false,
            board: Board::default(),
            board_seq: 0,
            board_open_url: None,
            target: None,
            pending_discard: None,
            history: None,
            hist_seq: 0,
            modal: None,
            modal_focus: false,
            pending_dialog: None,
            last_branch: String::new(),
            remote_open: default_remote_open(),
            remote_all: Vec::new(),
            tx,
            rx,
        };
        // 上次开过的仓库直接恢复（存在 core 那张 settings 表里，和 React 版共用同一份）。
        // 偏好卡上那枚开关关掉之后，这条路整个不走——不是「恢复后再关掉」。
        if app.restore_last {
            let last = app
                .conn
                .as_ref()
                .and_then(|c| db::get_setting(c, "gitx_last_repo").ok().flatten());
            let known = last.filter(|p| app.repos.iter().any(|r| &r.path == p));
            let start = known.or_else(|| app.repos.first().map(|r| r.path.clone()));
            if let Some(path) = start {
                app.open(&path);
            }
        }
        app
    }

    /// 把一个路径登记成书签并立刻打开。填到仓库里的子目录也算——`find_repo_root` 往上走，
    /// 再按核心层的规矩用 `is_repo` 复核一次（ stray `.git` 最多换来一次失败的添加）。
    /// 没有数据库时照样开：那只在内存里记一份，本次会话能用。
    pub(crate) fn add_repo(&mut self, path: &str) -> bool {
        let path = path.trim();
        let root = git::find_repo_root(path).filter(|r| git::is_repo(r));
        let Some(root) = root else {
            self.note = Some(tr!(format!("那不是仓库：{path}"), format!("Not a repository: {path}")));
            return false;
        };
        if !self.repos.iter().any(|r| r.path == root) {
            let name = std::path::Path::new(&root)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.clone());
            if let Some(c) = &self.conn {
                let _ = db::add_repo(c, &root, &name);
            }
            self.repos.push(Repo {
                id: None,
                path: root.clone(),
                name,
                added_at: None,
                last_fetched_at: None,
                group_name: String::new(),
            });
        }
        self.open(&root);
        true
    }

    pub(crate) fn open(&mut self, path: &str) {
        self.repo = Some(path.to_string());
        // 打开仓库 = 离开首页。点仓库 tab、添加仓库、克隆回来都汇到这一条路上，
        // 所以在入口处翻这一位，而不是在每个调用点各写一遍。
        self.home = false;
        if let Some(c) = &self.conn {
            let _ = db::set_setting(c, "gitx_last_repo", path);
        }
        self.snap = None;
        self.sel = Sel::Uncommitted;
        self.commit_files.clear();
        self.file = None;
        self.diff.clear();
        self.diff_rows.clear();
        self.collapsed.clear();
        self.diff_search.clear();
        self.patch_label.clear();
        // 分页从头再来：序号加一是让**上一轮**在途的页回复作废（对不上号就丢），
        // 同时把「还有下一页」重新拧成真——换仓库/换排序后那是另一份列表。
        self.log_seq += 1;
        self.log_more = true;
        self.log_more_busy = false;
        // 搜索整档跟着仓库清零：上一本的命中列表对着这一本的提交号，画出来是假结果。
        // 序号加一同时让**在途**的那次查询作废（`Msg::Search` 对不上号就丢）。
        self.search_seq += 1;
        self.search_open = false;
        self.search_q.clear();
        self.search_hits.clear();
        self.search_busy = false;
        // 历史面板跟着仓库走：上一个仓库的文件路径在这里没有意义，在途的回复也作废。
        self.history = None;
        self.hist_seq += 1;
        self.busy = true;
        spawn_snapshot(&self.tx, path, self.topo, &self.scope);
        // 开仓库那一帧就要把差异取回来：日志第一行「Uncommitted changes」已经是选中态了，
        // 右边却写着「没有差异」——真窗口 2026-10-23 16:16 拍到的正是这张。选中行和
        // 差异区各说各话，无头判据全绿（它们都是先点一行再看，没人看过只 `open()` 的壳）。
        self.request(Target::Working { staged: false, path: None, untracked: false });
    }

    /// 筛选条上那枚「层级顺序」。换排序 = 重新取一次列表：图序列的分组跟着变，
    /// 但行距、带界这些几何不动。
    pub(crate) fn set_topo(&mut self, on: bool) {
        if self.topo == on {
            return;
        }
        self.topo = on;
        let Some(repo) = self.repo.clone() else { return };
        self.open(&repo);
    }

    /// 筛选条最左那枚「所有分支 / 当前分支」：换范围 = 重新取一次列表，和换排序同一个理由。
    /// 先比后写，在同一档上再点一次不该把快照整个丢掉。
    pub(crate) fn set_scope(&mut self, scope: git::Scope) {
        if self.scope == scope {
            return;
        }
        self.scope = scope;
        let Some(repo) = self.repo.clone() else { return };
        self.open(&repo);
    }

    /// 日志滚到了最底下：取下一页。两道闸——知道到底了就不问、上一页还在路上不问。
    /// 参数一次抓齐：闭包跑在 `snap` 的借用里，碰不到 `self`。
    pub(crate) fn ask_for_more(&mut self) {
        if !self.log_more || self.log_more_busy {
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        let Some(snap) = self.snap.as_ref() else { return };
        let skip = snap.commits.len() as u32;
        self.log_more_busy = true;
        let (seq, topo, scope, tx) = (
            self.log_seq,
            self.topo,
            eff_scope(&self.scope, snap.status.detached_at.is_some()),
            self.tx.clone(),
        );
        gitx::lang::spawn(move || {
            let _ = tx.send(match git::commits_page(&repo, skip, LOG_PAGE, topo, &scope) {
                Ok(commits) => Msg::More { repo, seq, commits },
                Err(e) => Msg::Failed(format!("{repo}: {e}")),
            });
        });
    }

    /// 打开搜索那一档（唯一入口是日志条右端那枚「搜索」小按钮）：把右边那一格换成搜索框。
    /// 已经开着就不动命中列表——人只是又点了一下那枚按钮（想不起搜索在哪），
    /// 把上一次的结果抹掉等于告诉他「你刚才搜的那一下不算」。
    pub(crate) fn open_search(&mut self) {
        self.search_open = true;
        self.search_focus = true;
    }

    /// 关掉这一档：词与命中一起清，在途那次查询也作废（序号加一）。
    /// 留着上一次的词会让「再打开时那一格写着什么」变成看运气——列表已经跟着仓库清了。
    pub(crate) fn close_search(&mut self) {
        self.search_open = false;
        self.search_q.clear();
        self.search_hits.clear();
        self.search_busy = false;
        self.search_seq += 1;
    }

    /// 搜索框里回车那一下：把词打到 git（说明一路 + 文件名一路）。
    /// 空词不发查询，直接把命中清空——`git log --grep=` 那个空串是**全命中**，
    /// 画出来会看着像「搜索坏了，把整本历史都算成结果」。
    pub(crate) fn run_search(&mut self) {
        let q = self.search_q.trim().to_owned();
        self.search_seq += 1;
        let seq = self.search_seq;
        if q.is_empty() {
            self.search_hits.clear();
            self.search_busy = false;
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        let detached = self
            .snap
            .as_ref()
            .map(|s| s.status.detached_at.is_some())
            .unwrap_or(false);
        // 范围跟着日志条**当前那一档**（所有分支 / 当前分支 / 某条支线）：搜索结果和
        // 用户正在看的那本历史得是同一批提交，不然「我明明在 main 上，搜出来的却是
        // 支线独有的那条」。
        let scope = eff_scope(&self.scope, detached);
        self.search_busy = true;
        spawn_search(&self.tx, &repo, &q, &scope, seq);
    }

    pub(crate) fn oid(&self, i: usize) -> Option<String> {
        self.snap.as_ref().and_then(|s| s.commits.get(i)).map(|c| c.id.clone())
    }

    /// 唯一一处「选行」：日志行、搜索结果行、侧栏「文件状态 / 历史 / 已贮藏」都走这里。
    pub(crate) fn select(&mut self, sel: Sel) {
        // 三份列表共用一个下标位，所以「这一格是从哪本列表数出来的」必须**跟着走**：
        // 只按下标回话，上一本的在途回复就能盖到这一本的选中行上（见 `Msg::CommitFiles.search`）。
        let (index, stash, search) = match &sel {
            Sel::Commit(i) => (*i, false, false),
            Sel::Search(i) => (*i, false, true),
            Sel::Stash(i) => (*i, true, false),
            Sel::Uncommitted => {
                self.sel = Sel::Uncommitted;
                self.file = None;
                self.commit_files.clear();
                self.request(Target::Working { staged: false, path: None, untracked: false });
                return;
            }
        };
        // 贮藏不在日志里，得从侧栏那份列表取它的提交号；提交则按下标问日志 / 问命中。
        let oid = if stash {
            self.snap.as_ref().and_then(|s| s.stashes.get(index)).map(|s| s.oid.clone())
        } else if search {
            self.search_hits.get(index).map(|h| h.commit.id.clone())
        } else {
            self.oid(index)
        };
        let Some(oid) = oid else { return };
        let Some(repo) = self.repo.clone() else { return };
        self.sel = sel;
        self.file = None;
        self.commit_files.clear();
        let tx = self.tx.clone();
        {
            let (repo2, oid2) = (repo.clone(), oid.clone());
            gitx::lang::spawn(move || {
                let files = if stash {
                    git::stash_diff(&repo2, &oid2)
                } else {
                    git::commit_diff(&repo2, &oid2)
                };
                match files {
                    Ok(files) => {
                        let msg = if stash {
                            Msg::StashFiles { repo: repo2, index, files }
                        } else {
                            Msg::CommitFiles { repo: repo2, index, files, search }
                        };
                        let _ = tx.send(msg);
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Failed(format!("{oid2}: {e}")));
                    }
                }
            });
        }
        self.request(if stash {
            Target::Stash { oid, path: None }
        } else {
            Target::Commit { oid, path: None }
        });
    }

    /// 起一次差异请求。`label` 先算好：它只取决于请求本身，不取决于回复。
    /// 顺手把 `target` 记下来：hunk 那两枚按钮要照着它决定文案，
    /// 一次 git 动作之后也要照它把这一格差异重新取回来。
    pub(crate) fn request(&mut self, target: Target) {
        let Some(repo) = self.repo.clone() else { return };
        self.seq += 1;
        self.pending_discard = None;
        self.target = Some(target.clone());
        let (seq, label, tx) = (self.seq, patch_label(&target), self.tx.clone());
        // 整份差异里要画文件头带：多文件时它是唯一的分界；单文件差异顶上的标题已经写着路径。
        let show_files = match &target {
            Target::Working { path, .. } | Target::Commit { path, .. } | Target::Stash { path, .. } => path.is_none(),
        };
        self.busy = true;
        gitx::lang::spawn(move || {
            let lines = patch_of(&repo, &target).map(|p| layout(&p, show_files)).unwrap_or_default();
            let _ = tx.send(Msg::Patch { repo, seq, label, lines });
        });
    }

    /// 起一次克隆。`clone_url` 和 `remote_url` 给同一个：核心层留了「用加速节点抓、
    /// 落地的 origin 指真地址」那个口子，这个壳还没有节点概念，就不自己编一个。
    pub(crate) fn start_clone(&mut self, url: String, dest: String) {
        self.busy = true;
        let tx = self.tx.clone();
        gitx::lang::spawn(move || {
            let r = git::clone_repo(&url, &url, &dest).map_err(|e| format!("{e}"));
            let _ = tx.send(Msg::Cloned(r));
        });
    }

    /// 切到看板（左栏整栏换掉，DESIGN §2.2）。三步都在 `land` 里跑、不在帧里：
    /// 认 provider 是纯字符串解析（用快照里已有的远端 URL，不为这一眼再 fork 一次 git），
    /// 「看过」那份记录要读库，取数交给线程。
    pub(crate) fn show_board(&mut self) {
        self.board_open = true;
        let urls: Vec<String> = self
            .snap
            .as_ref()
            .map(|s| s.remotes.iter().map(|r| r.url.clone()).collect())
            .unwrap_or_default();
        let host = provider::detect(&urls);
        let repo_ref = host.and_then(|h| provider::repo_ref_for(h, &urls));
        self.board = Board { host, repo_ref, ..Default::default() };
        // 「看过」按 provider + `owner/repo` 存：切走再回来不该看见满屏蓝点——
        // 那枚点的意思正是「你上次看之后它又动了」，每次进看板都全亮就是没有意思。
        let seen: Option<String> = self
            .board
            .seen_key()
            .and_then(|k| self.conn.as_ref().and_then(|c| db::get_setting(c, &k).ok().flatten()));
        if let Some(json) = seen {
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<u32, String>>(&json) {
                self.board.seen = map;
            }
            // 读不懂就当没看过：一次写坏的记录不该把蓝点永久钉死（同 `flag()` 那条规矩）。
        }
        self.fetch_board();
    }

    /// 起一次看板取数。请求由 provider 拼（那一步有 fixture 判据），这里只管发与收。
    /// 没绑令牌 / 认不出 provider → **不发请求**，把原因直接落在看板上：转圈转到超时
    /// 再报错，比一上来就写「未绑定 GitHub 令牌」慢了一整圈。
    pub(crate) fn fetch_board(&mut self) {
        let (Some(host), Some(repo_ref)) = (self.board.host, self.board.repo_ref.clone()) else {
            self.board.error = Some(
                tr!(
                    "这本仓库的远端不是认得出的代码托管（GitHub / Gitee）。",
                    "This repository's remote is not a hosting service we recognise (GitHub / Gitee)."
                )
                .to_owned(),
            );
            self.board.loaded = true;
            return;
        };
        let token = self
            .provider_tokens
            .iter()
            .find(|(id, _)| *id == host.id())
            .map(|(_, t)| t.clone())
            .unwrap_or_default();
        let req = match host.pull_requests_request(&repo_ref, &token) {
            Ok(r) => r,
            Err(e) => {
                self.board.error = Some(e.to_string());
                self.board.loaded = true;
                return;
            }
        };
        self.board_seq += 1;
        let seq = self.board_seq;
        self.board.fetching = true;
        self.board.error = None;
        // 列表重取：每一行的补充请求都要重新允许发一次（在飞的那些会因为号对不上被丢，
        // 不清这本账就永远没人再问）。**结果缓存 `facts` 留着**：同一版提交不必问第二遍。
        self.board.asked.clear();
        let tx = self.tx.clone();
        gitx::lang::spawn(move || {
            let r = gitx::net::execute(&req)
                .and_then(|body| host.parse_pull_requests(&body))
                .map_err(|e| e.to_string());
            let _ = tx.send(Msg::PullRequests { seq, prs: r });
        });
    }

    /// 给**看得见的**那几行补上列表不给的两格：`mergeable_state`（按号取详情）和
    /// 检查汇总（按 head 的 sha 取）。
    ///
    /// 为什么按可见行而不是整屏一起取：这两个端点都是**每条一次**，50 条开放 PR 就是
    /// 100 个请求，一次刷新打不完、也不该打——用户扫一眼看板只需要头顶那几行的状态。
    /// 滚下去才补下去，取过的一次不再取（`asked` / `facts` 两本账）。
    /// 上限 `ENRICH_IN_FLIGHT`：一次刷屏若不限并发，会把带宽和平台的速率预算一起打爆。
    pub(crate) fn enrich_rows(&mut self, indices: &[usize]) {
        let (Some(host), Some(repo_ref)) = (self.board.host, self.board.repo_ref.clone()) else {
            return;
        };
        let caps = host.capabilities();
        let wants_detail = caps.contains(&provider::Capability::PullRequestDetail);
        let wants_checks = caps.contains(&provider::Capability::CiStatus);
        if !wants_detail && !wants_checks {
            return; // 这一家不给：一行请求都不发，状态列自己会说「还没算出来」
        }
        let token = self
            .provider_tokens
            .iter()
            .find(|(id, _)| *id == host.id())
            .map(|(_, t)| t.clone())
            .unwrap_or_default();
        // 没令牌 = 一行都不问。和 `fetch_board` 同一条规矩（「没令牌不发请求，把原因当场
        // 写在栏上」）：逐行去拼请求会把同一句话写成 N 份，更糟的是把「拼不出来」记成
        // 「这一行取过了」—— 用户回头绑上令牌、按一下刷新，那些行就永久 blank。
        if token.trim().is_empty() {
            self.board.error.get_or_insert_with(|| {
                tr!(
                    format!("未绑定 {} 的令牌，请在「设置 → 代码托管」中填写。", host.label()),
                    format!("No {} token bound — fill one in under Preferences → Hosting.", host.label())
                )
                .to_owned()
            });
            return;
        }
        // 在飞的账见 `Board::outstanding`（并发上限和「首次建账走完了没」用的是同一个数）。
        let seq = self.board_seq;
        let tx = self.tx.clone();
        for &i in indices {
            if self.board.outstanding() >= ENRICH_IN_FLIGHT {
                return;
            }
            let Some(pr) = self.board.prs.get(i).cloned() else { continue };
            if pr.head_sha.is_empty() {
                continue; // 没有 sha 就拼不出第二个请求的 URL；不猜一个
            }
            let key = (pr.number, pr.head_sha.clone());
            let facts = self.board.facts.get(&key).cloned().unwrap_or_default();
            for ask in [Ask::Detail, Ask::Checks] {
                let already_have = match ask {
                    Ask::Detail => facts.detail.is_some(),
                    Ask::Checks => facts.checks.is_some(),
                };
                let allowed = match ask {
                    Ask::Detail => wants_detail,
                    Ask::Checks => wants_checks,
                };
                if already_have || !allowed || self.board.asked.contains(&(key.clone(), ask)) {
                    continue;
                }
                let built = match ask {
                    Ask::Detail => host
                        .pull_request_detail_request(&repo_ref, &token, pr.number)
                        .map_err(|e| e.to_string()),
                    Ask::Checks => host
                        .checks_request(&repo_ref, &token, &pr.head_sha)
                        .map_err(|e| e.to_string()),
                };
                let req = match built {
                    Ok(r) => r,
                    Err(e) => {
                        // 拼不出请求 = 这一行永远不会有结果。记成「已经回过了」，
                        // 否则每帧都重拼一次、每帧都失败一次。
                        let f = self.board.facts.entry(key.clone()).or_default();
                        match ask {
                            Ask::Detail => f.detail = Some(String::new()),
                            Ask::Checks => f.checks = Some(gitx::models::Ci::Unknown),
                        }
                        self.board.error.get_or_insert(e);
                        continue;
                    }
                };
                self.board.asked.insert((key.clone(), ask));
                let number = key.0;
                let sha = key.1.clone();
                let tx = tx.clone();
                match ask {
                    Ask::Detail => {
                        gitx::lang::spawn(move || {
                            let r = gitx::net::execute(&req)
                                .and_then(|b| host.parse_pull_request_detail(&b))
                                .map(|one| one.mergeable_state)
                                .map_err(|e| e.to_string());
                            let _ = tx.send(Msg::PrDetail { seq, number, sha, r });
                        });
                    }
                    Ask::Checks => {
                        gitx::lang::spawn(move || {
                            let r = gitx::net::execute(&req)
                                .and_then(|b| host.parse_checks(&b))
                                .map(|checks| gitx::provider::aggregate_ci(&checks))
                                .map_err(|e| e.to_string());
                            let _ = tx.send(Msg::PrChecks { seq, number, sha, r });
                        });
                    }
                }
            }
        }
    }

    /// 看板那一击办两件事：把「看到这儿了」落库，请在浏览器里打开那条 PR。
    /// 走 `Context::open_url` 而不是 `open <url>`：前者进 egui 的命令队列，
    /// 无头门禁量得到这一条命令，也不会因为跑测试真的弹开浏览器（同 `CopyText` 那一招）。
    pub(crate) fn open_pull_request(&mut self, index: usize) -> Option<String> {
        let Some(pr) = self.board.prs.get(index).cloned() else { return None };
        let host = self.board.host?;
        // 存的是**签名**而不是 `updated_at`：检查跑完不改 `updated_at`（实测），只存它的话
        // 「CI 从在跑到全绿」这次用户看得见的变化永远不会让蓝点亮回来。
        self.board.seen.insert(pr.number, pr.signature());
        self.persist_seen();
        let url = self
            .board
            .repo_ref
            .as_ref()
            .map(|r| host.pull_request_web_url(r, pr.number))
            .filter(|u| !u.is_empty())
            // provider 没给地址时退回它自己带的那条 `html_url`：两条都是空就是真没地址，
            // 这时候不推命令——推一条空 URL 等于用户按了没反应。
            .or_else(|| (!pr.url.is_empty()).then_some(pr.url.clone()))?;
        Some(url)
    }

    /// 「看过」那本账落库。点开一行和首次建账两条路写的是同一本账，所以那段
    /// 「键怎么拼、值怎么编」只写一处。
    pub(crate) fn persist_seen(&mut self) {
        let Some(key) = self.board.seen_key() else { return };
        let json = serde_json::to_string(&self.board.seen).unwrap_or_default();
        if let Some(c) = self.conn.as_mut() {
            let _ = db::set_setting(c, &key, &json);
        }
    }

    /// 起一次 `git init`。产物和克隆一样是「磁盘上多了一本仓库」，所以回复共用
    /// `Msg::Cloned`，由 `add_repo` 去登记并打开——两条路只需要一处「开新仓库」的代码。
    pub(crate) fn start_init(&mut self, path: String) {
        self.busy = true;
        let tx = self.tx.clone();
        gitx::lang::spawn(move || {
            let r = git::init_repo(&path).map_err(|e| format!("{e}"));
            let _ = tx.send(Msg::Cloned(r));
        });
    }

    /// 消费上一帧立起来的原生面板。**只在 `eframe::App::ui` 里被调，无头判据碰不到这里**
    /// （它们直接调 `shell()`）——这条边界就是「跑测试不会弹真面板」。
    /// 面板**取消**是正常结局：不报错、不立 `busy`、不留半成品，直接返回。
    /// 面板之后的那段接线一律交出去做（`archive_to`/`fill_folder`），因为这一位本身测不了。
    fn run_dialogs(&mut self) {
        let Some(dialog) = self.pending_dialog.take() else { return };
        match dialog {
            Dialog::Archive { repo, oid, name } => {
                let Some(path) = rfd::FileDialog::new().set_title(tr!("归档成 zip", "Archive as zip")).set_file_name(&name).save_file() else {
                    return;
                };
                self.archive_to(&repo, &oid, &path.to_string_lossy());
            }
            Dialog::PickFolder { field } => {
                let Some(path) = rfd::FileDialog::new().set_title(tr!("选一个目录", "Choose a directory")).pick_folder() else {
                    return;
                };
                self.fill_folder(field, &path.to_string_lossy());
            }
        }
    }

    /// 把 `oid` 那一版导成 `dest`。从 `run_dialogs` 里拆出来只为一件事：原生面板在无头帧里
    /// 一辈子跑不到，**面板之后那三个参数交得对不对**得有地方能被问一句
    /// （`archiving_a_row_hands_core_that_rows_oid_not_head`）。
    pub(crate) fn archive_to(&mut self, repo: &str, oid: &str, dest: &str) {
        self.busy = true;
        let (tx, repo, oid, dest) = (self.tx.clone(), repo.to_owned(), oid.to_owned(), dest.to_owned());
        gitx::lang::spawn(move || {
            let _ = match git::archive(&repo, &oid, &dest) {
                Ok(()) => tx.send(Msg::Archived(dest)),
                Err(e) => tx.send(Msg::Failed(format!("{e}"))),
            };
        });
    }

    /// 面板选回来的目录填进卡的**那一格**。卡已经不在了就不复活它——面板跑完可能隔了几帧，
    /// 中间人可能已经把卡关掉了。
    pub(crate) fn fill_folder(&mut self, field: FolderField, text: &str) {
        let Some(m) = self.modal.as_mut() else { return };
        match (&mut m.form, field) {
            // 只在**还停在克隆那一档**时才填目标目录：牌子立起来之后用户可能把档切走了，
            // 那时候这一格已经从卡上消失，填进去就是往一格看不见的格子里写字。
            (Form::AddRepo { tab, dest }, FolderField::CloneDest) => {
                if *tab == AddTab::Clone {
                    *dest = text.to_owned();
                }
            }
            (Form::AddRepo { .. }, FolderField::Main) => m.text = Some(text.to_owned()),
            _ => {}
        }
    }

    /// 发一条动作。**返回值 = 这一步有没有被接住**：`false` 只有两种当场就办不成的路
    /// （填的路径不是仓库、手上还没有仓库），调用方据此决定那张模态卡要不要留着。
    /// 起线程那几条一律 `true`——成败要等回复回来说，那时候卡本来就该关了。
    pub(crate) fn act(&mut self, action: Action) -> bool {
        // 克隆和新建是两条**手上还没有仓库**也要能跑的动作：它们要的 `repo` 正是自己的产物，
        // 所以这道守卫在它们之前放行不了，各单开一条路。
        match &action {
            Action::Clone { url, dest } => {
                let (url, dest) = (url.clone(), dest.clone());
                self.start_clone(url, dest);
                return true;
            }
            Action::Init { path } => {
                let path = path.clone();
                self.start_init(path);
                return true;
            }
            // 唯一一条**当场就知道成不成**的路（`find_repo_root` 是一次同步的 git 调用）。
            // 报 false 让调用方把那张卡留着：路径打错一个字母不必从头再打一遍。
            Action::AddExisting { path } => {
                let path = path.clone();
                return self.add_repo(&path);
            }
            _ => {}
        }
        // 到不了动作那一格，但这是「这一步没被接住」的第二种说法：手上没仓库，
        // 任何一条 git 都无处可跑。静默吞掉的话，卡关掉、什么也没发生、连提示都没有。
        let Some(repo) = self.repo.clone() else { return false };
        // 归档不在这里跑 git：先问用户存哪儿。只把 `pending_dialog` 立起来，面板由下一帧
        // `ui()` 最前面那段消费（嵌套事件循环不许插在 egui 画帧中间，见 `Dialog` 的注释）。
        // `busy` **不立**：面板随时可能被取消，取消没有回复，立了就没人把它落回来。
        if let Action::Archive(oid) = &action {
            let short: String = oid.chars().take(8).collect();
            let name = format!("{}-{short}.zip", repo_name(&repo));
            self.pending_dialog = Some(Dialog::Archive { repo, oid: oid.clone(), name });
            return true;
        }
        // 弹出/丢弃之后那条贮藏就没了，后面的 `stash@{n}` 会顶上来。继续按旧下标看，
        // 界面会静默换成**另一条**贮藏——选回工作区，下一份快照落地时差异也跟着回工作区。
        if matches!(action, Action::StashPop(_) | Action::StashDrop(_))
            && matches!(self.sel, Sel::Stash(_))
        {
            self.sel = Sel::Uncommitted;
            self.file = None;
            self.commit_files.clear();
            self.target = Some(Target::Working { staged: false, path: None, untracked: false });
        }
        self.busy = true;
        // 只有提交**成功**落地才清空那格信息：git 报错、pre-commit 钩子拦下来、空信息被 core 挡掉，
        // 都得把字留着——否则用户得重打一遍。
        self.clear_msg_on_load =
            matches!(action, Action::Commit(_) | Action::Amend(_) | Action::CommitAndPush { .. });
        // 推送这一路要让状态栏报「正在推送…」：进线程之前先记下来，线程里只能靠 `Msg` 通信，
        // 碰不到 `self`。`CommitAndPush` 也走这儿——「提交并推送」整段都算推送。
        if matches!(action, Action::PushRefs { .. } | Action::CommitAndPush { .. }) {
            self.push_state = Some(PushPhase::Pushing);
        }
        let tx = self.tx.clone();
        let (topo, scope) = (self.topo, self.scope.clone());
        gitx::lang::spawn(move || {
            let r = match &action {
                Action::Stage(p) => git::stage(&repo, p),
                Action::Unstage(p) => git::unstage(&repo, p),
                Action::Discard(p) => git::discard(&repo, p),
                Action::StageAll => git::stage_all(&repo),
                Action::UnstageAll => git::unstage(&repo, &[]),
                Action::StageHunk { path, hunk } => git::stage_hunk(&repo, path, *hunk),
                Action::UnstageHunk { path, hunk } => git::unstage_hunk(&repo, path, *hunk),
                Action::DiscardHunk { path, hunk } => git::discard_hunk(&repo, path, *hunk),
                Action::Commit(m) => git::commit(&repo, m, false, &[], false, false).map(|_| ()),
                Action::Amend(m) => git::commit(&repo, m, true, &[], false, false).map(|_| ()),
                // 先落提交再推：推送失败时报错，但提交**已经在**了，不往回滚——
                // 回滚一条已经落地的提交比留着它危险得多。
                Action::CommitAndPush { msg, amend } => git::commit(&repo, msg, *amend, &[], false, false)
                    .map(|_| ())
                    .and_then(|_| git::push(&repo, None, None, false, false)),
                Action::Checkout(name) => git::checkout_branch(&repo, name),
                // 侧栏「远端」那一条：带的是整条远程 ref 名（`origin/main`），落到 git 就是
                // 切到它对应的本地分支——没有同名本地分支时由 `checkout_remote_branch` 建一条。
                Action::CheckoutRemote(name) => git::checkout_remote_branch(&repo, name),
                Action::Fetch => git::fetch(&repo, None, None),
                Action::FetchRemote(name) => git::fetch(&repo, Some(name), None),
                // 「拉进哪条本地分支」那一格：git 的 `pull <远端> <分支>` 只会拉进**当前**
                // 分支，没有第二条路，所以目标不是当前那条时先切过去再拉。切失败就不拉——
                // 在错的分支上拉一次，冲突落到工作区里比报一句错难收拾得多。
                Action::PullFrom { remote, branch, into, opts } => {
                    let mut r = Ok(());
                    if let Some(target) = into {
                        let current = git::status(&repo).map(|s| s.branch);
                        r = match current {
                            Ok(c) if &c == target => Ok(()),
                            Ok(_) => git::checkout_branch(&repo, target),
                            Err(e) => Err(e),
                        };
                    }
                    r.and_then(|_| {
                        git::pull(&repo, Some(remote.as_str()), Some(branch.as_str()), opts, None)
                    })
                }
                // 多条分支就是**依次**几条 `git push`：某一条失败，前面那几条已经落地了，
                // 不回滚（git 自己推多条也是这个行为）。标签排在分支之后：`--tags` 是一次
                // 全量，前面那条分支没推上去时推标签只会把远端弄成半新半旧。
                Action::PushRefs { remote, refs, force, tags } => {
                    let mut r = Ok(());
                    for pr in refs.iter() {
                        r = git::push_ref(
                            &repo,
                            remote,
                            &pr.local,
                            &pr.remote_name,
                            *force,
                            pr.track,
                        );
                        if r.is_err() {
                            break;
                        }
                    }
                    if r.is_ok() && *tags {
                        r = git::push_all_tags(&repo, remote);
                    }
                    r
                }
                Action::CreateBranch { name, start, checkout } => {
                    git::create_branch(&repo, name, start.as_deref(), *checkout)
                }
                Action::DeleteBranch(name) => git::delete_branch(&repo, name, false),
                Action::RenameBranch { from, to } => git::rename_branch(&repo, from, to),
                Action::Merge(name) => git::merge_branch(&repo, name),
                Action::CreateTag { name, target } => git::create_tag(&repo, name, target, None),
                Action::DeleteTag(name) => git::delete_tag(&repo, name),
                Action::StashSave { message, include_untracked } => {
                    git::stash_push(&repo, message, *include_untracked)
                }
                Action::StashApply(i) => git::stash_apply(&repo, *i),
                Action::StashPop(i) => git::stash_pop(&repo, *i),
                Action::StashDrop(i) => git::stash_drop(&repo, *i),
                Action::Ignore(p) => git::ignore_path(&repo, p),
                Action::RestoreFile { oid, path } => git::restore_file(&repo, oid, path),
                // 哪一档由卡上那排页签定，落到 git 就是一条 `git reset <flag>`。
                // 三档里只有硬会碰工作区，而那一档在卡上写明了「找不回来」，
                // 所以这条路不再叠确认框（叠了会让人以为第一下已经落了）。
                Action::Reset { oid, mode } => git::reset(&repo, oid, *mode),
                Action::ResolveOurs(p) => git::resolve_ours(&repo, p),
                Action::ResolveTheirs(p) => git::resolve_theirs(&repo, p),
                Action::MarkResolved(p) => git::mark_resolved(&repo, p),
                Action::MarkUnresolved(p) => git::mark_unresolved(&repo, p),
                Action::ContinueOp { .. } => git::continue_op(&repo),
                Action::AbortOp { .. } => git::abort_op(&repo),
                // 走 `act` 而不是在帧里直接跑：`git remote get-url` 也是一次子进程，
                // 在帧里等它就把整帧冻住了——和其余每一条 git 调用同一个理由。
                Action::BrowseRemote(name) => git::remote_url(&repo, name).and_then(|url| {
                    match provider::parse_remote(&url) {
                        Some(r) => std::process::Command::new("open")
                            .arg(format!("https://{}/{}", r.host, r.path))
                            .spawn()
                            .map(|_| ())
                            .map_err(|e| GitxError::Other(tr!(format!("打不开浏览器：{e}"), format!("Cannot open a browser: {e}")))),
                        None => Err(GitxError::Other(tr!(
                            format!("{url} 不是「主机/所有者/仓库」那种远端地址"),
                            format!("{url} is not a host/owner/repository remote address")
                        ))),
                    }
                }),
                // 到不了这儿：`act` 在起线程之前就把这三条拦下走了各自那条路
                // （它们没有 `repo` 可用，也没有快照可重取）。这一条只为穷尽而写，
                // 真被走到就是有人挪动了那道守卫——**当场响**，别让它静默变成「点了没反应」。
                // 归档同一个理由：它在 `act` 最前面就被换成了存盘面板，到不了这条线程；
                // 用 `Err` 而不是 `unreachable!`，因为线程里 panic 会把 `tx` 一起带走，
                // `busy` 就永远没人落了——报错至少把牌子落回工作区。
                Action::Clone { .. } | Action::Init { .. } | Action::AddExisting { .. }
                | Action::Archive(_) => Err(
                    GitxError::Other(tr!(
                        "开仓库和归档那几条不该走到这条路上（act 的守卫被挪了）",
                        "adding a repository and archiving must not reach this path (the guard in act moved)"
                    ).to_owned()),
                ),
            };
            // 推送这一路单独走 `Msg::PushDone`，让状态栏报「正在推送 / 推送完成 / 推送失败」，
            // 而不是只弹一条 toast。`CommitAndPush` 也是推送。
            let is_push = matches!(action, Action::PushRefs { .. } | Action::CommitAndPush { .. });
            match r {
                Ok(()) => {
                    // 动作之后重新取一次快照：排序方式得跟着用户那枚开关，不然「层级顺序」
                    // 只在开仓那一次生效，点完提交按钮日志就退回日期序。
                    let _ = tx.send(match snapshot_of(&repo, topo, &scope) {
                        Ok(s) => Msg::Loaded { repo, snap: Box::new(s) },
                        Err(e) => Msg::Failed(format!("{repo}: {e}")),
                    });
                    if is_push {
                        let _ = tx.send(Msg::PushDone(Ok(())));
                    }
                }
                Err(e) => {
                    if is_push {
                        // 推送失败只发 `PushDone(Err)`：状态栏报「推送失败」，原因进 toast。
                        let _ = tx.send(Msg::PushDone(Err(e.to_string())));
                    } else {
                        // pull / merge 这类会改写工作区的操作失败时，工作区往往已经进了冲突态
                        // （git 把冲突文件写进工作区之后才非零退出）。若只弹错误、不重取快照，
                        // 界面就停在操作前的状态，unmerged 文件不显示，`op_banner`/`conflict_section`
                        // 也就不画——用户于是以为「冲突没法处理」。这里补一次快照，把已有的冲突
                        // 解决入口（中止 / 继续 / 标记为已解决）露出来。
                        if matches!(action, Action::PullFrom { .. } | Action::Merge(_)) {
                            if let Ok(s) = snapshot_of(&repo, topo, &scope) {
                                let _ = tx.send(Msg::Loaded { repo, snap: Box::new(s) });
                            }
                        }
                        let _ = tx.send(Msg::Failed(e.to_string()));
                    }
                }
            }
        });
        true
    }

    /// 卡片打开着的时候，**别的地方一律不接事件**：卡片自己占一层、一个可点的矩形铺满整屏，
    /// 卡片外面的行因此点不着；而卡片里面除了「取消」那一格，别的都不算关掉。
    /// 于是调用方只要问一次：`if !self.modal.is_none() { return; }` 就完事。
    pub(crate) fn modal_layer(&mut self, ctx: &egui::Context) {
        let Some(mut m) = self.modal.take() else { return };
        // `viewport_rect`（context.rs:3420）是**窗口**那格；`ctx.screen_rect()` 在 0.37 已经没了。
        // 无头帧里只有 `RawInput::screen_rect` 有值，`ViewportCommand::OuterColor`/`RectChanged`
        // 那一条事件根本不会发，所以读 `i.viewport_rect` 会得到 `Rect::NOTHING` —— 卡片整块不画
        // （本批第一条判据就是这么红的：`modal` 明明 Some，帧里却一个字的形状都没有）。
        let screen = ctx.viewport_rect();
        let (enter, esc) =
            ctx.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        // 卡片高度自己算（见 `Modal::rows`），偏好卡另按量来的窗口尺寸摆（见 `card_rect`）：
        // 居中要在布局之前定位置，问 egui 要「自然高度」得等布局跑完。
        let card = card_rect(&m, screen);
        let focus = std::mem::take(&mut self.modal_focus);
        // 这张卡是不是**那道闸自己**（确认卡）。闸拦在 `land` 的出口上，而确认卡上按「确定」
        // 发出去的那一条正是闸要问的那一句的回答：再让它过一遍闸，就会永远停在确认卡上。
        let from_confirm = matches!(m.form, Form::Confirm { .. });
        // 推送/拉取两张卡要念的清单**每帧现读**（不是打开那一刻钉死的那一批）：
        // 「远程分支」那一列跟着当前快照走，所以「刷新」跑完 `git fetch <远端>`、新快照落地，
        // 那一列自己就变了。偏好卡「推送」那一页要的也只是远端名单，同一条路。
        // 借用得在这儿断干净——下面那个闭包只碰 `m` 和这几个本地值。
        let mut card_remotes: Vec<gitx::models::Remote> = Vec::new();
        let mut card_remote_branches: Vec<String> = Vec::new();
        let mut card_locals: Vec<String> = Vec::new();
        let allow_force = self.allow_force;
        if let Some(s) = &self.snap {
            match &m.form {
                Form::Push { .. } | Form::Pull { .. } | Form::Settings { .. } => {
                    card_remotes = s.remotes.clone();
                }
                _ => {}
            }
            if let Form::Pull { remote, .. } = &m.form {
                let prefix = format!("{remote}/");
                card_remote_branches = s
                    .remote_refs
                    .iter()
                    .filter(|r| r.name.starts_with(&prefix))
                    .map(|r| r.name[prefix.len()..].to_owned())
                    .collect();
                card_locals = s.branches.iter().map(|b| b.name.clone()).collect();
            }
        }
        let mut fetch_remote: Option<String> = None;
        let mut fired: Option<Action> = None;
        // 「浏览…」按下去要问的是哪一格。和 `fired` 同一套路：面板不能在这帧里跑，
        // 只能带出去立 `pending_dialog`（见 `Dialog`）。
        let mut pick: Option<FolderField> = None;
        let mut primary: Option<Action> = None;
        let mut primary_hit = false;
        let mut close = false;
        egui::Area::new(egui::Id::unique("gitx-modal"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // 这一格铺满整屏的可点矩形就是遮罩：它把这一层的命中区域撑到全屏，
                // 卡片后面的行才点不着；点它自己也等于「关掉卡片」。
                let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(bg, 0.0, MODAL_SCRIM);
                // `Painter` 外面套的是 Arc，clone 一份就不占着 `ui` 的借用（卡里要横跨
                // 一整层控件）。
                let p = ui.painter().clone();
                // §5.1 那一套：白底 + 圆角 12 + 1px `BORDER_SUBTLE` + **真阴影**
                // （offset 0/8、blur 24、黑 40%）。上一版画的是遮罩 `from_black_alpha(120)`
                // 加**两圈越来越淡的空心描边冒充投影**，spec 的遮罩是带色的、投影是能画的
                // （`Shadow::as_shape` 出一条带 `blur_width` 的 `RectShape`），两样都换了真的。
                // 阴影那一笔先落，才压在卡底下面。
                p.add(CARD_SHADOW.as_shape(card, CARD_R));
                p.rect_filled(card, CARD_R, Color32::WHITE);
                p.rect_stroke(card, CARD_R, Stroke::new(1.0, BORDER_SUBTLE), egui::StrokeKind::Inside);
                let mut y = card.min.y;
                // 标题带。§5.2「标题/正文/按钮区之间**只靠间距分组，不画分隔线**（现实现里的
                // 标题带下划线与 footer 上划线废除）」——这两条线以前都在。
                put_left(
                    ui,
                    Rect::from_min_max(pos2(card.min.x + MODAL_PAD, y), pos2(card.max.x - MODAL_PAD, y + MODAL_TITLE_H)),
                    |ui| {
                        ui.add(Label::new(RichText::new(m.title).size(14.0).color(TEXT_PRIMARY).family(crate::ui::fonts::semibold_family()))
                            .selectable(false));
                    },
                );
                // §5.2 标题带右端那一枚 ×。偏好卡按同一条**没有**「取消」，这一枚就是它唯一的
                // 鼠标出口（Esc 还在，但那是键盘）。
                if card_close(ui, card) {
                    close = true;
                }
                y += MODAL_TITLE_H;
                if let Some(d) = &m.detail {
                    modal_text(ui, card, y, d, TEXT_SECONDARY, 11.0);
                    y += MODAL_ROW_H;
                }
                // 「添加仓库」那三档排在主输入**上面**：先说要办哪件事，再填对应那一格。
                // 走 §4.4 那套**分段选择**（容器 `BG_INSET` 圆角 8、选中段浮成白药丸），
                // 和重置卡、偏好卡同一枚 `seg_tabs`——以前这三档是三枚 `selectable_label`，
                // 底色铺满整格又不收口，看着像三行而不是「换档」。
                if let Form::AddRepo { tab, .. } = &mut m.form {
                    seg_tabs(
                        ui,
                        Rect::from_min_max(
                            pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - SEG_H) / 2.0),
                            pos2(card.max.x - MODAL_PAD, y + (MODAL_ROW_H + SEG_H) / 2.0),
                        ),
                        &[
                            (tr!("打开已有", "Open existing"), AddTab::Open),
                            (tr!("克隆到本地", "Clone locally"), AddTab::Clone),
                            (tr!("新建仓库", "New repository"), AddTab::New),
                        ],
                        tab,
                    );
                    y += MODAL_ROW_H;
                }
                let hint = m.hint();
                // 「添加仓库」那三档都在填磁盘上的路径，靠手打太苦：右边挂一枚「浏览…」。
                // 判据要它出现在卡里，所以这一位单独算出来，别在 `if let` 的借用里再问 `m.form`。
                let browse = matches!(m.form, Form::AddRepo { .. });
                if let Some(text) = &mut m.text {
                    let mut right = card.max.x - MODAL_PAD;
                    if browse {
                        // 先画按钮、拿它**实测**的宽，文本框右缘才知道要让到哪儿。
                        let (hit, w) = card_button(ui, tr!("浏览…", "Browse…"), right, y + MODAL_ROW_H / 2.0, false);
                        if hit {
                            pick = Some(FolderField::Main);
                        }
                        right -= w + 6.0;
                    }
                    let r = Rect::from_min_max(
                        pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - FIELD_H) / 2.0),
                        pos2(right, y + (MODAL_ROW_H + FIELD_H) / 2.0),
                    );
                    let resp = card_field(ui, r, hint, text, 1, false);
                    if focus {
                        resp.request_focus();
                    }
                    y += MODAL_ROW_H;
                }
                // 这两类卡下面没有别的正文了，所以 `y` 在这儿读完就收工：底部那两枚按
                // `card.max.y` 定位，不跟着正文行数是有意为之（卡片高度已经算进 `rows`）。
                match &mut m.form {
                    Form::StashSave { include_untracked } => {
                        pref_switch(ui, card, y, tr!("包含未跟踪文件", "Include untracked files"), include_untracked);
                    }
                    Form::Merge { branches } => {
                        for (i, b) in branches.iter().enumerate() {
                            if modal_row(ui, card, y + MODAL_ROW_H * i as f32, b) {
                                fired = Some(Action::Merge(b.clone()));
                            }
                        }
                    }
                    // 重置卡：三档页签 + 两句说明。两句各占**固定一行**（见 `Modal::rows`）：
                    // 换一档卡片高度跟着跳一下，看着就像已经按下去了。
                    // 卡本身就是说「会丢什么」的那一句问话，所以它不再叠一层确认框。
                    Form::Reset { mode, .. } => {
                        seg_tabs(
                            ui,
                            Rect::from_min_max(
                                pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - SEG_H) / 2.0),
                                pos2(card.max.x - MODAL_PAD, y + (MODAL_ROW_H + SEG_H) / 2.0),
                            ),
                            &git::ResetMode::ALL.map(|m| (reset_words(m).0, m)),
                            mode,
                        );
                        modal_text(ui, card, y + MODAL_ROW_H, reset_shares(), TEXT_SECONDARY, 11.0);
                        modal_text(
                            ui,
                            card,
                            y + MODAL_ROW_H * 2.0,
                            reset_words(*mode).1,
                            TEXT_PRIMARY,
                            11.0,
                        );
                    }
                    // 第二格文本框（只有克隆那一档有）。`y` 在这儿不再往后推：底部那两枚按
                    // `card.max.y` 定位，这一格的高度已经记进 `rows()`。
                    Form::AddRepo { tab: AddTab::Clone, dest } => {
                        // 目标目录同样给一枚「浏览…」。这一格只有克隆那一档存在，
                        // 所以 `FolderField` 直接给 `CloneDest`，不用再问一次当前是哪一档。
                        let mut right = card.max.x - MODAL_PAD;
                        let (hit, w) = card_button(ui, tr!("浏览…", "Browse…"), right, y + MODAL_ROW_H / 2.0, false);
                        if hit {
                            pick = Some(FolderField::CloneDest);
                        }
                        right -= w + 6.0;
                        let r = Rect::from_min_max(
                            pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - FIELD_H) / 2.0),
                            pos2(right, y + (MODAL_ROW_H + FIELD_H) / 2.0),
                        );
                        let _ = card_field(ui, r, tr!("目标目录", "Destination folder"), dest, 1, false);
                    }
                    // 描述那一格 + 两枚勾。行高记在 `rows()` 里（`DESC_ROWS` + 2）。
                    Form::Commit { desc, amend, push_after } => {
                        let r = Rect::from_min_max(
                            pos2(card.min.x + MODAL_PAD, y),
                            pos2(card.max.x - MODAL_PAD, y + MODAL_ROW_H * DESC_ROWS as f32),
                        );
                        let _ = card_field(ui, r, tr!("扩展描述…", "Extended description…"), desc, DESC_ROWS, false);
                        let y = y + MODAL_ROW_H * DESC_ROWS as f32;
                        pref_switch(ui, card, y, tr!("修正上一条提交", "Amend the last commit"), amend);
                        let y = y + MODAL_ROW_H;
                        pref_switch(ui, card, y, tr!("提交之后立刻推送", "Push right after committing"), push_after);
                    }
                    // 偏好卡：页签一排 + 每页那几条。开关改一格当场落库当场生效（见 `apply_prefs`）。
                    Form::Settings { prefs, page, repos, database } => {
                        seg_tabs(
                            ui,
                            Rect::from_min_max(
                                pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - SEG_H) / 2.0),
                                pos2(card.max.x - MODAL_PAD, y + (MODAL_ROW_H + SEG_H) / 2.0),
                            ),
                            &[
                                (tr!("通用", "General"), PrefPage::General),
                                (tr!("推送与拉取", "Push & pull"), PrefPage::Push),
                                (tr!("代码托管", "Hosting"), PrefPage::Hosting),
                                (tr!("关于", "About"), PrefPage::About),
                            ],
                            page,
                        );
                        let y = y + MODAL_ROW_H + MODAL_GAP;
                        match *page {
                            PrefPage::General => {
                                // 三条**都真接得通**才摆得上来：一条点了没反应的开关比没有更糟。
                                // 「启动时重新打开」改的是下一次启动（读配置在 `Gitx::new`），
                                // 另两条这一帧就看得见效果。
                                pref_switch(ui, card, y, tr!("启动时重新打开上一次的仓库", "Reopen the last repository at launch"), &mut prefs.restore_last);
                                pref_switch(ui, card, y + MODAL_ROW_H, tr!("日志默认按层级顺序（而不是日期）", "Log defaults to ancestry order (not date)"), &mut prefs.topo);
                                pref_switch(ui, card, y + MODAL_ROW_H * 2.0, tr!("描述列显示远程分支那枚药丸", "Show the remote-branch pill in the message column"), &mut prefs.show_remote);
                                // 第四格：界面语言。分段选择（§4.4），改完**当场整屏换字**——
                                // 语言不是一次取数或一份快照，它只由下一帧 `shell()` 开头那次
                                // `lang::set` 决定，所以不需要重取任何东西。
                                let ry = y + MODAL_ROW_H * 3.0;
                                put_left(
                                    ui,
                                    Rect::from_min_max(
                                        pos2(card.min.x + MODAL_PAD, ry),
                                        pos2(card.min.x + MODAL_PAD + 96.0, ry + MODAL_ROW_H),
                                    ),
                                    |ui| {
                                        ui.label(RichText::new(tr!("界面语言：", "Language:")).size(11.5));
                                    },
                                );
                                seg_tabs(
                                    ui,
                                    Rect::from_min_max(
                                        pos2(card.min.x + MODAL_PAD + 100.0, ry + (MODAL_ROW_H - SEG_H) / 2.0),
                                        pos2(card.max.x - MODAL_PAD, ry + (MODAL_ROW_H + SEG_H) / 2.0),
                                    ),
                                    &[("中文", Lang::Zh), ("English", Lang::En)],
                                    &mut prefs.lang,
                                );
                            }
                            PrefPage::Push => {
                                // 这一页四条全都真的改得出**看得见的区别**，所以四条都摆（判据见
                                // `the_push_preferences_change_what_the_push_card_starts_with`）：
                                // 默认远端换的是卡上那一格的初值，那一档换的是表里预先勾上哪几条，
                                // 两枚勾换的是推送卡底部那两枚的初值/摆不摆。
                                // 「默认远端」只在当前分支**没有上游**时才用得上（`push_modal` 那条链），
                                // 有上游时上游说话——那是更具体的事实。
                                let names: Vec<String> =
                                    card_remotes.iter().map(|r| r.name.clone()).collect();
                                put_left(
                                    ui,
                                    Rect::from_min_max(
                                        pos2(card.min.x + MODAL_PAD, y),
                                        pos2(card.max.x - MODAL_PAD, y + MODAL_ROW_H),
                                    ),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        ui.label(RichText::new(tr!("默认远端：", "Default remote:")).size(11.5));
                                        let label = if prefs.default_remote.is_empty() {
                                            tr!("（没有）", "(none)")
                                        } else {
                                            prefs.default_remote.as_str()
                                        };
                                        if let Some(p) = popup_button(ui, label, &names) {
                                            prefs.default_remote = p;
                                        }
                                    },
                                );
                                put_left(
                                    ui,
                                    Rect::from_min_max(
                                        pos2(card.min.x + MODAL_PAD, y + MODAL_ROW_H),
                                        pos2(card.max.x - MODAL_PAD, y + MODAL_ROW_H * 2.0),
                                    ),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        ui.label(RichText::new(tr!("推送时：", "On push:")).size(11.5));
                                        // 五项文案逐字取自本机那枚 `NSPopUpButton` 的菜单
                                        // （测量记录见 docs/PROCESS.md）；存的是 git 那个键名。
                                        let items: Vec<String> = PushBehavior::labels()
                                            .iter()
                                            .map(|l| l.to_string())
                                            .collect();
                                        if let Some(p) =
                                            popup_button(ui, prefs.push_behavior.label(), &items)
                                        {
                                            if let Some(i) =
                                                PushBehavior::labels().iter().position(|l| *l == p)
                                            {
                                                prefs.push_behavior = PushBehavior::from_index(i);
                                            }
                                        }
                                    },
                                );
                                pref_switch(
                                    ui,
                                    card,
                                    y + MODAL_ROW_H * 2.0,
                                    tr!("推送所有标签到远程仓库", "Push all tags to the remote"),
                                    &mut prefs.push_tags,
                                );
                                pref_switch(
                                    ui,
                                    card,
                                    y + MODAL_ROW_H * 3.0,
                                    tr!("允许强制推送", "Allow force push"),
                                    &mut prefs.allow_force,
                                );
                            }
                            PrefPage::Hosting => {
                                // 一格一家：令牌输入框（掩码——它会被人站在屏幕后面看见）+
                                // 下面一行「怎么拿」。只摆**声明了 PR 能力**的 provider：给做不了
                                // 事的 provider 留一格输入框，就是留一格按下去没反应的空壳。
                                let mut row = 0usize;
                                for h in gitx::provider::HOSTS.iter().filter(|h| {
                                    h.capabilities()
                                        .contains(&gitx::provider::Capability::PullRequests)
                                }) {
                                    let ry = y + MODAL_ROW_H * row as f32;
                                    let r = Rect::from_min_max(
                                        pos2(card.min.x + MODAL_PAD, ry + (MODAL_ROW_H - FIELD_H) / 2.0),
                                        pos2(card.max.x - MODAL_PAD, ry + (MODAL_ROW_H + FIELD_H) / 2.0),
                                    );
                                    card_field(
                                        ui,
                                        r,
                                        &tr!(format!("{} 访问令牌", h.label()), format!("{} access token", h.label())).to_owned(),
                                        prefs.token_mut(h.id()),
                                        1,
                                        true,
                                    );
                                    let bound = if prefs.token(h.id()).is_empty() {
                                        tr!("未绑定", "not linked")
                                    } else {
                                        tr!("已绑定 · 只存本机", "linked · stored on this machine only")
                                    };
                                    modal_text(
                                        ui,
                                        card,
                                        ry + MODAL_ROW_H,
                                        &format!("{bound} · {}", h.token_hint()),
                                        TEXT_SECONDARY,
                                        11.0,
                                    );
                                    row += 2;
                                }
                            }
                            PrefPage::About => {
                                modal_text(ui, card, y, &tr!(format!("书签里的仓库：{repos} 本"), format!("Repositories in the shelf: {repos}")), TEXT_PRIMARY, 11.5);
                                modal_text(
                                    ui,
                                    card,
                                    y + MODAL_ROW_H,
                                    &database.clone().unwrap_or_else(|| tr!("没有可用的数据库", "No database available").to_owned()),
                                    TEXT_SECONDARY,
                                    11.0,
                                );
                            }
                        }
                    }
                    // 推送/拉取两张卡：卡里每一格的坐标都是从 nib 量出来的（theme.rs 那两组
                    // 常量），所以这一层只把 `push_card`/`pull_card` 端上来，不数行、不排布。
                    // 内容区从标题那条带下面起算，`top` 就是它的左上角。
                    Form::Push { remote, rows, force, tags } => {
                        push_card(
                            ui,
                            pos2(card.min.x, y),
                            &card_remotes,
                            remote,
                            rows,
                            force,
                            tags,
                            allow_force,
                        );
                    }
                    Form::Pull { remote, branch, into, opts } => {
                        if pull_card(
                            ui,
                            pos2(card.min.x, y),
                            &card_remotes,
                            remote,
                            &card_remote_branches,
                            branch,
                            &card_locals,
                            into,
                            opts,
                        ) {
                            fetch_remote = Some(remote.clone());
                        }
                    }
                    _ => {}
                }
                // 底部那枚主按钮在最右（macOS 那套）。偏好卡没有「取消」——开关是当场落的，
                // 说「取消」会谎报还没算的那部分能回退。
                // §5.2：卡脚上面**不画线**，那 52pt 带子自己就是分组。
                let cy = card.max.y - MODAL_FOOT_H / 2.0;
                let mut right = card.max.x - MODAL_PAD;
                if !m.primary().is_empty() {
                    let (hit, w) = card_button(ui, m.primary(), right, cy, true);
                    primary_hit = hit;
                    right -= w + 8.0;
                }
                let mut cancel_hit = false;
                if !primary_closes(&m) {
                    let (hit, _) = card_button(ui, tr!("取消", "Cancel"), right, cy, false);
                    cancel_hit = hit;
                }
                if cancel_hit || bg_resp.clicked() {
                    close = true;
                }
            });
        if esc {
            close = true;
        }
        // 「刷新」跑的是 `git fetch <那一条远端>`：它不改工作区也不改分支，只是让远端那一列
        // 分支跟上外面的世界，所以卡**留着不关**——新快照落地时那一列自己就换了（见上面那段）。
        if let Some(r) = fetch_remote {
            self.act(Action::FetchRemote(r));
        }
        // 主按钮只做一件事，且只有**一处**决定：鼠标那一下和 Enter 都汇到这里。
        // 分成两处判会让一条判据只杀得死一半（契约锁，不是判别门）。
        if primary_hit || (enter && !close) {
            if primary_closes(&m) {
                close = true;
            } else {
                primary = primary_action(&mut m);
            }
        }
        // 「浏览…」按下去只把牌子立起来，面板要等这一帧之外才跑（见 `Dialog`）。
        // 卡片**保持开着**：这时候还没有答案，关掉等于让用户重填一遍。
        if let Some(field) = pick {
            self.pending_dialog = Some(Dialog::PickFolder { field });
        }
        // 偏好卡的开关「改一格算一格」：卡还开着就要生效、就要落库，所以在这儿回写一次。
        // 换过语言要把这张卡**当场重拼**：卡的标题和说明是打开那一刻按当时语言算死的
        // `&'static str`，不重拼就是一屏英文挂着「设置」两个汉字。
        // 重拼只能在这儿做：`apply_prefs` 里读的是 `self.modal`，而这一帧卡正被上面那个
        // `take()` 握在手里，那儿是空的——以前挂在 `apply_prefs` 里的那段重拼是条死路，
        // 判据 `the_language_segment_switches_the_whole_shell_and_persists` 第一次跑就红在这儿。
        if let Form::Settings { page, repos, database, prefs } = &m.form {
            let (page, repos, database, prefs) = (*page, *repos, database.clone(), prefs.clone());
            if self.apply_prefs(prefs) {
                let mut card = Modal::settings(self.prefs(), repos, database);
                // 页签停在原来那一页：切语言不该把人从「代码托管」弹回「通用」。
                if let Form::Settings { page: now, .. } = &mut card.form {
                    *now = page;
                }
                self.modal = Some(card);
                return;
            }
        }
        // 关卡的决定只有这一处：按「取消」、点遮罩、按 Esc 都算 `close`；有动作要发时先按
        // 「动作会被接住」收，`land` 报没接住（填的路径不是仓库、手上根本还没仓库）就把卡连字
        // 一起端回去——打错一个字母不该从头再打一遍。
        if close {
            return;
        }
        let Some(a) = fired.or(primary) else {
            self.modal = Some(m);
            return;
        };
        // 名字空着走不到这儿：`primary_action` 对空串返回 `None`，`fired`/`primary` 也就都是空。
        // 除了确认卡自己那一条，发出去都走 `land` 而不是直接 `act`：确认卡拦在**出口**上
        // （`needs_confirm`），入口只会越来越多（侧栏、日志行、文件行、工具条、卡片），
        // 卡里那条「修正上一条提交」换掉的也是已经落地的提交，一样要先问一句。
        let accepted = if from_confirm {
            self.act(a)
        } else {
            self.land(Intent { act: Some(a), ..Default::default() })
        };
        if !accepted && self.modal.is_none() {
            self.modal = Some(m);
        }
    }

    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        let mut got = false;
        while let Ok(msg) = self.rx.try_recv() {
            got = true;
            match msg {
                Msg::Loaded { repo, snap } => {
                    if self.repo.as_deref() != Some(repo.as_str()) {
                        continue;
                    }
                    let snap = *snap;
                    // 记下「最后一次**在分支上**时的那条分支名」：分离 HEAD 之后 `status.branch`
                    // 是空的，而状态栏那句「回到 main」要的正是这个已经不再写在快照里的名字。
                    if !snap.status.branch.is_empty() {
                        self.last_branch = snap.status.branch.clone();
                    }
                    // 提交数可能变了（刚切了分支）：下标越界就退回工作区行。
                    self.sel = match &self.sel {
                        Sel::Commit(i) if *i < snap.commits.len() => Sel::Commit(*i),
                        // 命中那本列表**不跟着快照重取**（重取要再跑一遍 git，而人没让你搜第二遍）。
                        // 所以这一档的越界只可能是列表自己变短了——那时退回工作区行，
                        // 别让选中行停在一格已经不存在的命中上。
                        Sel::Search(i) if *i < self.search_hits.len() => Sel::Search(*i),
                        Sel::Stash(i) if *i < snap.stashes.len() => Sel::Stash(*i),
                        _ => Sel::Uncommitted,
                    };
                    let restage = self.target.clone().filter(|_| self.seq > 0);
                    // 首屏不满一页 = 这个仓库本来就没什么历史，「还有下一页」当场拧死。
                    // 少了这一句，小仓库每帧都会去问一次 `git log --skip`（短列表永远
                    // 「到底」），既白跑 git，也让无头门禁里的线程数不确定。
                    self.log_more = snap.commits.len() as u32 == LOG_PAGE;
                    self.snap = Some(snap);
                    // 看板开着的时候换了仓库（切 tab、点首页那张卡）：那一栏对着的
                    // provider 得跟着新快照重认，不然画的是上一本仓库的 PR 列表。
                    if self.board_open {
                        self.show_board();
                    }
                    self.busy = false;
                    if std::mem::take(&mut self.clear_msg_on_load) {
                        self.commit_msg.clear();
                        self.commit_desc.clear();
                        // 「修正上一条」是一次性的：落地后不把勾留着，否则下一次普通提交
                        // 会静默变成 amend——那正是最该防的误操作。
                        self.amend = false;
                    }
                    // 动作落地之后**按原来那一格**重新取差异。以前只在「没选文件」时重取整份
                    // 未暂存补丁，于是暂存了某个文件之后，右边那格还挂着已经暂存掉的内容；
                    // hunk 按钮更需要这一条——打完一块，剩下几块得立刻重排。
                    if let Some(t) = restage {
                        self.request(t);
                    }
                }
                Msg::More { repo, seq, commits } => {
                    // 对不上号 = 这一页属于上一轮（切了仓库或换了排序），丢掉。
                    // 只在号码对得上时才摘「在途」那块牌子，否则会把**新一轮**请求的
                    // 状态误清成可以重发。
                    if seq != self.log_seq || self.repo.as_deref() != Some(repo.as_str()) {
                        continue;
                    }
                    self.log_more_busy = false;
                    // 不满一页就是到底了：别再问，省掉每次滚到底的一次空 `git log`。
                    self.log_more = commits.len() as u32 == LOG_PAGE;
                    if let Some(snap) = self.snap.as_mut() {
                        append_page(snap, commits);
                    }
                }
                Msg::Search { repo, seq, query, commits } => {
                    // 三道闸，缺一都会把**上一次**那一下回车的结果画成这一次的答案：
                    // 号对不上（又回车了 / 换了仓库）、仓库不对、词已经改了（人在框里
                    // 又敲了几个字，但还没回车）。
                    if seq != self.search_seq
                        || self.repo.as_deref() != Some(repo.as_str())
                        || self.search_q.trim() != query
                    {
                        continue;
                    }
                    self.search_busy = false;
                    // 药丸和日期文案在**这一刻**算，不在帧里算：那本账是「命中数 × 引用数」
                    // 遍字符串比较，每帧现算就是拿它换一次滚动（和快照那两本同一理由）。
                    // 快照没到手（`None`）时算不出药丸——那时连搜索框都不该在，
                    // 真走到这儿就把药丸留空，别拿一份假账糊上去。
                    let (branches, tags, remote_refs, taken) = match &self.snap {
                        Some(s) => (&s.branches, &s.tags, &s.remote_refs, s.taken_at),
                        None => {
                            self.search_hits.clear();
                            continue;
                        }
                    };
                    let badges = ref_badges(&commits, branches, tags, remote_refs);
                    self.search_hits = commits
                        .into_iter()
                        .zip(badges)
                        .map(|(c, b)| SearchHit {
                            date: date_label(&c.date, taken),
                            commit: c,
                            badges: b,
                        })
                        .collect();
                }
                Msg::CommitFiles { repo, index, files, search } => {
                    let want = if search { Sel::Search(index) } else { Sel::Commit(index) };
                    if self.repo.as_deref() == Some(repo.as_str()) && self.sel == want {
                        self.commit_files = files;
                    }
                }
                Msg::StashFiles { repo, index, files } => {
                    if self.repo.as_deref() == Some(repo.as_str()) && self.sel == Sel::Stash(index) {
                        self.commit_files = files;
                    }
                }
                Msg::Patch { repo, seq, label, lines } => {
                    if self.repo.as_deref() == Some(repo.as_str()) && seq == self.seq {
                        // 横向可滚宽度只在这里算一次：差异区每帧问一遍就是给 28 875 行的补丁
                        // 每帧重扫一遍字符。
                        self.diff_cols = lines.iter().map(|l| columns(&l.text)).max().unwrap_or(0);
                        self.diff = lines;
                        // 折叠状态跟着补丁重来：上一份补丁的第 3 个文件，和这一份的第 3 个文件
                        // 不是同一个文件；把折叠留着会让用户打开新差异就少了一块。
                        self.collapsed.clear();
                        // 搜索框也清空：上一份补丁的词对着这一份多半一个字都不命中，
                        // 留着会让人打开新差异先看到「没有匹配的行」。
                        self.diff_search.clear();
                        self.diff_rows = (0..self.diff.len()).collect();
                        self.patch_label = label;
                        self.busy = false;
                    }
                }
                Msg::FileLog { repo, seq, path, entries } => {
                    if self.repo.as_deref() != Some(repo.as_str()) {
                        continue;
                    }
                    // 先取第一条要画的补丁，借完 `history` 再起线程。
                    let first = {
                        let Some(h) = self.history.as_mut() else { continue };
                        if h.seq != seq || h.path != path {
                            continue;
                        }
                        h.log_loading = false;
                        h.entries = entries;
                        h.sel = 0;
                        h.entries.first().map(|e| e.commit.id.clone())
                    };
                    if let Some(oid) = first {
                        if let Some(h) = self.history.as_mut() {
                            h.patch_loading = true;
                        }
                        spawn_history_patch(&self.tx, &repo, &oid, &path, seq, 0);
                    }
                }
                Msg::HistoryPatch { repo, seq, path, index, lines } => {
                    if self.repo.as_deref() != Some(repo.as_str()) {
                        continue;
                    }
                    if let Some(h) = self.history.as_mut() {
                        if h.seq == seq && h.path == path && h.sel == index {
                            h.diff = lines;
                            h.patch_loading = false;
                        }
                    }
                }
                Msg::HistoryText { repo, seq, path, index, text } => {
                    if self.repo.as_deref() != Some(repo.as_str()) {
                        continue;
                    }
                    if let Some(h) = self.history.as_mut() {
                        if h.seq == seq && h.path == path && h.full_for == Some(index) {
                            h.full = text.lines().map(str::to_owned).collect();
                            h.full_loading = false;
                        }
                    }
                }
                Msg::HistoryBlame { repo, seq, path, lines } => {
                    if self.repo.as_deref() != Some(repo.as_str()) {
                        continue;
                    }
                    if let Some(h) = self.history.as_mut() {
                        if h.seq == seq && h.path == path {
                            h.blame = lines;
                            h.blame_loading = false;
                        }
                    }
                }
                Msg::Cloned(r) => {
                    self.busy = false;
                    match r {
                        // 成了就当书签加进去并打开：`add_repo` 自己会发 `Msg::Loaded`。
                        Ok(root) => { self.add_repo(&root); }
                        Err(e) => self.note = Some(tr!(format!("仓库没落地：{e}"), format!("The repository never landed: {e}"))),
                    }
                }
                // 归档成了：只报一句落在哪儿，界面什么都不换（它没动仓库，快照还是对的）。
                Msg::Archived(dest) => {
                    self.busy = false;
                    self.note = Some(tr!(format!("已导出 {dest}"), format!("Exported to {dest}")));
                }
                Msg::PullRequests { seq, prs } => {
                    // 对不上号的回复一律丢：切仓库、按刷新、再点进看板都重新编过号，
                    // 旧回复落地就是「A 仓库的列表挂在 B 仓库名下」。
                    if seq != self.board_seq {
                        continue;
                    }
                    self.board.fetching = false;
                    self.board.loaded = true;
                    match prs {
                        Ok(list) => {
                            self.board.prs = list;
                            self.board.error = None;
                            // 列表是新的（那两格全空），`(号, sha)` 的缓存还有效：涂回去。
                            // 同一版提交不该因为「按了一次刷新」就被再问一遍。
                            self.board.apply_facts();
                            // 库里读回来是空的 = 这一栏第一次打开。见 `Board::seeding`。
                            self.board.seeding = self.board.seen.is_empty();
                        }
                        Err(e) => {
                            self.board.prs.clear();
                            self.board.error = Some(e);
                        }
                    }
                }
                // 单条 PR 的 `mergeable_state`（列表端点**不给**这一格，实测）。
                // 两道闸都在「涂进行」之前：号对不上 = 这一笔属于上一代列表；
                // sha 对不上 = 作者在这一趟在飞时推了新提交，旧那一版的「可以合并」
                // 挂在新提交上是这一栏最要命的那种谎。丢掉的回复**不解在途**——
                // 那一本账跟着列表重开，动它等于把谎接着说。
                Msg::PrDetail { seq, number, sha, r } => {
                    if seq != self.board_seq {
                        continue;
                    }
                    let live = self
                        .board
                        .prs
                        .iter()
                        .find(|p| p.number == number)
                        .map(|p| p.head_sha.clone());
                    if live.as_deref() != Some(sha.as_str()) {
                        continue;
                    }
                    let now = ctx.input(|i| i.time);
                    match r {
                        Err(e) => {
                            // 回不来就当这一格**没有结论**（空串），而不是让这一行
                            // 一辈子挂着「正在看它卡在哪」。
                            let key = (number, sha);
                            self.board.facts.entry(key).or_default().detail = Some(String::new());
                            self.board.error.get_or_insert(e);
                        }
                        Ok(ms) => {
                            // GitHub 那一格是后台补的，第一次常常还是 `unknown`。
                            // 当结论落地 = 这一行挂着一句「还没算出来」直到用户手动刷新，
                            // 所以排一次到点重问；试够了才认下来。见 `retry_unknown_details`。
                            let usable = !ms.is_empty() && ms != "unknown";
                            let exhausted = self
                                .board
                                .facts
                                .get(&(number, sha.clone()))
                                .map(|f| f.detail_tries >= DETAIL_RETRY_MAX)
                                .unwrap_or(false);
                            if !usable && !exhausted {
                                self.board
                                    .facts
                                    .entry((number, sha))
                                    .or_default()
                                    .retry_at = Some(now + DETAIL_RETRY_SECS);
                            } else {
                                self.board.facts.entry((number, sha.clone())).or_default().detail = Some(ms.clone());
                                if let Some(pr) = self.board.prs.iter_mut().find(|p| p.number == number) {
                                    pr.mergeable_state = ms;
                                }
                            }
                        }
                    }
                }
                // head 那一版提交的检查汇总（后台已经 `aggregate_ci` 过）。
                // 和详情同一两道闸，理由同上一条。失败也记成「已经有了」= `Ci::Unknown`：
                // 「取失败」和「还没取」是两件事，前者不许画成后者。
                Msg::PrChecks { seq, number, sha, r } => {
                    if seq != self.board_seq {
                        continue;
                    }
                    let live = self
                        .board
                        .prs
                        .iter()
                        .find(|p| p.number == number)
                        .map(|p| p.head_sha.clone());
                    if live.as_deref() != Some(sha.as_str()) {
                        continue;
                    }
                    let (ci, why) = match r {
                        Ok(ci) => (ci, None),
                        Err(e) => (gitx::models::Ci::Unknown, Some(e)),
                    };
                    // `sha` 那道闸已经证明列表里这一号的 head 就是它，按号找即可。
                    if let Some(pr) = self
                        .board
                        .prs
                        .iter_mut()
                        .find(|p| p.number == number)
                    {
                        pr.ci = ci.clone();
                    }
                    self.board.facts.entry((number, sha)).or_default().checks = Some(ci);
                    if let Some(e) = why {
                        self.board.error.get_or_insert(e);
                    }
                }
                Msg::PushDone(r) => {
                    // 推送状态落到状态栏：成功「推送完成」、失败「推送失败」，都带时间戳，
                    // 几秒后由 `status_bar` 清掉。失败原因同时进 toast（`note`）。
                    let now = ctx.input(|i| i.time);
                    match r {
                        Ok(()) => {
                            self.push_state = Some(PushPhase::Done(now));
                            // 推送成功保持静默：不弹 toast（改之前就没有），只靠底部状态栏
                            // 的「推送完成」提示（见 `status_bar`）。失败那一格才会进 toast。
                        }
                        Err(e) => {
                            self.push_state = Some(PushPhase::Failed(now));
                            if !e.is_empty() {
                                self.note = Some(e);
                            }
                        }
                    }
                    self.busy = false;
                }
                Msg::Failed(e) => {
                    if !e.is_empty() {
                        self.note = Some(e);
                    }
                    // 提交没落地：那格信息留着，让用户改完再按一次。
                    // 走 `act` 的动作本来就会重算这个标记，所以这行只挡住**不经 `act`** 的那条路
                    // （切仓库时的 `open`）——它删掉没有任何判据变红，是契约锁，不是判别门。
                    self.clear_msg_on_load = false;
                    self.busy = false;
                    // 历史面板的三颗「读取中」也不能永远亮着：线程里失败时没有更细的回复。
                    if let Some(h) = self.history.as_mut() {
                        h.log_loading = false;
                        h.patch_loading = false;
                        h.blame_loading = false;
                        h.full_loading = false;
                    }
                }
            }
        }
        // 看板那一击攒下的地址在这儿交给 egui：`land` 手上没有 `Context`，而起进程
        // `open <url>` 会让跑测试真的弹出浏览器。走命令队列 = 无头门禁量得到这一条。
        if let Some(url) = self.board_open_url.take() {
            ctx.output_mut(|o| {
                o.commands.push(egui::OutputCommand::OpenUrl(egui::OpenUrl::same_tab(url)))
            });
            ctx.request_repaint();
        }
        // 首次建账那一趟（见 `Board::seeding`）：把当前每一行的签名记成「看过了」。
        // 要等补充的两格都落定才记 —— 列表刚落地时那两格还是空的，拿那份签名当基线，
        // 「CI 在用户眼皮底下跑完」就会亮成一条他没看过的动静。
        if self.board.seeding && !self.board.fetching && self.board.outstanding() == 0 {
            if self.board.seed_seen() {
                self.persist_seen();
            }
        }
        // `unknown` 的到点重问。返回最早下一次到点，让静止的窗口自己醒来看这一眼：
        // 没人要重绘就没人看表，「等 4 秒再问」会等到用户下次动鼠标。
        let now = ctx.input(|i| i.time);
        if let Some(at) = self.retry_unknown_details(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.05)));
        }
        if got || self.busy {
            ctx.request_repaint();
        }
    }

    /// 到点重取那些「详情回了 `unknown`、还没试够」的行，返回最早的下一次到点。
    ///
    /// 只做「摘账」：把 `asked` 里那一格和 `facts.detail` 一起清掉，重发交给 `enrich_rows`
    /// —— 它本来就只给**看得见**的行发请求，所以滚出屏幕的行不会在这一趟里白问一次。
    /// 上限 `DETAIL_RETRY_MAX`：GitHub 后台算不出来时不该把这一格变成无休止的重发，
    /// 试够了就认下 `unknown`，状态那句老实说「还没算出来」。
    pub(crate) fn retry_unknown_details(&mut self, now: f64) -> Option<f64> {
        let due: Vec<(u32, String)> = self
            .board
            .facts
            .iter()
            .filter(|(_, f)| f.detail.is_none() && f.detail_tries < DETAIL_RETRY_MAX)
            .filter_map(|(k, f)| f.retry_at.map(|t| (t, k.clone())))
            .filter(|(t, _)| *t <= now)
            .map(|(_, k)| k)
            .collect();
        for key in &due {
            if let Some(f) = self.board.facts.get_mut(key) {
                f.retry_at = None;
                f.detail_tries += 1;
            }
            self.board.asked.remove(&(key.clone(), Ask::Detail));
        }
        self.board
            .facts
            .values()
            .filter(|f| f.detail.is_none() && f.detail_tries < DETAIL_RETRY_MAX)
            .filter_map(|f| f.retry_at)
            .min_by(|a, b| a.total_cmp(b))
    }

    // -------------------------------------------------------------------- 顶栏
    /// §2 那条 46pt 的带，从左到右：**红绿灯让出的 92pt** → 仓库胶囊 tab（含「＋ 添加仓库」）
    /// → 分支名 + `↑↓` → §4.1 那组 28×28 图标按钮。
    /// 旧的两条带（标题 52 + tab 24）合成一条；仓库名那行「xxx (Git)」删了——书签摊成 tab 之后
    /// tab 本身就是仓库名，§2 那一行也没有它。提示语走底部 toast（`toast::show`）。
    pub(crate) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut tool: Option<usize> = None;
        let mut pick: Option<String> = None;
        let mut want_add = false;
        let mut want_home = false;
        let current = self.repo.clone();
        let repos = self.repos.clone();
        // 分支那一格拆成两段：名字是次要灰，`↑n ↓m` 是主色 + 真字重（预览稿 `.branch-chip b`）。
        // 没有上游时一个数字都不许出现（契约：绝不显示 ↑0 ↓0）。
        let head: Option<(String, Option<String>)> = self.snap.as_ref().map(|s| {
            let st = &s.status;
            let counts = (st.ahead > 0 || st.behind > 0).then(|| format!("↑{} ↓{}", st.ahead, st.behind));
            (st.branch.clone(), counts)
        });
        let outer = ui.max_rect();
        egui::Panel::top("toolbar")
            .exact_size(HEADER_H)
            // 左 92 是给红绿灯让的（它们由操作系统画在这一行上面，x 20..72），右 12 = §2。
            // 上下各 9：`(46 − 28) / 2`，28 是这一行最高的控件（§4.1 图标按钮）；
            // 胶囊 tab 的 26 在净高里垂直居中。
            .frame(egui::Frame::NONE.fill(BG_WINDOW).inner_margin(Margin {
                left: TITLE_INSET,
                right: HEADER_RIGHT,
                top: HEADER_VPAD,
                bottom: HEADER_VPAD,
            }))
            .show(ui, |ui| {
                // 一整行。分两行的话按钮 + tab 会把 46pt 的带撑出去（无头测试量出来的）。
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = TAB_GAP;
                    // §2「窗口收窄时溢出收进 `»` 菜单」。右边那一组（分支 + 11 枚按钮）是**有界**
                    // 的，先量它；左边的仓库 tab 是无界的一串，只在剩下的宽度里排，排不下的收进 `»`。
                    // 不收纳的话英文档就露馅：「+ Add repository」比「＋ 添加仓库」宽出一截，
                    // 直接压进分支名和 `↑↓` 里（判据 `the_header_still_fits_in_english` 量到的就是它）。
                    let band_w = ui.available_width();
                    let right_w = header_right_group_width(ui, &head);
                    let add_label = tr!("＋ 添加仓库", "+ Add repository");
                    // 首页 tab 固定在最左（§2.1）、「添加仓库」永远是最后入口，两枚都不收纳。
                    let fixed_w = repo_tab_width(ui, home_tab(), self.home) + TAB_GAP
                        + ghost_tab_width(ui, &add_label) + TAB_GAP;
                    let tab_widths: Vec<f32> = repos
                        .iter()
                        .map(|r| repo_tab_width(ui, &r.name, !self.home && current.as_deref() == Some(r.path.as_str())))
                        .collect();
                    let (kept, needs_more) =
                        fit_header_tabs(&tab_widths, (band_w - right_w - fixed_w).max(0.0), more_tab_width(ui));
                    // 「首页」固定在仓库 tab 的**左边**，不跟着书签列表滚。选中它时仓库 tab
                    // 全部回到未选中态；点任一仓库 tab 就离开首页（`open()` 翻这一位）。
                    if repo_tab(ui, home_tab(), self.home) {
                        want_home = true;
                    }
                    for r in repos.iter().take(kept) {
                        if repo_tab(ui, &r.name, !self.home && current.as_deref() == Some(r.path.as_str())) {
                            pick = Some(r.path.clone());
                        }
                    }
                    if needs_more {
                        let resp = more_tab(ui);
                        // 菜单项按 §5.4 那套走 egui 的 menu 样式；`CloseOnClick` = 挑完就收，
                        // 点外面也收（同筛选条那三枚弹出按钮的行为）。
                        let chosen = egui::Popup::menu(&resp)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                            .show(|ui| {
                                let mut hit: Option<String> = None;
                                for r in &repos[kept..] {
                                    if ui.button(&r.name).clicked() {
                                        hit = Some(r.path.clone());
                                    }
                                }
                                hit
                            })
                            .and_then(|inner| inner.inner);
                        if let Some(p) = chosen {
                            pick = Some(p);
                        }
                    }
                    // 空列表时这枚是**唯一**的入口。以前它旁边摊着一格路径输入和
                    // 「打开 / 克隆…」两枚，用户说太简陋：三件事各有一格输入，挤在一条里根本
                    // 读不清在填什么，现在统一收成一张卡（`Modal::add_repo`）。
                    if ghost_tab(ui, &add_label) {
                        want_add = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = ICON_BTN_GAP;
                        // right_to_left 得反着加：先加的先贴右沿。
                        for i in (0..TOOLS.len()).rev() {
                            if icon_button(ui, i) {
                                tool = Some(i);
                            }
                        }
                        // 组与组之间让 14（`.header{gap:14}`），按钮自己那 2 已经吃进去了。
                        ui.add_space(HEADER_GROUP_GAP - ICON_BTN_GAP);
                        if let Some((branch, counts)) = &head {
                            ui.spacing_mut().item_spacing.x = BRANCH_GAP;
                            if let Some(c) = counts {
                                ui.label(RichText::new(c).size(BRANCH_FONT).color(TEXT_PRIMARY).family(crate::ui::fonts::semibold_family()));
                            }
                            ui.label(RichText::new(branch).size(BRANCH_FONT).color(TEXT_SECONDARY));
                        }
                    });
                });
            });
        // 下沿那条 1px 分隔线（§2「白底 + 下 1px 分隔线」）。画在**父 ui** 上：
        // 卡布里 `ui.max_rect()` 已经被 `inner_margin` 收到 92..1038、37 那格，
        // 在里面画只能画出"让了边距的一小截"；§2 要的是横贯整窗贴着带外沿的那一条。
        ui.painter_at(Rect::from_min_max(outer.min, pos2(outer.max.x, outer.min.y + HEADER_H))).line_segment(
            [pos2(outer.min.x, outer.min.y + HEADER_H - 0.5), pos2(outer.max.x, outer.min.y + HEADER_H - 0.5)],
            Stroke::new(1.0, BORDER_SUBTLE),
        );
        if want_add {
            self.show_modal(Modal::add_repo());
        }
        if want_home {
            self.home = true;
        }
        if let Some(p) = pick {
            self.open(&p);
        }
        if let Some(i) = tool {
            // 卡片开着的时候顶栏不再接第二下 —— 上面那层遮罩挡的是**鼠标**，键盘是共享的：
            // 卡里的文本框正在接 Enter，这时候再按 ⌥⌘ 之类的快捷键开第二张卡会把状态搞乱。
            if self.modal.is_none() {
                self.on_tool(i);
            }
        }
    }

    /// 工具条那 11 枚各自干什么。单独拆出来是因为 `toolbar` 里已经有一份对 `ui` 的借用，
    /// 而这些动作全都要读快照。
    pub(crate) fn on_tool(&mut self, i: usize) {
        match i {
            // 先把视线带回工作区那一行（提交框常驻在文件表下方），再端出提交那张卡。
            // SourceTree 的提交面是内联那一格（`CommitPaneView.nib`），它自己也有一张
            // 提交 sheet（`CommitSheet2.nib`，实测内容区 728×444，里面装的还是那一块面板）：
            // 我们这一张只放**信息**那一半，尺寸走自己那套卡片常量 —— 那张 sheet 的量来的数
            // 记在这里，是因为不记就成了「凭印象」。
            0 if self.snap.is_some() => {
                self.sel = Sel::Uncommitted;
                let branch = self.snap.as_ref().map(|s| s.status.branch.clone()).unwrap_or_default();
                let staged = self
                    .snap
                    .as_ref()
                    .map(|s| {
                        s.status
                            .files
                            .iter()
                            .filter(|f| f.staged && !is_conflict(f))
                            .count()
                    })
                    .unwrap_or(0);
                self.show_modal(Modal::commit(branch, staged));
            }
            // 没有快照 = 还没开仓库：卡里那格提交迟早要落到一本不存在的仓库上，
            // 先把话说清楚，而不是让按钮按下去一点反应都没有。
            0 => self.note = Some(tr!("还没有仓库，先按「添加仓库」开一本", "No repository yet — press \"Add repository\" to open one").to_owned()),
            // 拉取/推送那两枚先端的是**卡**，不是当场就跑的那一条 git：SourceTree 那两枚
            // 也是分别端出 `GitPullSheet` / `GitPushSheet`。卡上的初值全从打开那一刻的快照
            // 与偏好现算，算不出来就报一句，不要按下去一点反应都没有。
            1 => match self.pull_modal() {
                Ok(m) => self.show_modal(m),
                Err(note) => self.note = Some(note),
            },
            2 => match self.push_modal() {
                Ok(m) => self.show_modal(m),
                Err(note) => self.note = Some(note),
            },
            3 => { self.act(Action::Fetch); }
            4 if self.snap.is_some() => self.show_modal(Modal::new_branch()),
            5 => {
                // 合并要有对象：先把本地分支摊出来挑（当前那条自己不算）。
                let Some(s) = &self.snap else {
                    self.note = Some(tr!("还没有可用的快照", "No snapshot ready yet").to_owned());
                    return;
                };
                let branches: Vec<String> =
                    s.branches.iter().filter(|b| !b.is_current).map(|b| b.name.clone()).collect();
                if branches.is_empty() {
                    self.note = Some(tr!("没有别的分支可以合并", "There is no other branch to merge").to_owned());
                } else {
                    self.show_modal(Modal::merge(branches));
                }
            }
            6 if self.snap.is_some() => self.show_modal(Modal::stash_save()),
            7 if self.snap.is_some() => { self.act(Action::BrowseRemote("origin".to_owned())); }
            8 => {
                if let Some(p) = self.repo.clone() {
                    let _ = std::process::Command::new("open").arg(p).spawn();
                }
            }
            9 => {
                // `-a Terminal` 而不是 `x-terminal-emulator` 之类：这个壳现在只在 macOS 上跑
                // （打包、图标、门禁都按 macOS 钉），换平台时这一枚要跟着改。
                if let Some(p) = self.repo.clone() {
                    let _ = std::process::Command::new("open")
                        .arg("-a")
                        .arg("Terminal")
                        .arg(p)
                        .spawn();
                }
            }
            10 => self.show_modal(Modal::settings(self.prefs(), self.repos.len(), db_path())),
            n => self.note = Some(tr!(format!("egui 版还没接：{}（{n}）", tool_label(n)), format!("Not wired in the egui shell: {} ({n})", tool_label(n)))),
        }
    }

    /// 当前分支的上游，拆成（远端名，远端那一侧的分支名）。没有上游就是 `None`：
    /// 一条还没推过的分支，「远端上它叫什么」这件事根本还不存在，两张卡的相应那格留空。
    pub(crate) fn upstream_of(s: &Snapshot) -> Option<(String, String)> {
        let cur = s.branches.iter().find(|b| b.is_current)?;
        let (r, b) = cur.upstream.as_deref()?.split_once('/')?;
        Some((r.to_owned(), b.to_owned()))
    }

    /// 推送那张卡的初值。表里的行、远端那一格、两枚勾都在**打开这一刻**钉下来：
    /// 卡开着的时候后台刷新不该让表换人（勾了一半，「main」可能已经不是刚才那条 main）。
    ///
    /// 远端那一格只有两条来路，按先后：**当前分支的上游**那条远端（这条分支本来就归它管），
    /// 然后是偏好里的**默认远端**。两条都没有就留空、主按钮按不动——不替你猜 `origin`：
    /// 一本仓库可以先推到 fork 再推到 upstream，猜错一次就把提交推到别人的仓库上去。
    pub(crate) fn push_modal(&self) -> std::result::Result<Modal, String> {
        let Some(s) = self.snap.as_ref() else {
            return Err(tr!("还没有仓库，先按「添加仓库」开一本", "No repository yet — press \"Add repository\" to open one").to_owned());
        };
        if s.remotes.is_empty() {
            return Err(tr!("这本仓库还没有远端可推。", "This repository has no remote to push to.").to_owned());
        }
        let remote = Self::upstream_of(s)
            .map(|(r, _)| r)
            .or_else(|| Some(self.default_remote.clone()))
            .filter(|r| !r.is_empty());
        let rows = s
            .branches
            .iter()
            .map(|b| PushRow {
                local: b.name.clone(),
                // 默认同名：卡上那一格预先填好的是本地那一条的名字（SourceTree 也是）。
                remote_name: b.name.clone(),
                has_upstream: b.upstream.is_some(),
                picked: self.push_behavior.picks(b),
                // 「跟踪？」只对**还没有上游**的分支预先勾上：已有上游的那条，跟踪关系
                // 早就记好了，再 `-u` 一次只是把参数写错的机会摆在那儿。
                track: b.upstream.is_none(),
            })
            .collect();
        Ok(Modal::push(rows, remote.unwrap_or_default(), &self.prefs()))
    }

    /// 拉取那张卡的初值。远端和推送那一格同源；远程分支预先落在**当前分支的上游**那一条
    /// （没有上游就留空，逼这一格由人挑），本地那格就是当前分支——`git pull` 本来也只
    /// 拉得进当前分支，这一格摆出来是为了「目标不是当前分支时先切过去」那条路说得清。
    pub(crate) fn pull_modal(&self) -> std::result::Result<Modal, String> {
        let Some(s) = self.snap.as_ref() else {
            return Err(tr!("还没有仓库，先按「添加仓库」开一本", "No repository yet — press \"Add repository\" to open one").to_owned());
        };
        if s.remotes.is_empty() {
            return Err(tr!("这本仓库还没有远端可拉。", "This repository has no remote to pull from.").to_owned());
        }
        let up = Self::upstream_of(s);
        let remote = up
            .as_ref()
            .map(|(r, _)| r.clone())
            .or_else(|| Some(self.default_remote.clone()))
            .filter(|r| !r.is_empty())
            .unwrap_or_default();
        let current = s.branches.iter().find(|b| b.is_current).map(|b| b.name.clone());
        let branch = up.map(|(_, b)| b).unwrap_or_default();
        // 「拉取到本地分支」那一条：`current` 取不到时才退回状态里那条分支名，
        // 而且**不静默**——空串会让那一格显示占位文案、主按钮按不动，用户看得见。
        let into = current.unwrap_or_else(|| s.status.branch.clone());
        Ok(Modal::pull(remote, branch, into))
    }

    /// 把一张卡推到屏幕上，并让光标落在它的文本框里。
    /// 没有文本框的卡（挑分支、确认）这枚标记自然是 `false`，下一帧 `modal_layer` 就把它清掉。
    pub(crate) fn show_modal(&mut self, m: Modal) {
        self.modal_focus = m.text.is_some();
        self.modal = Some(m);
    }

    /// 界面上当下那几枚开关的位置。偏好卡打开时读它，回写时也比对它——
    /// 以**界面**为准而不是数据库那份：会话里改过、还没重启，此时库里那份和屏幕上不该有两样。
    pub(crate) fn prefs(&self) -> Prefs {
        Prefs {
            restore_last: self.restore_last,
            topo: self.topo,
            show_remote: self.show_remote,
            push_behavior: self.push_behavior,
            default_remote: self.default_remote.clone(),
            push_tags: self.push_tags,
            allow_force: self.allow_force,
            lang: self.lang,
            tokens: self.provider_tokens.clone(),
        }
    }

    /// 偏好卡上那几枚开关**当场生效、当场落库**（macOS 的偏好窗口没有「应用」按钮）。
    /// 一条都没动就直接返回：`topo` 那一条要重取整份快照，每帧白跑一次是看得见的开销。
    /// 返回值只有一件事要说：**语言换档了**——换档那一刻正开着的卡要按新档重拼一次，
    /// 而那张卡握在调用方手里（见 `modal_layer`），在这儿够不着。
    pub(crate) fn apply_prefs(&mut self, p: Prefs) -> bool {
        if p == self.prefs() {
            return false;
        }
        let re_sort = p.topo != self.topo;
        let re_lang = p.lang != self.lang;
        // 落库要的是**改之前**那一份完整的值：下面几条里有 `String` 字段会被移走，
        // 移走之后再 `p.save()` 就编译不过，所以先拷一份专门用来写。
        let to_save = p.clone();
        self.restore_last = p.restore_last;
        self.show_remote = p.show_remote;
        self.push_behavior = p.push_behavior;
        self.default_remote = p.default_remote;
        self.push_tags = p.push_tags;
        self.allow_force = p.allow_force;
        self.lang = p.lang;
        self.provider_tokens = p.tokens;
        to_save.save(self.conn.as_ref());
        if re_sort {
            // 排序变了要让日志重取一份，不然屏幕上还是旧顺序。
            self.set_topo(p.topo);
        }
        if re_lang {
            // 换档要把**后台拼好、缓存下来**的那些文案重算一遍：药丸上的「超前 2 个版本」、
            // 差异里的 hunk 头、历史卡每条提交下面的补丁名，都是取数线程按当时语言写死的。
            // 不重取，切完 English 屏幕上就留下一半英文一半中文——那是看得见的半截翻译。
            // 先把线程档位刷成新语言再重取：`shell()` 那一行要等下一帧才跑，而
            // `patch_label` 现在就在这条主线程上读档位。
            gitx::lang::set(self.lang);
            // 正开着的卡由调用方（`modal_layer`）按新档重拼——那张卡这一帧握在它手里，
            // 在这儿读 `self.modal` 是空的，够不着（判据第一次就跑红在那儿）。
            let hist = self.history.as_ref().map(|h| h.path.clone());
            let target = self.target.clone();
            // `set_topo` 里已经 `open()` 过一次，别在这儿再来一遍：快照会取两份。
            if !re_sort {
                if let Some(repo) = self.repo.clone() {
                    self.open(&repo);
                }
            }
            // `open()` 固定回到「未提交变更」，重新指回用户本来在看的那一格差异。
            if let Some(t) = target {
                self.request(t);
            }
            if let Some(path) = hist {
                self.open_history(&path);
            }
        }
        re_lang
    }

    /// 打开「文件历史」浮层。列表自己走线程，帧里不等 git。
    pub(crate) fn open_history(&mut self, path: &str) {
        let Some(repo) = self.repo.clone() else { return };
        self.hist_seq += 1;
        let seq = self.hist_seq;
        self.history = Some(FileHistory::new(path.to_owned(), seq));
        spawn_file_log(&self.tx, &repo, path, seq);
    }

    /// 文件历史浮层。和模态卡同一层、同一套遮罩；区别是它自己带页签与列表，
    /// 而且关掉之后不留动作（里面唯一会动手的「恢复」走 `land` 的确认卡）。
    pub(crate) fn history_layer(&mut self, ctx: &egui::Context) {
        let Some(mut h) = self.history.take() else { return };
        let repo = self.repo.clone();
        let tx = self.tx.clone();
        let screen = ctx.viewport_rect();
        // 确认卡开着时 Esc 归它：面板别抢。
        let esc = self.modal.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let card = Rect::from_center_size(
            screen.center(),
            vec2(
                (screen.width() - 80.0).clamp(520.0, HIST_W),
                (screen.height() - 24.0).clamp(240.0, HIST_H),
            ),
        );
        let mut close = false;
        let mut want_tab: Option<HistTab> = None;
        let mut want_view: Option<HistView> = None;
        let mut pick: Option<usize> = None;
        let mut restore = false;
        egui::Area::new(egui::Id::unique("gitx-history"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // 和模态卡一样：这一格铺满整屏的可点矩形既当遮罩，也当「点外面关掉」。
                let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(bg, 0.0, Color32::from_black_alpha(80));
                let p = ui.painter();
                p.rect_filled(card, 6.0, Color32::WHITE);
                p.rect_stroke(card, 6.0, Stroke::new(1.0, BORDER_SUBTLE), egui::StrokeKind::Middle);

                let head_cy = card.min.y + HIST_HEAD_H / 2.0;
                put_left(
                    ui,
                    Rect::from_min_max(
                        pos2(card.min.x + 12.0, card.min.y),
                        pos2(card.max.x - 80.0, card.min.y + HIST_HEAD_H),
                    ),
                    |ui| {
                        ui.add(
                            Label::new(
                                RichText::new(tr!(format!("文件历史 · {}", h.path), format!("File history · {}", h.path)))
                                    .size(12.5)
                                    .color(TEXT_PRIMARY)
                                    .strong(),
                            )
                            .truncate()
                            .selectable(false),
                        );
                    },
                );
                if hunk_button(ui, tr!("关闭", "Close"), card.max.x - HIST_PAD, head_cy).0 {
                    close = true;
                }

                let tab_y = card.min.y + HIST_HEAD_H;
                put_left(
                    ui,
                    Rect::from_min_max(
                        pos2(card.min.x + HIST_PAD, tab_y),
                        pos2(card.max.x - HIST_PAD, tab_y + HIST_TAB_H),
                    ),
                    |ui| {
                        ui.horizontal(|ui| {
                            if ui.selectable_label(h.tab == HistTab::History, tr!("历史", "History")).clicked() {
                                want_tab = Some(HistTab::History);
                            }
                            if ui.selectable_label(h.tab == HistTab::Blame, tr!("逐行归属", "Blame")).clicked() {
                                want_tab = Some(HistTab::Blame);
                            }
                            if h.tab == HistTab::History {
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    let can_restore = !h.entries.is_empty() && h.sel < h.entries.len();
                                    let btn = egui::Button::new(RichText::new(tr!("恢复到这一版", "Restore this version")).size(11.0));
                                    if ui.add_enabled(can_restore, btn).clicked() {
                                        restore = true;
                                    }
                                    if ui.selectable_label(h.view == HistView::Full, tr!("全文", "Full text")).clicked() {
                                        want_view = Some(HistView::Full);
                                    }
                                    if ui.selectable_label(h.view == HistView::Diff, tr!("改动", "Changes")).clicked() {
                                        want_view = Some(HistView::Diff);
                                    }
                                });
                            }
                        });
                    },
                );

                let body = Rect::from_min_max(
                    pos2(card.min.x + HIST_PAD, tab_y + HIST_TAB_H),
                    pos2(card.max.x - HIST_PAD, card.max.y - HIST_PAD),
                );
                match h.tab {
                    HistTab::History => {
                        let list_w = HIST_LIST_W.min(body.width() * 0.5);
                        let list = Rect::from_min_max(body.min, pos2(body.min.x + list_w, body.max.y));
                        let right = Rect::from_min_max(pos2(list.max.x + 8.0, body.min.y), body.max);
                        ui.painter().line_segment(
                            [pos2(list.max.x + 4.0, body.min.y), pos2(list.max.x + 4.0, body.max.y)],
                            Stroke::new(1.0, BORDER_SUBTLE),
                        );
                        if h.entries.is_empty() {
                            history_note(
                                ui,
                                list,
                                if h.log_loading { tr!("读取历史…", "Reading history…") } else { tr!("这个文件没有历史。", "This file has no history.") },
                            );
                        } else {
                            let now = chrono::Local::now();
                            ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
                                ui.set_clip_rect(list);
                                let row_w = ui.available_width();
                                ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                                    ui,
                                    HIST_ROW_H,
                                    h.entries.len(),
                                    |ui, range| {
                                        for i in range {
                                            bump_rows_painted();
                                            let e = &h.entries[i];
                                            let date = date_label(&e.commit.date, now);
                                            if history_row(ui, row_w, e, &date, i == h.sel).clicked() {
                                                pick = Some(i);
                                            }
                                        }
                                    },
                                );
                            });
                        }
                        ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
                            ui.set_clip_rect(right);
                            let row_w = ui.available_width();
                            match h.view {
                                HistView::Diff => {
                                    if h.diff.is_empty() && h.patch_loading {
                                        history_note(ui, right, tr!("读取改动…", "Reading the change…"));
                                    } else if h.diff.is_empty() {
                                        history_note(ui, right, tr!("这个版本对这个文件没有可显示的改动。", "This version shows no change for this file."));
                                    } else {
                                        ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                                            ui,
                                            DIFF_ROW_H,
                                            h.diff.len(),
                                            |ui, range| {
                                                for i in range {
                                                    bump_rows_painted();
                                                    history_diff_row(ui, row_w, &h.diff[i]);
                                                }
                                            },
                                        );
                                    }
                                }
                                HistView::Full => {
                                    if h.full.is_empty() && h.full_loading {
                                        history_note(ui, right, tr!("读取全文…", "Reading the full text…"));
                                    } else if h.full.is_empty() {
                                        history_note(ui, right, tr!("这个版本里没有内容。", "Nothing at this version."));
                                    } else {
                                        ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                                            ui,
                                            DIFF_ROW_H,
                                            h.full.len(),
                                            |ui, range| {
                                                for i in range {
                                                    bump_rows_painted();
                                                    history_text_row(ui, row_w, i + 1, &h.full[i]);
                                                }
                                            },
                                        );
                                    }
                                }
                            }
                        });
                    }
                    HistTab::Blame => {
                        if h.blame.is_empty() {
                            history_note(
                                ui,
                                body,
                                if h.blame_loading { tr!("读取逐行归属…", "Reading blame…") } else { tr!("这个文件没有可归属的行。", "This file has no lines to blame.") },
                            );
                        } else {
                            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                                ui.set_clip_rect(body);
                                let row_w = ui.available_width();
                                ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                                    ui,
                                    HIST_BLAME_ROW_H,
                                    h.blame.len(),
                                    |ui, range| {
                                        for i in range {
                                            bump_rows_painted();
                                            blame_row(ui, row_w, &h.blame[i]);
                                        }
                                    },
                                );
                            });
                        }
                    }
                }
                // 点面板外面的遮罩才关；点面板里的空处不关（`bg_resp` 会漏到这一格）。
                let inside = ctx
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|at| card.contains(at));
                if bg_resp.clicked() && !inside {
                    close = true;
                }
            });
        if esc {
            close = true;
        }
        if close {
            self.history = None;
            return;
        }

        if let Some(t) = want_tab {
            if h.tab != t {
                h.tab = t;
                if t == HistTab::Blame && h.blame.is_empty() && !h.blame_loading {
                    if let Some(repo) = &repo {
                        h.blame_loading = true;
                        spawn_history_blame(&tx, repo, &h.path, h.seq);
                    }
                }
            }
        }
        if let Some(v) = want_view {
            h.view = v;
            if v == HistView::Full && h.full_for != Some(h.sel) && !h.full_loading {
                if let (Some(repo), Some(e)) = (&repo, h.entries.get(h.sel)) {
                    h.full_loading = true;
                    h.full_for = Some(h.sel);
                    spawn_history_text(&tx, repo, &e.commit.id, &h.path, h.seq, h.sel);
                }
            }
        }
        if let Some(i) = pick {
            if i != h.sel || h.diff.is_empty() {
                h.sel = i;
                h.diff.clear();
                h.full.clear();
                h.full_for = None;
                if let (Some(repo), Some(e)) = (&repo, h.entries.get(i)) {
                    h.patch_loading = true;
                    spawn_history_patch(&tx, repo, &e.commit.id, &h.path, h.seq, i);
                    if h.view == HistView::Full {
                        h.full_loading = true;
                        h.full_for = Some(i);
                        spawn_history_text(&tx, repo, &e.commit.id, &h.path, h.seq, i);
                    }
                }
            }
        }
        let restore_oid = restore.then(|| h.entries.get(h.sel).map(|e| e.commit.id.clone())).flatten();
        let path = h.path.clone();
        self.history = Some(h);
        if let Some(oid) = restore_oid {
            // 覆盖工作区必须过确认卡，所以走 `land` 而不是 `act`。
            self.land(Intent { act: Some(Action::RestoreFile { oid, path }), ..Default::default() });
        }
    }

    /// 把一帧里从各处收上来的意图落地。顺序是定死的：**先说话、再挪位置、最后才动手**。
    ///
    /// 动手之前还有一道闸：`needs_confirm` 那几类不直接执行，而是换一张确认卡。
    /// 拦在这里而不是每一处菜单里，是因为入口只会越来越多（侧栏、日志行、文件行、工具条），
    /// 逐处加「确认」迟早会漏一处——而漏的那一处正好是点错时会丢工作的那一处。
    /// 把一个意图落地（说明、跳转、卡片、动作）并**回报这一步有没有被接住**——
    /// 只有 `act` 那一路可能是 false，卡片那一路（包括确认卡）都算接住了。
    pub(crate) fn land(&mut self, it: Intent) -> bool {
        if let Some(n) = it.note {
            self.note = Some(n);
        }
        // 侧栏「远端」的展开/收起。`Intent` 里那两格从加进来就没人接：注释写着「见 land」，
        // 而 land 里没有对应的一段——点一下远端行什么都不发生（2026-10-26 查 `origin` 默认
        // 展开时发现的）。收在这里一次接上，展开与「摊开全部」各一条。
        if let Some(name) = it.toggle_remote {
            match self.remote_open.iter().position(|n| n == &name) {
                Some(i) => {
                    self.remote_open.remove(i);
                }
                None => self.remote_open.push(name),
            }
        }
        if let Some(name) = it.toggle_remote_all {
            if self.remote_all.iter().any(|n| n == &name) {
                self.remote_all.retain(|n| n != &name);
            } else {
                self.remote_all.push(name);
            }
        }
        // 左栏那枚「看板 / 仓库」切换（DESIGN §2.2）：切过去就认 provider、读「看过」、
        // 起一次取数；切回来只翻旗子，`board` 里那份数据留着——下次进来先画旧的，
        // 新取数落地再顶掉（列表闪一下比整栏空白一下好）。
        if it.toggle_board {
            if self.board_open {
                self.board_open = false;
                // 首次建账只覆盖**这一趟**看着它取数的过程。关栏即结束：下一次打开
                // 有上一趟的基线可比，那时候「动了」才是真动了。
                self.board.seeding = false;
            } else {
                self.show_board();
            }
        }
        // 「刷新」只在看板开着时有意义：关着的那一栏没人看，请求发了也是白跑一次网络。
        if it.refresh_board && self.board_open {
            self.fetch_board();
        }
        // 点开一条 PR：记「看到这儿了」+ 把地址攒给 `poll` 推给 egui。没算出地址
        // （认不出 provider 又没有 `html_url`）就只记号、不推命令——推一条空 URL
        // 等于按下去没反应。
        if let Some(i) = it.open_pr {
            self.board_open_url = self.open_pull_request(i);
        }
        // 这一帧画在可见区里的那几行：补上列表不给的两格。放在最后，是因为上面
        // 「切栏 / 刷新」会换掉 `board.prs` 和 `asked` 那本账，先补就补到旧的一代上去。
        if !it.enrich.is_empty() {
            self.enrich_rows(&it.enrich);
        }
        // 日志条右端那枚「搜索」（侧栏那行 2026-10-26 已删）。放在 `goto` 之前：
        // 它点的是**开一格**、不是选一行，两者同一帧里不会互相覆盖
        // （开着搜索再点某一行，选中的还是那一行）。
        if it.open_search {
            self.open_search();        }
        if let Some(hit) = it.goto {
            self.select(hit);
        }
        if let Some(oid) = &it.reveal {
            let found = self
                .snap
                .as_ref()
                .and_then(|s| s.commits.iter().position(|c| c.id == *oid));
            match found {
                Some(i) => self.select(Sel::Commit(i)),
                None => {
                    let short = oid.chars().take(8).collect::<String>();
                    self.note = Some(tr!(
                        format!("{short} 还不在已经加载的那几页历史里（往下滚会取到）"),
                        format!("{short} is not in the pages loaded yet — scroll down to fetch more")
                    ));
                }
            }
        }
        if let Some(m) = it.card {
            self.show_modal(m);
            return true;
        }
        // 返回值只为一件事：那张卡要不要关。`false` = 这一步当场没被接住（路径不是仓库、
        // 手上根本还没仓库），调用方据此把卡原样留着，让打错的那个人改一个字再点。
        if let Some(a) = it.act {
            if needs_confirm(&a) {
                let (title, detail) = confirm_of(&a);
                self.show_modal(Modal::confirm(title, detail, a));
                true
            } else {
                self.act(a)
            }
        } else {
            true
        }
    }

    // -------------------------------------------------------------------- 首页
    /// 顶栏最左那枚「首页」tab 打开的那一页：仓库列表为主（A）+ 当前仓库的代码托管信息
    /// （少量 B，用户 2026-10-26 的要求）。详细的 provider 面板（PR / CI）留到下一批——
    /// 这一批不引 HTTP 依赖，也不摆按下去没反应的按钮。
    ///
    /// 首页**整页**替掉侧栏 / 日志 / 暂存带，只留顶栏与状态栏：那三块讲的都是「当前仓库」，
    /// 而首页面前还没有当前仓库。窗口里仍保留「首页」tab，随时点仓库 tab 回去。
    pub(crate) fn home_page(&mut self, ui: &mut egui::Ui) {
        let mut open_repo: Option<String> = None;
        let mut want_add = false;
        let mut want_clone = false;
        let mut browse: Option<String> = None;

        let repos = self.repos.clone();
        let current = self.repo.clone();
        // 当前仓库那一行右端的备注：分支 + 只在有上游时出现的 ↑↓。
        let current_note = self
            .snap
            .as_ref()
            .map(|s| {
                let st = &s.status;
                let mut note = st.branch.clone();
                if st.upstream.is_some() && (st.ahead > 0 || st.behind > 0) {
                    note.push_str(&format!(" · ↑{} ↓{}", st.ahead, st.behind));
                }
                note
            })
            .unwrap_or_default();
        let provider = self.snap.as_ref().and_then(home_provider);

        // 居中靠**自己划一块竖排矩形**，不是 `horizontal + add_space`：
        // `ScrollArea` 里的子 ui 会继承父布局，塞进 `horizontal` 之后整页内容会横着排、
        // 全部叠在同一列 y 上（2026-10-26 第一版就是这样，判据量出来每块文字 y≈70）。
        let outer = ui.available_rect_before_wrap();
        let width = outer.width().min(HOME_MAX_W);
        let side = ((outer.width() - width) / 2.0).max(16.0);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG_WINDOW))
            .show(ui, |ui| {
                let rect = Rect::from_min_size(outer.min + vec2(side, 0.0), vec2(width, outer.height()));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(Align::Min)),
                    |ui| {
                        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(width - 10.0);
                            ui.add_space(HOME_TOP);
                            ui.label(
                                RichText::new(home_tab())
                                    .size(20.0)
                                    .color(TEXT_PRIMARY)
                                    .family(crate::ui::fonts::semibold_family()),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(tr!("打开一个仓库继续，或添加新的仓库。", "Open a repository to continue, or add a new one."))
                                    .size(13.0)
                                    .color(TEXT_SECONDARY),
                            );
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(tr!("打开仓库…", "Open repository…")).size(13.0).color(Color32::WHITE),
                                        )
                                        .fill(ACCENT)
                                        .corner_radius(egui::CornerRadius::same(6)),
                                    )
                                    .clicked()
                                {
                                    want_add = true;
                                }
                                if ui.add(egui::Button::new(tr!("克隆仓库…", "Clone repository…")).small()).clicked() {
                                    want_clone = true;
                                }
                            });
                            ui.add_space(24.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(tr!("仓库", "Repositories"))
                                        .size(13.0)
                                        .color(TEXT_PRIMARY)
                                        .family(crate::ui::fonts::semibold_family()),
                                );
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(tr!(format!("{} 个", repos.len()), format!("{} shown", repos.len())))
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                });
                            });
                            ui.add_space(8.0);
                            if repos.is_empty() {
                                ui.label(
                                    RichText::new(tr!(
                                        "还没有添加仓库：用上面的「打开仓库…」或「克隆仓库…」开始。",
                                        "Nothing added yet — start with \"Open repository…\" or \"Clone repository…\" above."
                                    ))
                                        .size(12.0)
                                        .color(TEXT_MUTED),
                                );
                            } else {
                                ui.spacing_mut().item_spacing.y = HOME_CARD_GAP;
                                for r in &repos {
                                    let is_current = current.as_deref() == Some(r.path.as_str());
                                    let note = if is_current { current_note.as_str() } else { "" };
                                    if home_repo_card(ui, &r.name, &r.path, note, is_current) {
                                        open_repo = Some(r.path.clone());
                                    }
                                }
                            }
                            // 少量 B：当前仓库能认出是哪一家代码托管，就把那句 `GitHub · owner/repo`
                            // 摆出来，并给一个**真的能用**的「在浏览器打开」。PR/CI 那些详细 B
                            // 要等 provider 面板接上 HTTP 之后另开一批。
                            if let Some((label, path, remote)) = &provider {
                                ui.add_space(24.0);
                                ui.label(RichText::new(tr!("代码托管", "Hosting")).size(11.0).color(TEXT_SECONDARY));
                                ui.add_space(6.0);
                                egui::Frame::NONE
                                    .fill(BG_WINDOW)
                                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                                    .corner_radius(egui::CornerRadius::same(HOME_CARD_RAD))
                                    .inner_margin(Margin::symmetric(12, 10))
                                    .show(ui, |ui| {
                                        // 卡内宽 = 外层 710 − 左右内边距 24 − 边 2；
                                        // 再按 `width - 26` 算一次会多出 10pt，横向被裁。
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            provider_badge(ui, label);
                                            ui.label(
                                                RichText::new(path).size(12.0).color(TEXT_PRIMARY),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if ui
                                                        .add(egui::Button::new(tr!("在浏览器打开", "Open in browser")).small())
                                                        .clicked()
                                                    {
                                                        browse = Some(remote.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                            }
                            ui.add_space(HOME_TOP);
                        });
                    },
                );
            });

        if want_add {
            self.show_modal(Modal::add_repo());
        }
        if want_clone {
            let mut m = Modal::add_repo();
            m.form = Form::AddRepo { tab: AddTab::Clone, dest: String::new() };
            self.show_modal(m);
        }
        if let Some(path) = open_repo {
            self.open(&path);
        }
        if let Some(remote) = browse {
            self.act(Action::BrowseRemote(remote));
        }
    }

    // ------------------------------------------------------------------ 侧栏
    /// §2：188pt 灰底一栏，分组头 = 11px 灰字 + 右对齐计数（**无图标**），子项行高 26、
    /// 选中 = 浅蓝药丸（圆角 6），当前分支 = 蓝点 + 中黑字，底置一枚 26pt 筛选输入框。
    /// **没有**折叠三角那一枚字符（§2 明写）；「远端」那一组能点开，靠的是子项往右缩一档、
    /// 而不是一个箭头。其余几组的折叠归 S6-3b 和仓库分组一起接。
    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        let none = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (branches, tags, remotes, stashes, remote_refs) = match &self.snap {
            Some(s) => (&s.branches, &s.tags, &s.remotes, &s.stashes, &s.remote_refs),
            None => (&none.0, &none.1, &none.2, &none.3, &none.4),
        };
        let sel = self.sel.clone();
        let mut filter = self.side_filter.clone();
        // 展开着的那几条远端的名字，**开局读一份**：面板闭包里 `self` 正被借走，
        // 而这一份只用来决定画不画，改它走 `Intent::toggle_remote`（见 `land`）。
        let remote_open = self.remote_open.clone();
        // 已经点过「还有 N 个」的那些远端：默认每个远端只画前 3 条分支。
        let remote_all = self.remote_all.clone();
        // 看板那一栏要在闭包里画，`self` 那时候正被借走——所以数据**开局拷一份**。
        // `Board` 本身不 Clone（里面存的是 `&'static dyn Host`），拷的是画得出来的那几块。
        let board_head = self.board.host.map(|h| h.label().to_owned());
        // 底置那格筛选框在看板档筛的是**这一栏**（标题 / #号 / 状态那句），和它筛大纲同一句话。
        // 不接的话框还挂在那儿却什么都不筛——那是个看得见的假控件。
        // 行上带**原始下标**：`open_pr` 要指回 `board.prs` 里的那一条，不能指到筛完的位置。
        let needle = filter.trim().to_owned();
        let lang = self.lang;
        // 一句话状态不只读 PR 自己那一格：「正在看它卡在哪」要读这一行有没有在飞的
        // 补充请求，「大概率与本次改动无关」要读**别条 PR** 上是不是也挂着同一个 job。
        // 两件都不属于某一条 PR 的数据，所以从 `Board` 那两本账现取（见 `StatusContext`）。
        let ctx_of = |pr: &gitx::models::PullRequest| provider::StatusContext {
            fetching: self.board.row_fetching(pr),
            failing_elsewhere: self.board.failing_elsewhere(pr.number),
        };
        let board_rows: Vec<(usize, u32, String, String, Color32, bool)> = self
            .board
            .prs
            .iter()
            .enumerate()
            .filter(|(_, pr)| {
                needle.is_empty()
                    || pr.title.contains(needle.as_str())
                    || pr.number.to_string().contains(needle.as_str())
                    // 筛的是**看得见的那一句**：英文档下敲「ready」要命中的就是英文状态句。
                    || provider::pr_status(pr, &ctx_of(pr)).label(lang).contains(needle.as_str())
            })
            .map(|(i, pr)| {
                let status = provider::pr_status(pr, &ctx_of(pr));
                (
                    i,
                    pr.number,
                    pr.title.clone(),
                    status.label(lang).to_owned(),
                    board_status_color(status),
                    self.board.is_new(pr),
                )
            })
            .collect();
        // 「一行都没有」和「有一行但被筛掉了」是两件事：后者写「没有开放的 PR」就是骗人。
        let board_hidden = !needle.is_empty() && board_rows.is_empty() && !self.board.prs.is_empty();
        let (board_loaded, board_fetching, board_error) = (
            self.board.loaded,
            self.board.fetching,
            self.board.error.clone(),
        );
        let (board_open, board_hidden) = (self.board_open, board_hidden);
        let mut intent = Intent::default();
        egui::Panel::left("sidebar")
            .exact_size(SIDEBAR_W)
            .frame(
                egui::Frame::NONE
                    .fill(BG_SIDEBAR)
                    .inner_margin(Margin { left: SIDE_INSET, right: SIDE_INSET, top: SIDE_VPAD, bottom: SIDE_VPAD }),
            )
            .show(ui, |ui| {
                // 先把最底那条筛选框的高度扣掉再开滚动区：ScrollArea 默认吃掉全部可用高度，
                // 不扣的话「筛选器」会被挤到窗口外（无头测试里侧栏的底色矩形超出窗口 27pt）。
                let list_h = (ui.available_height() - SIDE_FILTER_H).max(60.0);
                ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
                    // §2.2 看板：**整栏替掉**下面那套大纲。左缘一行是回大纲的入口，
                    // 顶上是「代码托管 · 哪一家 · 几条」加一枚真的会重发请求的刷新。
                    if board_open {
                        if side_row(ui, tr!("‹ 仓库", "‹ Repositories"), false, false).clicked() {
                            intent.toggle_board = true;
                        }
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(
                                RichText::new(tr!("代码托管", "Hosting"))
                                    .size(SIDE_GROUP_FONT)
                                    .color(TEXT_SECONDARY),
                            );
                            if let Some(head) = &board_head {
                                ui.label(RichText::new(head).size(SIDE_GROUP_FONT).color(TEXT_PRIMARY));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add(egui::Button::new(RichText::new(tr!("刷新", "Fetch")).size(11.0)).small())
                                    .clicked()
                                {
                                    intent.refresh_board = true;
                                }
                            });
                        });
                        if board_rows.is_empty() {
                            // 三种「没有行」是三件不同的事，各说各的：在飞、失败、真的没有。
                            // 合成一句「没有开放的 PR」会在取数失败时骗人。
                            // 第四件是「有，但被筛选框滤掉了」——那一格单独一句，
                            // 因为清空筛选框就有行，说「没有开放的 PR」同样是在骗人。
                            let line = if board_hidden {
                                tr!("都被筛选框里那几个字滤掉了", "All hidden by what is in the filter box")
                            } else if board_fetching {
                                tr!("正在取…", "Fetching…")
                            } else if let Some(e) = &board_error {
                                e.as_str()
                            } else if board_loaded {
                                tr!("没有开放的 PR", "No open pull requests")
                            } else {
                                tr!("还没有取：按「刷新」取一次", "Never fetched — press \"Fetch\"")
                            };
                            ui.label(
                                RichText::new(line).size(11.0).color(if board_fetching {
                                    TEXT_MUTED
                                } else if board_error.is_some() {
                                    DANGER
                                } else {
                                    TEXT_MUTED
                                }),
                            );
                        }
                        for (i, number, title, status, color, is_new) in board_rows.iter()
                        {
                            let row = board_pr_row(ui, *number, title, status, *color, *is_new, false);
                            // 只补**画在可见区里**的那几行（见 `Intent::enrich`）。
                            // 滚动区外的行有自己的矩形，只是被 clip 掉了 —— 所以判据是
                            // 行矩形和 `clip_rect` 相交，不是「这行画没画」。
                            if row.rect.intersects(ui.clip_rect()) {
                                intent.enrich.push(*i);
                            }
                            if row.clicked() {
                                intent.open_pr = Some(*i);
                            }
                        }
                        return;
                    }
                    // 大纲最顶上一行 = 进看板的入口（§2.2）。点它和点「看板」那一组
                    // 是同一条路：认 provider、读「看过」、起一次取数。
                    if side_row(ui, tr!("看板", "Board"), false, false).clicked() {
                        intent.toggle_board = true;
                    }
                    // 「工作区」那一档固定三条，所以计数写死 3；它不是从 `items` 数出来的。
                    side_group_row(ui, tr!("工作区", "Working tree"), 3);
                    for (label, hit) in [
                        (tr!("文件状态", "File status"), Sel::Uncommitted),
                        (tr!("历史", "History"), Sel::Commit(0)),
                    ] {
                        if side_row(ui, label, sel == hit, false).clicked() {
                            intent.goto = Some(hit);
                        }
                    }
                    // SourceTree 的工作区组里有「搜索」这一行，点开的是它自己那块**带作用域条
                    // 的搜索面板**。本机没有 SourceTree.app（`/Applications` 和 `mdfind` 都查过），
                    // 那块面板的控件一个数都没量到，参考图也只拍到这一行本身——所以这里
                    // **不照抄它**，按 2026-10-21 规则②做成我们的增强：把日志条右边那一格
                    // 换成搜索框，搜整本历史的提交说明 + 改动过的文件名。文案与档位都是自定，
                    // 登记在 DESIGN §7。
                    // 2026-10-26 用户要求**去掉侧栏这一行**：入口只在日志条右边那一格，
                    // 免得「搜索中」看起来有两种状态。
                    side_category(
                        ui,
                        tr!("分支", "Branches"),
                        &filter,
                        branches,
                        |b| b.name.clone(),
                        // 分支这一组没有「选中」：`Sel` 里没这一档，点它就是切过去（会改 HEAD，
                        // 回来时 `is_current` 自己会挪地方）。§2 那颗蓝点标的是**当前**，不是选中。
                        |_| false,
                        |b| b.is_current,
                        |b, intent| intent.run(Action::Checkout(b.name.clone())),
                        |ui, b, intent| {
                            menu_item(ui, tr!("切换过去", "Check out"), || Action::Checkout(b.name.clone()), intent);
                            menu_card(
                                ui,
                                tr!("从这里新建分支", "New branch from here"),
                                || Modal::new_branch_at(Some(b.target.clone())),
                                intent,
                            );
                            menu_item(ui, tr!("合并到当前分支", "Merge into current branch"), || Action::Merge(b.name.clone()), intent);
                            menu_card(
                                ui,
                                tr!("重命名分支…", "Rename branch…"),
                                || Modal::rename_branch(b.name.clone()),
                                intent,
                            );
                            menu_item(ui, tr!("删除这个分支", "Delete this branch"), || Action::DeleteBranch(b.name.clone()), intent);
                        },
                        // 本地分支没有第三级：点开没有下一步，硬摆一格空的只会让用户以为坏了。
                        |_, _, _| {},
                        &mut intent,
                    );
                    side_category(
                        ui,
                        tr!("标签", "Tags"),
                        &filter,
                        tags,
                        |t| t.name.clone(),
                        |_| false,
                        |_| false,
                        |t, intent| intent.reveal = Some(t.target.clone()),
                        |ui, t, intent| {
                            menu_go(ui, tr!("到那条提交", "Go to that commit"), t.target.clone(), intent);
                            menu_item(ui, tr!("检出这个标签", "Check out this tag"), || Action::Checkout(t.target.clone()), intent);
                            menu_card(
                                ui,
                                tr!("在这条提交上新建分支", "New branch from this commit"),
                                || Modal::new_branch_at(Some(t.target.clone())),
                                intent,
                            );
                            menu_item(ui, tr!("删除这个标签", "Delete this tag"), || Action::DeleteTag(t.name.clone()), intent);
                        },
                        |_, _, _| {},
                        &mut intent,
                    );
                    side_category(
                        ui,
                        tr!("远端", "Remotes"),
                        &filter,
                        remotes,
                        |r| r.name.clone(),
                        |_| false,
                        |_| false,
                        // 左键管展开/收起。没有箭头字符（§2 明写侧栏不画折叠三角），
                        // 展开这件事靠下面那几行往右缩一档来说。
                        |r, intent| {
                            intent.toggle_remote = Some(r.name.clone());
                        },
                        |ui, r, intent| {
                            menu_item(ui, tr!("抓取这个远端", "Fetch this remote"), || Action::FetchRemote(r.name.clone()), intent);
                            menu_item(ui, tr!("抓取全部远端", "Fetch all remotes"), || Action::Fetch, intent);
                            menu_item(ui, tr!("在浏览器里打开主页", "Open home page in browser"), || Action::BrowseRemote(r.name.clone()), intent);
                            // 地址那一格从**点一下**挪到了右键：左键现在管展开，不能再兼着念地址。
                            if ui.button(tr!("看地址", "Show URL")).clicked() {
                                intent.note = Some(format!("{} → {}", r.name, r.url));
                                ui.close();
                            }
                        },
                        // 点开之后：这一条远端的远程分支，剥掉 `origin/` 那一截前缀
                        // （远端已经由上面那一行说了，行上要说的是分支）。
                        // 筛选只筛**远端名**：筛到 `fork` 时它的分支跟着出来，反过来
                        // 只命中分支名而远端名不命中时，那一组整个被筛掉了——这一档限制
                        // 登记在 `DESIGN.md` §7。
                        |ui, r, intent| {
                            if !remote_open.iter().any(|n| n == &r.name) {
                                return;
                            }
                            let prefix = format!("{}/", r.name);
                            let all = remote_all.iter().any(|n| n == &r.name);
                            let mut shown = 0usize;
                            let total = remote_refs.iter().filter(|x| x.name.starts_with(&prefix)).count();
                            for rr in remote_refs.iter().filter(|x| x.name.starts_with(&prefix)) {
                                // 默认只露前 3 条：`origin` 上的分支一多，侧栏会被一整列拉到底，
                                // 把「标签 / 已贮藏」挤到看不见（用户 2026-10-26 要求）。
                                if !all && shown == 3 {
                                    if side_sub_row(ui, &tr!(format!("还有 {} 个分支", total - shown), format!("{} more branches", total - shown))).clicked() {
                                        intent.toggle_remote_all = Some(r.name.clone());
                                    }
                                    break;
                                }
                                shown += 1;
                                let tail = &rr.name[prefix.len()..];
                                let resp = side_sub_row(ui, tail);
                                let name = rr.name.clone();
                                let target = rr.target.clone();
                                resp.context_menu(|ui| {
                                    menu_item(ui, tr!("切换过去", "Check out"), || Action::CheckoutRemote(name.clone()), intent);
                                    menu_go(ui, tr!("到那条提交", "Go to that commit"), target.clone(), intent);
                                });
                                if resp.clicked() {
                                    intent.run(Action::CheckoutRemote(rr.name.clone()));
                                }
                            }
                        },
                        &mut intent,
                    );
                    side_category(
                        ui,
                        tr!("已贮藏", "Stashed"),
                        &filter,
                        stashes,
                        // `stash@{0}:` 是列表包装、`On <分支>:` 是 git 的包装，两截在核心层都剥掉了。
                        // 说明留空（`-m ""`）时退回短 oid，否则那一行是空的、看不出是哪条。
                        |s| {
                            if s.message.is_empty() {
                                format!("stash {}", short(&s.oid))
                            } else {
                                s.message.clone()
                            }
                        },
                        |s| matches!(sel, Sel::Stash(i) if i == s.index as usize),
                        // 贮藏不是分支，没有 §2 那颗「当前分支」蓝点。
                        |_| false,
                        |s, intent| intent.goto = Some(Sel::Stash(s.index as usize)),
                        |ui, s, intent| {
                            menu_item(ui, tr!("应用到工作区", "Apply to working tree"), || Action::StashApply(s.index), intent);
                            menu_item(ui, tr!("弹出并删掉这条", "Pop this stash"), || Action::StashPop(s.index), intent);
                            menu_item(ui, tr!("丢弃这条贮藏", "Drop this stash"), || Action::StashDrop(s.index), intent);
                        },
                        |_, _, _| {},
                        &mut intent,
                    );
                });
                ui.add_space(SIDE_FILTER_GAP);
                // 这格 26pt 的框要**单独把 `interact_size.y` 压回 26**：egui 0.37 的 `TextEdit`
                // 底框高度取 `max(行高+内边距, 主题的 atom_layout.min_size.y)`，而那个 min_size
                // 就是全局 `spacing.interact_size`（default_style.rs:161「Same as a button」），
                // 全局那份又是 `Vec2::new(..., ROW_H)`（data.rs 里 style 那一坨）——日志行距从 26
                // 换成 §2 的 28 之后，这格框会**偷偷跟着日志行高长**（实测画出来 28）。
                // `TextEdit::min_size` 救不了：builder 那份只能**抬高**主题的地板（builder.rs:544
                // 「the builder's own `min_size` can only raise it」）。所以只能开一个子作用域，
                // 在里面把地板改回 §2 的 26，不动全局——全局一动，顶栏那 28×28 的图标按钮跟着塌。
                let box_h = SIDE_FILTER_H - SIDE_FILTER_GAP;
                ui.scope(|ui| {
                    ui.spacing_mut().interact_size.y = box_h;
                    ui.add_sized(
                        Vec2::new(ui.available_width(), box_h),
                        TextEdit::singleline(&mut filter).hint_text(tr!("筛选器", "Filter")),
                    );
                });
            });
        self.side_filter = filter;
        self.land(intent);
    }

    // ------------------------------------------------------- 底部「文件 | 差异」
    pub(crate) fn bottom_band(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("staging")
            .resizable(true)
            .default_size(STAGING_H)
            .min_size(120.0)
            // 显式 frame：默认的 `Frame::side_top_panel` 自带 8×2 内边距，
            // 会把文件表整体右移 8pt——参考图里文件表是紧贴侧栏的。内边距交给两个子面板自己给。
            .frame(egui::Frame::NONE.fill(Color32::WHITE))
            .show(ui, |ui| {
                egui::Panel::left("files")
                    .resizable(true)
                    .default_size(FILES_W)
                    .min_size(200.0)
                    // 横向内边距 0：文件表那几列的缩进**全部按面板左缘量**（S6-3 之后侧栏宽 188，
                    // 这一格的左缘就从 141 挪到 188；方框 8、图标 47、文件名 68 这三档归 S6-5 重新对）。
                    // 再让 egui 偷偷加 4pt 就对不上了。竖向留 2pt，给上下那条分隔线。
                    .frame(egui::Frame::NONE.fill(Color32::WHITE).inner_margin(Margin::symmetric(0, 2)))
                    .show(ui, |ui| self.file_list(ui));
                egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::WHITE).inner_margin(Margin::symmetric(0, 2))).show(ui, |ui| self.diff_pane(ui));
            });
    }

    pub(crate) fn file_list(&mut self, ui: &mut egui::Ui) {
        let mut act: Option<Action> = None;
        let mut pick: Option<Target> = None;
        let mut hist: Option<String> = None;
        let sel = self.sel.clone();

        if sel == Sel::Uncommitted {
            let none_files: Vec<FileStatus> = Vec::new();
            let files: &[FileStatus] = match &self.snap {
                Some(s) => &s.status.files,
                None => &none_files,
            };
            let conflicts: Vec<&FileStatus> = files.iter().filter(|f| is_conflict(f)).collect();
            // 冲突文件从这两组里**挑出去**：`parse_status` 把 `u` 行标成 `staged: true`，
            // 不挑的话它们会混进「已暂存文件」，用户会以为已经解决好了。
            let staged: Vec<&FileStatus> = files.iter().filter(|f| !is_conflict(f) && f.staged).collect();
            let unstaged: Vec<&FileStatus> = files.iter().filter(|f| !is_conflict(f) && !f.staged).collect();
            let repo_state = self.snap.as_ref().map(|s| s.status.repo_state.clone()).unwrap_or_default();
            // 参考图第 5 条：列表上方一条「待定的文件, 已依照路径排序」。这一行和下面的提交框
            // 都不跟着列表滚——SourceTree 的关系是「列表滚，两头钉」。
            ui.horizontal(|ui| {
                // 参考图这一条的文字起在 x 169.5，和下面分类行的标签**同一列**（面板左缘 + 8 + 13 + 7 = 28）。
                ui.add_space(CHK_HEAD_X + CHK_S + 7.0);
                ui.label(RichText::new(pending_sorted_by_path()).size(11.0).color(TEXT_MUTED));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button(tr!("全部暂存", "Stage all")).clicked() {
                        act = Some(Action::StageAll);
                    }
                    if ui.small_button(tr!("全部取消暂存", "Unstage all")).clicked() {
                        act = Some(Action::UnstageAll);
                    }
                });
            });
            // 有合并/遴选/变基停在半路时，文件表上方多一条横幅。正常仓库不画，几何不动。
            op_banner(ui, &repo_state, conflicts.len(), &mut act);
            let current = self.file.clone();
            // 列表只在剩下的空间里滚，把 `COMMIT_H` 那一格留给下面的提交框。
            // 不这么做的话文件一多，提交框整格被顶出窗口（真窗口目视抓到过，判据见
            // `the_commit_box_stays_inside_a_short_band`）。
            let list_h = (ui.available_height() - COMMIT_H).max(40.0);
            let total_files = conflicts.len() + staged.len() + unstaged.len();
            if total_files > 5_000 {
                // 普通 ScrollArea（非 show_rows）会为**所有**文件行生成图元、只靠裁剪隐藏，
                // 文件数到这个量级就可能把顶点缓冲顶爆。崩溃前若看到这条，元凶就是文件列表。
                eprintln!(
                    "gitx-app[WARN] file_list 未虚拟化渲染 {} 个文件 (conflicts={} staged={} unstaged={})",
                    total_files, conflicts.len(), staged.len(), unstaged.len()
                );
            }
            ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
                conflict_section(ui, &conflicts, &current, self.repo.as_deref().unwrap_or(""), &mut act, &mut pick, &mut hist);
                section(ui, tr!("已暂存文件", "Staged files"), &staged, true, &current, &mut act, &mut pick, &mut hist);
                section(ui, tr!("未暂存文件", "Unstaged files"), &unstaged, false, &current, &mut act, &mut pick, &mut hist);
                if files.is_empty() {
                    ui.label(RichText::new(tr!("工作区是干净的。", "The working tree is clean.")).color(TEXT_SECONDARY).size(12.0));
                }
            });

            // ---- 提交框：SourceTree 那一格是**两个框**（摘要 + 描述），都在文件表下方、都靠左。
            // 落进 git 是一条 `-m`，两框之间隔一个空行——和 SourceTree 落出去的一样。
            let branch = self.snap.as_ref().map(|s| s.status.branch.clone()).unwrap_or_default();
            let staged_count = staged.len();
            let w = ui.available_width();
            ui.add(TextEdit::singleline(&mut self.commit_msg).hint_text(tr!("提交摘要… (⌘/Ctrl+Enter 提交)", "Commit summary… (⌘/Ctrl+Enter to commit)")).desired_width((w - 8.0).max(80.0)));
            ui.add(
                TextEdit::multiline(&mut self.commit_desc)
                    .hint_text(tr!("扩展描述…", "Extended description…"))
                    .desired_rows(3)
                    .desired_width((w - 8.0).max(80.0)),
            );
            let msg = commit_text(&self.commit_msg, &self.commit_desc);
            // amend 允许没有新暂存内容：只改写上一条的信息也是合法用法。
            let can = !msg.is_empty() && (staged_count > 0 || self.amend);
            // 两句话分开报，措辞抄 core 那句（`gitx-core/src/git.rs:1082`）。
            let why = if msg.is_empty() {
                tr!("提交信息不能为空", "The commit message is empty")
            } else {
                tr!("先勾选要提交的文件", "Tick the files to commit first")
            };
            // ⌘/Ctrl+Enter 提交（参考版 placeholder 里就写着这条）。读不了 `Ui::input`——0.37 的
            // `InputState` 只挂在 `Context` 上（egui-0.37.0/src/ui.rs 里没有 `fn input`）。
            // `i.modifiers` 是**按下修饰键时那条 `ModifiersChanged`** 更新出来的（input_state/mod.rs:434），
            // 键盘事件自带的那份不算——所以无头测试必须像 winit 那样先发 `ModifiersChanged`。
            let enter = ui.ctx().input(|i| {
                i.key_pressed(egui::Key::Enter) && (i.modifiers.command || i.modifiers.ctrl)
            });
            if can && enter {
                // 键盘与鼠标各只有一处判断：同一个开关读两遍会让一条判据永远杀不死另一半。
                act = Some(if self.amend { Action::Amend(msg.clone()) } else { Action::Commit(msg.clone()) });
            }
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.amend, RichText::new(tr!("修正上一条", "Amend last")).size(11.0))
                    .on_hover_text(tr!(
                        "改写 HEAD：换掉上一条提交的信息，已暂存的内容会一起并进去",
                        "Rewrites HEAD: the last commit's message is replaced and staged content folds in"
                    ));
                let label = if self.amend {
                    tr!("修正上一条提交", "Amend the last commit").to_owned()
                } else {
                    tr!(format!("提交到 {branch} ({staged_count})"), format!("Commit to {branch} ({staged_count})"))
                };
                let btn = egui::Button::new(RichText::new(label).size(11.5));
                if ui.add_enabled(can, btn).clicked() {
                    act = Some(if self.amend { Action::Amend(msg.clone()) } else { Action::Commit(msg.clone()) });
                }
                if !can {
                    ui.label(RichText::new(why).size(11.0).color(TEXT_SECONDARY));
                }
            });
        } else {
            let files = &self.commit_files;
            let stash = matches!(sel, Sel::Stash(_));
            let oid = match &sel {
                Sel::Commit(i) => self.oid(*i),
                Sel::Search(i) => self.search_hits.get(*i).map(|h| h.commit.id.clone()),
                Sel::Stash(i) => self
                    .snap
                    .as_ref()
                    .and_then(|s| s.stashes.get(*i))
                    .map(|st| st.oid.clone()),
                Sel::Uncommitted => None,
            };
            let subject = match &sel {
                Sel::Search(i) => self.search_hits.get(*i).map(|h| h.commit.subject.clone()),
                _ => self
                    .snap
                    .as_ref()
                    .and_then(|s| match &sel {
                        Sel::Commit(i) => s.commits.get(*i).map(|c| c.subject.clone()),
                        Sel::Stash(i) => s.stashes.get(*i).map(|st| st.message.clone()),
                        Sel::Uncommitted => None,
                        // 上面那一档已经接走它了。
                        Sel::Search(_) => None,
                    }),
            }
            .unwrap_or_default();
            let head = if stash {
                tr!(format!("这条贮藏改了 {} 个文件", files.len()), format!("This stash touches {} file(s)", files.len()))
            } else {
                tr!(format!("这个提交改了 {} 个文件", files.len()), format!("This commit touches {} file(s)", files.len()))
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(head).size(11.0).color(TEXT_SECONDARY));
                if let Some(o) = &oid {
                    if ui.small_button(tr!("全部变更", "All changes")).clicked() {
                        pick = Some(if stash {
                            Target::Stash { oid: o.clone(), path: None }
                        } else {
                            Target::Commit { oid: o.clone(), path: None }
                        });
                    }
                }
            });
            ui.label(RichText::new(subject).size(11.5).color(TEXT_SECONDARY));
            if files.len() > 5_000 {
                eprintln!(
                    "gitx-app[WARN] file_list(commit) 未虚拟化渲染 {} 个文件",
                    files.len()
                );
            }
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let current = self.file.clone();
                for d in files {
                    let o = oid.clone();
                    let p = d.path.clone();
                    let tail = format!("+{} −{}", d.additions, d.deletions);
                    // 整行可点、文字不可拖选：`selectable(on)` 那套误会上一批已经修掉了，
                    // 见 `a_click_on_an_unpicked_file_row_switches_to_it`。
                    // 某条提交/贮藏的改动列表**没有方框**：那早就落库了，勾不勾都不动索引
                    // （参考图也只有工作区那两栏带方框）。
                    let (resp, _) = file_row(ui, &d.path, &d.status, current.as_deref() == Some(d.path.as_str()), &tail, None);
                    if resp.clicked() {
                        if let Some(o) = o {
                            pick = Some(if stash {
                                Target::Stash { oid: o, path: Some(p.clone()) }
                            } else {
                                Target::Commit { oid: o, path: Some(p.clone()) }
                            });
                        }
                    }
                    // 提交/贮藏里那些文件同样可以溯源：按**当前路径**问历史，`--follow` 会跟过改名。
                    resp.context_menu(|ui| commit_file_menu(ui, &p, &mut hist));
                }
            });
        }

        if let Some(action) = act {
            // 文件表这一层的动作也要过 `land`：右键菜单里的「放弃」和冲突菜单里那几条
            // 都会丢工作区改动，确认卡必须拦在出口上（`needs_confirm`），不能在入口逐个补。
            self.land(Intent { act: Some(action), ..Default::default() });
        }
        if let Some(target) = pick {
            self.file = target_path(&target).map(str::to_owned);
            self.request(target);
        }
        if let Some(path) = hist {
            self.open_history(&path);
        }
    }

    pub(crate) fn diff_pane(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy;
        let label = self.patch_label.clone();
        // 色块问的是**当前这一格差异对应的那个文件**：工作区那批从 `status.files` 查，
        // 某一条提交那批从 `commit_files` 查。查不到（比如「全部未暂存变更」整份补丁）就不画块。
        let badge_st = match &self.file {
            Some(p) => self
                .snap
                .as_ref()
                .and_then(|s| s.status.files.iter().find(|f| &f.path == p).map(|f| status_of(f).clone()))
                .or_else(|| {
                    self.commit_files.iter().find(|d| &d.path == p).map(|d| d.status.clone())
                }),
            None => None,
        };
        // 差异区顶上一格 = §3.1 `BG_INSET` 的灰带：状态色块 + **加粗、靠左**的文件名。
        // 左对齐是用户 2026-10-20 特别点过的一条（「描述、文件 diff 展示都是左对齐的」）。
        let head = rect_of(ui, DIFF_HEAD_H);
        let p = ui.painter();
        p.rect_filled(head, 0.0, DIFF_HEAD_BG);
        let mut x = head.min.x + DIFF_HEAD_PAD;
        if let Some(st) = &badge_st {
            let (fill, glyph) = badge_of(st);
            let b = Rect::from_min_size(pos2(x, head.center().y - BADGE_S / 2.0), Vec2::splat(BADGE_S));
            p.rect_filled(b, BADGE_R, fill);
            p.text(b.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(11.0), Color32::WHITE);
            x = b.max.x + 6.0;
        }
        if busy {
            p.text(head.right_center(), Align2::RIGHT_CENTER, tr!("读取中…", "Reading…"), FontId::proportional(11.0), TEXT_MUTED);
        }
        // 搜索框固定钉在灰带右端：参考图有这条，但没量到它的宽高（见 docs/PROCESS.md），
        // 宽度只按「搜索差异」四个字 + 输入余量估。
        let search = Rect::from_min_max(
            pos2(head.max.x - DIFF_SEARCH_W - 6.0, head.center().y - DIFF_SEARCH_H / 2.0),
            pos2(head.max.x - 6.0, head.center().y + DIFF_SEARCH_H / 2.0),
        );
        if !busy {
            let requery = put_left(ui, search, |ui| {
                ui.add_sized(
                    [DIFF_SEARCH_W, DIFF_SEARCH_H],
                    egui::TextEdit::singleline(&mut self.diff_search)
                        .id_salt("diff-search")
                        .hint_text(tr!("搜索差异", "Search the diff"))
                        .font(TextStyle::Small),
                )
                .changed()
            });
            if requery {
                // 只在敲字那一帧重扫整份补丁；空串走「只按折叠」的便宜分支。
                self.rebuild_diff_rows();
            }
        }
        let lines = &self.diff;
        let rows = &self.diff_rows;
        let collapsed = &self.collapsed;
        let r = Rect::from_min_max(
            pos2(x, head.min.y),
            pos2(head.max.x - if busy { 62.0 } else { DIFF_SEARCH_W + 14.0 }, head.max.y),
        );
        put_left(ui, r, |ui| {
            ui.add(Label::new(RichText::new(label.as_str()).size(12.0).color(TEXT_SECONDARY)).truncate())
        });
        // 搜索把每一行都筛掉了：整块空白会让人以为差异没取回来，得说清是「筛没了」。
        if rows.is_empty() {
            let msg = if lines.is_empty() { tr!("没有差异", "Nothing to diff") } else { tr!("没有匹配的行", "No matching lines") };
            ui.centered_and_justified(|ui| ui.label(RichText::new(msg).color(TEXT_MUTED)));
            return;
        }
        let width = ui.available_width();
        let font = ui.style().text_styles[&TextStyle::Monospace].clone();
        let th = ui.text_style_height(&TextStyle::Monospace);
        let fg = ui.visuals().text_color();
        // 每一行的**条子**（底色 + 行号那两格）要铺满多宽 = 「窗口这一格」与「最长那行」
        // 的较大者。窄的时候和以前一模一样（没有横向滚动条）；长行把条子撑到行尾之后，
        // 滚到底也能看见整行的底色，不会滚出半截白。字格数在补丁落地时算好（`Msg::Patch`），
        // 帧里不重扫 28 875 行。
        let char_w = ui.fonts_mut(|f| f.glyph_width(&font, 'M')).max(1.0);
        let gutter = NO_W * 2.0 + 6.0;
        let content_w = width.max(gutter + self.diff_cols as f32 * char_w + 16.0);
        // 「只动这一块」那两枚按钮的落点：只有**工作区**的单文件差异有区块可动
        // （某条提交早就落库了；未跟踪文件在 `git diff` 里根本没有补丁，打不出第二块）。
        let ops = self.hunk_ops();
        let pending = self.pending_discard;
        let mut hit: Option<Action> = None;
        // 「这一帧有没有人碰过放弃那枚」：`None` = 没碰，状态原样留着；
        // `Some(Some(n))` = 第一下， armed 在第 n 块；`Some(None)` = 第二下或点了别的块，拆掉。
        let mut arm: Option<Option<usize>> = None;
        // 点了哪个文件的折叠开关。文件行画在 `show_rows` 里，改可见行得等这一帧画完。
        let mut fold: Option<usize> = None;
        // 只画视口里的行：comic 那份 28 875 行的差异，帧里出现的行数与窗口高度有关，与文件多大无关。
        // 右键菜单的**容器响应必须注册在滚动区之前**：后注册的那一格会抢走整块差异的 hover，
        // 而 egui 的标签选区（`LabelSelectionState`）在右键按下时用「指针下面有没有标签悬停着」
        // 来判断这是不是「点到别处」——抢走 hover 就等于把选区清掉，复制也就没了
        // （用户 2026-10-26 报的「选中内容右击就没了」）。容器判据只看 rect，不要求悬停。
        let area = ui.interact(ui.available_rect_before_wrap(), ui.scope_id().with("diff-copy-menu"), Sense::hover());
        ScrollArea::both().auto_shrink([false, false]).show_rows(ui, DIFF_ROW_H, rows.len(), |ui, range| {
            if rows.len() > 20_000 {
                // 顶点缓冲上限 ~256MiB ≈ 8.3M 顶点（32B/顶点）。`show_rows` 本应只画视口内
                // 几十行；若下面这行的 `range` 长度接近 `rows.len()`，说明虚拟化没生效、整份
                // 补丁的图元全进了缓冲——那就是崩在 diff_pane。
                eprintln!(
                    "gitx-app[diag] diff_pane 视口绘制 {} 行 (range {}..{}) / 总计 {} 行",
                    range.len(), range.start, range.end, rows.len()
                );
            }
            for r in range {
                bump_rows_painted();
                let line = &lines[rows[r]];
                if line.kind == Kind::File {
                    // 文件头带：整份补丁里唯一的分界。整行可点，点一下折叠/展开这个文件。
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(content_w, DIFF_ROW_H), Sense::click());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 0.0, DIFF_HEAD_BG);
                    let folded = collapsed.contains(&line.file);
                    let label = format!("{}  {}", if folded { tr!("已折叠", "Folded") } else { tr!("文件", "File") }, line.text);
                    let avail = (ui.clip_rect().right() - rect.min.x - 64.0).max(24.0);
                    p.text(
                        band(rect, rect.min.x + 6.0, avail, th).left_center(),
                        Align2::LEFT_CENTER,
                        label,
                        font.clone(),
                        TEXT_SECONDARY,
                    );
                    p.text(
                        rect.right_center(),
                        Align2::RIGHT_CENTER,
                        if folded { tr!("点击展开", "Click to expand") } else { tr!("点击折叠", "Click to fold") },
                        FontId::proportional(10.5),
                        TEXT_MUTED,
                    );
                    if resp.clicked() {
                        fold = Some(line.file);
                    }
                    fix_pitch(ui, rect);
                    continue;
                }
                let (rect, _) = ui.allocate_exact_size(Vec2::new(content_w, DIFF_ROW_H), Sense::hover());
                let p = ui.painter_at(rect);
                let bg = match line.kind {
                    Kind::Add => ADD_BG,
                    Kind::Del => DEL_BG,
                    Kind::Hunk => BG_INSET,
                    // 文件头在上一段就 `continue` 了，走不到这儿。
                    Kind::File | Kind::Ctx => Color32::TRANSPARENT,
                };
                if bg != Color32::TRANSPARENT {
                    p.rect_filled(rect, 0.0, bg);
                }
                for (i, n) in [(0, line.old), (1, line.new)] {
                    let text = n.map(|v| v.to_string()).unwrap_or_default();
                    let r = band(rect, rect.min.x + NO_W * i as f32, NO_W, th);
                    p.text(r.right_center(), Align2::RIGHT_CENTER, text, font.clone(), TEXT_MUTED);
                }
                let x = rect.min.x + NO_W * 2.0 + 6.0;
                let rich = match line.kind {
                    // hunk 头在 `layout`（工作线程）里就已经翻成人话了，帧里只管画。
                    // 这儿原来还带一份自己的翻译，两条规则各写一遍——见 `a_hunk_row_reads_as_a_block_number_and_line_range`。
                    Kind::Hunk => RichText::new(&line.text).font(font.clone()).color(TEXT_MUTED),
                    // §3.1 的增/删两行是**连字一起染**（预览稿 `.dl.a`/`.dl.d` 同一条规则），
                    // 不是只铺一条底色带、正文仍然黑。
                    Kind::Add => RichText::new(&line.text).font(font.clone()).color(ADD_FG),
                    Kind::Del => RichText::new(&line.text).font(font.clone()).color(DEL_FG),
                    _ => RichText::new(&line.text).font(font.clone()).color(fg),
                };
                // 文字那一格只按**看得见**的宽度申请（和 hunk 按钮同一个道理：跟着
                // 内容右缘走会跑到屏幕外）。`.extend()` 不是 `.truncate()`：申请窄了
                // 也不许把尾巴收成三个点——尾巴要靠横向滚出来，省略号等于谎报「没有了」。
                let visible_right = ui.clip_rect().right();
                put_left(ui, band(rect, x, (visible_right - x).max(40.0), th), |ui| {
                    ui.add(Label::new(rich).extend());
                });
                // 参考图把「暂存区块 / 放弃区块」钉在 hunk 头那一行的**右端**
                // （x 977..1032、1041..1096，带右缘 1105），不是弹在鼠标底下——一直在，
                // 所以看得见也点得中。
                if let (Some(n), Some(&(staged, ref path))) = (line.hunk, ops.as_ref()) {
                    // 只有**头那一行**带按钮。`line.hunk` 在块里每一行都有值（将来按行暂存要用），
                    // 所以这一句 `Kind::Hunk` 不能省——第一版没写，结果一块六行就长了六组按钮，
                    // 判据 `the_buttons_follow_which_pane_the_diff_is` 数出来 12 枚。
                    if line.kind == Kind::Hunk {
                        let cy = rect.center().y;
                        // 按钮钉在**看得见的那条右缘**，不是内容右缘：横向滚动之后
                        // 内容右缘在屏幕外，按钮跟着跑就没法点了。
                        let mut right = ui.clip_rect().right() - HUNK_BTN_EDGE;
                        let (mut fired, mut disarm) = (None, false);
                        if !staged {
                            let armed = pending == Some(n);
                            let (hit_drop, w) = hunk_button(
                                ui,
                                if armed { tr!("确认放弃", "Confirm discard") } else { tr!("放弃区块", "Discard hunk") },
                                right,
                                cy,
                            );
                            if hit_drop {
                                // 第一下只把按钮翻成「确认放弃」，仓库一个字节不动；
                                // 第二下才真丢。见 `a_discard_needs_two_clicks`。
                                if armed {
                                    fired = Some(Action::DiscardHunk { path: path.clone(), hunk: n });
                                    disarm = true;
                                } else {
                                    arm = Some(Some(n));
                                }
                            }
                            right -= w + HUNK_BTN_GAP;
                        }
                        let (hit_stage, _) = hunk_button(
                            ui,
                            if staged { tr!("取消暂存区块", "Unstage hunk") } else { tr!("暂存区块", "Stage hunk") },
                            right,
                            cy,
                        );
                        if hit_stage {
                            fired = Some(if staged {
                                Action::UnstageHunk { path: path.clone(), hunk: n }
                            } else {
                                Action::StageHunk { path: path.clone(), hunk: n }
                            });
                            disarm = true;
                        }
                        if let Some(a) = fired {
                            hit = Some(a);
                        }
                        if disarm {
                            arm = Some(None);
                        }
                    }
                }
                fix_pitch(ui, rect);
            }
        });
        // 差异区的右键菜单：**复制**。
        //
        // 差异正文是 painter 画出来的，egui 的选中/复制那套只认 `Label` 与 `TextEdit`，
        // 所以这里不接「选中一段再复制」，而是给整块差异一个容器级菜单——右键落在
        // 任何一行（正文、文件头、hunk 头）都算。容器菜单不要求子控件让出点击
        // （`container_context_menu`），滚动区里的行照样能点。
        let file = self.file.clone().or_else(|| match &self.target {
            Some(Target::Working { path, .. })
            | Some(Target::Commit { path, .. })
            | Some(Target::Stash { path, .. }) => path.clone(),
            _ => None,
        });
        let oid = match &self.target {
            Some(Target::Commit { oid, .. }) | Some(Target::Stash { oid, .. }) => Some(oid.clone()),
            _ => None,
        };
        let mut copy: Option<String> = None;
        area.container_context_menu(|ui| {
            ui.label(RichText::new(tr!("复制", "Copy")).size(11.0).color(TEXT_MUTED));
            if ui
                .add_enabled(file.is_some(), egui::Button::new(tr!("文件路径", "File path")).small())
                .clicked()
            {
                copy = file.clone();
                ui.close();
            }
            if ui.add(egui::Button::new(tr!("整份差异", "Whole diff")).small()).clicked() {
                copy = Some(self.diff.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"));
                ui.close();
            }
            if oid.is_some() && ui.add(egui::Button::new(tr!("提交号", "Commit SHA")).small()).clicked() {
                copy = oid.clone();
                ui.close();
            }
        });
        if let Some(text) = copy {
            ui.ctx().copy_text(text);
        }
        if let Some(next) = arm {
            self.pending_discard = next;
        }
        if let Some(f) = fold {
            // 再点一次就是展开：`remove` 返回 false 说明刚才没折，那就折上。
            if !self.collapsed.remove(&f) {
                self.collapsed.insert(f);
            }
            self.rebuild_diff_rows();
        }
        if let Some(a) = hit {
            self.act(a);
        }
    }

    /// 按折叠与搜索重建可见行。只在状态变的那一下跑（点折叠 / 敲字 / 补丁落地），
    /// 不在帧里扫整份补丁：2.8 万行的补丁每帧过一遍，滚动立刻掉帧。
    ///
    /// 搜索的可见规则：命中的内容行 + 它所属的文件头 + 引入它的那一块 hunk 头。
    /// 文件头不是「碰巧留下」——没有它，几段命中混在一起就不知道各自在哪个文件里；
    /// hunk 头留下是因为行号差在它身上（`@@ -12,7 +12,9 @@`），删掉就只剩一串裸行。
    pub(crate) fn rebuild_diff_rows(&mut self) {
        use std::collections::HashSet;
        let q = self.diff_search.trim().to_lowercase();
        if q.is_empty() {
            self.diff_rows = self
                .diff
                .iter()
                .enumerate()
                .filter(|(_, l)| l.kind == Kind::File || !self.collapsed.contains(&l.file))
                .map(|(i, _)| i)
                .collect();
            return;
        }
        let mut files: HashSet<usize> = HashSet::new();
        let mut hunks: HashSet<(usize, usize)> = HashSet::new();
        for l in &self.diff {
            if matches!(l.kind, Kind::Add | Kind::Del | Kind::Ctx) && l.text.to_lowercase().contains(&q) {
                files.insert(l.file);
                if let Some(h) = l.hunk {
                    hunks.insert((l.file, h));
                }
            }
        }
        self.diff_rows = self
            .diff
            .iter()
            .enumerate()
            .filter(|(_, l)| match l.kind {
                Kind::File => files.contains(&l.file),
                Kind::Hunk => hunks.contains(&(l.file, l.hunk.unwrap_or(0))),
                _ => l.text.to_lowercase().contains(&q),
            })
            .filter(|(_, l)| l.kind == Kind::File || !self.collapsed.contains(&l.file))
            .map(|(i, _)| i)
            .collect();
    }

    /// 这一格差异能不能「只动一块」。返回 `(已暂存?, 路径)`——按钮的文案与动作方向全看那个
    /// `staged`。两种情况没有区块可动：某条提交的差异早就落库了；未跟踪文件在 `git diff`
    /// 里根本没有补丁（`working_patch` 回一句中文说明，抠不出 hunk）。
    pub(crate) fn hunk_ops(&self) -> Option<(bool, String)> {
        match &self.target {
            Some(Target::Working { staged, path: Some(p), untracked: false }) => Some((*staged, p.clone())),
            _ => None,
        }
    }

    // ------------------------------------------------------------------- 日志
    pub(crate) fn log_band(&mut self, ui: &mut egui::Ui) {
        let mut intent = Intent::default();
        let mut query = self.log_filter.clone();
        let sel = self.sel.clone();
        let Some(snap) = self.snap.as_ref() else {
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::WHITE)).show(ui, |ui| {
                ui.centered_and_justified(|ui| ui.label(RichText::new(tr!("左侧选一个仓库", "Pick a repository on the left")).color(TEXT_MUTED)));
            });
            return;
        };
        // 环下面那条竖线接的是**第一条提交**的车道，所以取 graph 第一行。
        let stem_lane = snap.graph.first().map(|g| g.lane).unwrap_or(0);
        // 搜索那一档的几位，全在面板闭包**外面**读好：闭包里 `self` 正被 `snap` 借走，
        // 写不了（理由同 `Intent`）。`hits` 和 `snap` 是两个不可变借用，可以并存。
        let searching = self.search_open;
        let hits = &self.search_hits;
        let search_busy = self.search_busy;
        let focus_search = self.search_focus;
        let mut sq = self.search_q.clone();
        let mut run_search = false;
        let mut close_search = false;
        // 日志条右端那枚「搜索」：侧栏那一行去掉之后它是**唯一**的入口。
        let mut open_search = false;
        let width = ui.available_width();
        let mut want_topo = self.topo;
        let mut want_scope = self.scope.clone();
        let mut want_remote = self.show_remote;
        // 闭包里碰不了 `self`，所以先把「远程药丸看不看」拷成局部。名字避开 `show_remote()`
        // 那枚文案函数：同名的局部会把函数项盖掉，`show_remote()` 就变成「调用一个 bool」。
        let remote_shown = self.show_remote;
        let scope_label = scope_label_for(&self.scope, snap.status.detached_at.is_some());
        // 前两档是 SourceTree 官方的两态（LogBranchesAll ⇄ LogBranchesCurrent）；
        // 后面列本地分支是我们自己加的——它那一档只切两态，想只看某条支线得自己挑。
        let scope_items: Vec<String> = std::iter::once(all_branches().to_owned())
            .chain(std::iter::once(current_branch().to_owned()))
            .chain(snap.branches.iter().map(|b| b.name.clone()))
            .collect();
        let mut need_more = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::WHITE).inner_margin(Margin::symmetric(0, 2)))
            .show(ui, |ui| {
                // 内容筛选条（参考图 y≈110..150）：三枚**弹出按钮**（当前那项 + ▼）
                // + 一枚还没接的「紧凑视图」+ 右侧「转到Commit的SHA-1」。
                ui.horizontal(|ui| {
                    if let Some(p) = popup_button(ui, &scope_label, &scope_items) {
                        want_scope = if p == all_branches() {
                            git::Scope::All
                        } else if p == current_branch() {
                            git::Scope::Current
                        } else {
                            git::Scope::Rev(p)
                        };
                    }
                    if let Some(p) = popup_button(
                        ui,
                        if want_remote { show_remote() } else { hide_remote() },
                        &[show_remote().to_owned(), hide_remote().to_owned()],
                    ) {
                        want_remote = p == show_remote();
                    }
                    if let Some(p) = popup_button(
                        ui,
                        if want_topo { topo_order() } else { date_order() },
                        &[date_order().to_owned(), topo_order().to_owned()],
                    ) {
                        want_topo = p == topo_order();
                    }
                    // 没有箭头 = 点开没有第二层。这一枚的真实行为还没对着参考图量过（见 PROCESS
                    // 「仍然没量过的」），与其给个假菜单，不如把「还没接」这句话留着。
                    if ui.add(egui::Button::new(tr!("紧凑视图", "Compact view")).small()).clicked() {
                        intent.note = Some(tr!("egui 版还没接：紧凑视图", "Not wired in the egui shell: compact view").to_owned());
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if searching {
                            // ✕ 先加：右到左的排布里它因此落在**最右**，正好是这一档的出口。
                            // 字形复用卡片那枚 `CLOSE_GLYPH`（`×` U+00D7 的字体表已在 S6-6
                            // 那条判据里验过有字形，别换一个没验过的叉）。
                            if ui
                                .add(egui::Button::new(RichText::new(CLOSE_GLYPH).size(14.0)).frame(false))
                                .clicked()
                            {
                                close_search = true;
                            }
                            let w = (ui.available_width() - 8.0).clamp(90.0, 260.0);
                            let resp = ui.add_sized(
                                Vec2::new(w, 20.0),
                                TextEdit::singleline(&mut sq)
                                    .hint_text(search_hint())
                                    .id_salt("log-search"),
                            );
                            // 光标落在这一格：点「搜索」的人下一步就是打字，不该再点一下框。
                            // `focus_search` 是上一帧 `open_search()` 立的牌子，这一帧消费掉。
                            if focus_search {
                                resp.request_focus();
                            }
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                run_search = true;
                            }
                            // 状态那一格（在途 / 几条 / 没搜到）跟着框一起换位置，
                            // 免得搜索开着的时候日志条左半边多出一个会跳的数。
                            let status = if search_busy {
                                tr!("搜索中…", "Searching…").to_owned()
                            } else if sq.trim().is_empty() {
                                tr!("回车开搜", "Press Enter to search").to_owned()
                            } else if hits.is_empty() {
                                tr!("没搜到", "Nothing found").to_owned()
                            } else {
                                tr!(format!("{} 条命中", hits.len()), format!("{} hits", hits.len()))
                            };
                            ui.label(RichText::new(status).size(11.0).color(TEXT_SECONDARY));
                            return;
                        }
                        let w = (ui.available_width() - 8.0).clamp(90.0, 260.0);
                        ui.add_sized(Vec2::new(w, 20.0), TextEdit::singleline(&mut query).hint_text(jump_to_sha()));
                        // 搜索入口：侧栏那一行 2026-10-26 去掉了（用户嫌重复），
                        // 现在只在日志条右端这枚小按钮上——点一下就把跳转框换成搜索框。
                        if ui.add(egui::Button::new(tr!("搜索", "Search")).small()).clicked() {
                            close_search = false;
                            run_search = false;
                            open_search = true;
                        }
                    });
                });
                header_row(ui, width);
                // 搜索开着、这一次什么都没捞到：列表那一块是空的，`show_rows` 一个字都不画。
                // 空态得写明，不然看着像「搜索坏了」而不像「没有命中」——尤其框里那个词
                // 是人自己打的，他会怀疑自己敲错了地方。
                if searching && hits.is_empty() && !search_busy {
                    ui.label(RichText::new(tr!(
                        format!("没有提交命中「{}」：说明和改过的文件名都不含它", sq.trim()),
                        format!("No commit matches \"{}\" — neither the message nor the file names it touched", sq.trim())
                    )).size(11.0).color(TEXT_MUTED));
                }
                let q = query.trim().to_lowercase();
                let keep: Vec<usize> = snap
                    .commits
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        q.is_empty()
                            || c.subject.to_lowercase().contains(&q)
                            || c.author.to_lowercase().contains(&q)
                            || c.short_id.starts_with(&q)
                    })
                    .map(|(i, _)| i)
                    .collect();
                // 虚拟行永远在第一位，不参与过滤（参考图第 2 条）。
                // 搜索开着时**没有虚拟行**：那一行是「工作区里还没提交的东西」，
                // 它不属于任何一次搜索命中——按文件名搜「a.txt」把它列出来是骗人的，
                // 那些改动还没落到任何提交里。
                let total = if searching { hits.len() } else { keep.len() + 1 };
                // 筛过的日志**不画图序列**：行与行断开了，竖线会连到一条没被显示的提交上，
                // 看着像 bug。SourceTree 筛选时那一列同样空着。
                let unfiltered = keep.len() == snap.commits.len();
                ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, ROW_H, total, |ui, range| {
                    // 视口的最后一行追平列表尾 = 滚到底了。这里只能记在局部变量上：
                    // `snap` 还借用着 `self`，闭包里碰不了 `self.log_*`。
                    // 搜索开着时这一位不算：命中列表不是分页日志，`ask_for_more` 会把
                    // 日志的下一页接进快照，画面上却没人看它——白跑一次 git。
                    if !searching && range.end >= total {
                        need_more = true;
                    }
                    for r in range {
                        bump_rows_painted();
                        if searching {
                            // 命中行和日志行**同一种行**（同一枚 `log_row`、同一套列宽），
                            // 只是来源不同、不画图序列。这样搜索结果点下去和点日志行
                            // 是同一件事：选中它、右边出它的文件表与补丁。
                            let h = &hits[r];
                            let cells = Cells {
                                subject: h.commit.subject.clone(),
                                short: h.commit.short_id.clone(),
                                author: format!("{} <{}>", h.commit.author, h.commit.author_email),
                                date: h.date.clone(),
                                badges: visible_badges(&h.badges, remote_shown),
                                ring: None,
                                stem: false,
                            };
                            let hit = Sel::Search(r);
                            // `push_id` 吃的是下标，和日志那一档 `Sel::Commit(i) => *i` 共用一个
                            // 数字空间——两份列表不会在同一帧里出现，所以撞不上；真哪天并排画，
                            // 这里要换成按提交编号发。
                            let resp = ui
                                .push_id(r, |ui| log_row(ui, width, &cells, sel == hit, None))
                                .inner;
                            if resp.clicked() && hit != sel {
                                intent.goto = Some(hit);
                            }
                            let (oid, subject) = (h.commit.id.clone(), h.commit.subject.clone());
                            resp.context_menu(|ui| commit_menu(ui, &oid, &subject, &mut intent));
                            continue;
                        }
                        let hit = if r == 0 { Sel::Uncommitted } else { Sel::Commit(keep[r - 1]) };
                        let cells = if r == 0 {
                            // 参考图这一行只有「Uncommitted changes」五个词，日期是**拍照时刻**
                            // （「今天 14:46」）而不是分支名。几个文件待提交写在下面文件表那一格里。
                            Cells {
                                subject: "Uncommitted changes".to_owned(),
                                // 虚拟行不是提交，没有号：这一格空着，整列仍然占位对齐。
                                short: String::new(),
                                author: "*".to_owned(),
                                date: date_label(&snap.taken_at.to_rfc3339(), snap.taken_at),
                                badges: Vec::new(),
                                ring: Some(stem_lane),
                                // 下面有提交可连才画那根线。
                                stem: unfiltered && !keep.is_empty(),
                            }
                        } else {
                            let i = keep[r - 1];
                            let c = &snap.commits[i];
                            Cells {
                                subject: c.subject.clone(),
                                short: c.short_id.clone(),
                                // 作者列：名字带尖括号邮箱（旧参考图这一格是
                                // `baorong <superlbr@126.com>`；§2 只给了列宽 96，超出的部分
                                // 由 `band` 截断）。
                                author: format!("{} <{}>", c.author, c.author_email),
                                date: snap.dates.get(i).cloned().unwrap_or_default(),
                                badges: snap.badges.get(i).map(|b| visible_badges(b, remote_shown)).unwrap_or_default(),
                                ring: None,
                                stem: false,
                            }
                        };
                        let graph = if r > 0 && unfiltered {
                            snap.graph.get(keep[r - 1])
                        } else {
                            None
                        };
                        // 行的 Id 由**这一行代表哪条提交**决定，不吃 `show_rows` 发的槽位 Id。
                        // 现象：滚轮之后**下一帧**就点，`clicked()` 一直是空的——egui 在松开那一帧
                        // 发现按下的 Id 不在当前帧的注册表里，就把 `potential_click_id` 清空
                        // （`interaction.rs:117-123`）。`skip_ahead_auto_ids(min_row)`
                        // （`scroll_area.rs:1113`）本该让槽位 Id 跨帧稳定，实测在滚动刚开始那一帧
                        // 仍然对不上；换成按提交编号发就稳了。
                        // 判据：`a_click_on_the_frame_right_after_the_wheel_still_lands`。
                        let key = match &hit {
                            Sel::Uncommitted => usize::MAX,
                            // 贮藏没有日志行；这一档只为穷尽 `Sel`，真被走到也是
                            // 「选中项不是日志行」，不发 Id 就够。
                            Sel::Stash(_) => usize::MAX,
                            Sel::Commit(i) => *i,
                            // 搜索结果那一档在**上面**单独发了 Id（`push_id(r)`），走不到这儿；
                            // 真被走到就是「选中项不是日志行」，不发 Id 就够。
                            Sel::Search(_) => usize::MAX,
                        };
                        let resp = ui
                            .push_id(key, |ui| log_row(ui, width, &cells, sel == hit, graph))
                            .inner;
                        if resp.clicked() && hit != sel {
                            intent.goto = Some(hit);
                        }
                        // 只有真提交那几行有菜单。虚拟行（工作区）的操作挂在下面文件表那一侧，
                        // 挂在这一行上会和「文件状态」那格重复。
                        if r > 0 {
                            let c = &snap.commits[keep[r - 1]];
                            let (oid, subject) = (c.id.clone(), c.subject.clone());
                            resp.context_menu(|ui| commit_menu(ui, &oid, &subject, &mut intent));
                        }
                    }
                });
            });
        self.log_filter = query;
        if need_more {
            self.ask_for_more();
        }
        // 搜索那几位，顺序是有讲究的：先把框里的字写回去（`run_search` 读的就是它），
        // 再消费「这一帧要请求焦点」那块牌子，最后才开搜/关档。
        self.search_q = sq;
        self.search_focus = false;
        if open_search {
            self.open_search();
        }
        if run_search {
            self.run_search();
        }
        if close_search {
            self.close_search();
        }
        // 远程药丸只是**画不画**，列表一个字都没改，所以这一枚不重取快照。
        self.show_remote = want_remote;
        if want_topo != self.topo {
            self.set_topo(want_topo);
        }
        if want_scope != self.scope {
            // 搜索开着时换范围 = 命中那一批的**前提**变了（它们是按旧范围取的两条 `git log`）。
            // 留着旧命中会画出「范围写着所有分支、列表却是当前分支」这种自相矛盾的帧，
            // 所以清掉并让在途那次作废；框里的词留着，换完范围回车就是新的。
            if self.search_open {
                self.search_hits.clear();
                self.search_busy = false;
                self.search_seq += 1;
            }
            self.set_scope(want_scope);
        }
        self.land(intent);
    }

    /// 底部状态栏：左边当前分支与领先落后，右边仓库路径。
    /// 内容（分支名、有上游才出现的 ↑↓ 计数——契约：没有上游绝不显示 ↑0 ↓0、正在进行的操作）
    /// 是挑的没有争议的三样；几何与配色走 §2 STATUS + §3.4「辅助」档，11px 灰字那一档跟 S6-5 进来。
    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        // 「回到 main」那一格点下去要发一条检出。和其余面板同一个套路：闭包里不许碰 `self`
        // （正被 `snap` 借着），所以只把要办的事带出去，等 `show` 返回再落地。
        let mut go_back: Option<String> = None;
        egui::Panel::bottom("status")
            .exact_size(STATUS_H)
            .frame(egui::Frame::NONE.fill(BG_WINDOW))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                // 顶上那条 1px 分隔线（§2「STATUS 24pt：白底 + 上 1px 分隔线」）：
                // 状态栏和内容区同白底，边界只能靠这条线。
                ui.painter().line_segment(
                    [pos2(rect.min.x, rect.min.y + 0.5), pos2(rect.max.x, rect.min.y + 0.5)],
                    Stroke::new(1.0, BORDER_SUBTLE),
                );
                let mut left: Vec<String> = Vec::new();
                let mut back: Option<String> = None;
                // 推送状态优先占住左下角：推送中/完成/失败时不显示分支名（那格平时就一个
                // `main`，用户觉得没意义）。`Done`/`Failed` 带时间戳，超过 6 秒清回 `None`。
                let push_now = ui.ctx().input(|i| i.time);
                let push_text: Option<String> = match &mut self.push_state {
                    None => None,
                    Some(PushPhase::Pushing) => Some(tr!("正在推送…", "Pushing…").to_owned()),
                    Some(PushPhase::Done(t)) => {
                        if push_now - *t > 6.0 {
                            self.push_state = None;
                            None
                        } else {
                            Some(tr!("推送完成", "Push complete").to_owned())
                        }
                    }
                    Some(PushPhase::Failed(t)) => {
                        if push_now - *t > 6.0 {
                            self.push_state = None;
                            None
                        } else {
                            Some(tr!("推送失败", "Push failed").to_owned())
                        }
                    }
                };
                if let Some(text) = push_text {
                    left.push(text);
                } else if let Some(snap) = &self.snap {
                    let st = &snap.status;
                    match &st.detached_at {
                        // 分离 HEAD。`branch.head` 那一行 git 写的是字面量 `(detached)`，
                        // 把它当分支名印出来，用户读到的是「有一条分支叫 (detached)」，
                        // 而他真正想知道的两件事一个字没说：HEAD 现在**在哪条提交**上，
                        // 以及怎么回去。
                        Some(oid) => {
                            let short: String = oid.chars().take(8).collect();
                            left.push(tr!(format!("分离 HEAD · {short}"), format!("detached HEAD · {short}")));
                            if !self.last_branch.is_empty() {
                                back = Some(self.last_branch.clone());
                            }
                        }
                        None => left.push(st.branch.clone()),
                    }
                    // ↑↓ 各自只在有数的时候出现：`↑0 ↓0` 是噪音，没上游时更是谎言。
                    if st.upstream.is_some() {
                        if st.ahead > 0 {
                            left.push(format!("↑{}", st.ahead));
                        }
                        if st.behind > 0 {
                            left.push(format!("↓{}", st.behind));
                        }
                    }
                    if !st.repo_state.is_empty() && st.repo_state != "none" {
                        left.push(tr!(
                            format!("· {}进行中", op_noun(&st.repo_state)),
                            format!("· {} in progress", op_noun(&st.repo_state))
                        ));
                    }
                }
                // 一段一段画而不是拼成一条：段与段之间要能被判据单独认出来
                // （「↑2 跟没跟在分支名后面」按文字起点比，拼成一串就无从比）。
                // 字级跟着 §2「STATUS：11px 灰字」和 §3.4（辅助 11/400 的用途列写着「状态栏」）；
                // 灰 = §3.1 `TEXT_MUTED`，它那行用途同样明写了「状态栏」。
                // 旧档是分支名 12px `TEXT_PRIMARY`，比内容区正文还重，状态栏读起来像在抢日志区的话。
                let mut x = rect.min.x + 8.0;
                for part in left {
                    let r = ui.painter().text(
                        pos2(x, rect.center().y),
                        Align2::LEFT_CENTER,
                        part,
                        FontId::proportional(11.0),
                        TEXT_MUTED,
                    );
                    x = r.max.x + 4.0;
                }
                if let Some(b) = back {
                    let r = ui.painter().text(
                        pos2(x + 8.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        tr!(format!("回到 {b}"), format!("Back to {b}")),
                        FontId::proportional(11.0),
                        ACCENT,
                    );
                    let spot =
                        Rect::from_min_max(pos2(x + 6.0, rect.min.y), pos2(r.max.x + 2.0, rect.max.y));
                    if ui
                        .interact(spot, egui::Id::unique("gitx-status-go-back"), Sense::click())
                        .clicked()
                    {
                        go_back = Some(b);
                    }
                }
                // 版本号常驻最右：空态（没开仓库）也要能看见这一版是哪个。
                let version = ui.painter().text(
                    pos2(rect.max.x - 8.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    format!("v{}", crate::ui::VERSION),
                    FontId::proportional(11.0),
                    TEXT_MUTED,
                );
                if let Some(path) = &self.repo {
                    // 路径让出右边这一格版本号，不再贴死右缘。
                    ui.painter().text(
                        pos2(version.min.x - 10.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        path,
                        FontId::proportional(11.0),
                        TEXT_MUTED,
                    );
                }
            });
        if let Some(b) = go_back {
            self.act(Action::Checkout(b));
        }
    }

    /// 面板装配顺序。egui 的规矩是**后声明的避让先声明的**，所以从窗底往外排：
    /// 状态栏最贴底（横贯整窗）→ 侧栏（让出状态栏，到 519 为止）→ 暂存带（再让出侧栏，
    /// 从 141 起步）→ CentralPanel 拿剩下的日志区。中间哪个顺序颠倒，底色带就会
    /// 多出一条斜茬（无头判据 `bands_match_the_measured_reference` 量得出来）。
    /// 单独抽出来是为了让无头测试能拿到同一份 `Ui`，不必伪造 `eframe::Frame`。
    pub(crate) fn shell(&mut self, ui: &mut egui::Ui) {
        // 每帧第一件事：把这档偏好刷给当前线程。`tr!` 读的就是它，而核心层那些
        // 拼给人看的错误解释也由同一档位决定（后台那份走 `lang::spawn`，见 `gitx::lang`）。
        gitx::lang::set(self.lang);
        reset_rows_painted();
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.toolbar(ui);
        // 底部面板从窗底往上叠：先声明的贴着窗底。状态栏要贴底，所以排在暂存带前面。
        self.status_bar(ui);
        if self.home {
            // 首页整页替掉侧栏 / 日志 / 暂存带：那三块都对着「当前仓库」，
            // 而首页面前还没选仓库。状态栏留着——版本号与当前仓库路径仍然有用。
            self.home_page(ui);
        } else {
            self.sidebar(ui);
            self.bottom_band(ui);
            self.log_band(ui);
        }
        // 卡片与历史浮层压在最上面一层（`Order::Foreground`），它们自己那一格可点矩形铺满整屏，
        // 所以底下的面板这帧只会收到 hover=false / clicked=false —— 不用在每个面板里各问一次。
        // 顺序就是层序：确认卡最后画，压住历史面板。
        self.history_layer(&ctx);
        self.modal_layer(&ctx);
        // toast 也压在最上层，但要盖过确认卡（提示比还没确认的动作更急）。
        crate::ui::toast::show(&ctx, &mut self.note, &mut self.note_at);
    }
}

impl eframe::App for Gitx {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 每次把窗口从后台切回前台就拉一次远程，不必手动按刷新。只看本帧那条
        // `WindowFocused(true)` 事件，所以只在「失焦 → 聚焦」那一帧触发，不会每帧重拉；
        // 仓库还没加载好（`busy`）或根本没打开仓库时跳过，免得启动/加载中误触发。
        if self.repo.is_some() && !self.busy {
            let regained = ui.ctx().input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::WindowFocused(true)))
            });
            if regained {
                self.land(Intent {
                    act: Some(Action::Fetch),
                    ..Default::default()
                });
            }
        }
        // 原生面板画在 `shell` **之前**：`rfd` 的面板是嵌套事件循环，塞进一帧的中间等于让
        // 同一个窗口同时在两个循环里跑。`take()` 一次消费一个，所以一次点击最多弹一个面板。
        // 无头判据调的是 `shell()`，永远走不到这一行——这是「测试不会弹真面板」那条边界。
        self.run_dialogs();
        self.shell(ui);
    }

    /// macOS 上**关窗必崩**：带 Touch Bar 的机器（这台是 MacBookPro17,1）在 `Window` 被 drop
    /// 之后，下一轮 display-cycle flush 里 AppKit 会对一条已经不存在的 KVO 观察做
    /// `removeObserver`，抛 objc 异常 → `abort()`。上游记在 winit #4728（master）和 #4764
    /// （0.30 维护分支，eframe/egui 还住在这里），两条都还没合。
    ///
    /// 这里直接结束进程绕过它：eframe 的 `save_and_destroy` 顺序是 `egui_ctx.on_exit()` →
    /// `save()` → `app.on_exit()` → 收拾 painter → 才 drop 窗口，所以走到这一位时**该存的都存完了**，
    /// 崩的那一条路整个不走。数据没有可丢的：偏好、书签、上次那本仓库全是改一格就当场落库，
    /// 没有攒到退出才写的东西（见 `apply_prefs`、`Prefs::save`）。
    /// 这一格能被判据钉住，靠的是判据读**这段源码本身**（`quitting_exits_before_the_window_is_dropped`）：
    /// 崩溃要真窗口 + 真显示周期才复现，无头画布量不到，能钉的只有「结束进程写在不在写在这一位」。
    #[cfg(target_os = "macos")]
    fn on_exit(&mut self) {
        std::process::exit(0);
    }
}
