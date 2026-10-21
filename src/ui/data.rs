//! （拆分自 main.rs；见 PROCESS.md「拆分 main.rs」。）

use chrono::Datelike;
use eframe::egui::{self, Align2, Color32, FontId, Label, Rect, RichText, Sense, Stroke, TextStyle, Vec2, pos2, vec2};
use gitx::models::{Branch, ChangeStatus, Commit, FileStatus, RemoteRef, Tag};
use gitx::git;
use std::sync::mpsc::Sender;

use crate::ui::theme::*;
use crate::ui::state::*;
use crate::ui::widgets::*;
use crate::ui::graph::*;
use crate::ui::diffmodel::layout;

// -------------------------------------------------------------------- 数据/线程
/// 日志日期列的文案。参考图量到的只有「今天 12:27」这一种（那张截图里最老的一行就是当天的），
/// 「昨天」按同一形状推；年内给「MM-DD HH:MM」、跨年才补年份，是照 macOS 的惯例定的，
/// **没有参考图依据**，等一张更老的截图再校。
pub(crate) fn date_label(iso: &str, now: chrono::DateTime<chrono::Local>) -> String {
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return short_date(iso);
    };
    let then = parsed.with_timezone(&chrono::Local);
    let hm = then.format("%H:%M");
    let days = (now.date_naive() - then.date_naive()).num_days();
    if days == 0 {
        format!("今天 {hm}")
    } else if days == 1 {
        format!("昨天 {hm}")
    } else if then.year() == now.year() {
        format!("{} {hm}", then.format("%m-%d"))
    } else {
        then.format("%Y-%m-%d").to_string()
    }
}

/// 把「哪个提交号上挂着哪些引用」排成与 `commits` 一一对应的药丸列表。
///
/// 为什么不在帧里现算：这一句要跑「提交数 × 引用数」遍字符串比较（500 × 几十），
/// 而它的结果在一份快照里根本不会变——和图序列的车道是同一类活儿，所以同样归 worker 线程。
/// 顺序照参考图：本地分支在前（它的「超前 N 个版本」紧贴其后），远程跟踪指针其次，标签最后。
pub(crate) fn ref_badges(
    commits: &[Commit],
    branches: &[Branch],
    tags: &[Tag],
    remote_refs: &[RemoteRef],
) -> Vec<Vec<RefBadge>> {
    bump_badge_layouts();
    let mut by_oid: std::collections::HashMap<String, Vec<RefBadge>> = Default::default();
    let mut push = |oid: &str, badge: RefBadge| {
        by_oid.entry(oid.to_owned()).or_default().push(badge);
    };
    for b in branches {
        push(&b.target, RefBadge { label: b.name.clone(), kind: RefKind::Branch });
        // 参考图只在超前时给「超前3个版本」。落后那一支没有依据，先按同一形状给。
        if b.ahead > 0 {
            push(&b.target, RefBadge { label: format!("超前{}个版本", b.ahead), kind: RefKind::Ahead });
        } else if b.behind > 0 {
            push(&b.target, RefBadge { label: format!("落后{}个版本", b.behind), kind: RefKind::Ahead });
        }
    }
    for r in remote_refs {
        push(&r.target, RefBadge { label: r.name.clone(), kind: RefKind::Remote });
    }
    for t in tags {
        push(&t.target, RefBadge { label: t.name.clone(), kind: RefKind::Tag });
    }
    commits.iter().map(|c| by_oid.get(&c.id).cloned().unwrap_or_default()).collect()
}

/// 一次取多少条提交。500 条按行高 24pt 是 12000pt 的列表，一屏才二十几条——
/// 首屏要够滚，又不能完全打开一个十万提交的仓库（`git log` 那次要解析全部行）。
/// 到底了由 `Msg::More` 用 `--skip` 再取一页。
pub(crate) const LOG_PAGE: u32 = 500;

pub(crate) fn snapshot_of(repo: &str, topo: bool, rev: Option<&str>) -> gitx::Result<Snapshot> {
    // 日期序是 SourceTree 的默认；「层级顺序」把分支聚在一起，图序列读起来才是一条条分支。
    // 第一页走 `skip = 0`：核心层不发消息里那条 `--skip`（`the_page_offset_only_adds_skip` 钉过），
    // 所以这一句和原来分三条路的写法发给 git 的 argv 逐字节相同。
    let commits = git::commits_page(repo, 0, LOG_PAGE, topo, rev)?;
    // 车道在这里排完：这一句跑在 worker 线程上，帧里只剩「按行号取一条 GraphRow」。
    let graph = layout_graph(&commits);
    let branches = git::branches(repo)?;
    let tags = git::tags(repo)?;
    // 远程跟踪指针（`origin/main`）：「我的分支在哪」和「世界在哪」是两件事，
    // 参考图里 `37295d6d` 那一行就是两枚并排的浅蓝药丸。现在**存进快照**而不是算完就丢——
    // 下一页接上来时要把整列药丸重算，少了它就认不出「超前/落后」。
    let remote_refs = git::remote_refs(repo)?;
    // 「现在」取一次就好：日期文案与虚拟行的「今天 HH:MM」都以它为准，
    // 否则同一份快照刷两帧，日期列会自己跳。
    let taken_at = chrono::Local::now();
    let badges = ref_badges(&commits, &branches, &tags, &remote_refs);
    let dates = commits.iter().map(|c| date_label(&c.date, taken_at)).collect();
    Ok(Snapshot {
        status: git::status(repo)?,
        commits,
        branches,
        tags,
        remotes: git::remotes(repo)?,
        remote_refs,
        stashes: git::stash_list(repo)?,
        graph,
        badges,
        dates,
        taken_at,
    })
}

/// 把下一页接到已有列表后面，并重算三列「与提交一一对应」的派生数据。
/// 都是整列重算，不做增量：增量要维护「车道的跨页延续」这种状态，而一页 500 条时
/// 整列重排也就几毫秒——省下的复杂度远比那点时间值钱。
/// `taken_at` 沿用旧值，所以已显示的行日期不动，新页按同一时刻换算。
pub(crate) fn append_page(snap: &mut Snapshot, more: Vec<Commit>) {
    snap.commits.extend(more);
    snap.graph = layout_graph(&snap.commits);
    snap.badges = ref_badges(&snap.commits, &snap.branches, &snap.tags, &snap.remote_refs);
    snap.dates = snap.commits.iter().map(|c| date_label(&c.date, snap.taken_at)).collect();
}

pub(crate) fn spawn_snapshot(tx: &Sender<Msg>, repo: &str, topo: bool, rev: Option<&str>) {
    let (tx, repo, rev) = (tx.clone(), repo.to_owned(), rev.map(str::to_owned));
    std::thread::spawn(move || {
        let _ = tx.send(match snapshot_of(&repo, topo, rev.as_deref()) {
            Ok(s) => Msg::Loaded { repo, snap: Box::new(s) },
            Err(e) => Msg::Failed(format!("{repo}: {e}")),
        });
    });
}

/// 文件历史一次取多少条。SourceTree 的文件历史是一张可滚动的长表，这里先钉 200：
/// 再多就得上分页，而这一批的判据只需要「能列出改动过它的提交」。
pub(crate) const FILE_LOG_MAX: u32 = 200;

pub(crate) fn spawn_file_log(tx: &Sender<Msg>, repo: &str, path: &str, seq: u64) {
    let (tx, repo, path) = (tx.clone(), repo.to_owned(), path.to_owned());
    std::thread::spawn(move || {
        // 未跟踪文件本来就没有历史，`git log --follow -- path` 会成功返回空列表。
        let entries = git::file_log(&repo, &path, FILE_LOG_MAX).unwrap_or_default();
        let _ = tx.send(Msg::FileLog { repo, seq, path, entries });
    });
}

/// 历史面板里选中那条提交对该文件的补丁。路径必须带上：同一提交可能改了很多文件，
/// 面板只认打开的这一条。
pub(crate) fn spawn_history_patch(tx: &Sender<Msg>, repo: &str, oid: &str, path: &str, seq: u64, index: usize) {
    let (tx, repo, path, oid) = (tx.clone(), repo.to_owned(), path.to_owned(), oid.to_owned());
    std::thread::spawn(move || {
        let lines = git::commit_patch(&repo, &oid, Some(&path)).map(|p| layout(&p)).unwrap_or_default();
        let _ = tx.send(Msg::HistoryPatch { repo, seq, path, index, lines });
    });
}

/// 文件在某个版本里的全文。和补丁分开走：看全文的人通常是在找「这行原来长什么样」。
pub(crate) fn spawn_history_text(tx: &Sender<Msg>, repo: &str, oid: &str, path: &str, seq: u64, index: usize) {
    let (tx, repo, path, oid) = (tx.clone(), repo.to_owned(), path.to_owned(), oid.to_owned());
    std::thread::spawn(move || {
        // 文件在那个版本可能不存在（比如这一条就是删掉它的提交）。不把错误当成空文件：
        // 让面板显示原因，比显示一页空白诚实。
        let text = git::rev_file_text(&repo, &oid, &path)
            .unwrap_or_else(|e| format!("（读不出这个版本的文件：{e}）"));
        let _ = tx.send(Msg::HistoryText { repo, seq, path, index, text });
    });
}

/// 逐行归属。`oid = None` 走工作区那份：面板问的是「现在这些行是谁写的」，
/// 不是「某条提交里的那份是谁写的」。
pub(crate) fn spawn_history_blame(tx: &Sender<Msg>, repo: &str, path: &str, seq: u64) {
    let (tx, repo, path) = (tx.clone(), repo.to_owned(), path.to_owned());
    std::thread::spawn(move || {
        let lines = git::blame(&repo, &path, None).unwrap_or_default();
        let _ = tx.send(Msg::HistoryBlame { repo, seq, path, lines });
    });
}

pub(crate) fn patch_of(repo: &str, target: &Target) -> gitx::Result<String> {
    match target {
        Target::Working { staged, path, untracked } => {
            let text = git::working_patch(repo, *staged, path.as_deref())?;
            // 未跟踪文件 `git diff` 什么都说不出，改成展示文件内容，否则点它像没反应。
            if text.trim().is_empty() && *untracked {
                if let Some(p) = path {
                    return git::untracked_patch(repo, p);
                }
            }
            Ok(text)
        }
        Target::Commit { oid, path } => git::commit_patch(repo, oid, path.as_deref()),
    }
}

pub(crate) fn patch_label(t: &Target) -> String {
    match t {
        Target::Working { staged, path: None, .. } => if *staged { "全部已暂存变更" } else { "全部未暂存变更" }.to_owned(),
        Target::Working { staged, path: Some(p), .. } => format!("{} · {}", if *staged { "已暂存" } else { "未暂存" }, p),
        Target::Commit { oid, path: None } => format!("{} · 提交的全部改动", short(oid)),
        Target::Commit { oid, path: Some(p) } => format!("{} · {}", short(oid), p),
    }
}

/// 摘要 + 描述 → 一条 git 信息。描述为空时**不带**那个空行：
/// `git commit -m "标题\n\n"` 会把尾部空行原样留在信息里。
pub(crate) fn commit_text(summary: &str, desc: &str) -> String {
    let s = summary.trim();
    let d = desc.trim();
    if d.is_empty() {
        s.to_owned()
    } else {
        format!("{s}\n\n{d}")
    }
}

pub(crate) fn target_path(t: &Target) -> Option<&str> {
    match t {
        Target::Working { path, .. } | Target::Commit { path, .. } => path.as_deref(),
    }
}

pub(crate) fn short(s: &str) -> String {
    s.chars().take(7).collect()
}

pub(crate) fn short_date(iso: &str) -> String {
    iso.chars().take(10).collect()
}

/// 一行文件状态该显示哪个字母：已暂存的行看索引侧，未暂存的行看工作树侧。
pub(crate) fn status_of(f: &FileStatus) -> &ChangeStatus {
    if f.staged {
        &f.index_status
    } else {
        &f.worktree_status
    }
}

/// 这一行是不是一个未解决的冲突。`git status --porcelain=v2` 的 `u` 行两侧都是 `U`，
/// 但只查一侧就够——另一侧真被改成别的状态时，它也就不再是冲突了。
pub(crate) fn is_conflict(f: &FileStatus) -> bool {
    f.index_status == ChangeStatus::Unmerged || f.worktree_status == ChangeStatus::Unmerged
}

pub(crate) fn mark(s: &ChangeStatus) -> &'static str {
    match s {
        ChangeStatus::Added => "A",
        ChangeStatus::Modified => "M",
        ChangeStatus::Deleted => "D",
        ChangeStatus::Renamed => "R",
        ChangeStatus::Copied => "C",
        ChangeStatus::Untracked => "?",
        ChangeStatus::Unmerged => "U",
        ChangeStatus::Unknown => "·",
    }
}

/// 状态色块的底色 + 字母。参考图里**改动那枚写的是 `…` 不是 `M`**——照它。
/// 没量到的那几种落灰底，字母沿用 `mark`（见 `BADGE_*` 那一段注释）。
pub(crate) fn badge_of(s: &ChangeStatus) -> (Color32, &'static str) {
    match s {
        ChangeStatus::Modified => (BADGE_MOD, "…"),
        ChangeStatus::Renamed => (BADGE_REN, "R"),
        ChangeStatus::Untracked => (BADGE_UNK, "?"),
        other => (BADGE_NONE, mark(other)),
    }
}

/// 文件历史行上那半个字的状态说明。参考图没有文件历史面板可量，这里只用中文，
/// 和文件表里的单字母色块区分开。
pub(crate) fn change_label(s: &ChangeStatus) -> &'static str {
    match s {
        ChangeStatus::Added => "新增",
        ChangeStatus::Modified => "修改",
        ChangeStatus::Deleted => "删除",
        ChangeStatus::Renamed => "改名",
        ChangeStatus::Copied => "复制",
        ChangeStatus::Untracked => "未跟踪",
        ChangeStatus::Unmerged => "冲突",
        ChangeStatus::Unknown => "未知",
    }
}

/// 参考图那种 macOS 方框：勾上 = 蓝底白勾，没勾 = 白底灰边。
/// 返回「这一帧有没有人点它」。
///
/// 为什么自己画而不用 `ui.checkbox`：egui 那枚的尺寸跟着字号走（11.5 号的字撑出 18pt 的框），
/// 而参考图三枚量到都是 13.5 见方，且**钉在面板的固定列上**（分类行 8、文件行 25）——
/// 交给布局系统排，缩进就由前面那格说了算，列就对不齐。
///
/// 勾是两段折线，不是字形：字体里没有 U+2713，画成字会撞上
/// `every_rune_the_shell_paints_has_a_glyph`。
pub(crate) fn check_box(ui: &mut egui::Ui, x: f32, cy: f32, checked: bool) -> bool {
    let r = Rect::from_min_size(pos2(x, cy - CHK_S / 2.0), Vec2::splat(CHK_S));
    let p = ui.painter();
    if checked {
        p.rect_filled(r, 3.0, CHK_BLUE);
        let s = Stroke::new(1.8, Color32::WHITE);
        let f = |ax: f32, ay: f32| pos2(r.min.x + r.width() * ax, r.min.y + r.height() * ay);
        p.line_segment([f(0.24, 0.52), f(0.42, 0.72)], s);
        p.line_segment([f(0.42, 0.72), f(0.78, 0.30)], s);
    } else {
        p.rect_filled(r, 3.0, Color32::WHITE);
        p.rect_stroke(r, 3.0, Stroke::new(1.0, CHK_EDGE), egui::StrokeKind::Middle);
    }
    ui.allocate_rect(r, Sense::click()).clicked()
}

/// 文件表的一行：方框（工作区那两栏才有）+ 状态色块 + 路径，整行可点，当前那一行铺 `PICK_BG` 那条灰带。
///
/// 为什么不走 `ui.add(Label)`：参考图那一行是「色块 + 文字」两样东西，交给布局系统排
/// 就得让色块占一格、文字占一格，行高被撑到 30 以上；而 SourceTree 这一行是 27pt。
/// 色块用画笔摆，路径仍然走 `Label`（要它的 `truncate`，长路径不能溢出面板）。
///
/// 返回 `(整行那个 Response, 点了方框)`。前者不折成 `bool` 是因为调用方还要给这一行挂右键菜单，
/// 菜单得挂在同一个命中矩形上。
///
/// 实测「点了整行」和「点了方框」**不会同时为真**：方框那枚 `allocate_rect` 压在整行的
/// `Sense::click()` 上面，但 egui 0.37 的命中裁决把这次点击只给了面积更小的那枚，所以调用方
/// 各问各的就够，不要再叠一层 `&& !toggled`（把那个守卫删掉，`ticking_a_file_box_stages_only_that_file`
/// 和 `a_click_on_a_rows_text_selects_the_row` 两条门禁照样绿——它们钉的是行为，不是这行代码）。
pub(crate) fn file_row(ui: &mut egui::Ui, path: &str, st: &ChangeStatus, current: bool, tail: &str, check: Option<bool>) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if current {
        ui.painter().rect_filled(rect, 5.0, PICK_BG);
    }
    let mut toggled = false;
    if let Some(on) = check {
        toggled = check_box(ui, rect.min.x + CHK_ROW_X, rect.center().y, on);
    }
    // **没有方框的那一栏，方框占的那一档要一起收掉**（用户 2026-10-21：「勾选的 box 没了，
    // 但是没有移动位置」）。某条提交的改动列表不能勾——早落库了，勾不动索引——可它一直按
    // 有方框的落点排，右边白留 22pt。所以色块挪到方框那一档（+8），文字挪到**分类标题同一列**
    // （8 + 13.5 + 7 = 28.5，和「已暂存文件」那几个字对齐）。
    let (icon_x, name_x) = if check.is_some() { (ICON_X, NAME_X) } else { (NO_BOX_ICON_X, NO_BOX_NAME_X) };
    let p = ui.painter();
    let (fill, glyph) = badge_of(st);
    let badge = Rect::from_min_size(pos2(rect.min.x + icon_x, rect.center().y - BADGE_S / 2.0), Vec2::splat(BADGE_S));
    p.rect_filled(badge, 4.0, fill);
    p.text(badge.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(10.5), Color32::WHITE);
    if !tail.is_empty() {
        p.text(rect.right_center(), Align2::RIGHT_CENTER, tail, FontId::proportional(11.0), MUTED);
    }
    let right = rect.max.x - if tail.is_empty() { 0.0 } else { 76.0 };
    let text = Rect::from_min_max(pos2(rect.min.x + name_x, rect.min.y), pos2(right, rect.max.y));
    put_left(ui, text, |ui| {
        ui.add(Label::new(RichText::new(path).size(12.0)).truncate().selectable(false).sense(Sense::hover()))
    });
    (resp, toggled)
}

/// 文件行的右键菜单。动作直接写进 `act`（和方框、和「放弃整组」那条按钮同一条路），
/// 不绕 `Intent`：这一层本来就在收 out-param，绕一圈只会多一套出口。
pub(crate) fn file_menu(
    ui: &mut egui::Ui,
    path: &str,
    staged: bool,
    untracked: bool,
    act: &mut Option<Action>,
    hist: &mut Option<String>,
) {
    if staged {
        menu_act(ui, "取消暂存这个文件", || Action::Unstage(vec![path.to_owned()]), act);
    } else {
        menu_act(ui, "暂存这个文件", || Action::Stage(vec![path.to_owned()]), act);
    }
    // 未跟踪那份**不是**「放弃改动」——它没有旧版本可回，git 是把文件删了。文案得说实话。
    menu_act(
        ui,
        if untracked { "删掉这个未跟踪的文件" } else { "放弃这个文件的改动" },
        || Action::Discard(vec![path.to_owned()]),
        act,
    );
    menu_act(ui, "忽略这个文件（写进 .gitignore）", || Action::Ignore(path.to_owned()), act);
    if ui.button("查看文件历史").clicked() {
        *hist = Some(path.to_owned());
        ui.close();
    }
}

/// 冲突文件行的右键菜单。菜单项按 SourceTree 的「Mark Resolved / Mark Unresolved /
/// Resolve Using 'Mine' / Resolve Using 'Theirs'」排，文案用官方中文。
/// 前四条都会改工作区，`needs_confirm` 在出口拦下会丢改动的那几条。
pub(crate) fn conflict_menu(
    ui: &mut egui::Ui,
    path: &str,
    act: &mut Option<Action>,
    hist: &mut Option<String>,
) {
    menu_act(ui, "使用「我的」版本解决", || Action::ResolveOurs(path.to_owned()), act);
    menu_act(ui, "使用「他们的」版本解决", || Action::ResolveTheirs(path.to_owned()), act);
    menu_act(ui, "标记为已解决", || Action::MarkResolved(path.to_owned()), act);
    menu_act(ui, "标记为未解决", || Action::MarkUnresolved(path.to_owned()), act);
    if ui.button("查看文件历史").clicked() {
        *hist = Some(path.to_owned());
        ui.close();
    }
}

/// 某条提交的改动列表里文件行的右键菜单。这一栏的文件**已经在库里**，没有暂存/放弃可言，
/// 能做的只有溯源。
pub(crate) fn commit_file_menu(ui: &mut egui::Ui, path: &str, hist: &mut Option<String>) {
    if ui.button("查看文件历史").clicked() {
        *hist = Some(path.to_owned());
        ui.close();
    }
}

pub(crate) fn db_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    Some(format!("{home}/Library/Application Support/com.gitx.app/gitx.db"))
}

pub(crate) fn style(ctx: &egui::Context) {
    // 字体与样式必须在**同一个入口**里装好：无头判据走的就是这个函数，真窗口也走它。
    // 分开装过一次，结果所有量文字宽度的判据都跑在 egui 自带的字体链上——那条链没有汉字，
    // 每个汉字落到 `.notdef` 上，宽度正好是字号的一半（实测「中」@14 = 7.00pt，装完 Noto = 14.00pt）。
    // 于是判据眼里的中文列宽只有真窗口的一半，「这一列放得下」那种结论全是假的。
    crate::ui::fonts::install(ctx);
    // 强制浅色：`style_mut_of` 只改 Light 那份样式，系统要是处于深色模式，egui 按
    // `theme_preference` 取的是 Dark 那份——整份调色板（含 panel_fill、文字色）都会静默失效。
    ctx.set_theme(egui::Theme::Light);
    // 按住 0.8 秒以上才松手，egui 默认**不当它是点击**（`InputOptions::max_click_duration`，
    // 按下去的那一刻起算，超过就转成拖拽）。列表选中跟着鼠标按下走，慢一点的点击
    // 全被吞掉——就是「有时候点不中」。放到 3 秒：坐得住、也还留得出右键那种长按。
    ctx.options_mut(|o| o.input_options.max_click_duration = 3.0);
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.visuals.panel_fill = Color32::WHITE;
        style.visuals.window_fill = Color32::WHITE;
        style.visuals.extreme_bg_color = Color32::WHITE;
        // 行距 0：日志与差异都靠 show_rows 定行高，item_spacing.y 会把行距算进周期里。
        style.spacing.item_spacing = Vec2::new(6.0, 0.0);
        style.spacing.button_padding = Vec2::new(4.0, 1.0);
        // 22pt = ROW_H：控件超过这个高度，55pt 的工具栏带会被撑出去（无头测试抓到过）。
        style.spacing.interact_size = Vec2::new(28.0, ROW_H);
        style.visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(0x22, 0x26, 0x30);
        style.text_styles.insert(TextStyle::Body, FontId::proportional(12.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(10.5));
        style.text_styles.insert(TextStyle::Monospace, FontId::monospace(11.5));
    });
}
