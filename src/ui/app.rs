//! （拆分自 main.rs；见 PROCESS.md「拆分 main.rs」。）

use eframe::egui::{self, Align, Align2, Color32, FontId, Label, Margin, Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, Vec2, pos2, vec2};
use gitx::models::{FileStatus, Repo};
use gitx::{db, git, provider, GitxError};
use std::sync::mpsc::channel;

use crate::ui::theme::*;
use crate::ui::state::*;
use crate::ui::widgets::*;
use crate::ui::data::*;
use crate::ui::diffmodel::*;

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
        let mut app = Gitx {
            conn,
            repos,
            repo: None,
            snap: None,
            sel: Sel::Uncommitted,
            commit_files: Vec::new(),
            file: None,
            diff: Vec::new(),
            patch_label: String::new(),
            seq: 0,
            busy: false,
            note: None,
            log_filter: String::new(),
            side_filter: String::new(),
            topo: false,
            log_rev: None,
            show_remote: true,
            // 真壳一上来就允许翻页：第一页取回不满一页时，`Msg::More` 会把这块牌子拧死。
            log_more: true,
            log_more_busy: false,
            log_seq: 0,
            commit_msg: String::new(),
            commit_desc: String::new(),
            clear_msg_on_load: false,
            add_open: false,
            add_path: String::new(),
            target: None,
            pending_discard: None,
            modal: None,
            modal_focus: false,
            tx,
            rx,
        };
        // 上次开过的仓库直接恢复（存在 core 那张 settings 表里，和 React 版共用同一份）。
        let last = app.conn.as_ref().and_then(|c| db::get_setting(c, "gitx_last_repo").ok().flatten());
        let known = last.filter(|p| app.repos.iter().any(|r| &r.path == p));
        let start = known.or_else(|| app.repos.first().map(|r| r.path.clone()));
        if let Some(path) = start {
            app.open(&path);
        }
        app
    }

    /// 把一个路径登记成书签并立刻打开。填到仓库里的子目录也算——`find_repo_root` 往上走，
    /// 再按核心层的规矩用 `is_repo` 复核一次（ stray `.git` 最多换来一次失败的添加）。
    /// 没有数据库时照样开：那只在内存里记一份，本次会话能用。
    pub(crate) fn add_repo(&mut self, path: &str) {
        let path = path.trim();
        let root = git::find_repo_root(path).filter(|r| git::is_repo(r));
        let Some(root) = root else {
            self.note = Some(format!("那不是仓库：{path}"));
            return;
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
            });
        }
        self.add_open = false;
        self.add_path.clear();
        self.open(&root);
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
        self.patch_label.clear();
        // 分页从头再来：序号加一是让**上一轮**在途的页回复作废（对不上号就丢），
        // 同时把「还有下一页」重新拧成真——换仓库/换排序后那是另一份列表。
        self.log_seq += 1;
        self.log_more = true;
        self.log_more_busy = false;
        self.busy = true;
        spawn_snapshot(&self.tx, path, self.topo, self.log_rev.as_deref());
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

    /// 「所有分支」那枚弹出按钮：换范围 = 重新取一次列表，和换排序同一个理由。
    /// 先比后写，`Some("main")` 点回同一个分支不该把快照整个丢掉。
    pub(crate) fn set_rev(&mut self, rev: Option<String>) {
        if self.log_rev == rev {
            return;
        }
        self.log_rev = rev;
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
        let (seq, topo, rev, tx) = (self.log_seq, self.topo, self.log_rev.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(match git::commits_page(&repo, skip, LOG_PAGE, topo, rev.as_deref()) {
                Ok(commits) => Msg::More { repo, seq, commits },
                Err(e) => Msg::Failed(format!("{repo}: {e}")),
            });
        });
    }

    pub(crate) fn oid(&self, i: usize) -> Option<String> {
        self.snap.as_ref().and_then(|s| s.commits.get(i)).map(|c| c.id.clone())
    }

    /// 唯一一处「选行」：日志行、侧栏「文件状态 / 历史」都走这里。
    pub(crate) fn select(&mut self, sel: Sel) {
        let index = match sel {
            Sel::Commit(i) => i,
            Sel::Uncommitted => {
                self.sel = Sel::Uncommitted;
                self.file = None;
                self.commit_files.clear();
                self.request(Target::Working { staged: false, path: None, untracked: false });
                return;
            }
        };
        let Some(oid) = self.oid(index) else { return };
        let Some(repo) = self.repo.clone() else { return };
        self.sel = sel;
        self.file = None;
        self.commit_files.clear();
        let tx = self.tx.clone();
        {
            let (repo2, oid2) = (repo.clone(), oid.clone());
            std::thread::spawn(move || match git::commit_diff(&repo2, &oid2) {
                Ok(files) => {
                    let _ = tx.send(Msg::CommitFiles { repo: repo2, index, files });
                }
                Err(e) => {
                    let _ = tx.send(Msg::Failed(format!("{oid2}: {e}")));
                }
            });
        }
        self.request(Target::Commit { oid, path: None });
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
        self.busy = true;
        std::thread::spawn(move || {
            let lines = patch_of(&repo, &target).map(|p| layout(&p)).unwrap_or_default();
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

    pub(crate) fn act(&mut self, action: Action) {
        // 克隆是唯一一条**手上还没有仓库**也要能跑的动作：它要的 `repo` 正是它的产物，
        // 所以这道守卫在它之前放行不了，单开一条路。
        if let Action::Clone { url, dest } = action {
            self.start_clone(url, dest);
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        self.busy = true;
        // 只有提交**成功**落地才清空那格信息：git 报错、pre-commit 钩子拦下来、空信息被 core 挡掉，
        // 都得把字留着——否则用户得重打一遍。
        self.clear_msg_on_load = matches!(action, Action::Commit(_));
        let tx = self.tx.clone();
        let (topo, rev) = (self.topo, self.log_rev.clone());
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
                Action::Checkout(name) => git::checkout_branch(&repo, name),
                Action::Fetch => git::fetch(&repo, None, None),
                Action::FetchRemote(name) => git::fetch(&repo, Some(name), None),
                Action::Pull => git::pull(&repo, None, None, None),
                Action::Push => git::push(&repo, None, None, false, false),
                Action::CreateBranch { name, start, checkout } => {
                    git::create_branch(&repo, name, start.as_deref(), *checkout)
                }
                Action::DeleteBranch(name) => git::delete_branch(&repo, name, false),
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
                // 到不了这儿：`act` 在起线程之前就把克隆单独拦下走了 `Msg::Cloned`
                // （它没有 `repo` 可用，也没有快照可重取）。这一条只为穷尽而写，
                // 真被走到就是有人挪动了那道守卫——**当场响**，别让它静默变成「点了没反应」。
                Action::Clone { .. } => {
                    Err(GitxError::Other("克隆不该走到这条路上（act 的守卫被挪了）".to_owned()))
                }
            };
            match r {
                Ok(()) => {
                    // 动作之后重新取一次快照：排序方式得跟着用户那枚开关，不然「层级顺序」
                    // 只在开仓那一次生效，点完提交按钮日志就退回日期序。
                    let _ = tx.send(match snapshot_of(&repo, topo, rev.as_deref()) {
                        Ok(s) => Msg::Loaded { repo, snap: Box::new(s) },
                        Err(e) => Msg::Failed(format!("{repo}: {e}")),
                    });
                }
                Err(e) => {
                    let _ = tx.send(Msg::Failed(e.to_string()));
                }
            }
        });
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
        // 卡片高度自己算（见 `Modal::rows`）：居中要在布局之前定位置。
        let card = Rect::from_center_size(
            screen.center(),
            vec2(MODAL_W, MODAL_TITLE_H + MODAL_ROW_H * m.rows() as f32 + MODAL_FOOT_H),
        );
        let focus = std::mem::take(&mut self.modal_focus);
        let mut fired: Option<Action> = None;
        let mut primary: Option<Action> = None;
        let mut close = false;
        egui::Area::new(egui::Id::unique("gitx-modal"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // 这一格铺满整屏的可点矩形就是遮罩：它把这一层的命中区域撑到全屏，
                // 卡片后面的行才点不着；点它自己也等于「关掉卡片」。
                let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(bg, 0.0, Color32::from_black_alpha(80));
                let p = ui.painter();
                p.rect_filled(card, 6.0, Color32::WHITE);
                p.rect_stroke(card, 6.0, Stroke::new(1.0, RULE), egui::StrokeKind::Middle);
                let mut y = card.min.y;
                put_left(
                    ui,
                    Rect::from_min_max(pos2(card.min.x + 12.0, y), pos2(card.max.x - 12.0, y + MODAL_TITLE_H)),
                    |ui| {
                        ui.add(Label::new(RichText::new(m.title).size(12.5).color(INK_STRONG).strong())
                            .selectable(false));
                    },
                );
                y += MODAL_TITLE_H;
                if let Some(d) = &m.detail {
                    modal_text(ui, card, y, d, MUTED, 11.0);
                    y += MODAL_ROW_H;
                }
                let hint = m.hint();
                if let Some(text) = &mut m.text {
                    let r = Rect::from_min_max(pos2(card.min.x + 10.0, y + 2.0), pos2(card.max.x - 10.0, y + MODAL_ROW_H - 2.0));
                    let resp = ui.put(
                        r,
                        TextEdit::singleline(text).hint_text(hint).align(Align2::LEFT_CENTER),
                    );
                    if focus {
                        resp.request_focus();
                    }
                    y += MODAL_ROW_H;
                }
                // 这两类卡下面没有别的正文了，所以 `y` 在这儿读完就收工：底部那两枚按
                // `card.max.y` 定位，不跟着正文行数是有意为之（卡片高度已经算进 `rows`）。
                match &mut m.form {
                    Form::StashSave { include_untracked } => {
                        put_left(ui, Rect::from_min_max(pos2(card.min.x + 10.0, y), pos2(card.max.x - 10.0, y + MODAL_ROW_H)), |ui| {
                            ui.checkbox(include_untracked, "包含未跟踪文件");
                        });
                    }
                    Form::Merge { branches } => {
                        for (i, b) in branches.iter().enumerate() {
                            if modal_row(ui, card, y + MODAL_ROW_H * i as f32, b) {
                                fired = Some(Action::Merge(b.clone()));
                            }
                        }
                    }
                    // 第二格文本框。`y` 在这儿不再往后推：底部那两枚按 `card.max.y` 定位，
                    // 这一格的高度已经记进 `rows()`。
                    Form::Clone { dest } => {
                        let r = Rect::from_min_max(
                            pos2(card.min.x + 10.0, y + 2.0),
                            pos2(card.max.x - 10.0, y + MODAL_ROW_H - 2.0),
                        );
                        let _ = ui.put(
                            r,
                            TextEdit::singleline(dest)
                                .hint_text("目标目录")
                                .align(Align2::LEFT_CENTER),
                        );
                    }
                    _ => {}
                }
                // 底部那两枚：主按钮在最右（macOS 那套），「取消」贴着它左边。
                let cy = card.max.y - MODAL_FOOT_H / 2.0;
                let mut right = card.max.x - 10.0;
                if !m.primary().is_empty() {
                    let (hit, w) = hunk_button(ui, m.primary(), right, cy);
                    if hit {
                        primary = primary_action(&mut m);
                    }
                    right -= w + 8.0;
                }
                let (hit_cancel, _) = hunk_button(ui, "取消", right, cy);
                if hit_cancel || bg_resp.clicked() {
                    close = true;
                }
            });
        if esc {
            close = true;
        }
        if enter && primary.is_none() {
            primary = primary_action(&mut m);
        }
        // 名字空着不关卡：git 会拒，但先把「该填什么」留在那儿让用户补，别让他从头打一遍。
        self.modal = if close || fired.is_some() || primary.is_some() { None } else { Some(m) };
        if let Some(a) = fired.or(primary) {
            self.act(a);
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
                    // 提交数可能变了（刚切了分支）：下标越界就退回工作区行。
                    self.sel = match &self.sel {
                        Sel::Commit(i) if *i < snap.commits.len() => Sel::Commit(*i),
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
                Msg::CommitFiles { repo, index, files } => {
                    if self.repo.as_deref() == Some(repo.as_str()) && self.sel == Sel::Commit(index) {
                        self.commit_files = files;
                    }
                }
                Msg::Patch { repo, seq, label, lines } => {
                    if self.repo.as_deref() == Some(repo.as_str()) && seq == self.seq {
                        self.diff = lines;
                        self.patch_label = label;
                        self.busy = false;
                    }
                }
                Msg::Cloned(r) => {
                    self.busy = false;
                    match r {
                        // 成了就当书签加进去并打开：`add_repo` 自己会发 `Msg::Loaded`。
                        Ok(root) => self.add_repo(&root),
                        Err(e) => self.note = Some(format!("克隆没成：{e}")),
                    }
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
                }
            }
        }
        if got || self.busy {
            ctx.request_repaint();
        }
    }

    // ---------------------------------------------------------------- 工具栏
    /// 第一行 = 参考图那条标题带，**只放参考图有的东西**：仓库名、分支、11 枚竖按钮。
    /// 提示语与「添加仓库」以前也塞在这一行，横向布局给不了它们预算，于是
    /// `书签 ▼` 压住了 `⤴`、`添加仓库` 压住了 `≡`（无头量出来的两对交叠，见
    /// `the_toolbar_row_has_no_overlapping_widgets`）。它们现在搬到 `tab_bar`。
    pub(crate) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut tool: Option<usize> = None;
        let name = self
            .repo
            .as_ref()
            .and_then(|p| self.repos.iter().find(|r| &r.path == p).map(|r| r.name.clone()))
            .unwrap_or_else(|| "没有仓库".to_owned());
        let head = self.snap.as_ref().map(|s| {
            let st = &s.status;
            let mut t = st.branch.clone();
            if st.ahead > 0 || st.behind > 0 {
                t.push_str(&format!("  ↑{} ↓{}", st.ahead, st.behind));
            }
            t
        });
        egui::Panel::top("toolbar")
            .exact_size(TOOLBAR_H)
            // 左边 91.5 是给红绿灯让的（它们由操作系统画在这一行上面，x 20..72），
            // 右边 21 是参考图最右一枚按钮的余量。上下 5 让 21pt 图标 + 标签落在带里。
            .frame(egui::Frame::NONE.fill(TOOLBAR_BG).inner_margin(Margin {
                left: TITLE_INSET,
                right: TOOLBAR_RIGHT,
                top: 5,
                bottom: 5,
            }))
            .show(ui, |ui| {
                // 一整行：左边仓库名 + 分支，右边 11 个竖按钮。
                // 分两行的话 43pt 的按钮会把 55pt 的带撑到 74pt（无头测试量出来的）。
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{name} (Git)")).size(13.5).strong());
                    if let Some(h) = &head {
                        ui.label(RichText::new(h).size(11.5).color(MUTED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        // right_to_left 得反着加。
                        for i in (0..TOOLS.len()).rev() {
                            if tool_button(ui, i) {
                                tool = Some(i);
                            }
                        }
                    });
                });
            });
        if let Some(i) = tool {
            // 卡片开着的时候工具条不再接第二下 —— 上面那层遮罩挡的是**鼠标**，键盘是共享的：
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
            // 「提交」在 SourceTree 里是切到提交视图；这个壳只有一页，提交框常驻在
            // 文件表下方，所以那枚按钮做的就是把视线带回工作区那一行。
            0 => self.sel = Sel::Uncommitted,
            1 => self.act(Action::Pull),
            2 => self.act(Action::Push),
            3 => self.act(Action::Fetch),
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
            7 if self.snap.is_some() => self.act(Action::BrowseRemote("origin".to_owned())),
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
            n => self.note = Some(format!("egui 版还没接：{}（{n}）", TOOLS[n].1)),
        }
    }

    /// 把一张卡推到屏幕上，并让光标落在它的文本框里。
    /// 没有文本框的卡（挑分支、确认）这枚标记自然是 `false`，下一帧 `modal_layer` 就把它清掉。
    pub(crate) fn show_modal(&mut self, m: Modal) {
        self.modal_focus = m.text.is_some();
        self.modal = Some(m);
    }

    /// 把一帧里从各处收上来的意图落地。顺序是定死的：**先说话、再挪位置、最后才动手**。
    ///
    /// 动手之前还有一道闸：`needs_confirm` 那几类不直接执行，而是换一张确认卡。
    /// 拦在这里而不是每一处菜单里，是因为入口只会越来越多（侧栏、日志行、文件行、工具条），
    /// 逐处加「确认」迟早会漏一处——而漏的那一处正好是点错时会丢工作的那一处。
    pub(crate) fn land(&mut self, it: Intent) {
        if let Some(n) = it.note {
            self.note = Some(n);
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
            return;
        }
        if let Some(a) = it.act {
            if needs_confirm(&a) {
                let (title, detail) = confirm_of(&a);
                self.show_modal(Modal::confirm(title, detail, a));
            } else {
                self.act(a);
            }
        }
    }

    // ------------------------------------------------------------- 仓库 tab 条
    /// 第二行：书签列表摊成一排 tab，点一枚就是把那个仓库开起来。
    /// SourceTree 把这些放在**另一个窗口**（书签窗），这个壳只有一个窗口，所以列表常驻在这一行。
    /// 行尾那格是给开发期提示（`note`）的固定预算——它以前挤在标题带里，把工具按钮压住了。
    pub(crate) fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut pick: Option<String> = None;
        let mut note = self.note.take();
        let mut clear = false;
        let mut add_open = self.add_open;
        let mut add_path = std::mem::take(&mut self.add_path);
        let mut try_add = false;
        let mut try_clone = false;
        let current = self.repo.clone();
        let repos = self.repos.clone();
        egui::Panel::top("tabs")
            .exact_size(TAB_H)
            .frame(egui::Frame::NONE.fill(TOOLBAR_BG).inner_margin(Margin::symmetric(6, 2)))
            .show(ui, |ui| {
                // 控件高度钉死在这一条的净高上：`spacing.interact_size.y` 是 22（= ROW_H），
                // 直接 `ui.button` 会把 24pt 的带撑到 30（无头量出来的），
                // 于是 tab 条的底色越界压到侧栏和日志顶上几像素。
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), TAB_H - 4.0),
                    egui::Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for r in &repos {
                            if tab_button(ui, &r.name, current.as_deref() == Some(r.path.as_str())) {
                                pick = Some(r.path.clone());
                            }
                        }
                        // 空列表时这枚按钮是**唯一**的入口。
                        // 不能用 `egui::Button`：它的下限是 `spacing.interact_size.y`(22) 再加
                        // 内边距，24pt 的带会被撑到 30，底色越界压到侧栏与日志顶上（无头量出来的）。
                        if tab_button(ui, "添加仓库", false) {
                            add_open = !add_open;
                        }
                        if add_open {
                            ui.add_sized(
                                Vec2::new(200.0, 19.0),
                                TextEdit::singleline(&mut add_path).hint_text("仓库路径…"),
                            );
                            let can = !add_path.trim().is_empty();
                            if can && tab_button(ui, "打开", false) {
                                try_add = true;
                            }
                            // 「克隆…」不跟着 `can` 走：这一格填的是**已有仓库的路径**，
                            // 克隆要的是地址，两件事各填各的（地址在卡里那两格）。
                            if tab_button(ui, "克隆…", false) {
                                try_clone = true;
                            }
                        }
                    },
                );
                // tab 再多也不能把提示挤出这一行：它单独占一格**固定**宽度，靠右摆。
                // 不走布局流（`ui.horizontal` 里塞进去就得跟 tab 抢宽度），直接按面板矩形
                // 右下角算绝对位置——`truncate()` 在横向布局里会吃掉整行剩余宽度，
                // 那正是上一批重叠的成因。
                if let Some(n) = &note {
                    let r = ui.max_rect();
                    let rect = Rect::from_min_max(
                        pos2(r.max.x - NOTE_W, r.min.y + 2.0),
                        pos2(r.max.x, r.max.y - 2.0),
                    );
                    let hit = put_left(ui, rect, |ui| {
                        ui.add(Label::new(RichText::new(n).size(10.5).color(WARN)).truncate().selectable(false))
                    });
                    clear = hit.clicked();
                }
            });
        if clear {
            note = None;
        }
        self.note = note;
        self.add_open = add_open;
        self.add_path = add_path;
        if try_add {
            let path = self.add_path.clone();
            self.add_repo(&path);
        }
        if try_clone {
            self.show_modal(Modal::new_clone());
        }
        if let Some(p) = pick {
            self.open(&p);
        }
    }

    // ------------------------------------------------------------------ 侧栏
    /// 参考图：分类是「图标 + 浅灰标签」一行 47pt，子项 26pt 一条、**和分类标签同一列**，
    /// 被选中的那一条铺一枚灰色圆角药丸。**没有**折叠三角，也**没有**「（3）」计数——
    /// 这两样我们原来都有，是凭 SourceTree 3 的印象加的，4.2.19 的图上不存在。
    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        let none = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (branches, tags, remotes, stashes) = match &self.snap {
            Some(s) => (&s.branches, &s.tags, &s.remotes, &s.stashes),
            None => (&none.0, &none.1, &none.2, &none.3),
        };
        let sel = self.sel.clone();
        let mut filter = self.side_filter.clone();
        let mut intent = Intent::default();
        egui::Panel::left("sidebar")
            .exact_size(SIDEBAR_W)
            .frame(egui::Frame::NONE.fill(SIDEBAR_BG).inner_margin(Margin::same(SIDE_INSET)))
            .show(ui, |ui| {
                // 先把最底那条筛选框的高度扣掉再开滚动区：ScrollArea 默认吃掉全部可用高度，
                // 不扣的话「筛选器」会被挤到窗口外（无头测试里侧栏的底色矩形超出窗口 27pt）。
                let list_h = (ui.available_height() - SIDE_FILTER_H).max(60.0);
                ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
                    side_row(ui, SIDE_HEAD_H, "工作区", 12.5, SIDE_LABEL_C, false, Some(icon_monitor));
                    for (label, hit) in [("文件状态", Sel::Uncommitted), ("历史", Sel::Commit(0))] {
                        if side_row(ui, ROW_H, label, 12.0, SIDE_INK, sel == hit, None).clicked() {
                            intent.goto = Some(hit);
                        }
                    }
                    if side_row(ui, ROW_H, "搜索", 12.0, SIDE_INK, false, None).clicked() {
                        intent.note = Some("egui 版还没接：搜索面板（SourceTree 是带作用域条的独立面板）".to_owned());
                    }
                    side_category(
                        ui,
                        "分支",
                        icon_branch,
                        &filter,
                        branches,
                        |b| b.name.clone(),
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
                            menu_item(ui, "删除这个分支", || Action::DeleteBranch(b.name.clone()), intent);
                        },
                        &mut intent,
                    );
                    side_category(
                        ui,
                        "标签",
                        icon_tag,
                        &filter,
                        tags,
                        |t| t.name.clone(),
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
                        &mut intent,
                    );
                    side_category(
                        ui,
                        "远端",
                        icon_cloud,
                        &filter,
                        remotes,
                        |r| r.name.clone(),
                        |_| false,
                        |r, intent| {
                            intent.note = Some(format!("{} → {}（右键这一行有抓取、打开主页）", r.name, r.url));
                        },
                        |ui, r, intent| {
                            menu_item(ui, "抓取这个远端", || Action::FetchRemote(r.name.clone()), intent);
                            menu_item(ui, "抓取全部远端", || Action::Fetch, intent);
                            menu_item(ui, "在浏览器里打开主页", || Action::BrowseRemote(r.name.clone()), intent);
                        },
                        &mut intent,
                    );
                    side_category(
                        ui,
                        "已贮藏",
                        icon_stash,
                        &filter,
                        stashes,
                        |s| format!("stash@{{{}}}: {}", s.index, s.message),
                        |_| false,
                        |s, intent| {
                            intent.note = Some(format!("右键这一行才有操作：应用 / 弹出 / 丢弃（stash@{{{}}}）", s.index));
                        },
                        |ui, s, intent| {
                            menu_item(ui, "应用到工作区", || Action::StashApply(s.index), intent);
                            menu_item(ui, "弹出并删掉这条", || Action::StashPop(s.index), intent);
                            menu_item(ui, "丢弃这条贮藏", || Action::StashDrop(s.index), intent);
                        },
                        &mut intent,
                    );
                });
                ui.add_space(4.0);
                ui.add_sized(Vec2::new(ui.available_width(), 20.0), TextEdit::singleline(&mut filter).hint_text("筛选器"));
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
                    // 横向内边距 0：文件表那几列的缩进**全部按面板左缘量**（141 起算：方框 8、
                    // 图标 47、文件名 68），再让 egui 偷偷加 4pt 就对不上参考图了。
                    // 竖向留 2pt，给上下那条分隔线。
                    .frame(egui::Frame::NONE.fill(Color32::WHITE).inner_margin(Margin::symmetric(0, 2)))
                    .show(ui, |ui| self.file_list(ui));
                egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::WHITE).inner_margin(Margin::symmetric(0, 2))).show(ui, |ui| self.diff_pane(ui));
            });
    }

    pub(crate) fn file_list(&mut self, ui: &mut egui::Ui) {
        let mut act: Option<Action> = None;
        let mut pick: Option<Target> = None;
        let sel = self.sel.clone();

        if sel == Sel::Uncommitted {
            let none_files: Vec<FileStatus> = Vec::new();
            let files: &[FileStatus] = match &self.snap {
                Some(s) => &s.status.files,
                None => &none_files,
            };
            let staged: Vec<&FileStatus> = files.iter().filter(|f| f.staged).collect();
            let unstaged: Vec<&FileStatus> = files.iter().filter(|f| !f.staged).collect();
            // 参考图第 5 条：列表上方一条「待定的文件, 已依照路径排序」。这一行和下面的提交框
            // 都不跟着列表滚——SourceTree 的关系是「列表滚，两头钉」。
            ui.horizontal(|ui| {
                // 参考图这一条的文字起在 x 169.5，和下面分类行的标签**同一列**（面板左缘 141 + 28.5）。
                ui.add_space(CHK_HEAD_X + CHK_S + 7.0);
                ui.label(RichText::new(format!("待定的文件, 共 {} 个 · 按路径排序", files.len())).size(11.0).color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("全部暂存").clicked() {
                        act = Some(Action::StageAll);
                    }
                    if ui.small_button("全部取消暂存").clicked() {
                        act = Some(Action::UnstageAll);
                    }
                });
            });
            let current = self.file.clone();
            // 列表只在剩下的空间里滚，把 `COMMIT_H` 那一格留给下面的提交框。
            // 不这么做的话文件一多，提交框整格被顶出窗口（真窗口目视抓到过，判据见
            // `the_commit_box_stays_inside_a_short_band`）。
            let list_h = (ui.available_height() - COMMIT_H).max(40.0);
            ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
                section(ui, "已暂存文件", &staged, true, &current, &mut act, &mut pick);
                section(ui, "未暂存文件", &unstaged, false, &current, &mut act, &mut pick);
                if files.is_empty() {
                    ui.label(RichText::new("工作区是干净的。").color(MUTED).size(12.0));
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
            let can = staged_count > 0 && !msg.is_empty();
            // 两句话分开报，措辞抄 core 那句（`gitx-core/src/git.rs:1082`）。
            let why = if staged_count == 0 { "先勾选要提交的文件" } else { "提交信息不能为空" };
            // ⌘/Ctrl+Enter 提交（参考版 placeholder 里就写着这条）。读不了 `Ui::input`——0.37 的
            // `InputState` 只挂在 `Context` 上（egui-0.37.0/src/ui.rs 里没有 `fn input`）。
            // `i.modifiers` 是**按下修饰键时那条 `ModifiersChanged`** 更新出来的（input_state/mod.rs:434），
            // 键盘事件自带的那份不算——所以无头测试必须像 winit 那样先发 `ModifiersChanged`。
            let enter = ui.ctx().input(|i| {
                i.key_pressed(egui::Key::Enter) && (i.modifiers.command || i.modifiers.ctrl)
            });
            if can && enter {
                // 键盘与鼠标各只有一处判断：同一个开关读两遍会让一条判据永远杀不死另一半。
                act = Some(Action::Commit(msg.clone()));
            }
            ui.horizontal(|ui| {
                let btn = egui::Button::new(RichText::new(format!("提交到 {branch} ({staged_count})")).size(11.5));
                if ui.add_enabled(can, btn).clicked() {
                    act = Some(Action::Commit(msg.clone()));
                }
                if !can {
                    ui.label(RichText::new(why).size(11.0).color(MUTED));
                }
            });
        } else {
            let files = &self.commit_files;
            let oid = match &sel {
                Sel::Commit(i) => self.oid(*i),
                _ => None,
            };
            let subject = self
                .snap
                .as_ref()
                .and_then(|s| match &sel {
                    Sel::Commit(i) => s.commits.get(*i).map(|c| c.subject.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("这个提交改了 {} 个文件", files.len())).size(11.0).color(MUTED));
                if let Some(o) = &oid {
                    if ui.small_button("全部变更").clicked() {
                        pick = Some(Target::Commit { oid: o.clone(), path: None });
                    }
                }
            });
            ui.label(RichText::new(subject).size(11.5).color(MUTED));
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let current = self.file.clone();
                for d in files {
                    let o = oid.clone();
                    let p = d.path.clone();
                    let tail = format!("+{} −{}", d.additions, d.deletions);
                    // 整行可点、文字不可拖选：`selectable(on)` 那套误会上一批已经修掉了，
                    // 见 `a_click_on_an_unpicked_file_row_switches_to_it`。
                    // 某条提交的改动列表**没有方框**：那早就落库了，勾不勾都不动索引（参考图也只有
                    // 工作区那两栏带方框）。
                    let (resp, _) = file_row(ui, &d.path, &d.status, current.as_deref() == Some(d.path.as_str()), &tail, None);
                    if resp.clicked() {
                        if let Some(o) = o {
                            pick = Some(Target::Commit { oid: o, path: Some(p) });
                        }
                    }
                }
            });
        }

        if let Some(action) = act {
            self.act(action);
        }
        if let Some(target) = pick {
            self.file = target_path(&target).map(str::to_owned);
            self.request(target);
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
        let lines = &self.diff;
        // 差异区顶上一格 = 参考图那条 (236,236,236) 的灰带：状态色块 + **加粗、靠左**的文件名。
        // 左对齐是用户 2026-10-20 特别点过的一条（「描述、文件 diff 展示都是左对齐的」）。
        let head = rect_of(ui, DIFF_HEAD_H);
        let p = ui.painter();
        p.rect_filled(head, 0.0, DIFF_HEAD_BG);
        let mut x = head.min.x + DIFF_HEAD_PAD;
        if let Some(st) = &badge_st {
            let (fill, glyph) = badge_of(st);
            let b = Rect::from_min_size(pos2(x, head.center().y - BADGE_S / 2.0), Vec2::splat(BADGE_S));
            p.rect_filled(b, 4.0, fill);
            p.text(b.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(10.5), Color32::WHITE);
            x = b.max.x + 6.0;
        }
        if busy {
            p.text(head.right_center(), Align2::RIGHT_CENTER, "读取中…", FontId::proportional(11.0), MUTED);
        }
        let r = Rect::from_min_max(pos2(x, head.min.y), pos2(head.max.x - if busy { 62.0 } else { 6.0 }, head.max.y));
        put_left(ui, r, |ui| {
            ui.add(Label::new(RichText::new(label.as_str()).size(12.0).color(INK_STRONG).strong()).truncate())
        });
        if lines.is_empty() {
            ui.centered_and_justified(|ui| ui.label(RichText::new("没有差异").color(MUTED)));
            return;
        }
        let width = ui.available_width();
        let font = ui.style().text_styles[&TextStyle::Monospace].clone();
        let th = ui.text_style_height(&TextStyle::Monospace);
        let fg = ui.visuals().text_color();
        // 「只动这一块」那两枚按钮的落点：只有**工作区**的单文件差异有区块可动
        // （某条提交早就落库了；未跟踪文件在 `git diff` 里根本没有补丁，打不出第二块）。
        let ops = self.hunk_ops();
        let pending = self.pending_discard;
        let mut hit: Option<Action> = None;
        // 「这一帧有没有人碰过放弃那枚」：`None` = 没碰，状态原样留着；
        // `Some(Some(n))` = 第一下， armed 在第 n 块；`Some(None)` = 第二下或点了别的块，拆掉。
        let mut arm: Option<Option<usize>> = None;
        // 只画视口里的行：comic 那份 28 875 行的差异，帧里出现的行数与窗口高度有关，与文件多大无关。
        ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, DIFF_ROW_H, lines.len(), |ui, range| {
            for r in range {
                bump_rows_painted();
                let line = &lines[r];
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, DIFF_ROW_H), Sense::hover());
                let p = ui.painter_at(rect);
                let bg = match line.kind {
                    Kind::Add => ADD_BG,
                    Kind::Del => DEL_BG,
                    Kind::Hunk => HUNK_BG,
                    Kind::Ctx => Color32::TRANSPARENT,
                };
                if bg != Color32::TRANSPARENT {
                    p.rect_filled(rect, 0.0, bg);
                }
                for (i, n) in [(0, line.old), (1, line.new)] {
                    let text = n.map(|v| v.to_string()).unwrap_or_default();
                    let r = band(rect, rect.min.x + NO_W * i as f32, NO_W, th);
                    p.text(r.right_center(), Align2::RIGHT_CENTER, text, font.clone(), LINE_NO);
                }
                let x = rect.min.x + NO_W * 2.0 + 6.0;
                let rich = match line.kind {
                    // hunk 头在 `layout`（工作线程）里就已经翻成人话了，帧里只管画。
                    // 这儿原来还带一份自己的翻译，两条规则各写一遍——见 `a_hunk_row_reads_as_a_block_number_and_line_range`。
                    Kind::Hunk => RichText::new(&line.text).font(font.clone()).color(MUTED),
                    _ => RichText::new(&line.text).font(font.clone()).color(fg),
                };
                put_left(ui, band(rect, x, (rect.max.x - x).max(40.0), th), |ui| {
                    ui.add(Label::new(rich).truncate());
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
                        let mut right = rect.max.x - HUNK_BTN_EDGE;
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
        if let Some(a) = hit {
            self.act(a);
        }
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
                ui.centered_and_justified(|ui| ui.label(RichText::new("左侧选一个仓库").color(MUTED)));
            });
            return;
        };
        // 环下面那条竖线接的是**第一条提交**的车道，所以取 graph 第一行。
        let stem_lane = snap.graph.first().map(|g| g.lane).unwrap_or(0);
        let width = ui.available_width();
        let mut want_topo = self.topo;
        let mut want_rev = self.log_rev.clone();
        let mut want_remote = self.show_remote;
        let show_remote = self.show_remote;
        let scope_label = self
            .log_rev
            .clone()
            .unwrap_or_else(|| ALL_BRANCHES.to_owned());
        // 「所有分支」那一档列的是本地分支：核心层把它当 rev 传给 `git log`（见 `log_args`
        // 里那段「rev 必须排在 `--` 前面」）。
        let scope_items: Vec<String> = std::iter::once(ALL_BRANCHES.to_owned())
            .chain(snap.branches.iter().map(|b| b.name.clone()))
            .collect();
        let mut need_more = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::WHITE).inner_margin(Margin::symmetric(0, 2)))
            .show(ui, |ui| {
                // 内容筛选条（参考图 y≈110..150）：三枚**弹出按钮**（当前那项 + ▼）
                // + 一枚还没接的「紧凑视图」+ 右侧「跳转到:」。
                ui.horizontal(|ui| {
                    if let Some(p) = popup_button(ui, &scope_label, &scope_items) {
                        want_rev = if p == ALL_BRANCHES { None } else { Some(p) };
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
                        let w = (ui.available_width() - 8.0).clamp(90.0, 260.0);
                        ui.add_sized(Vec2::new(w, 20.0), TextEdit::singleline(&mut query).hint_text("跳转到:"));
                    });
                });
                header_row(ui, width);
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
                let total = keep.len() + 1;
                // 筛过的日志**不画图序列**：行与行断开了，竖线会连到一条没被显示的提交上，
                // 看着像 bug。SourceTree 筛选时那一列同样空着。
                let unfiltered = keep.len() == snap.commits.len();
                ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, ROW_H, total, |ui, range| {
                    // 视口的最后一行追平列表尾 = 滚到底了。这里只能记在局部变量上：
                    // `snap` 还借用着 `self`，闭包里碰不了 `self.log_*`。
                    if range.end >= total {
                        need_more = true;
                    }
                    for r in range {
                        bump_rows_painted();
                        let hit = if r == 0 { Sel::Uncommitted } else { Sel::Commit(keep[r - 1]) };
                        let cells = if r == 0 {
                            // 参考图这一行只有「Uncommitted changes」五个词，日期是**拍照时刻**
                            // （「今天 14:46」）而不是分支名。几个文件待提交写在下面文件表那一格里。
                            Cells {
                                sha: "*".to_owned(),
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
                                sha: c.short_id.clone(),
                                subject: c.subject.clone(),
                                // 作者列参考图给的是 `baorong <superlbr@126.com>`：名字带尖括号邮箱。
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
                            Sel::Commit(i) => *i,
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
        // 远程药丸只是**画不画**，列表一个字都没改，所以这一枚不重取快照。
        self.show_remote = want_remote;
        if want_topo != self.topo {
            self.set_topo(want_topo);
        }
        if want_rev != self.log_rev {
            self.set_rev(want_rev);
        }
        self.land(intent);
    }

    /// 四块面板的装配顺序 = 参考图的分块顺序：先工具栏（横贯顶）与侧栏（贯穿到底），
    /// 再声明底部带，最后 CentralPanel 才拿到剩下的日志区。
    /// 单独抽出来是为了让无头测试能拿到同一份 `Ui`，不必伪造 `eframe::Frame`。
    pub(crate) fn shell(&mut self, ui: &mut egui::Ui) {
        reset_rows_painted();
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.toolbar(ui);
        self.tab_bar(ui);
        self.sidebar(ui);
        self.bottom_band(ui);
        self.log_band(ui);
        // 卡片压在最上面一层（`Order::Foreground`），它自己那一格可点矩形铺满整屏，
        // 所以底下的面板这帧只会收到 hover=false / clicked=false —— 不用在每个面板里各问一次。
        self.modal_layer(&ctx);
    }
}

impl eframe::App for Gitx {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell(ui);
    }
}
