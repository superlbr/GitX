//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use eframe::egui::{self, Align, Align2, Color32, FontId, Label, Margin, Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, Vec2, pos2, vec2};
use gitx::models::{FileStatus, Repo};
use gitx::{db, git, provider, GitxError};
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

/// 存盘面板里那格建议文件名的前半段：路径最后一段当仓库名。
/// `Path::file_name` 自己会忽略尾斜杠（`/a/b/` 也给出 `b`），所以不先 trim。
/// 归档的 zip 落到用户自己选的位置，这个名字只是**默认值**，面板里可以改。
fn repo_name(repo: &str) -> String {
    std::path::Path::new(repo)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| repo.to_owned())
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
            // 搜索一上来是关着的：那一格连日志条都不在（点开侧栏「搜索」才换上去）。
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
            target: None,
            pending_discard: None,
            history: None,
            hist_seq: 0,
            modal: None,
            modal_focus: false,
            pending_dialog: None,
            last_branch: String::new(),
            remote_open: Vec::new(),
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
            self.note = Some(format!("那不是仓库：{path}"));
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
        std::thread::spawn(move || {
            let _ = tx.send(match git::commits_page(&repo, skip, LOG_PAGE, topo, &scope) {
                Ok(commits) => Msg::More { repo, seq, commits },
                Err(e) => Msg::Failed(format!("{repo}: {e}")),
            });
        });
    }

    /// 侧栏「搜索」那一行：把日志条右边那一格换成搜索框。
    /// 已经开着就不动命中列表——人只是又点了一下那一行（想不起搜索在哪），
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
            std::thread::spawn(move || {
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
        std::thread::spawn(move || {
            let lines = patch_of(&repo, &target).map(|p| layout(&p, show_files)).unwrap_or_default();
            let _ = tx.send(Msg::Patch { repo, seq, label, lines });
        });
    }

    /// 起一次克隆。`clone_url` 和 `remote_url` 给同一个：核心层留了「用加速节点抓、
    /// 落地的 origin 指真地址」那个口子，这个壳还没有节点概念，就不自己编一个。
    pub(crate) fn start_clone(&mut self, url: String, dest: String) {
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = git::clone_repo(&url, &url, &dest).map_err(|e| format!("{e}"));
            let _ = tx.send(Msg::Cloned(r));
        });
    }

    /// 起一次 `git init`。产物和克隆一样是「磁盘上多了一本仓库」，所以回复共用
    /// `Msg::Cloned`，由 `add_repo` 去登记并打开——两条路只需要一处「开新仓库」的代码。
    pub(crate) fn start_init(&mut self, path: String) {
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
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
                let Some(path) = rfd::FileDialog::new().set_title("归档成 zip").set_file_name(&name).save_file() else {
                    return;
                };
                self.archive_to(&repo, &oid, &path.to_string_lossy());
            }
            Dialog::PickFolder { field } => {
                let Some(path) = rfd::FileDialog::new().set_title("选一个目录").pick_folder() else {
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
        std::thread::spawn(move || {
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
        let tx = self.tx.clone();
        let (topo, scope) = (self.topo, self.scope.clone());
        std::thread::spawn(move || {
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
                            .map_err(|e| GitxError::Other(format!("打不开浏览器：{e}"))),
                        None => Err(GitxError::Other(format!(
                            "{url} 不是「主机/所有者/仓库」那种远端地址"
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
                    GitxError::Other("开仓库和归档那几条不该走到这条路上（act 的守卫被挪了）".to_owned()),
                ),
            };
            match r {
                Ok(()) => {
                    // 动作之后重新取一次快照：排序方式得跟着用户那枚开关，不然「层级顺序」
                    // 只在开仓那一次生效，点完提交按钮日志就退回日期序。
                    let _ = tx.send(match snapshot_of(&repo, topo, &scope) {
                        Ok(s) => Msg::Loaded { repo, snap: Box::new(s) },
                        Err(e) => Msg::Failed(format!("{repo}: {e}")),
                    });
                }
                Err(e) => {
                    let _ = tx.send(Msg::Failed(e.to_string()));
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
                            ("打开已有", AddTab::Open),
                            ("克隆到本地", AddTab::Clone),
                            ("新建仓库", AddTab::New),
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
                        let (hit, w) = card_button(ui, "浏览…", right, y + MODAL_ROW_H / 2.0, false);
                        if hit {
                            pick = Some(FolderField::Main);
                        }
                        right -= w + 6.0;
                    }
                    let r = Rect::from_min_max(
                        pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - FIELD_H) / 2.0),
                        pos2(right, y + (MODAL_ROW_H + FIELD_H) / 2.0),
                    );
                    let resp = card_field(ui, r, hint, text, 1);
                    if focus {
                        resp.request_focus();
                    }
                    y += MODAL_ROW_H;
                }
                // 这两类卡下面没有别的正文了，所以 `y` 在这儿读完就收工：底部那两枚按
                // `card.max.y` 定位，不跟着正文行数是有意为之（卡片高度已经算进 `rows`）。
                match &mut m.form {
                    Form::StashSave { include_untracked } => {
                        pref_switch(ui, card, y, "包含未跟踪文件", include_untracked);
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
                        modal_text(ui, card, y + MODAL_ROW_H, RESET_SHARES, TEXT_SECONDARY, 11.0);
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
                        let (hit, w) = card_button(ui, "浏览…", right, y + MODAL_ROW_H / 2.0, false);
                        if hit {
                            pick = Some(FolderField::CloneDest);
                        }
                        right -= w + 6.0;
                        let r = Rect::from_min_max(
                            pos2(card.min.x + MODAL_PAD, y + (MODAL_ROW_H - FIELD_H) / 2.0),
                            pos2(right, y + (MODAL_ROW_H + FIELD_H) / 2.0),
                        );
                        let _ = card_field(ui, r, "目标目录", dest, 1);
                    }
                    // 描述那一格 + 两枚勾。行高记在 `rows()` 里（`DESC_ROWS` + 2）。
                    Form::Commit { desc, amend, push_after } => {
                        let r = Rect::from_min_max(
                            pos2(card.min.x + MODAL_PAD, y),
                            pos2(card.max.x - MODAL_PAD, y + MODAL_ROW_H * DESC_ROWS as f32),
                        );
                        let _ = card_field(ui, r, "扩展描述…", desc, DESC_ROWS);
                        let y = y + MODAL_ROW_H * DESC_ROWS as f32;
                        pref_switch(ui, card, y, "修正上一条提交", amend);
                        let y = y + MODAL_ROW_H;
                        pref_switch(ui, card, y, "提交之后立刻推送", push_after);
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
                                ("通用", PrefPage::General),
                                ("推送与拉取", PrefPage::Push),
                                ("关于", PrefPage::About),
                            ],
                            page,
                        );
                        let y = y + MODAL_ROW_H + MODAL_GAP;
                        match *page {
                            PrefPage::General => {
                                // 三条**都真接得通**才摆得上来：一条点了没反应的开关比没有更糟。
                                // 「启动时重新打开」改的是下一次启动（读配置在 `Gitx::new`），
                                // 另两条这一帧就看得见效果。
                                pref_switch(ui, card, y, "启动时重新打开上一次的仓库", &mut prefs.restore_last);
                                pref_switch(ui, card, y + MODAL_ROW_H, "日志默认按层级顺序（而不是日期）", &mut prefs.topo);
                                pref_switch(ui, card, y + MODAL_ROW_H * 2.0, "描述列显示远程分支那枚药丸", &mut prefs.show_remote);
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
                                        ui.label(RichText::new("默认远端：").size(11.5));
                                        let label = if prefs.default_remote.is_empty() {
                                            "（没有）"
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
                                        ui.label(RichText::new("推送时：").size(11.5));
                                        // 五项文案逐字取自本机那枚 `NSPopUpButton` 的菜单
                                        // （测量记录见 docs/PROCESS.md）；存的是 git 那个键名。
                                        let items: Vec<String> = PushBehavior::LABELS
                                            .iter()
                                            .map(|l| l.to_string())
                                            .collect();
                                        if let Some(p) =
                                            popup_button(ui, prefs.push_behavior.label(), &items)
                                        {
                                            if let Some(i) =
                                                PushBehavior::LABELS.iter().position(|l| *l == p)
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
                                    "推送所有标签到远程仓库",
                                    &mut prefs.push_tags,
                                );
                                pref_switch(
                                    ui,
                                    card,
                                    y + MODAL_ROW_H * 3.0,
                                    "允许强制推送",
                                    &mut prefs.allow_force,
                                );
                            }
                            PrefPage::About => {
                                modal_text(ui, card, y, &format!("书签里的仓库：{repos} 本"), TEXT_PRIMARY, 11.5);
                                modal_text(
                                    ui,
                                    card,
                                    y + MODAL_ROW_H,
                                    &database.clone().unwrap_or_else(|| "没有可用的数据库".to_owned()),
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
                    let (hit, _) = card_button(ui, "取消", right, cy, false);
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
        if let Form::Settings { prefs, .. } = &m.form {
            self.apply_prefs(prefs.clone());
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
                        Err(e) => self.note = Some(format!("仓库没落地：{e}")),
                    }
                }
                // 归档成了：只报一句落在哪儿，界面什么都不换（它没动仓库，快照还是对的）。
                Msg::Archived(dest) => {
                    self.busy = false;
                    self.note = Some(format!("已导出 {dest}"));
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
        if got || self.busy {
            ctx.request_repaint();
        }
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
                    for r in &repos {
                        if repo_tab(ui, &r.name, current.as_deref() == Some(r.path.as_str())) {
                            pick = Some(r.path.clone());
                        }
                    }
                    // 空列表时这枚是**唯一**的入口。以前它旁边摊着一格路径输入和
                    // 「打开 / 克隆…」两枚，用户说太简陋：三件事各有一格输入，挤在一条里根本
                    // 读不清在填什么，现在统一收成一张卡（`Modal::add_repo`）。
                    if ghost_tab(ui, "＋ 添加仓库") {
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
                        ui.add_space(14.0 - ICON_BTN_GAP);
                        if let Some((branch, counts)) = &head {
                            ui.spacing_mut().item_spacing.x = 6.0;
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
            0 => self.note = Some("还没有仓库，先按「添加仓库」开一本".to_owned()),
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
                    self.note = Some("还没有可用的快照".to_owned());
                    return;
                };
                let branches: Vec<String> =
                    s.branches.iter().filter(|b| !b.is_current).map(|b| b.name.clone()).collect();
                if branches.is_empty() {
                    self.note = Some("没有别的分支可以合并".to_owned());
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
            n => self.note = Some(format!("egui 版还没接：{}（{n}）", TOOLS[n].1)),
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
            return Err("还没有仓库，先按「添加仓库」开一本".to_owned());
        };
        if s.remotes.is_empty() {
            return Err("这本仓库还没有远端可推。".to_owned());
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
            return Err("还没有仓库，先按「添加仓库」开一本".to_owned());
        };
        if s.remotes.is_empty() {
            return Err("这本仓库还没有远端可拉。".to_owned());
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
        }
    }

    /// 偏好卡上那几枚开关**当场生效、当场落库**（macOS 的偏好窗口没有「应用」按钮）。
    /// 一条都没动就直接返回：`topo` 那一条要重取整份快照，每帧白跑一次是看得见的开销。
    pub(crate) fn apply_prefs(&mut self, p: Prefs) {
        if p == self.prefs() {
            return;
        }
        let re_sort = p.topo != self.topo;
        // 落库要的是**改之前**那一份完整的值：下面几条里有 `String` 字段会被移走，
        // 移走之后再 `p.save()` 就编译不过，所以先拷一份专门用来写。
        let to_save = p.clone();
        self.restore_last = p.restore_last;
        self.show_remote = p.show_remote;
        self.push_behavior = p.push_behavior;
        self.default_remote = p.default_remote;
        self.push_tags = p.push_tags;
        self.allow_force = p.allow_force;
        to_save.save(self.conn.as_ref());
        if re_sort {
            // 排序变了要让日志重取一份，不然屏幕上还是旧顺序。
            self.set_topo(p.topo);
        }
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
                                RichText::new(format!("文件历史 · {}", h.path))
                                    .size(12.5)
                                    .color(TEXT_PRIMARY)
                                    .strong(),
                            )
                            .truncate()
                            .selectable(false),
                        );
                    },
                );
                if hunk_button(ui, "关闭", card.max.x - HIST_PAD, head_cy).0 {
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
                            if ui.selectable_label(h.tab == HistTab::History, "历史").clicked() {
                                want_tab = Some(HistTab::History);
                            }
                            if ui.selectable_label(h.tab == HistTab::Blame, "逐行归属").clicked() {
                                want_tab = Some(HistTab::Blame);
                            }
                            if h.tab == HistTab::History {
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    let can_restore = !h.entries.is_empty() && h.sel < h.entries.len();
                                    let btn = egui::Button::new(RichText::new("恢复到这一版").size(11.0));
                                    if ui.add_enabled(can_restore, btn).clicked() {
                                        restore = true;
                                    }
                                    if ui.selectable_label(h.view == HistView::Full, "全文").clicked() {
                                        want_view = Some(HistView::Full);
                                    }
                                    if ui.selectable_label(h.view == HistView::Diff, "改动").clicked() {
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
                                if h.log_loading { "读取历史…" } else { "这个文件没有历史。" },
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
                                        history_note(ui, right, "读取改动…");
                                    } else if h.diff.is_empty() {
                                        history_note(ui, right, "这个版本对这个文件没有可显示的改动。");
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
                                        history_note(ui, right, "读取全文…");
                                    } else if h.full.is_empty() {
                                        history_note(ui, right, "这个版本里没有内容。");
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
                                if h.blame_loading { "读取逐行归属…" } else { "这个文件没有可归属的行。" },
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
        // 侧栏「搜索」那一行。放在 `goto` 之前：那一行点的是**开一格**，不是选一行，
        // 两者同一帧里不会互相覆盖（开着搜索再点某一行，选中的还是那一行）。
        if it.open_search {
            self.open_search();
        }
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
                    self.note = Some(format!("{short} 还不在已经加载的那几页历史里（往下滚会取到）"));
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
        // 「搜索」那一行开着的时候它得看起来是选中的：`Sel` 里没这一档（搜索不是一个**行**，
        // 是日志条那一格的另一种状态），所以单独读一份布尔进来。
        let search_open = self.search_open;
        // 展开着的那几条远端的名字，**开局读一份**：面板闭包里 `self` 正被借走，
        // 而这一份只用来决定画不画，改它走 `Intent::toggle_remote`（见 `land`）。
        let remote_open = self.remote_open.clone();
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
                    // 「工作区」那一档固定三条，所以计数写死 3；它不是从 `items` 数出来的。
                    side_group_row(ui, "工作区", 3);
                    for (label, hit) in [("文件状态", Sel::Uncommitted), ("历史", Sel::Commit(0))] {
                        if side_row(ui, label, sel == hit, false).clicked() {
                            intent.goto = Some(hit);
                        }
                    }
                    // SourceTree 这一行点开的是它自己那块**带作用域条的搜索面板**。本机没有
                    // SourceTree.app（`/Applications` 和 `mdfind` 都查过），那块面板的控件一个数
                    // 都没量到，参考图也只拍到这一行本身——所以这里**不照抄它**，按 2026-10-21
                    // 规则②做成我们的增强：把日志条右边那一格换成搜索框，搜整本历史的
                    // 提交说明 + 改动过的文件名。文案与档位都是自定，登记在 DESIGN §7。
                    if side_row(ui, "搜索", search_open, false).clicked() {
                        intent.open_search = true;
                    }
                    side_category(
                        ui,
                        "分支",
                        &filter,
                        branches,
                        |b| b.name.clone(),
                        // 分支这一组没有「选中」：`Sel` 里没这一档，点它就是切过去（会改 HEAD，
                        // 回来时 `is_current` 自己会挪地方）。§2 那颗蓝点标的是**当前**，不是选中。
                        |_| false,
                        |b| b.is_current,
                        |b, intent| intent.run(Action::Checkout(b.name.clone())),
                        |ui, b, intent| {
                            menu_item(ui, "切换过去", || Action::Checkout(b.name.clone()), intent);
                            menu_card(
                                ui,
                                "从这里新建分支",
                                || Modal::new_branch_at(Some(b.target.clone())),
                                intent,
                            );
                            menu_item(ui, "合并到当前分支", || Action::Merge(b.name.clone()), intent);
                            menu_card(
                                ui,
                                "重命名分支…",
                                || Modal::rename_branch(b.name.clone()),
                                intent,
                            );
                            menu_item(ui, "删除这个分支", || Action::DeleteBranch(b.name.clone()), intent);
                        },
                        // 本地分支没有第三级：点开没有下一步，硬摆一格空的只会让用户以为坏了。
                        |_, _, _| {},
                        &mut intent,
                    );
                    side_category(
                        ui,
                        "标签",
                        &filter,
                        tags,
                        |t| t.name.clone(),
                        |_| false,
                        |_| false,
                        |t, intent| intent.reveal = Some(t.target.clone()),
                        |ui, t, intent| {
                            menu_go(ui, "到那条提交", t.target.clone(), intent);
                            menu_item(ui, "检出这个标签", || Action::Checkout(t.target.clone()), intent);
                            menu_card(
                                ui,
                                "在这条提交上新建分支",
                                || Modal::new_branch_at(Some(t.target.clone())),
                                intent,
                            );
                            menu_item(ui, "删除这个标签", || Action::DeleteTag(t.name.clone()), intent);
                        },
                        |_, _, _| {},
                        &mut intent,
                    );
                    side_category(
                        ui,
                        "远端",
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
                            menu_item(ui, "抓取这个远端", || Action::FetchRemote(r.name.clone()), intent);
                            menu_item(ui, "抓取全部远端", || Action::Fetch, intent);
                            menu_item(ui, "在浏览器里打开主页", || Action::BrowseRemote(r.name.clone()), intent);
                            // 地址那一格从**点一下**挪到了右键：左键现在管展开，不能再兼着念地址。
                            if ui.button("看地址").clicked() {
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
                            for rr in remote_refs.iter().filter(|x| x.name.starts_with(&prefix)) {
                                let tail = &rr.name[prefix.len()..];
                                let resp = side_sub_row(ui, tail);
                                let name = rr.name.clone();
                                let target = rr.target.clone();
                                resp.context_menu(|ui| {
                                    menu_item(ui, "切换过去", || Action::CheckoutRemote(name.clone()), intent);
                                    menu_go(ui, "到那条提交", target.clone(), intent);
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
                        "已贮藏",
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
                            menu_item(ui, "应用到工作区", || Action::StashApply(s.index), intent);
                            menu_item(ui, "弹出并删掉这条", || Action::StashPop(s.index), intent);
                            menu_item(ui, "丢弃这条贮藏", || Action::StashDrop(s.index), intent);
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
                        TextEdit::singleline(&mut filter).hint_text("筛选器"),
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
                ui.label(RichText::new(PENDING_SORTED_BY_PATH).size(11.0).color(TEXT_MUTED));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("全部暂存").clicked() {
                        act = Some(Action::StageAll);
                    }
                    if ui.small_button("全部取消暂存").clicked() {
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
            ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
                conflict_section(ui, &conflicts, &current, &mut act, &mut pick, &mut hist);
                section(ui, "已暂存文件", &staged, true, &current, &mut act, &mut pick, &mut hist);
                section(ui, "未暂存文件", &unstaged, false, &current, &mut act, &mut pick, &mut hist);
                if files.is_empty() {
                    ui.label(RichText::new("工作区是干净的。").color(TEXT_SECONDARY).size(12.0));
                }
            });

            // ---- 提交框：SourceTree 那一格是**两个框**（摘要 + 描述），都在文件表下方、都靠左。
            // 落进 git 是一条 `-m`，两框之间隔一个空行——和 SourceTree 落出去的一样。
            let branch = self.snap.as_ref().map(|s| s.status.branch.clone()).unwrap_or_default();
            let staged_count = staged.len();
            let w = ui.available_width();
            ui.add(TextEdit::singleline(&mut self.commit_msg).hint_text("提交摘要… (⌘/Ctrl+Enter 提交)").desired_width((w - 8.0).max(80.0)));
            ui.add(
                TextEdit::multiline(&mut self.commit_desc)
                    .hint_text("扩展描述…")
                    .desired_rows(3)
                    .desired_width((w - 8.0).max(80.0)),
            );
            let msg = commit_text(&self.commit_msg, &self.commit_desc);
            // amend 允许没有新暂存内容：只改写上一条的信息也是合法用法。
            let can = !msg.is_empty() && (staged_count > 0 || self.amend);
            // 两句话分开报，措辞抄 core 那句（`gitx-core/src/git.rs:1082`）。
            let why = if msg.is_empty() { "提交信息不能为空" } else { "先勾选要提交的文件" };
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
                ui.checkbox(&mut self.amend, RichText::new("修正上一条").size(11.0))
                    .on_hover_text("改写 HEAD：换掉上一条提交的信息，已暂存的内容会一起并进去");
                let label = if self.amend {
                    "修正上一条提交".to_owned()
                } else {
                    format!("提交到 {branch} ({staged_count})")
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
                format!("这条贮藏改了 {} 个文件", files.len())
            } else {
                format!("这个提交改了 {} 个文件", files.len())
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(head).size(11.0).color(TEXT_SECONDARY));
                if let Some(o) = &oid {
                    if ui.small_button("全部变更").clicked() {
                        pick = Some(if stash {
                            Target::Stash { oid: o.clone(), path: None }
                        } else {
                            Target::Commit { oid: o.clone(), path: None }
                        });
                    }
                }
            });
            ui.label(RichText::new(subject).size(11.5).color(TEXT_SECONDARY));
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
            p.text(head.right_center(), Align2::RIGHT_CENTER, "读取中…", FontId::proportional(11.0), TEXT_MUTED);
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
                        .hint_text("搜索差异")
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
            let msg = if lines.is_empty() { "没有差异" } else { "没有匹配的行" };
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
        ScrollArea::both().auto_shrink([false, false]).show_rows(ui, DIFF_ROW_H, rows.len(), |ui, range| {
            for r in range {
                bump_rows_painted();
                let line = &lines[rows[r]];
                if line.kind == Kind::File {
                    // 文件头带：整份补丁里唯一的分界。整行可点，点一下折叠/展开这个文件。
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(content_w, DIFF_ROW_H), Sense::click());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 0.0, DIFF_HEAD_BG);
                    let folded = collapsed.contains(&line.file);
                    let label = format!("{}  {}", if folded { "已折叠" } else { "文件" }, line.text);
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
                        if folded { "点击展开" } else { "点击折叠" },
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
                                if armed { "确认放弃" } else { "放弃区块" },
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
                            if staged { "取消暂存区块" } else { "暂存区块" },
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
                ui.centered_and_justified(|ui| ui.label(RichText::new("左侧选一个仓库").color(TEXT_MUTED)));
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
        let width = ui.available_width();
        let mut want_topo = self.topo;
        let mut want_scope = self.scope.clone();
        let mut want_remote = self.show_remote;
        let show_remote = self.show_remote;
        let scope_label = scope_label(&self.scope);
        // 前两档是 SourceTree 官方的两态（LogBranchesAll ⇄ LogBranchesCurrent）；
        // 后面列本地分支是我们自己加的——它那一档只切两态，想只看某条支线得自己挑。
        let scope_items: Vec<String> = std::iter::once(ALL_BRANCHES.to_owned())
            .chain(std::iter::once(CURRENT_BRANCH.to_owned()))
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
                        want_scope = match p.as_str() {
                            ALL_BRANCHES => git::Scope::All,
                            CURRENT_BRANCH => git::Scope::Current,
                            _ => git::Scope::Rev(p),
                        };
                    }
                    if let Some(p) = popup_button(
                        ui,
                        if want_remote { SHOW_REMOTE } else { HIDE_REMOTE },
                        &[SHOW_REMOTE.to_owned(), HIDE_REMOTE.to_owned()],
                    ) {
                        want_remote = p == SHOW_REMOTE;
                    }
                    if let Some(p) = popup_button(
                        ui,
                        if want_topo { TOPO_ORDER } else { DATE_ORDER },
                        &[DATE_ORDER.to_owned(), TOPO_ORDER.to_owned()],
                    ) {
                        want_topo = p == TOPO_ORDER;
                    }
                    // 没有箭头 = 点开没有第二层。这一枚的真实行为还没对着参考图量过（见 PROCESS
                    // 「仍然没量过的」），与其给个假菜单，不如把「还没接」这句话留着。
                    if ui.add(egui::Button::new("紧凑视图").small()).clicked() {
                        intent.note = Some("egui 版还没接：紧凑视图".to_owned());
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
                                    .hint_text(SEARCH_HINT)
                                    .id_salt("log-search"),
                            );
                            // 光标落在这一格：点侧栏「搜索」的人下一步就是打字，不该再点一下框。
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
                                "搜索中…".to_owned()
                            } else if sq.trim().is_empty() {
                                "回车开搜".to_owned()
                            } else if hits.is_empty() {
                                "没搜到".to_owned()
                            } else {
                                format!("{} 条命中", hits.len())
                            };
                            ui.label(RichText::new(status).size(11.0).color(TEXT_SECONDARY));
                            return;
                        }
                        let w = (ui.available_width() - 8.0).clamp(90.0, 260.0);
                        ui.add_sized(Vec2::new(w, 20.0), TextEdit::singleline(&mut query).hint_text(JUMP_TO_SHA));
                    });
                });
                header_row(ui, width);
                // 搜索开着、这一次什么都没捞到：列表那一块是空的，`show_rows` 一个字都不画。
                // 空态得写明，不然看着像「搜索坏了」而不像「没有命中」——尤其框里那个词
                // 是人自己打的，他会怀疑自己敲错了地方。
                if searching && hits.is_empty() && !search_busy {
                    ui.label(RichText::new(format!("没有提交命中「{}」：说明和改过的文件名都不含它", sq.trim())).size(11.0).color(TEXT_MUTED));
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
                                author: format!("{} <{}>", h.commit.author, h.commit.author_email),
                                date: h.date.clone(),
                                badges: visible_badges(&h.badges, show_remote),
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
                                // 作者列：名字带尖括号邮箱（旧参考图这一格是
                                // `baorong <superlbr@126.com>`；§2 只给了列宽 96，超出的部分
                                // 由 `band` 截断）。
                                author: format!("{} <{}>", c.author, c.author_email),
                                date: snap.dates.get(i).cloned().unwrap_or_default(),
                                badges: snap.badges.get(i).map(|b| visible_badges(b, show_remote)).unwrap_or_default(),
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
                if let Some(snap) = &self.snap {
                    let st = &snap.status;
                    match &st.detached_at {
                        // 分离 HEAD。`branch.head` 那一行 git 写的是字面量 `(detached)`，
                        // 把它当分支名印出来，用户读到的是「有一条分支叫 (detached)」，
                        // 而他真正想知道的两件事一个字没说：HEAD 现在**在哪条提交**上，
                        // 以及怎么回去。
                        Some(oid) => {
                            let short: String = oid.chars().take(8).collect();
                            left.push(format!("分离 HEAD · {short}"));
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
                        left.push(format!("· {}进行中", op_noun(&st.repo_state)));
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
                        format!("回到 {b}"),
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
        reset_rows_painted();
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.toolbar(ui);
        // 底部面板从窗底往上叠：先声明的贴着窗底。状态栏要贴底，所以排在暂存带前面。
        self.status_bar(ui);
        self.sidebar(ui);
        self.bottom_band(ui);
        self.log_band(ui);
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
        // 原生面板画在 `shell` **之前**：`rfd` 的面板是嵌套事件循环，塞进一帧的中间等于让
        // 同一个窗口同时在两个循环里跑。`take()` 一次消费一个，所以一次点击最多弹一个面板。
        // 无头判据调的是 `shell()`，永远走不到这一行——这是「测试不会弹真面板」那条边界。
        self.run_dialogs();
        self.shell(ui);
    }
}
