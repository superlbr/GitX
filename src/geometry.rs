//! 门禁：全部无头断言。模块名保持 geometry_tests（scripts 与 PROCESS 里的过滤串不变）。

use eframe::egui::{self, Color32, FontId, Pos2, Rect, pos2, vec2};
use gitx::models::{Branch, ChangeStatus, Commit, DiffEntry, FileStatus, RemoteRef, Repo, RepoStatus, Tag};
use gitx::{db, git};
use std::sync::mpsc::channel;

use crate::ui::data::*;
use crate::ui::diffmodel::*;
use crate::ui::graph::*;
use crate::ui::state::*;
use crate::ui::theme::*;
use crate::ui::widgets::*;
// 窗口约束在无头画布上量不到，判据直接读壳里那份 `viewport()`。
use crate::viewport;

/// 一个壳，数据可选填。填了就能钉排版，不填就是空态分支。
    fn shell(snap: Option<Snapshot>, diff: Vec<DiffLine>) -> Gitx {
        let (tx, rx) = channel();
        Gitx {
            conn: None,
            repos: Vec::new(),
            repo: snap.as_ref().map(|_| "/fixture".to_owned()),
            snap,
            sel: Sel::Uncommitted,
            commit_files: Vec::new(),
            file: None,
            diff,
            patch_label: "全部未暂存变更".to_owned(),
            seq: 0,
            busy: false,
            note: None,
            note_at: None,
            log_filter: String::new(),
            side_filter: String::new(),
            topo: false,
            log_rev: None,
            show_remote: true,
            // 夹具里把「还有下一页」拧死：不然「滚到底要下一页」那格判据会在无头帧里真起线程，
            // 拿 `/fixture` 当仓库跑 git，回来的 `Msg::Failed` 会污染别的断言。
            log_more: false,
            log_more_busy: false,
            log_seq: 0,
            commit_msg: String::new(),
            commit_desc: String::new(),
            clear_msg_on_load: false,
            add_open: false,
            add_path: String::new(),
            // 夹具默认停在「全部未暂存变更」那一格：和上面 `patch_label` 对得上，
            // 也让 hunk 那两枚按钮在整份补丁上**不出现**（没有单一文件路径可打）。
            target: Some(Target::Working { staged: false, path: None, untracked: false }),
            pending_discard: None,
            history: None,
            hist_seq: 0,
            modal: None,
            modal_focus: false,
            tx,
            rx,
        }
    }

    /// 没打开任何仓库的壳：各面板走空态分支，但带照样分块——几何与数据无关。
    fn blank() -> Gitx {
        shell(None, Vec::new())
    }

    /// 一份够用的假数据：三条提交、已暂存/未暂存/未跟踪各一条、一段带 hunk 头的补丁。
    /// 目的不是测 git，是让日志、文件表、差异区**真的有内容可排**——空态壳钉不住排版。
    fn loaded() -> Gitx {
        filled(3, &layout(PATCH))
    }

    /// 某一条提交的改动列表：两行，一行改、一行新增，选中挪到第一条提交上。
    /// 专给「这一栏**没有方框**」那两条判据用——`loaded()` 那三条是工作区的，带方框。
    fn commit_list() -> Gitx {
        let mut app = loaded();
        // 直接挪字段，不走 `select()`：那会对着假路径 `/fixture` 开线程跑 git。
        app.sel = Sel::Commit(0);
        app.commit_files = vec![
            DiffEntry {
                path: "src/changed.rs".to_owned(),
                old_path: None,
                status: ChangeStatus::Modified,
                additions: 4,
                deletions: 2,
            },
            DiffEntry {
                path: "docs/added.md".to_owned(),
                old_path: None,
                status: ChangeStatus::Added,
                additions: 1,
                deletions: 0,
            },
        ];
        app
    }

    /// 自己决定喂多少行。虚拟化的判据比的是「数据翻一千倍、画出来的行数不变」，
    /// 所以数据量必须是参数，不能写死在 `loaded()` 里。
    fn filled(commits: usize, diff: &[DiffLine]) -> Gitx {
        let file = |path: &str, staged: bool, worktree: ChangeStatus| FileStatus {
            path: path.to_owned(),
            old_path: None,
            index_status: if staged { ChangeStatus::Modified } else { ChangeStatus::Unknown },
            worktree_status: worktree,
            staged,
        };
        let commit = |i: usize| Commit {
            id: format!("{i:040}"),
            short_id: format!("a{i:04}"),
            subject: format!("主题{i}：一条中文提交信息"),
            author: "包融".to_owned(),
            author_email: "br@example.com".to_owned(),
            date: "2026-10-20T11:41:07+08:00".to_owned(),
            // 列表是「新的在上」，所以提交 i 的父提交是**下面**那条 i+1，最底下那条才是根。
            // 方向反了图序列就断了：`git log` 保证父提交排在子提交下面（core 里钉着这条）。
            parents: if i + 1 == commits { Vec::new() } else { vec![format!("{:040}", i + 1)] },
        };
        let commits: Vec<Commit> = (0..commits).map(commit).collect();
        let graph = layout_graph(&commits);
        // 让引用真的挂在某一行上：本地分支 `main` 挂在**第一行**（超前 2 个版本 → 该有那枚
        // 深藏青药丸），远程跟踪指针 `origin/main` 挂在第三行（那里只有浅蓝药丸，没有藏青）。
        let oid = |i: usize| commits.get(i).map(|c| c.id.clone()).unwrap_or_default();
        let branches = vec![Branch {
            name: "main".to_owned(),
            is_current: true,
            upstream: Some("origin/main".to_owned()),
            ahead: 2,
            behind: 0,
            target: oid(0),
        }];
        let remote_refs = vec![RemoteRef { name: "origin/main".to_owned(), target: oid(2) }];
        let taken_at = chrono::DateTime::parse_from_rfc3339("2026-10-20T14:46:00+08:00")
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap();
        let badges = ref_badges(&commits, &branches, &[], &remote_refs);
        let dates = commits.iter().map(|c| date_label(&c.date, taken_at)).collect();
        let snap = Snapshot {
            status: RepoStatus {
                branch: "main".to_owned(),
                upstream: Some("origin/main".to_owned()),
                ahead: 2,
                behind: 0,
                files: vec![
                    file("src/staged.rs", true, ChangeStatus::Unknown),
                    file("src/loose.rs", false, ChangeStatus::Modified),
                    file("notes.md", false, ChangeStatus::Untracked),
                ],
                stash_count: 0,
                repo_state: String::new(),
            },
            commits,
            branches,
            tags: Vec::new(),
            remotes: Vec::new(),
            remote_refs,
            stashes: Vec::new(),
            graph,
            badges,
            dates,
            taken_at,
        };
        shell(Some(snap), diff.to_vec())
    }

    /// 上面那三份改动之外再加 `n` 份未暂存改动——「文件多到列表必须滚」只有这一种造法，
    /// 别在每个判据里各抄一遍 push 循环。
    fn many_unstaged(n: usize) -> Gitx {
        let mut app = filled(3, &layout(PATCH));
        for i in 0..n {
            app.snap.as_mut().unwrap().status.files.push(FileStatus {
                path: format!("src/overflow/{i}.rs"),
                old_path: None,
                index_status: ChangeStatus::Unknown,
                worktree_status: ChangeStatus::Modified,
                staged: false,
            });
        }
        app
    }

    /// 两次合并、三条支线先后并回（新的在上）：
    /// `M3` 并 `Y1`，`M2` 并 `X1`，`L2`/`X1`/`Y1` 都从 `L1` 长出来。
    /// 车道分配的三件事全靠这份形状才测得出来：合并开新道、并回后把道腾出来、
    /// 尾巴上的空道裁掉（旧 React 版从不释放，两次合并后就占三条）。
    fn branch_commits() -> Vec<Commit> {
        let node = |id: &str, parents: &[&str]| Commit {
            id: id.to_owned(),
            short_id: id.to_owned(),
            subject: format!("主题{id}"),
            author: "包融".to_owned(),
            author_email: "br@example.com".to_owned(),
            date: "2026-10-20T11:41:07+08:00".to_owned(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
        };
        vec![
            node("M3", &["M2", "Y1"]),
            node("M2", &["L2", "X1"]),
            node("L2", &["L1"]),
            node("X1", &["L1"]),
            node("Y1", &["L1"]),
            node("L1", &["R"]),
            node("R", &[]),
        ]
    }

    fn branched() -> Gitx {
        let commits = branch_commits();
        let graph = layout_graph(&commits);
        let mut app = filled(1, &[]);
        // 只换提交和图序列，其余（状态、文件表）继续用同一份假数据。
        let snap = app.snap.as_mut().expect("filled 造好了快照");
        snap.commits = commits;
        snap.graph = graph;
        app
    }

    /// 长短不齐的一屏日志：提交号 2/3/4 个字符、作者一行一个字一行一串邮箱、日期宽度也各不相同。
    /// 对齐方式的判据必须要**长短不一**的数据——全都一样长时，左对齐与右对齐在帧里长得完全相同，
    /// 那种判据删掉也不会红。
    fn ragged() -> Gitx {
        let node = |sha: &str, author: &str, date: &str, subject: &str| Commit {
            id: format!("{sha:0<40}"),
            short_id: sha.to_owned(),
            subject: subject.to_owned(),
            author: author.to_owned(),
            author_email: "br@example.com".to_owned(),
            date: date.to_owned(),
            parents: Vec::new(),
        };
        let mut app = filled(1, &[]);
        let snap = app.snap.as_mut().expect("filled 造好了快照");
        snap.commits = vec![
            node("a1", "包融", "2026-10-20T09:05:07+08:00", "短"),
            node(
                "b22",
                "baorong <superlbr@126.com>",
                "2026-10-19T23:58:07+08:00",
                "一条长得多的中文提交信息，用来把这一列撑到要截断",
            ),
            node("c333", "R", "2020-03-04T05:06:07+08:00", "中等长度的一条"),
        ];
        // 提交换了，按提交算出来的两列就得跟着重算：`dates`/`badges` 是 worker 线程按
        // **下标**对齐到 `commits` 的，留着 `filled(1)` 那份只会让下面两行拿到空日期、
        // 第一行凭空挂上别人的分支药丸。
        let taken = snap.taken_at;
        snap.dates = snap.commits.iter().map(|c| date_label(&c.date, taken)).collect();
        // 这一份夹具量的是「四列各自的左缘」，而药丸恰恰会把描述往后推，所以这里不挂引用。
        // 药丸自己的判据用 `filled`（它有真的 main + 超前 2 个版本）。
        snap.badges = vec![Vec::new(); snap.commits.len()];
        snap.graph = layout_graph(&snap.commits);
        app
    }

    /// 一段最小的统一差异：一条 hunk 头、两行加、一行删、两行上下文。
    const PATCH: &str = "--- a/src/loose.rs\n+++ b/src/loose.rs\n@@ -8,4 +8,5 @@\n 上下文甲\n-删掉的一行\n+加上的一行\n+又加一行\n 上下文乙\n";

    /// 无头跑一帧。为什么绕开截图：屏幕会锁屏/休眠，`screencapture` 拿回来的可能是一张无关的图；
    /// 而「边界差 1pt 就露馅」「行距不是 21pt 就是排版塌了」这种判据本来就该是断言。
    /// 截图留给颜色、字号那种主观核对。
    /// 空事件的一帧。`frame_events` 是同一件事的完整版（点击、敲字、回车都得从那儿喂）。
    /// 无头跑没有后端去应用字体纹理增量，所以那边统一 `clear`，不 clear 的话 `out` 落栈就 panic。
    fn frame(app: &mut Gitx) -> egui::FullOutput {
        frame_events(app, Vec::new())
    }

    /// 顶部两条带（标题带 + tab 条）里**任何两块文字都不许交叠**。
    /// 上一批的重叠就是这么量出来的：`书签 ▼` 压在 `⤴` 上、`添加仓库` 压在 `≡` 上，
    /// 目视只看到「顶部栏有点乱」，说不清是谁压了谁。
    /// 仓库给到 6 个、提示给到 30 字，是为了把两条带都塞满——空列表那帧什么都压不到。
    #[test]
    fn the_top_bands_have_no_overlapping_widgets() {
        let mut app = filled(3, &layout(PATCH));
        for i in 0..6 {
            app.repos.push(Repo {
                id: None,
                path: format!("/fixture{i}"),
                name: format!("仓库{i}"),
                added_at: None,
                last_fetched_at: None,
            });
        }
        app.note = Some("egui 版还没接：搜索面板（SourceTree 是带作用域条的独立面板）".to_owned());
        let out = frame(&mut app);
        let items: Vec<(String, Rect)> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    (r.center().y <= TOP_H).then(|| (t.galley.text().to_owned(), r))
                }
                _ => None,
            })
            .collect();
        assert!(items.len() > 20, "顶部两带只认出 {} 块文字，这一帧大概什么都没画", items.len());
        let mut bad = Vec::new();
        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                let o = items[i].1.intersect(items[j].1);
                // 交叠超过 1×3pt 才算压住：同一块文字的多 section 会自交几个像素。
                if o.width() > 1.0 && o.height() > 3.0 {
                    bad.push(format!("{:?} 压住 {:?}（{:.1}×{:.1}pt）", items[i].0, items[j].0, o.width(), o.height()));
                }
            }
        }
        assert!(bad.is_empty(), "顶部栏重叠：\n{}", bad.join("\n"));
    }

    /// tab 条点一枚 = 开那个仓库。这一条是 #23 的全部行为：列表摊成 tab，点就是开。
    #[test]
    fn a_repo_tab_opens_that_repo() {
        let mut app = filled(3, &layout(PATCH));
        app.repos.push(Repo {
            id: None,
            path: "/other".to_owned(),
            name: "comic".to_owned(),
            added_at: None,
            last_fetched_at: None,
        });
        assert_eq!(app.repo.as_deref(), Some("/fixture"));
        let ctx = egui::Context::default();
        style(&ctx);
        let at = locate(&ctx, &mut app, "comic").expect("tab 条上没有「comic」这枚 tab");
        click_at(&ctx, &mut app, at);
        assert_eq!(app.repo.as_deref(), Some("/other"), "点了 tab 没换仓库");
    }

    /// 无头滚一滚：同一份数据、同一个 `Context`，先站住，喂进滚轮事件，再站几帧把滚动放完。
    /// 两个坑都是从 egui 源码读出来的：
    /// * `ScrollArea` 只给**指针悬停的那一块**吃滚轮（scroll_area.rs:1324），所以每帧都得有
    ///   一个落在日志带里的 `PointerMoved`；
    /// * 一次离散的滚轮事件会被 egui **摊平到后面几帧**（`wheel_state.rs` 顶部那段注释），
    ///   所以 `time` 必须往前走、还要多跑几帧，不然这一帧只放出 42pt 里的一小段。
    /// 返回「滚动前」和「滚停之后」两帧的（提交描述 → 落点 y）。
    fn scroll_log(dy: f32) -> (Vec<(String, f32)>, Vec<(String, f32)>) {
        let ctx = egui::Context::default();
        style(&ctx);
        let over_log = pos2((SIDEBAR_W + WIN_W) / 2.0, (TOP_H + (WIN_H - STATUS_H - STAGING_H)) / 2.0);
        let hover = vec![egui::Event::PointerMoved(over_log)];
        let raw = |time: f64, events: Vec<egui::Event>| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let subjects = |out: &mut egui::FullOutput| {
            let rows: Vec<(String, f32)> = texts(out)
                .into_iter()
                .filter(|(s, _)| s.starts_with("主题"))
                .map(|(s, p)| (s, p.y))
                .collect();
            out.textures_delta.clear();
            rows
        };
        let mut app = filled(200, &[]);
        let before = subjects(&mut ctx.run_ui(raw(0.0, hover.clone()), |ui| app.shell(ui)));
        let mut after = before.clone();
        for i in 0..40 {
            let events = if i == 0 {
                vec![
                    egui::Event::PointerMoved(over_log),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, -dy),
                        phase: egui::TouchPhase::Move,
                        // 只认「来自滚轮」：source 换成 Trackpad 那条门禁就不吃。
                        source: egui::MouseWheelSource::Wheel,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                hover.clone()
            };
            // 每帧 16ms。egui 的滚轮是**按帧衰减**摊到 `smooth_scroll_delta` 里的，
            // 尾巴要几十帧才收干净：跑 12 帧时滚 52pt 只落到 51，判据会误报成「内容和位置分家」。
            after = subjects(&mut ctx.run_ui(raw(0.016 * (i as f64 + 1.0), events), |ui| app.shell(ui)));
        }
        (before, after)
    }

    /// 把嵌在 `Shape::Vec` 组里的矩形**拆出来**（带上整组的 clip）。egui 有时把几笔折成
    /// 一组发出——toast 卡片（底色+阴影一笔一笔算）就是这么画的，不拆组的话整张卡
    /// 对 `rects()`/`rounded()` 隐形（toast 两判据全红就是它）。
    fn collect_rects<'a>(shape: &'a egui::Shape, clip: Rect, into: &mut Vec<(&'a egui::epaint::RectShape, Rect)>) {
        match shape {
            egui::Shape::Rect(r) => into.push((r, clip)),
            egui::Shape::Vec(v) => {
                for s in v {
                    collect_rects(s, clip, into);
                }
            }
            _ => {}
        }
    }

    /// 这一帧画出的**所有有底色的矩形**：四块带的底色、增删行的底色都是纯色矩形。
    fn rects(out: &egui::FullOutput) -> Vec<(Color32, Rect)> {
        let mut found = Vec::new();
        for c in &out.shapes {
            collect_rects(&c.shape, c.clip_rect, &mut found);
        }
        found
            .into_iter()
            .filter(|(r, _)| r.fill != Color32::TRANSPARENT)
            // 和 clip_rect 求交后再比：Frame 底色按「内容+边距」画，内容比面板高一丝时
            // （换字体行高会变），多出来的那条**被面板裁掉、屏幕上根本不出现**——
            // 判据只对看得见的部分负责。
            .map(|(r, clip)| (r.fill, clip.intersect(r.rect)))
            .filter(|(_, v)| v.is_positive())
            .collect()
    }

    /// 这一帧画出的**圆角矩形**（药丸那一类底色块）：底色、矩形、四个圆角。
    /// 带圆角的矩形在 egui 里仍是 `Shape::Rect`，只是 `corner_radius` 非 0——
    /// 分成两份问是因为 `rects()` 那份要把四块带的**方角**底色认出来，混进药丸就没法数了。
    fn rounded(out: &egui::FullOutput) -> Vec<(Color32, Rect, egui::CornerRadius)> {
        let mut found = Vec::new();
        for c in &out.shapes {
            collect_rects(&c.shape, c.clip_rect, &mut found);
        }
        found
            .into_iter()
            .filter(|(r, _)| r.fill != Color32::TRANSPARENT && r.corner_radius != egui::CornerRadius::ZERO)
            .map(|(r, _)| (r.fill, r.rect, r.corner_radius))
            .collect()
    }

    /// 这一帧画出的**只有描边、没有底色**的圆角矩形，连描边颜色一起给。
    /// `rects()` 和 `rounded()` 都按「底色非透明」筛，看不见空心那一枚——未勾选的方框正是空心，
    /// 它的边框有没有画出来只能问这里。
    fn stroked(out: &egui::FullOutput) -> Vec<(Color32, Rect)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Rect(r)
                    if r.fill == Color32::TRANSPARENT
                        && r.stroke.color != Color32::TRANSPARENT
                        && r.corner_radius != egui::CornerRadius::ZERO =>
                {
                    Some((r.stroke.color, r.rect))
                }
                _ => None,
            })
            .collect()
    }

    /// 虚拟行那枚**空心环**的圆心：白底 + MUTED 灰边。提交是车道色实心点，两者一眼分得开。
    fn rings(out: &egui::FullOutput) -> Vec<Pos2> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Circle(d) if d.fill == Color32::WHITE && d.stroke.color == RING_GREY => Some(d.center),
                _ => None,
            })
            .collect()
    }

    /// 这一块文字的**落笔颜色**（第一个 section 的）。药丸那枚深藏青要白字，
    /// 颜色只能问 galley，`texts()` 给的是内容 + 位置。
    fn text_color(out: &egui::FullOutput, label: &str, y: std::ops::RangeInclusive<f32>) -> Color32 {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    let in_band = t.galley.text().starts_with(label)
                        && r.center().y >= *y.start()
                        && r.center().y <= *y.end();
                    in_band.then(|| t.galley.job.sections.first().map(|s| s.format.color))
                }
                _ => None,
            })
            .flatten()
            .unwrap_or_else(|| panic!("y {y:?} 里没有以「{label}」开头的文字"))
    }

    /// 这一帧画出的**所有文字块**（内容 + 左上角）。行距、行序这类判据只能问它们。
    fn texts(out: &egui::FullOutput) -> Vec<(String, Pos2)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((t.galley.text().to_owned(), t.pos)),
                _ => None,
            })
            .collect()
    }

    /// 表头那一行的 y 带。按「图表」定位——它是表头独有的词，行高一改它自己跟着走。
    /// 判据里别再抄 78/95/96 这种数字：这一批把行距从 21 改成 26，抄过数字的三条全红了。
    fn head_band(out: &egui::FullOutput) -> std::ops::RangeInclusive<f32> {
        let r = text_rect(out, "图表", 55.0..=(WIN_H - STATUS_H - STAGING_H));
        (r.center().y - HEADER_H / 2.0)..=(r.center().y + HEADER_H / 2.0)
    }

    /// 日志数据区的 y 带：表头下沿一直到暂存带上沿。
    fn rows_band(out: &egui::FullOutput) -> std::ops::RangeInclusive<f32> {
        *head_band(out).end()..=(WIN_H - STATUS_H - STAGING_H)
    }

    /// 日志数据区四列的**左缘**。`COL_INSET` 只有一个来源，判据不再各抄一份 `+8.0`——
    /// 这一批把间距从 8 改成实测的 5，抄了常量的判据绿、硬编码的那条恰好把旧值钉死了。
    fn col_edges(min_x: f32, right: f32) -> [f32; 4] {
        [
            min_x + COL_GRAPH + COL_INSET,
            min_x + COL_GRAPH + COL_SHA + COL_INSET,
            right - COL_DATE - COL_AUTHOR + COL_INSET,
            right - COL_DATE + COL_INSET,
        ]
    }

    /// 一块文字的**绝对**矩形：`t.pos` 是偏移，`galley.rect` 是它自己的局部矩形。
    /// 判据要的是「这一格的左缘在哪」，那正是 `rect.min.x`。
    fn text_rect(out: &egui::FullOutput, label: &str, y: std::ops::RangeInclusive<f32>) -> Rect {
        all_text_rects(out, y.clone(), &[label])
            .into_iter()
            .next()
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("y {y:?} 这条带子里没有以「{label}」开头的文字"))
    }

    /// 同 [`text_rect`]，但找不到返回 `None` 而不是 panic——「不许出现」类的判据
    /// 要的正是不存在的证据。
    fn text_rect_or_none(
        out: &egui::FullOutput,
        label: &str,
        y: std::ops::RangeInclusive<f32>,
    ) -> Option<Rect> {
        all_text_rects(out, y, &[label]).into_iter().next().map(|(_, r)| r)
    }

    /// 某条 y 带子里所有以 `prefixes` 之一开头的文字，按绘制顺序返回（文字, 绝对矩形）。
    /// **注意**：这里的矩形**没有**裁到 `clip_rect`——ScrollArea 把滚出界外的行剪掉但形状照发，
    /// 所以这一族能看到「屏幕上一个像素都没有」的文字。问「看不看得见」用 `visible_texts`。
    fn all_text_rects(
        out: &egui::FullOutput,
        y: std::ops::RangeInclusive<f32>,
        prefixes: &[&str],
    ) -> Vec<(String, Rect)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let text = t.galley.text();
                    if !prefixes.iter().any(|p| text.starts_with(p)) {
                        return None;
                    }
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    (r.center().y >= *y.start() && r.center().y <= *y.end()).then(|| (text.to_owned(), r))
                }
                _ => None,
            })
            .collect()
    }

    /// **裁剪后还看得见**的文字矩形（文字, 绝对矩形），整帧。
    /// 求交叠、找「压住」必须用它：拿未裁剪的矩形扫一整帧，会把滚一半出界的文件行
    /// 当成压在提交框上（这一批第一版就是这么误判的）。完全在界外的条目直接丢掉。
    fn visible_texts(out: &egui::FullOutput) -> Vec<(String, Rect)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2()).intersect(c.clip_rect);
                    (!r.is_negative()).then(|| (t.galley.text().to_owned(), r))
                }
                _ => None,
            })
            .collect()
    }

    /// 有多少块文字被 `clip_rect` **切掉了一部分**（裁剪后的高比原 galley 少 1pt 以上）。
    /// 用来证明「这帧确实溢出了」——不然裁剪后的求交可能只是在比一堆屏幕外的矩形。
    fn cut_texts(out: &egui::FullOutput) -> usize {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((t.galley.rect.translate(t.pos.to_vec2()), c.clip_rect)),
                _ => None,
            })
            .filter(|(r, clip)| {
                let v = r.intersect(*clip);
                // 只算**切掉一部分**的：整块都在界外的（交集是负矩形）不算溢出，
                // 那种条目 egui 直接把行裁掉了，用它当「这帧有内容放不下」的证据是假的。
                !v.is_negative() && v.height() < r.height() - 1.0
            })
            .count()
    }

    /// 找一块四边都落在 `want` ±1.5pt 内的矩形，返回它的底色。
    /// 容差 1.5 是给 egui 那条 1px 分隔线的：带与带之间画线，底色矩形让出这一像素。
    /// 失败时把整帧的矩形打出来——第一次跑挂的时候这就是现场。
    fn band(rects: &[(Color32, Rect)], want: Rect) -> Color32 {
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        match rects.iter().find(|(_, r)| {
            near(r.min.x, want.min.x) && near(r.min.y, want.min.y) && near(r.max.x, want.max.x) && near(r.max.y, want.max.y)
        }) {
            Some((color, _)) => *color,
            None => panic!("没有这块带 {want:?}\n画出来的：{rects:?}"),
        }
    }

    #[test]
    fn bands_match_the_measured_reference() {
        let r = rects(&frame(&mut blank()));
        let bottom = WIN_H - STATUS_H;
        let staging_top = bottom - STAGING_H;
        // 工具栏：横贯顶部的一条 55pt。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, 0.0), pos2(WIN_W, TOOLBAR_H))), TOOLBAR_BG, "工具栏带");
        // tab 条：紧贴工具栏下面那条 24pt，把书签摊成一排。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, TOOLBAR_H), pos2(WIN_W, TOP_H))), TOOLBAR_BG, "tab 条带");
        // 侧栏：从两条顶带下沿一直贯穿到状态栏顶。SourceTree 的左栏是全高的，
        // 这一条也正是 2026-10-15「双左栏合一」被推翻的落点。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, TOP_H), pos2(SIDEBAR_W, bottom))), SIDEBAR_BG, "侧栏带");
        // 暂存带：侧栏右边、贴着状态栏上沿的一条 245pt；带内再切 435pt 文件表 + 差异区。
        assert_eq!(band(&r, Rect::from_min_max(pos2(SIDEBAR_W, staging_top), pos2(WIN_W, bottom))), Color32::WHITE, "暂存带");
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(SIDEBAR_W, staging_top), pos2(SIDEBAR_W + FILES_W, bottom))),
            Color32::WHITE,
            "文件表"
        );
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(SIDEBAR_W + FILES_W, staging_top), pos2(WIN_W, bottom))),
            Color32::WHITE,
            "差异区"
        );
        // 状态栏：贴着窗底的 26pt 灰条，暂存带和侧栏都为它让了位。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, bottom), pos2(WIN_W, WIN_H))), TOOLBAR_BG, "状态栏带");
        // 日志带 = 谁都没要走的那块。
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(SIDEBAR_W, TOP_H), pos2(WIN_W, staging_top))),
            Color32::WHITE,
            "日志带"
        );
    }

    /// 反向变异的靶子：把任何一个面板从 `shell` 里摘掉，这条就该红。
    #[test]
    fn every_band_is_painted() {
        let r = rects(&frame(&mut blank()));
        assert!(r.len() > 4, "整帧只有 {} 个色块，多半是某个面板没跑", r.len());
    }

    /// 侧栏里的一块文字。`text_rect` 只按 y 带筛，而「工作区」「搜索」「main」这些词
    /// 在工具栏和暂存带里也有，所以这里两维都筛：工具栏下面、左缘落在侧栏里的才算。
    fn side_text(out: &egui::FullOutput, label: &str) -> Rect {
        // 上界让到状态栏顶：状态栏左边也写着「main ↑2」，按绘制顺序排在侧栏**之前**，
        // 不卡 y 就会先撞上它（那行贴着窗底，不属于侧栏）。
        all_text_rects(out, TOP_H..=(WIN_H - STATUS_H), &[label])
            .into_iter()
            .filter(|(_, r)| r.min.x < SIDEBAR_W)
            .map(|(_, r)| r)
            .next()
            .unwrap_or_else(|| panic!("侧栏里没有「{label}」"))
    }

    /// 图标格（参考图 x 18..38）里有没有墨。圆、线段、方框三种都算——侧栏图标全是线描。
    fn icon_ink(out: &egui::FullOutput, cy: f32) -> usize {
        let (l, r) = (SIDE_ICON_X - 10.0, SIDE_ICON_X + 10.0);
        let hit = |x0: f32, x1: f32, y0: f32, y1: f32| x0 >= l - 0.01 && x1 <= r + 0.01 && y0 >= cy - 12.0 && y1 <= cy + 12.0;
        out.shapes
            .iter()
            .filter(|c| match &c.shape {
                egui::Shape::Circle(d) => hit(d.center.x - d.radius, d.center.x + d.radius, d.center.y - d.radius, d.center.y + d.radius),
                egui::Shape::LineSegment { points, .. } => hit(points[0].x.min(points[1].x), points[0].x.max(points[1].x), points[0].y.min(points[1].y), points[0].y.max(points[1].y)),
                egui::Shape::Rect(q) => hit(q.rect.min.x, q.rect.max.x, q.rect.min.y, q.rect.max.y),
                _ => false,
            })
            .count()
    }

    /// 侧栏常量表：这七个数是这一批从参考图上量的**唯一**落点。布局判据全比常量，
    /// 所以常量本身必须由这条钉住——上一批 M2 抓到过「常量和自己比」：改常量什么都不红。
    #[test]
    fn the_sidebar_constants_are_the_measured_reference_numbers() {
        assert_eq!((SIDE_TEXT_X, SIDE_ICON_X), (41.0, 28.0), "文字列 x=41、图标格 18..38 的中心 28");
        assert_eq!((SIDE_CAT_H, SIDE_HEAD_H), (47.0, 30.0), "分类行 47pt、「工作区」那条 30pt");
        assert_eq!((SIDE_PILL_L, SIDE_PILL_R), (10.0, 129.5), "药丸 x 10..129.5");
        assert_eq!((SIDE_DOT_X, SIDE_DOT_R), (29.5, 3.5), "蓝点圆心 29.5、直径 7");
        // 颜色也得钉：M5 把蓝点改成纯黑，**一条都没红**——判据按常量筛圆，常量改了筛子跟着改。
        assert_eq!(
            (SIDE_DOT_C, SIDE_PILL_BG, SIDE_ICON_C, SIDE_LABEL_C, SIDE_INK),
            (
                Color32::from_rgb(36, 93, 184),
                Color32::from_rgb(204, 208, 205),
                Color32::from_rgb(69, 81, 108),
                Color32::from_rgb(164, 167, 165),
                Color32::from_rgb(61, 62, 61),
            ),
            "侧栏五档颜色不是参考图量到的那五个值"
        );
    }

    /// 2026-10-20 这批新画的几格（差异带头、状态色块、tab 条）的数比**字面值**，理由和上一条
    /// 一样：几何判据全比常量，常量自己没被钉过就是「常量和自己比」。
    /// 上面每个数的出处写在常量注释里（哪张图、哪一段 y/x 范围）。
    /// `TAB_H` 不在这里——SourceTree 把书签放在**另一个窗口**，这 24pt 没有参考图可对，
    /// 它是用户诉求（tab 展示仓库列表）落地时定的，跟着 `ROW_H` 那类自定尺寸走。
    #[test]
    fn the_diff_header_constants_are_the_measured_reference_numbers() {
        assert_eq!(DIFF_HEAD_H, 30.0, "差异区那格文件标题带：两张参考图都是 30pt");
        assert_eq!(DIFF_HEAD_BG, Color32::from_rgb(236, 236, 236), "同一条带的底色");
        assert_eq!(DIFF_HEAD_PAD, 8.0, "带里色块离左缘的距离");
        assert_eq!(BADGE_S, 14.0, "状态色块 14×14");
        assert_eq!(
            (BADGE_MOD, BADGE_REN, BADGE_UNK),
            (
                Color32::from_rgb(238, 197, 91),
                Color32::from_rgb(163, 115, 122),
                Color32::from_rgb(101, 73, 129),
            ),
            "三枚状态色块的颜色不是参考图量到的那三个"
        );
    }

    /// 侧栏九条标签**全部**起在参考图量到的 x=41：分类不缩进、子项也不缩进，
    /// 图标和圆点住在 18..38 那一格。我们原来是「标题 5、子项 20」两档缩进。
    #[test]
    fn the_sidebar_labels_all_start_at_the_measured_column() {
        let out = frame(&mut loaded());
        for label in ["工作区", "文件状态", "历史", "搜索", "分支", "main", "标签", "远端", "已贮藏"] {
            assert_eq!(side_text(&out, label).min.x, SIDE_TEXT_X, "「{label}」没起在文字列上");
        }
    }

    /// 分类行 47pt 一条；两个分类之间夹着一条子项时，差的正是 47 + 26。
    /// 这里**抄的是实测字面量**而不是常量：上一批 M2 抓到过「拿常量和自己比」——
    /// 把 47 改成 26，比常量的那条照样绿，只有钉死数字的才会红。
    #[test]
    fn the_sidebar_category_rows_step_by_47pt() {
        let out = frame(&mut loaded());
        let c = |l: &str| side_text(&out, l).center().y;
        let near = |a: f32, b: f32| (a - b).abs() < 0.6;
        assert!(near(c("远端") - c("标签"), 47.0), "标签→远端 = {}，参考图量的是 47", c("远端") - c("标签"));
        assert!(near(c("已贮藏") - c("远端"), 47.0), "远端→已贮藏 = {}，参考图量的是 47", c("已贮藏") - c("远端"));
        assert!(near(c("标签") - c("分支"), 47.0 + 26.0), "分支→标签中间只有一条 main 子项，应该是 73：实际 {}", c("标签") - c("分支"));
    }

    /// 选中那一条铺的灰色药丸：参考图 x 10..129.5、高 = 行高 26。
    #[test]
    fn the_selected_sidebar_row_is_the_measured_pill() {
        let out = frame(&mut loaded());
        let (_, rect, radius) = rounded(&out)
            .into_iter()
            .find(|(fill, _, _)| *fill == SIDE_PILL_BG)
            .expect("这一帧没有 SIDE_PILL_BG 的色块，侧栏药丸没画出来");
        assert_eq!(rect.min.x, SIDE_PILL_L, "药丸左缘不是参考图的 10");
        assert_eq!(rect.max.x, SIDE_PILL_R, "药丸右缘不是参考图的 129.5");
        assert_eq!(rect.height(), ROW_H, "药丸高不是行高");
        assert_ne!(radius, egui::CornerRadius::ZERO, "参考图那枚药丸是圆的，不是直角");
        let row = side_text(&out, "文件状态");
        assert!((rect.center().y - row.center().y).abs() < 0.6, "药丸没罩住选中的那一条");
    }

    /// 每个分类都带一枚图标，子项不带（当前分支那颗蓝点是圆点，不住在图标格里）。
    #[test]
    fn every_sidebar_category_has_an_icon() {
        let out = frame(&mut loaded());
        for label in ["工作区", "分支", "标签", "远端", "已贮藏"] {
            assert!(icon_ink(&out, side_text(&out, label).center().y) > 0, "「{label}」那一行的图标格 18..38 是空的");
        }
        assert_eq!(icon_ink(&out, side_text(&out, "搜索").center().y), 0, "子项行里冒出了图标");
    }

    /// 当前分支前面那颗点是**蓝**的，落在 x=29.5。原来我们画的是黑色「●」字符，跟着文字列走。
    #[test]
    fn the_current_branch_is_marked_with_the_measured_blue_dot() {
        let out = frame(&mut loaded());
        let cy = side_text(&out, "main").center().y;
        let dots: Vec<Pos2> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Circle(d) if d.fill == SIDE_DOT_C => Some(d.center),
                _ => None,
            })
            .collect();
        assert!(
            dots.iter().any(|p| (p.x - SIDE_DOT_X).abs() < 0.01 && (p.y - cy).abs() < 0.6),
            "当前分支前面没有落在 x={SIDE_DOT_X} 的蓝点：{dots:?}"
        );
    }

    /// 提示（`note`）现在是底部 toast：悬在状态栏上方、水平居中、8 秒自己消失。
    /// 以前它挤在 tab 条右端的 240pt 定格里，长提示被截断（判据随之改写）。
    #[test]
    fn the_note_shows_as_a_toast_above_the_status_bar() {
        let mut app = loaded();
        app.note = Some("egui 版还没接：搜索面板（SourceTree 是带作用域条的独立面板）".to_owned());
        // toast 挂在 `Order::Foreground`，而全新 Context 的第一遍不画那一层（见 `warm`）；
        // 另外 Area 要几帧才把尺寸/位置收敛（第一遍只量尺寸，落位在下一帧），无头里时间
        // 不走，收敛只能靠多跑几遍。
        let ctx = warm(&mut app);
        for _ in 0..3 {
            let _ = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        }
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        out.textures_delta.clear();
        // 卡片底缘贴着「状态栏顶 - 8pt」那条线，水平居中。找圆角矩形：内容区那些白带
        // 底缘都不到这里，只有 toast 悬在这个高度。（底色带 Area 的淡入透明度，
        // 无头时间停在 0 永远到不了纯白，所以只认位置和宽度。）
        let want_bottom = WIN_H - STATUS_H - 8.0;
        let all = rounded(&out);
        let card = all
            .iter()
            .find(|(_, r, _)| {
                (r.max.y - want_bottom).abs() < 1.5
                    && (r.center().x - WIN_W / 2.0).abs() < 2.0
                    && r.width() > 100.0
            })
            .unwrap_or_else(|| panic!("状态栏上方没有 toast 卡片：{all:#?}"));
        assert!(card.1.width() <= 544.0, "toast 卡片超宽：{}", card.1.width());
        // 文字真的在卡片里。
        let t = text_rect(&out, "egui 版还没接", (want_bottom - 40.0)..=want_bottom);
        assert!(card.1.contains(t.center()), "toast 文字跑出了卡片 {t:?}");
    }

    /// toast 的三段生命周期：出现 → 点 × 立刻关 → 不点也活不过 8 秒。
    #[test]
    fn the_toast_closes_on_click_and_fades_after_eight_seconds() {
        let mut app = loaded();
        app.note = Some("egui 版还没接：搜索面板".to_owned());
        // 点击 × 的位置：卡片右端那枚 20×20。先量出来再按下抬起。
        // 量卡片和点卡片必须共用同一个 Context：toast 在 Foreground 层，换 Context 就等于
        // 那一层从没画过（见 `warm`），点击会落到空气里。Area 还要几帧才落位。
        let ctx = warm(&mut app);
        for _ in 0..3 {
            let _ = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        }
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        out.textures_delta.clear();
        let want_bottom = WIN_H - STATUS_H - 8.0;
        let card = rounded(&out)
            .iter()
            .find(|(_, r, _)| {
                (r.max.y - want_bottom).abs() < 1.5
                    && r.center().x > WIN_W / 2.0 - 200.0
                    && r.width() > 100.0
            })
            .expect("toast 没出现")
            .1;
        let close_at = pos2(card.max.x - 16.0, card.center().y);
        click_at(&ctx, &mut app, close_at);
        assert!(app.note.is_none(), "点了 × 提示还在");
        // 再来一条，不点它：8 秒后必须自己消失。
        app.note = Some("第二条提示".to_owned());
        app.note_at = None;
        let _ = frame_at_time(&mut app, 0.0);
        assert!(app.note.is_some(), "toast 没出现");
        let _ = frame_at_time(&mut app, 8.5);
        assert!(app.note.is_none(), "8 秒了 toast 还赖着");
    }

    /// 状态栏贴着窗底横贯整窗；暂存带、文件表、日志带的整体底边都因为让出这 26pt
    /// 而上移。这条把「谁在窗底」钉死。
    #[test]
    fn the_status_bar_bands_the_bottom_edge() {
        // 常量自己先钉住：26 是行节奏（= ROW_H）挑的、不是量出来的，哪天有了带状态栏的
        // 参考图，改这里 + theme 注释，别让「量不到」滑成「随手改」。
        assert_eq!(STATUS_H, 26.0, "状态栏高度：参考图裁掉了，按行节奏定的 26");
        let out = frame(&mut blank());
        let bottom = WIN_H - STATUS_H;
        assert_eq!(
            band(&rects(&out), Rect::from_min_max(pos2(0.0, bottom), pos2(WIN_W, WIN_H))),
            TOOLBAR_BG,
            "状态栏带"
        );
        let staging_top = bottom - STAGING_H;
        assert_eq!(
            band(&rects(&out), Rect::from_min_max(pos2(SIDEBAR_W, staging_top), pos2(WIN_W, bottom))),
            Color32::WHITE,
            "暂存带底边让出了状态栏"
        );
    }

    /// 状态栏的内容：分支名 + 有上游才有的 ↑↓ + 仓库路径。
    /// 契约第②条变体：**没有上游时绝不出现 ↑0 ↓0**——比「不显示全零」更严，
    /// 连个位数都不许冒出来骗人。
    #[test]
    fn the_status_bar_names_the_branch_and_counts() {
        let out = frame(&mut loaded());
        let band = (WIN_H - STATUS_H)..=WIN_H;
        let branch = text_rect(&out, "main", band.clone());
        assert!(branch.min.x < SIDEBAR_W + 20.0, "分支名没贴左：{}", branch.min.x);
        let up = text_rect(&out, "↑2", band.clone());
        assert!(up.min.x > branch.max.x, "↑2 没跟在分支名后面");
        assert!(text_rect_or_none(&out, "↓0", band.clone()).is_none(), "落后 0 不许显示 ↓0");
        assert!(text_rect_or_none(&out, "↑0", band).is_none(), "超前 0 不许显示 ↑0");
    }

    /// 差异区顶上那格：参考图是一条 `(236,236,236)` 的灰带，里面「状态色块 + 加粗、靠左的文件名」。
    /// 「靠左」是用户 2026-10-20 特别点过的一条，所以这条问的不是「有没有那块文字」，
    /// 而是**文字左缘贴没贴着色块右缘**——把名字改回居中，这条必红。
    #[test]
    fn the_diff_header_is_a_grey_band_with_the_status_badge() {
        let mut app = loaded();
        app.file = Some("src/loose.rs".to_owned());
        app.patch_label = "未暂存 · src/loose.rs".to_owned();
        let out = frame(&mut app);
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        let (left, top) = (SIDEBAR_W + FILES_W, WIN_H - STATUS_H - STAGING_H);
        let head = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == DIFF_HEAD_BG)
            .map(|(_, r)| r)
            .find(|r| near(r.min.x, left) && near(r.max.x, WIN_W) && r.height() == DIFF_HEAD_H && near(r.min.y, top + 2.0))
            .unwrap_or_else(|| panic!("差异区顶上没那条灰带：左 {} 右 {} 顶 {} 高 {DIFF_HEAD_H}", left, WIN_W, top + 2.0));
        let badge = rounded(&out)
            .into_iter()
            .filter(|(c, _, _)| *c == BADGE_MOD)
            .map(|(_, r, _)| r)
            .find(|r| head.contains_rect(*r) && near(r.width(), BADGE_S) && near(r.height(), BADGE_S))
            .expect("灰带里没有那枚状态色块（14×14，琥珀底）");
        assert!(
            near(badge.min.x - head.min.x, DIFF_HEAD_PAD),
            "色块离带左缘 {}pt，参考图两张都是 {DIFF_HEAD_PAD}",
            badge.min.x - head.min.x
        );
        let label = text_rect(&out, "未暂存 · src/loose.rs", head.min.y..=head.max.y);
        assert!(
            label.min.x - badge.max.x < 12.0,
            "文件名没贴着色块：左缘 {}，色块右缘 {}",
            label.min.x,
            badge.max.x
        );
        assert!(label.max.x <= head.max.x + 0.01, "文件名溢出灰带：右缘 {}", label.max.x);
        assert_eq!(
            text_color(&out, "未暂存 · src/loose.rs", head.min.y..=head.max.y),
            INK_STRONG,
            "灰带里的文件名不是那近黑色"
        );
    }

    /// hunk 头在人眼里是「块 1： 行 8-11（新 8-12）」，不是 `@@ -8,4 +8,5 @@`。这条钉两件事：
    /// 整帧**没有** git 原始的那串（翻译只许写在 `layout` 一处，删掉它这条就红），
    /// 以及两侧区间各自按 `start,count` 算（旧 8,4 → 8-11，新 8,5 → 8-12；把 count 当末行号就是 8-4/8-5）。
    #[test]
    fn a_hunk_row_reads_as_a_block_number_and_line_range() {
        let t = texts(&frame(&mut loaded()));
        let hit = t.iter().find(|(s, _)| s.contains("块 1： 行 8-11（新 8-12）"));
        assert!(
            hit.is_some(),
            "hunk 行没翻译成「块 N： 行 A-B（新 C-D）」：{:?}",
            t.iter().map(|(s, _)| s).filter(|s| s.contains("行") || s.contains("@@")).collect::<Vec<_>>()
        );
        assert!(t.iter().all(|(s, _)| !s.contains("@@")), "帧里还留着原始的 @@ 头");
    }

    /// 差异区**一行 git 文件头都不许有**。参考图 `sourcetree-4.2.19-repo-window.png`：
    /// 文件名那条灰带 (236,236,236) 止于 y=368pt，y=368..396 直接就是 hunk 那条带
    /// (240,244,241) —— 中间一行 `diff --git`/`index`/`---`/`+++` 都没有。
    ///
    /// 反向变异：把 `layout` 里那句 `continue` 删掉。`+++ b/src/loose.rs` 首字节是 `+`、
    /// `--- a/…` 是 `-`，两条会被当成真的增/删行画出来（还带行号），第一行也就不是「块 1」了。
    /// 所以这条**不吃 `META` 那份名单**：把名单清空它照样红 —— 名单是筛的理由，落点是结果。
    ///
    /// 「commit / Author: / Date: / 提交说明」那四行是 `git show` 自己塞的，不是我们排的，
    /// 判据因此在核心层（`gitx::git` 的 `commit_patch_carries_no_log_header`，
    /// 用真临时仓库跑真 git）。
    #[test]
    fn the_diff_pane_starts_at_the_first_hunk() {
        let out = frame(&mut loaded());
        let top = WIN_H - STATUS_H - STAGING_H + DIFF_HEAD_H;
        let mut rows: Vec<(String, Rect)> = visible_texts(&out)
            .into_iter()
            // 空串是**行号那两格**为 hunk 行画的（那一行没有旧/新行号，画的是空文本），
            // 它和 hunk 文字同一条 y，不排掉就会顶到第一格上（实测 y 都是 334，空串先出）。
            // 会漏进正文的文件头每一行都带字（`--- a/…`、`+++ b/…`、`index …`），排掉空的不会放水。
            .filter(|(s, _)| !s.is_empty())
            .filter(|(_, r)| r.min.x > SIDEBAR_W + FILES_W && r.min.y >= top - 0.5)
            .collect();
        assert!(rows.len() > 4, "差异区这一帧没画出正文：{rows:?}");
        rows.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
        assert!(
            rows[0].0.contains("块 1"),
            "差异区第一行是 {:?}，不是 hunk 头 —— git 的文件头又画回来了",
            rows[0].0
        );
        let leaks: Vec<&String> = rows.iter().map(|(s, _)| s).filter(|s| s.starts_with("+++ ") || s.starts_with("--- ") || s.starts_with("index ") || s.starts_with("diff ")).collect();
        assert!(leaks.is_empty(), "文件头混进正文：{leaks:?}");
    }

    /// 同一句话的另一半：**新旧等长时不许带「（新 …）」**。
    /// 参考图那句就是光秃秃的「块 1： 行 569-575」，多带一段就成了我们自己的排版。
    /// 只测不等长那条的话，把 `if` 写成永远走 else 也照样绿——所以两条都得在。
    #[test]
    fn an_equal_length_hunk_reads_as_a_single_range() {
        // 与 `PATCH` 只差 hunk 头：删一行、加一行，新旧都是 4 行。
        const EVEN: &str = "--- a/src/loose.rs\n+++ b/src/loose.rs\n@@ -8,4 +8,4 @@\n 上下文甲\n-删掉的一行\n+加上的一行\n 上下文乙\n 上下文丙\n";
        let t = texts(&frame(&mut filled(3, &layout(EVEN))));
        let hit = t.iter().find(|(s, _)| s.contains("块 1： 行 8-11"));
        assert!(hit.is_some(), "等长的块没照参考图写成单区间：{:?}", t.iter().map(|(s, _)| s).filter(|s| s.contains("行")).collect::<Vec<_>>());
        let said = &hit.unwrap().0;
        assert!(!said.contains("（新"), "等长还多带一句新行号：{said}");
    }

    /// hunk 头那一行的右端钉着两枚小按钮（参考图 x 977..1032、1041..1096，带右缘 1105）。
    /// 断的是**尺寸与落点**，不只是「字在不在」：这两枚是差异区唯一的写操作入口，
    /// 排歪一点就压到正文，或者干脆掉到裁剪框外面（上一批的提交框就是这么丢的）。
    ///
    /// 这儿一律写**字面量**，不引用 `HUNK_BTN_*` 常量：引用常量就成了「常量比自己」——
    /// 把 `HUNK_BTN_W` 从 55 改成 60，画的那头和断的那头一起挪，判据永远绿。
    /// 第一版就是这么绿的（改宽试验没红），常量的出处交给
    /// `the_hunk_button_constants_are_the_measured_reference_numbers` 去钉。
    #[test]
    fn the_hunk_row_has_two_measured_buttons_at_the_right() {
        let mut app = loaded();
        app.target = Some(Target::Working { staged: false, path: Some("src/loose.rs".to_owned()), untracked: false });
        let out = frame(&mut app);
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        // 先找到那一行本身（底色带），再问带里的两枚按钮。
        let row = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == Color32::from_rgb(240, 244, 241))
            .map(|(_, r)| r)
            .find(|r| r.min.x > SIDEBAR_W + FILES_W - 1.0)
            .expect("差异区里没有 hunk 头那一条带");
        let btns: Vec<Rect> = rounded(&out)
            .into_iter()
            .filter(|(_, r, _)| row.contains_rect(*r) && near(r.height(), 15.0))
            .map(|(_, r, _)| r)
            .collect();
        assert_eq!(btns.len(), 2, "hunk 行右端该有两枚按钮，找到 {btns:?}");
        let (a, b) = (btns[0].min.x.min(btns[1].min.x), btns[0].max.x.max(btns[1].max.x));
        for r in &btns {
            assert!(near(r.width(), 55.0), "按钮宽 {}，参考图两枚都是 55", r.width());
        }
        assert!(near(b - a, 2.0 * 55.0 + 9.0), "两枚加中间间隔合起来 {}，参考图是 {}", b - a, 2.0 * 55.0 + 9.0);
        assert!(near(btns[0].max.x.max(btns[1].max.x), row.max.x - 9.0), "按钮没贴到带的右缘（差 9pt 那一格）");
        // 文案：未暂存那格是「暂存区块 / 放弃区块」。
        let t = texts(&out);
        for want in ["暂存区块", "放弃区块"] {
            let hit = t.iter().find(|(s, _)| s.contains(want)).unwrap_or_else(|| panic!("按钮上没有「{want}」：{:?}", t.iter().map(|(s, _)| s).collect::<Vec<_>>()));
            assert!(hit.1.y >= row.min.y - 1.0 && hit.1.y <= row.max.y, "「{want}」没落在 hunk 那一行里：{}", hit.1);
        }
    }

    /// 上面那条用的字面量，得有个地方钉住它们是参考图量到的数——否则改常量把画的那头挪走，
    /// 只有几何判据红，没人知道「本来该是多少」。
    #[test]
    fn the_hunk_button_constants_are_the_measured_reference_numbers() {
        assert_eq!((HUNK_BTN_W, HUNK_BTN_H), (55.0, 15.0), "参考图那两枚按钮 55×15");
        assert_eq!((HUNK_BTN_GAP, HUNK_BTN_EDGE), (9.0, 9.0), "两枚之间 9pt、离带右缘 9pt");
        assert_eq!(HUNK_BTN_STROKE, Color32::from_rgb(228, 232, 229), "1pt 描边");
        assert_eq!(HUNK_BTN_FG, Color32::from_rgb(109, 110, 109), "按钮上的灰字");
        assert_eq!(HUNK_BG, Color32::from_rgb(240, 244, 241), "hunk 头那条带的底色");
    }

    /// 已暂存那一格只有「取消暂存区块」，**没有**「放弃区块」：索引里的东西不会被这一格丢掉。
    /// 某条提交的差异两枚都不许出现（早就落库了，没有区块可动）。
    /// 问的是**整组文案**而不是「有没有某个词」——「取消暂存区块」里含着「暂存区块」，
    /// 逐词 `contains` 的写法会把这一条判据自己骗过去（第一版就是这么假绿了一次）。
    #[test]
    fn the_buttons_follow_which_pane_the_diff_is() {
        let labels = |app: &mut Gitx| {
            let mut v: Vec<String> = texts(&frame(app))
                .into_iter()
                .map(|(s, _)| s)
                .filter(|s| s.contains("区块"))
                .collect();
            v.sort();
            v
        };
        let unstaged = {
            let mut a = loaded();
            a.target = Some(Target::Working { staged: false, path: Some("src/loose.rs".to_owned()), untracked: false });
            labels(&mut a)
        };
        assert_eq!(unstaged, ["放弃区块", "暂存区块"], "未暂存那格该是这两枚，多一枚少一枚都算错");
        let staged = {
            let mut a = loaded();
            a.target = Some(Target::Working { staged: true, path: Some("src/loose.rs".to_owned()), untracked: false });
            labels(&mut a)
        };
        assert_eq!(staged, ["取消暂存区块"], "已暂存那格的按钮组不对");
        for (what, t) in [
            ("某条提交的差异", Target::Commit { oid: "0".repeat(40), path: Some("src/loose.rs".to_owned()) }),
            ("整份差异（没有单一文件）", Target::Working { staged: false, path: None, untracked: false }),
            ("未跟踪文件", Target::Working { staged: false, path: Some("src/loose.rs".to_owned()), untracked: true }),
        ] {
            let mut a = loaded();
            a.target = Some(t);
            assert!(labels(&mut a).is_empty(), "{what}里冒出了区块按钮：{:?}", labels(&mut a));
        }
    }

    /// 当前文件那一行铺一条灰带（参考图选中行是灰的，不是蓝的——焦点蓝留给目视那批）。
    /// 「没有当前文件时一帧都不该有这条带」也在同一条里问：不这么钉的话，
    /// 把 `current` 判断删掉——每行都涂灰——判据照样绿。
    #[test]
    fn the_current_file_row_gets_the_grey_band() {
        let none = rects(&frame(&mut loaded()));
        assert!(
            none.iter().all(|(c, _)| *c != PICK_BG),
            "没选中任何文件，文件表里却冒出了灰带"
        );

        let mut app = loaded();
        app.file = Some("src/loose.rs".to_owned());
        let out = frame(&mut app);
        let bands: Vec<Rect> = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == PICK_BG)
            .map(|(_, r)| r)
            .collect();
        assert_eq!(bands.len(), 1, "当前文件行该只铺一条灰带：{bands:?}");
        let band = bands[0];
        assert_eq!(band.height(), ROW_H, "灰带不是整整一行高");
        assert!(band.min.x >= SIDEBAR_W && band.max.x <= SIDEBAR_W + FILES_W, "灰带不在文件表那一栏里：{band:?}");
        let label = text_rect(&out, "src/loose.rs", (WIN_H - STATUS_H - STAGING_H)..=(WIN_H - STATUS_H));
        assert!((label.center().y - band.center().y).abs() < 1.0, "灰带没套在文件名那一行上：带 {:.1}、字 {:.1}", band.center().y, label.center().y);
        assert!(label.min.x >= band.min.x, "文件名跑到灰带左边去了");
    }

    /// 文件行那三列画在哪儿，断的是**画出来的位置**，字面量写死在这儿（引 `CHK_*`/`ICON_X`/
    /// `NAME_X` 就成了常量比自己，改常量两边一起挪、判据永不变红——上一批 `HUNK_BTN_W` 就是这么假绿的）。
    /// 25 / 47 / 68 是参考图实测（面板左缘 141 起算，方框 x 165、图标 x 188、名字 x 208.5），
    /// 出处与取整记在 `the_file_row_columns_are_the_reference_offsets`。
    ///
    /// 文件行**按文件名那一格找**，不按 x 找：没有方框那一栏的色块现在挪到了 +8，
    /// 和分类行的全选框同列，用 x 分列分不出谁是谁。
    #[test]
    fn the_file_row_columns_are_where_the_reference_puts_them() {
        let out = frame(&mut loaded());
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        let squares: Vec<(Color32, Rect)> = rounded(&out)
            .into_iter()
            .filter(|(_, r, _)| {
                (r.width() - r.height()).abs() < 1.0
                    && r.width() > 13.0
                    && r.width() < 15.0
                    && r.min.x > SIDEBAR_W
                    && r.min.x < SIDEBAR_W + FILES_W
            })
            .map(|(c, r, _)| (c, r))
            .collect();
        let mut blanks = 0usize;
        for path in ["src/staged.rs", "src/loose.rs", "notes.md"] {
            let name = text_rect(&out, path, (WIN_H - STATUS_H - STAGING_H)..=(WIN_H - STATUS_H));
            let row: Vec<&(Color32, Rect)> = squares
                .iter()
                .filter(|(_, r)| (r.center().y - name.center().y).abs() < 1.0)
                .collect();
            assert!(!row.is_empty(), "「{path}」这一行左边没有方框：{squares:?}");
            // 这一行该有一枚方框 + 一枚状态图标，两枚都是 13.5 见方，所以只能按 x 分档问。
            let at = |x: f32| row.iter().any(|(_, r)| near(r.min.x - SIDEBAR_W, x));
            assert!(at(25.0), "「{path}」的方框不在离面板左缘 25 这一档（参考图 x 165）：{row:?}");
            assert!(at(47.0), "「{path}」的状态图标不在离面板左缘 47 这一档（参考图 x 188）：{row:?}");
            assert_eq!(row.len(), 2, "「{path}」一行画了 {} 枚方框大小的东西，应该是方框 + 图标两枚", row.len());
            assert!(
                near(name.min.x - SIDEBAR_W, 68.0),
                "「{path}」离面板左缘 {}，参考图是 68（x 208.5 取整）",
                name.min.x - SIDEBAR_W
            );
            if row.iter().any(|(c, _)| *c == Color32::WHITE) {
                blanks += 1;
            }
        }
        // 未勾选的两枚（未暂存 + 未跟踪）必须是白底，勾上的那枚必须是参考图那枚蓝。
        assert_eq!(blanks, 2, "未暂存与未跟踪那两行的方框该是白底：{squares:?}");
        let blues: Vec<&Color32> = squares.iter().map(|(c, _)| c).filter(|c| **c == CHK_BLUE).collect();
        assert_eq!(blues.len(), 2, "两枚勾上（已暂存分类标题 + src/staged.rs）：找到 {blues:?}");
        assert_eq!(blues[0], &Color32::from_rgb(71, 141, 247), "勾上的方框不是参考图那枚蓝");
    }

    /// 分类标题那一行的方框：钉「标题行有一枚、离面板左缘 8、文字在它右边 7pt」。
    /// 按**标题文字那一格的 y** 去认（文件行的方框在 +25，本可用 x 分列，
    /// 但按行认对布局改动更钝感）。把 `CHK_HEAD_X` 挪走只会红这一条，
    /// 把 `CHK_ROW_X`/`ICON_X`/`NAME_X` 挪走红的是上面那条。
    #[test]
    fn the_section_box_sits_in_its_own_column() {
        let out = frame(&mut loaded());
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        let squares: Vec<Rect> = rounded(&out)
            .into_iter()
            .map(|(_, r, _)| r)
            .filter(|r| near(r.width(), 13.5) && near(r.height(), 13.5) && r.min.x > SIDEBAR_W && r.min.x < SIDEBAR_W + FILES_W)
            .collect();
        for title in ["已暂存文件", "未暂存文件"] {
            let t = text_rect(&out, title, (WIN_H - STATUS_H - STAGING_H)..=(WIN_H - STATUS_H));
            let row: Vec<&Rect> = squares.iter().filter(|r| (r.center().y - t.center().y).abs() < 1.0).collect();
            assert_eq!(row.len(), 1, "「{title}」这一行找到 {} 枚方框，分类标题只该有一枚全选框", row.len());
            assert!(near(row[0].min.x - SIDEBAR_W, 8.0), "「{title}」的方框离面板左缘 {}，要的是 8（和文件行齐平）", row[0].min.x - SIDEBAR_W);
            assert!(t.min.x > row[0].max.x, "「{title}」的文字压到方框左边去了");
            assert!(near(t.min.x - row[0].max.x, 7.0), "标签离方框 {}，参考图是 7", t.min.x - row[0].max.x);
        }
    }

    /// 未勾选那几枚方框必须有**一圈描边**：白底铺在白面板上，没边框等于没画，用户无从知道能点。
    /// 上面两条只认底色（`rounded()` 按「底色非透明」筛），删掉 `check_box` 里 `rect_stroke`
    /// 那行它们照样绿，所以描边单独问一次。
    ///
    /// 这条钉的是「有没有」，钉不住颜色：变异实测把 `CHK_EDGE` 从 (190,190,190) 改成 (120,120,120)
    /// 全绿——`rects()`/`rounded()` 只看 `fill`，而两张参考截图里一枚未勾选的方框都没有，
    /// 这个灰是估的，等有真样本再钉数值。
    #[test]
    fn an_unchecked_box_has_a_border_and_a_checked_one_does_not() {
        let out = frame(&mut loaded());
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        let is_box = |r: &Rect| near(r.width(), CHK_S) && near(r.height(), CHK_S);
        let in_pane = |r: &Rect| r.min.x > SIDEBAR_W && r.min.x < SIDEBAR_W + FILES_W;
        let filled = |c: Color32| -> Vec<Rect> {
            rounded(&out)
                .into_iter()
                .filter(|(col, r, _)| *col == c && is_box(r) && in_pane(r))
                .map(|(_, r, _)| r)
                .collect()
        };
        let borders: Vec<Rect> =
            stroked(&out).into_iter().map(|(_, r)| r).filter(|r| is_box(r) && in_pane(r)).collect();
        let (white, blue) = (filled(Color32::WHITE), filled(CHK_BLUE));
        assert_eq!((white.len(), blue.len()), (3, 2), "这一帧该有三枚未勾选（未暂存分类标题 + 两行文件）和两枚勾上（已暂存分类标题 + 一行文件）的方框");
        for w in &white {
            assert!(
                borders.iter().any(|b| (b.center() - w.center()).length() < 1.0),
                "{w:?} 这枚白方框没有描边，铺在白面板上等于没画"
            );
        }
        for b in &blue {
            assert!(
                !borders.iter().any(|s| (s.center() - b.center()).length() < 1.0),
                "{b:?} 这枚勾上的方框是实心的，参考图没有边框"
            );
        }
    }

    /// 上面那条用的字面量，出处钉在这儿。参考图实测是 **8 / 24 / 47 / 67.5**
    /// （面板左缘 141 起算：方框 x 149、165，图标 x 188，名字 x 208.5），取整为 8 / 25 / 47 / 68。
    /// 中间有一批各减 8 又各减 9，这一批改回参考图（缘由见常量那条的注释）。
    /// 断言右侧写成 `24.0 + 1.0` 这样的**字面量算式**：把取整改成别的（比如 `- 1.0`）就会红，
    /// 而它不吃 `CHK_ROW_X` 本身。
    #[test]
    fn the_file_row_columns_are_the_reference_offsets() {
        assert_eq!(CHK_S, 13.5, "参考图三枚方框、两枚图标量到都是 13.5 见方");
        assert_eq!(CHK_HEAD_X, 8.0, "分类行的方框照参考图（x 149 → 离面板左缘 8）");
        assert_eq!((CHK_ROW_X, ICON_X, NAME_X), (24.0 + 1.0, 47.0, 67.5 + 0.5), "文件行三档 = 参考图取整（方框 165、图标 188、名字 208.5）");
        assert_eq!(CHK_BLUE, Color32::from_rgb(71, 141, 247), "勾上的方框中心色");
    }

    /// 某条提交的改动列表**没有方框**（早就落库了，勾不动索引），所以那一档不能照留空位。
    /// 用户 2026-10-21 的原话：「勾选的 box 没了，但是没有移动位置」。
    /// 断的是**画出来的位置**（28.5 是字面量，和分类标题那一列同一个数），不吃 `NO_BOX_*` 常量。
    /// 反向变异：把 `file_row` 里那句 `if check.is_some()` 的两个分支写成一样（= 不回收），
    /// 这一条两行都红；把色块那一档单独改回 `ICON_X`（只收文字）也只红这一条。
    #[test]
    fn a_commit_file_row_collapses_the_missing_box_column() {
        let mut app = commit_list();
        let out = frame(&mut app);
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        let band = (WIN_H - STATUS_H - STAGING_H)..=(WIN_H - STATUS_H);
        for path in ["src/changed.rs", "docs/added.md"] {
            let name = text_rect(&out, path, band.clone());
            assert!(
                near(name.min.x - SIDEBAR_W, 28.5),
                "「{path}」离面板左缘 {}，没有方框那一栏该收到 28.5（和分类标题同列）",
                name.min.x - SIDEBAR_W
            );
            // 这一行只该有一枚方块大的东西：**状态色块**，在方框那一档（+8）。
            let squares: Vec<(Color32, Rect)> = rounded(&out)
                .into_iter()
                .filter(|(_, r, _)| {
                    (r.width() - r.height()).abs() < 1.0
                        && r.width() > 13.0
                        && r.width() < 15.0
                        && r.min.x > SIDEBAR_W
                        && r.min.x < SIDEBAR_W + FILES_W
                        && (r.center().y - name.center().y).abs() < 1.0
                })
                .map(|(c, r, _)| (c, r))
                .collect();
            assert_eq!(squares.len(), 1, "「{path}」一行找到 {squares:?}，只该有一枚状态色块（方框没了就别再占一格）");
            let (fill, rect) = &squares[0];
            assert!(near(rect.min.x - SIDEBAR_W, 8.0), "状态色块离面板左缘 {}，方框收掉后它顶到 +8", rect.min.x - SIDEBAR_W);
            assert!(*fill != Color32::WHITE && *fill != CHK_BLUE, "{rect:?} 这枚是方框的配色，说明方框还画着");
        }
    }

    /// 窗口缩到**地板尺寸**时，不许有任何两块看得见的文字互相压着。尺寸从 `MIN_W`/`MIN_H`
    /// 起步：把这两个常量往下调，这一条立刻红（765 那一档实测「main ↑2 ↓0」压住 ⊕ 1.9pt）。
    /// 它钉的是「布局开始撞车的宽度」和「窗口允许的宽度」是同一个数，不是常量自己比大小。
    #[test]
    fn no_visible_text_overlaps_at_the_smallest_allowed_window() {
        let mut app = many_unstaged(24);
        // 先确认这一帧**真的溢出了**：地板尺寸上有滚出界外的文件行和差异行。没有的话，
        // 下面这个求交只是在比一堆根本不在屏幕上的矩形，绿得没有意义。
        let tight = frame_at(&mut app, MIN_W, MIN_H);
        assert!(cut_texts(&tight) > 0, "地板尺寸那一帧一块被剪掉的文字都没有：内容没溢出，探针失效");
        for (w, h) in [(MIN_W, MIN_H), (MIN_W, WIN_H), (WIN_W, WIN_H), (WIN_W, 900.0), (1400.0, 900.0)] {
            let out = frame_at(&mut app, w, h);
            let vis = visible_texts(&out);
            assert!(vis.len() > 40, "{w}×{h} 只认出 {} 块文字，这一帧大概什么都没画", vis.len());
            let mut bad = Vec::new();
            for (s, r) in &vis {
                if r.min.x < -0.5 || r.min.y < -0.5 || r.max.x > w + 0.5 || r.max.y > h + 0.5 {
                    bad.push(format!("出界 {s:?} {:?}", (r.min.x, r.min.y, r.max.x, r.max.y)));
                }
            }
            for i in 0..vis.len() {
                for j in (i + 1)..vis.len() {
                    let o = vis[i].1.intersect(vis[j].1);
                    // 交叠超过 1×3pt 才算压住：同一块文字的多 section 会自交几个像素。
                    if o.width() > 1.0 && o.height() > 3.0 {
                        bad.push(format!("{:?} 压住 {:?}（{:.1}×{:.1}pt）", vis[i].0, vis[j].0, o.width(), o.height()));
                    }
                }
            }
            assert!(bad.is_empty(), "{w}×{h}：\n{}", bad.join("\n"));
        }
    }

    /// 地板这条线必须真的交给窗口，不然上面那条只是在测一个没人用的常量。
    /// `viewport()` 拆成函数就是为了让判据读得到——无头画布管不了窗口约束。
    #[test]
    fn the_window_itself_is_floored_at_the_measured_size() {
        let v = viewport();
        assert_eq!(v.min_inner_size, Some(vec2(MIN_W, MIN_H)), "窗口的地板没交给窗口系统：{:?}", v.min_inner_size);
        assert_eq!(v.inner_size, Some(vec2(WIN_W, WIN_H)), "窗口不是按参考图的尺寸开的：{:?}", v.inner_size);
    }

    /// 参考图那一行 52pt 里同时有红绿灯、仓库名、11 枚按钮 —— 它**就是标题栏**。
    /// 所以窗口必须配成"内容铺到标题栏底下、系统标题不画"，否则我们是在标题栏下面
    /// 又画一条带，窗口顶上白占 28pt，而且仓库名会被红绿灯压住。
    /// 这三条开关在 `egui::ViewportBuilder` 上是可读字段（eframe 把它们映射给 winit 的
    /// macOS 扩展），所以这条判据读的是配置对象，不是帧。
    #[test]
    fn the_toolbar_band_is_the_window_titlebar() {
        let v = viewport();
        assert_eq!(v.titlebar_shown, Some(false), "标题栏还单独占一条：内容没铺到标题栏底下");
        assert_eq!(v.fullsize_content_view, Some(true), "内容没开 full-size：工具栏会在标题栏下面另起一行");
        assert_eq!(v.title_shown, Some(false), "系统标题还在画：会和我们左对齐的仓库名撞成两个标题");
    }

    /// 红绿灯由操作系统画在 x 20..72、y 20..32（两张参考图都是这个数），盖在我们内容上面。
    /// 所以那一行的左边 72pt 里**不许有任何文字**，仓库名必须从量出来的 91.5pt 起。
    #[test]
    fn nothing_is_drawn_under_the_traffic_lights() {
        // 带高单独钉一次：52 是两张参考图逐行扫底色扫出来的（y0..51 是 244,249,246，
        // y52 是 202,206,203 的分隔线）。不写这一句的话 `TOOLBAR_H` 改成多少都没人管 ——
        // 下面所有断言都拿它当尺子，那是**常量自比**，不算判据。
        assert_eq!(TOOLBAR_H, 52.0, "参考图标题栏那一条实测 52pt");
        let out = frame(&mut loaded());
        let vis = visible_texts(&out);
        assert!(vis.len() > 40, "这一帧大概什么都没画");
        let under: Vec<&String> = vis
            .iter()
            .filter(|(_, r)| r.min.y < TOOLBAR_H && r.min.x < TRAFFIC_RIGHT)
            .map(|(s, _)| s)
            .collect();
        assert!(under.is_empty(), "红绿灯底下有文字：{under:?}");
        let name = vis
            .iter()
            .find(|(s, _)| s.ends_with(" (Git)"))
            .expect("仓库名没画出来");
        assert!(
            (name.1.min.x - TITLE_LEFT).abs() < 1.0,
            "仓库名左沿 {:+.1}pt，量出来应该是 {TITLE_LEFT}pt",
            name.1.min.x
        );
    }

    /// 文件多到一屏装不下时，提交框**必须还在窗口里**。真窗口目视抓到过一次（2026-10-20）：
    /// 未暂存 7 个文件就把摘要行推到带的下沿，描述框和提交按钮整格掉到窗口外面——
    /// 因为列表不滚、提交框排在列表后面。修法就是「列表滚、两头钉」。
    #[test]
    fn the_commit_box_stays_inside_a_short_band() {
        let mut app = many_unstaged(24);
        let t = texts(&frame(&mut app));
        let has = |needle: &str| t.iter().any(|(s, _)| s.contains(needle));
        assert!(has("提交到"), "提交按钮掉到窗口外面（列表把提交框挤下去了）");
        assert!(has("扩展描述"), "描述框整个不见了");
        assert!(has("src/staged.rs"), "列表第一行都没画出来");
        assert!(!has("src/overflow/23.rs"), "第 24 行也画了出来：列表根本没滚");
    }

    /// 参考图的侧栏**没有**折叠三角，也**没有**「（3）」计数。这两样是凭 SourceTree 3 的印象加的。
    #[test]
    fn the_sidebar_has_no_counts_or_disclosure_triangles() {        let out = frame(&mut loaded());
        let t: Vec<String> = all_text_rects(&out, TOOLBAR_H..=WIN_H, &[""])
            .into_iter()
            .filter(|(_, r)| r.min.x < SIDEBAR_W)
            .map(|(s, _)| s)
            .collect();
        assert!(t.iter().all(|s| !s.contains('（')), "侧栏标题里还有计数：{t:?}");
        assert!(t.iter().all(|s| !s.contains('▼') && !s.contains('●')), "侧栏还在画三角/黑点：{t:?}");
    }

    /// 行距判据：`show_rows` 的周期 = row_height + item_spacing.y，所以 `style()` 把
    /// `item_spacing.y` 钉成 0 才有意义。这条把「有意义」变成断言——把 0 改成 4，它必红。
    #[test]
    fn log_rows_step_by_exactly_one_row_height() {
        let t = texts(&frame(&mut loaded()));
        // 只认描述列那几块：提交号、作者、日期也都是文字，它们和描述同一行、不同列。
        let subject = |s: &str| s.starts_with("主题") || s.starts_with("Uncommitted changes");
        let mut rows: Vec<(f32, String)> = t
            .iter()
            .filter(|(s, _)| subject(s))
            .map(|(s, p)| (p.y, s.clone()))
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(rows.len(), 4, "日志里应该出现虚拟行 + 三条提交：{rows:?}");
        // 参考图第 2 条：虚拟行常驻第一行。
        assert!(
            rows[0].1.starts_with("Uncommitted changes"),
            "第一行不是虚拟行，而是「{}」",
            rows[0].1
        );
        for w in rows.windows(2) {
            assert!(
                (w[1].0 - w[0].0 - ROW_H).abs() < 0.01,
                "相邻两行间距 {} 不是 ROW_H {ROW_H}",
                w[1].0 - w[0].0
            );
        }
    }

    /// 差异区：增删行整行铺底、行周期 = DIFF_ROW_H、并且只在差异区那块里铺。
    #[test]
    fn diff_rows_are_painted_row_by_row() {
        let out = frame(&mut loaded());
        let r = rects(&out);
        let top = WIN_H - STATUS_H - STAGING_H;
        let rows = |bg: Color32| {
            let mut ys: Vec<(f32, f32)> = r
                .iter()
                .filter(|(c, rc)| *c == bg && rc.min.x > SIDEBAR_W + FILES_W - 1.0 && rc.min.y > top)
                .map(|(_, rc)| (rc.min.y, rc.height()))
                .collect();
            ys.sort_by(|a, b| a.0.total_cmp(&b.0));
            ys
        };
        let (add, del) = (rows(ADD_BG), rows(DEL_BG));
        assert_eq!(add.len(), 2, "补丁里有两行新增，铺了几行：{add:?}");
        assert_eq!(del.len(), 1, "补丁里有一行删除：{del:?}");
        for w in add.windows(2) {
            assert!(
                (w[1].0 - w[0].0 - DIFF_ROW_H).abs() < 0.01,
                "差异行周期 {} 不是 {DIFF_ROW_H}，全部行：{add:?}",
                w[1].0 - w[0].0
            );
        }
        // 整行铺底：宽度必须吃掉差异区那一整块，只铺到文字末尾就不是 SourceTree 那个观感。
        let wide = r.iter().find(|(c, _)| *c == ADD_BG).map(|(_, rc)| *rc).expect("没有增行底色");
        assert!((wide.max.x - WIN_W).abs() < 1.5, "增行底色右边没收齐到窗口：{wide:?}");
    }

    /// 行距判据的**动态那一半**。`Ui::put` 会把父游标倒回行中间（见 `fix_pitch`），
    /// 倒回的量静态看只是 3.4pt；但 `ScrollArea::show_rows` 用 `cursor.min.y / row_height`
    /// 反解可见区间，游标每行少走 3.4pt，滚过一段之后**画的内容和画的位置就分家了**。
    /// 判据：滚两行（`2 * ROW_H`），前后两帧都在的那几行提交，落点必须也精确移动那么多。
    #[test]
    fn scrolling_moves_every_log_row_by_exactly_the_scrolled_pixels() {
        let scroll = 2.0 * ROW_H;
        let (before, after) = scroll_log(scroll);
        let after: std::collections::HashMap<String, f32> = after.into_iter().collect();
        let moved: Vec<(String, f32)> = before
            .iter()
            .filter_map(|(s, y)| after.get(s).map(|y2| (s.clone(), y2 - y)))
            .collect();
        assert!(
            moved.len() >= 3,
            "滚 {scroll}pt 后两帧只剩 {} 行可对照，滚动多半根本没生效：前 {} 行、后 {} 行",
            moved.len(),
            before.len(),
            after.len()
        );
        for (s, d) in &moved {
            assert!((d.abs() - scroll).abs() < 0.5, "滚 {scroll}pt，「{s}」移动了 {d}：内容和位置不同步");
        }
    }

    /// 文件表：三条文件一行不多一行不少，且「未跟踪」那条也在（它走的是另一条分支）。
    #[test]
    fn file_list_shows_every_working_change_once() {
        let t = texts(&frame(&mut loaded()));
        let top = WIN_H - STATUS_H - STAGING_H;
        // 只看文件表那一格：差异区的补丁头「+++ b/src/loose.rs」也以同样的路径结尾。
        let listed = |path: &str| {
            t.iter()
                .filter(|(s, p)| {
                    s.ends_with(path) && p.x > SIDEBAR_W && p.x < SIDEBAR_W + FILES_W && p.y > top
                })
                .count()
        };
        for path in ["src/staged.rs", "src/loose.rs", "notes.md"] {
            assert_eq!(listed(path), 1, "{path} 在文件表里不止出现一次");
        }
        // 分组标题自带条数（参考图第 5 条）。
        assert!(t.iter().any(|(s, _)| s.contains("已暂存文件（1）")), "没有「已暂存文件（1）」");
        assert!(t.iter().any(|(s, _)| s.contains("未暂存文件（2）")), "没有「未暂存文件（2）」");
    }

    /// 「比 SourceTree 快」这句口号里，目前唯一能被机器验证的那一半：一帧排多少行只跟窗口高度
    /// 有关，跟仓库多大无关。判据读 `ROWS_PAINTED`（行闭包真跑了几次），**不是**落图形状数——
    /// egui 在细分阶段按 clip_rect 剪掉屏幕外的行，「4000 条全排一遍」和「只排视口那 9 行」
    /// 在 `FullOutput` 里长得一模一样（实测都是 9 行），只有这个计数分得开。
    #[test]
    fn a_frame_paints_the_window_not_the_repo() {
        let rows = |app: &mut Gitx| {
            frame(app);
            rows_painted()
        };
        let patch = |n: usize| -> String {
            (0..n).map(|i| format!("+行{i}\n")).collect()
        };

        // 日志：差异区空着，计数就只剩日志行（外加那条常驻的「未提交」行）。
        let (log_few, log_many) = (rows(&mut filled(40, &[])), rows(&mut filled(4_000, &[])));
        assert!(log_few > 1 && log_few < 20, "日志排了 {log_few} 行：不是没数据就是没虚拟化");
        assert_eq!(log_few, log_many, "提交 40→4000，日志行数 {log_few}→{log_many}：虚拟化失效");

        // 差异区：只给一条提交，日志那边固定占两行，多出来的都是差异行。
        let (diff_few, diff_many) = (
            rows(&mut filled(1, &layout(&patch(400)))),
            rows(&mut filled(1, &layout(&patch(40_000)))),
        );
        assert!(diff_few > log_few, "差异区总共才排 {} 行，比日志还少：差异没进帧", diff_few);
        assert_eq!(diff_few, diff_many, "差异 400→40000 行，行数 {diff_few}→{diff_many}：虚拟化失效");
    }

    /// 任务 #9（真实数据通路）的证据：真的开一次仓库——就用这个仓库自己，走
    /// `open()` → 工作线程 `snapshot_of` → mpsc → 下一帧 `poll` 的**真通路**，不是假数据。
    /// 「异步」这件事是**结构**上断言的，不靠时序赌运气：`open()` 返回的那一刻 `busy` 已是 true、
    /// `snap` 还是 None（同步加载的话这里就有数据了）；之后每帧只做一次非阻塞 `try_recv`，
    /// 帧照样画得出来。判据：几十帧之内数据必须自己冒出来，且中间每一帧都不是空白。
    #[test]
    fn a_real_repo_arrives_through_the_worker_without_blocking_a_frame() {
        // 合并后 crate 根就是仓库根，真仓库就是本仓库自己。
        let repo = env!("CARGO_MANIFEST_DIR").to_owned();
        let mut app = blank();
        app.open(&repo);
        assert!(
            app.busy && app.snap.is_none(),
            "open() 刚返回就有 snapshot 了：那是同步加载，帧会被 git 卡住"
        );

        let mut frames = 0;
        while app.snap.is_none() && frames < 600 {
            let out = frame(&mut app);
            assert!(!out.shapes.is_empty(), "第 {frames} 帧一个形状都没有");
            frames += 1;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let Some(snap) = app.snap.as_ref() else {
            panic!(
                "600 帧（{}s）还没等到 snapshot。note = {:?}",
                frames, app.note
            );
        };
        assert!(!snap.status.branch.is_empty(), "真仓库的当前分支名是空的");
        assert!(!snap.commits.is_empty(), "真仓库一条提交都没有");
        assert!(!app.busy, "snapshot 都到了，busy 还挂着");
        // 图序列也必须是**这一次 snapshot 一起排好**送回来的，行数跟提交数一一对上。
        assert_eq!(snap.graph.len(), snap.commits.len(), "图序列和提交列表对不上行号");
        // 排布局的线程不是主线程：帧里一次都没排，图序列却已经画得出来了。
        assert_eq!(graph_layouts(), 0, "主线程排了 {} 次图序列：这笔开销又回到帧里了", graph_layouts());
    }

    // ------------------------------------------------------------ 图序列（提交 DAG）

    /// 图序列那一列里落出来的形状：`(圆点, 直线段, 弯线)`。
    /// 按**车道配色**认，不按坐标认：横坐标筛会把侧栏那条 140.5pt 的分隔线当成图元，
    /// 而「形状有没有越出这一列」恰恰是判据本身，先用坐标筛掉就等于把判据筛没了。
    fn graph_marks(out: &egui::FullOutput) -> (Vec<Pos2>, Vec<[Pos2; 2]>, Vec<[Pos2; 4]>) {
        let lane32 = |c: Color32| LANE_COLORS.contains(&c);
        let lane = |m: &egui::epaint::ColorMode| {
            matches!(m, egui::epaint::ColorMode::Solid(c) if lane32(*c))
        };
        let (mut dots, mut lines, mut arms) = (Vec::new(), Vec::new(), Vec::new());
        for c in &out.shapes {
            match &c.shape {
                egui::Shape::Circle(d) if lane32(d.fill) => dots.push(d.center),
                egui::Shape::LineSegment { points, stroke } if lane32(stroke.color) => lines.push(*points),
                egui::Shape::CubicBezier(b) if lane(&b.stroke.color) => arms.push(b.points),
                _ => {}
            }
        }
        (dots, lines, arms)
    }

    /// 把落图的圆点按行序读成车道号：最左边那颗算第 0 道，往后每 `LANE_W` 一条。
    /// 不写死绝对 x——那一列的左边距由面板 margin 决定，margin 一改就是一片假红；
    /// 但**相邻两道之差必须是整条车道**，这条用 0.6pt 的残差兜住。
    fn lanes_of(dots: &[Pos2]) -> Vec<usize> {
        let left = dots.iter().map(|d| d.x).fold(f32::INFINITY, f32::min);
        let mut by_row = dots.to_vec();
        by_row.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        by_row
            .iter()
            .map(|d| {
                let raw = (d.x - left) / LANE_W;
                assert!((raw - raw.round()).abs() < 0.6, "圆心落在 {raw} 道上，不是整条车道：{d:?}");
                raw.round() as usize
            })
            .collect()
    }

    /// 无头点一下写着 `label` 的那个控件。坐标不是猜的：从**这一帧真的画出来的文字**
    /// 反推它的矩形中心，控件挪了位置它也跟得上。按下、抬起分两帧喂，egui 才算一次点击。
    /// 返回 false 表示这一帧里没找到那块文字。
    /// 注意它只在**面板**那一层可靠：新 Context 的第一遍画不出模态卡，卡上的东西用 `click_warm`。
    fn click_text(app: &mut Gitx, label: &str) -> bool {
        let ctx = egui::Context::default();
        style(&ctx);
        let Some(at) = locate(&ctx, app, label) else { return false };
        click_at(&ctx, app, at);
        true
    }

    /// 方框上没有文字，`locate` 那一套按字定位的用不上：先按**同一行的文件名**找到 y，
    /// 再在那一行的 `col` 列上认那枚 13.5 的方框。名字在帧里出现好几处（差异区那格标题也写着
    /// 路径），所以逐个试、返回第一个真找到方框的 —— 只有文件表那一行旁边才画着方框。
    fn box_at(out: &egui::FullOutput, name: &str, col: f32) -> Option<Pos2> {
        let boxes = rounded(out);
        texts(out).into_iter().filter(|(s, _)| s.contains(name)).find_map(|(_, p)| {
            boxes.iter().find(|(c, r, _)| {
                (*c == Color32::WHITE || *c == CHK_BLUE)
                    && (r.width() - CHK_S).abs() < 1.0
                    && (r.min.x - (SIDEBAR_W + col)).abs() < 1.0
                    && (r.center().y - p.y - 6.0).abs() < 10.0
            })
            .map(|(_, r, _)| r.center())
        })
    }

    /// 无头点某一行的方框。**找不到就 panic，不返回 false** —— 静默的 false 会让端到端判据
    /// 在「方框根本没画出来」那种改坏下照样绿。
    fn click_box_in_row(app: &mut Gitx, name: &str, col: f32) {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let at = box_at(&out, name, col);
        // 和 `locate`/`frame` 同一套规矩：`FullOutput` 带着没交给 GPU 的纹理增量，
        // 不清掉就在 Drop 的 debug_assert 里炸。
        out.textures_delta.clear();
        let Some(at) = at else { panic!("「{name}」那一行 {col} 列上没有方框"); };
        click_at(&ctx, app, at);
    }

    fn click_file_box(app: &mut Gitx, name: &str) {
        click_box_in_row(app, name, CHK_ROW_X);
    }

    fn click_section_box(app: &mut Gitx, title: &str) {
        click_box_in_row(app, title, CHK_HEAD_X);
    }

    /// 一次完整点击的三帧。焦点记在 `Context` 的 memory 里，所以要留住同一个 ctx 让它跨帧活著
    /// ——换个 Context 就等于换了个窗口，前面那下点的焦点就没了（`type_into` 靠这一点）。
    fn click_at(ctx: &egui::Context, app: &mut Gitx, at: Pos2) {
        let btn = |pressed: bool| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(at)],
            vec![egui::Event::PointerMoved(at), btn(true)],
            vec![egui::Event::PointerMoved(at), btn(false)],
        ] {
            // 这三帧的输出没人读，但 `textures_delta` 还是得清：无头跑没有后端去应用它，
            // `FullOutput` 落栈会 panic。
            ctx.run_ui(raw_input(events), |ui| app.shell(ui)).textures_delta.clear();
        }
    }

    /// 滚一下、把鼠标按在日志某一行上、`hold` 秒后松手；`settle` 是滚动之后、按下之前空跑的帧数。
    /// 返回松手那刻选中了哪一条——`Sel::Uncommitted`（夹具的初始值）就等于「这一点击被吞了」。
    fn click_after_wheel(settle: usize, hold: f64) -> Sel {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut app = filled(200, &[]);
        let over = pos2((SIDEBAR_W + WIN_W) / 2.0, TOOLBAR_H + 120.0);
        let btn = |pressed: bool| egui::Event::PointerButton {
            pos: over,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -40.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        };
        let mut frames: Vec<(f64, Vec<egui::Event>)> = vec![
            (0.0, vec![egui::Event::PointerMoved(over)]),
            (1.0, vec![egui::Event::PointerMoved(over), wheel]),
        ];
        for i in 0..settle {
            frames.push((2.0 + i as f64 * 0.016, vec![egui::Event::PointerMoved(over)]));
        }
        frames.push((3.0, vec![egui::Event::PointerMoved(over), btn(true)]));
        frames.push((3.0 + hold, vec![egui::Event::PointerMoved(over), btn(false)]));
        for (t, events) in frames {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H))),
                    time: Some(t),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| app.shell(ui),
            )
            .textures_delta
            .clear();
        }
        app.sel
    }

    /// 滚完**下一帧**就点，点击不能丢。丢的原因见 `log_band` 里那段 `push_id`：
    /// egui 松开那一帧在注册表里找不到按下的 Id，就把 `potential_click_id` 清空
    /// （`interaction.rs:117-123`）——表现就是「滚完这一下点什么都点不中」。
    /// 把行 Id 换成按提交编号发之前，`settle = 0` 那一档是红的。
    #[test]
    fn a_click_on_the_frame_right_after_the_wheel_still_lands() {
        for settle in [0usize, 1, 2, 12] {
            assert!(
                matches!(click_after_wheel(settle, 0.016), Sel::Commit(_)),
                "滚完静置 {settle} 帧再点，什么都没选中——点击被丢了"
            );
        }
    }

    /// 按住一秒才松手，仍然算一次点击。锁的是 `style()` 里那条 `max_click_duration`：
    /// egui 默认 0.8 秒，超过就把这次按下当成拖拽，慢一点的点击全被吞掉。
    #[test]
    fn a_slow_click_is_still_a_click() {
        assert!(
            matches!(click_after_wheel(0, 1.0), Sel::Commit(_)),
            "按住 1 秒再松手，点击被当成拖拽丢了"
        );
    }

    /// 点在**行内那块文字上**必须还是选中这一行。egui 默认 `selectable_labels = true`
    /// （style.rs:1599），开着文字选择的 `Label` 会给自己叠一层 `Sense::click_and_drag()`
    /// （label.rs:154-170），它就压在整行那个 `Sense::click()` 矩形**上面**：按下被文字选择
    /// 抢走，行什么都收不到，光标还变成插入态（label_text_selection.rs:645）。
    /// 「鼠标移到描述上就点不中」是这一条，不是命中区有缝。日志行和侧栏行同一个病，
    /// 所以两处在一张判据里验。
    #[test]
    fn a_click_on_a_rows_text_selects_the_row() {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut app = loaded();
        let at = locate(&ctx, &mut app, "主题1").expect("描述列没画出来");
        click_at(&ctx, &mut app, at);
        assert!(app.sel == Sel::Commit(1), "点在描述文字上没选中那一行");
        let at = locate(&ctx, &mut app, "历史").expect("侧栏没有「历史」那一行");
        click_at(&ctx, &mut app, at);
        assert!(app.sel == Sel::Commit(0), "点在侧栏文字上没跳到第一条提交");
    }

    /// 文件行同理，而且更狠：那几行写的是 `Label::selectable(on)`，`on` 想表达
    /// 「这一行是当前看的文件」，但 egui 的 `selectable` 管的是**文字能不能拖选**——
    /// 传 false 时那块矩形只剩 `Sense::hover()`（label.rs:143-170），`clicked()` 根本不响。
    /// 于是「没选中的文件点不动」，而初始状态就是**所有**文件都没选中 = 整列点不动。
    #[test]
    fn a_click_on_an_unpicked_file_row_switches_to_it() {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut app = loaded();
        assert!(app.file.is_none(), "夹具一开始就选中了文件，这一格就没法验「从没选中到选中」");
        let at = locate_has(&ctx, &mut app, "notes.md").expect("文件表里没有 notes.md 那一行");
        click_at(&ctx, &mut app, at);
        assert_eq!(app.file.as_deref(), Some("notes.md"), "点未选中的文件行没反应");
    }

    /// 三条各管一件事：点数（没画多余的）、车道（没乱开新道）、行距（图元跟着行走）。
    /// 直排历史：一行一个点、全在第 0 道、点与点的行距正好 `ROW_H`。
    #[test]
    fn a_linear_history_keeps_one_dot_per_row_in_the_first_lane() {
        let out = frame(&mut loaded());
        let (dots, _, arms) = graph_marks(&out);
        assert_eq!(dots.len(), 3, "三条提交该有三个圆点，实际 {} 个：{dots:?}", dots.len());
        assert_eq!(lanes_of(&dots), vec![0, 0, 0], "直排历史不该开出第二条车道");
        assert!(arms.is_empty(), "直排历史没有合并，却画了 {} 条弯线", arms.len());
        let mut ys: Vec<f32> = dots.iter().map(|d| d.y).collect();
        ys.sort_by(|a, b| a.total_cmp(b));
        for w in ys.windows(2) {
            assert!(
                (w[1] - w[0] - ROW_H).abs() < 0.01,
                "相邻两个提交点差了 {}，图元没跟着行走",
                w[1] - w[0]
            );
        }
    }

    /// 车道分配的**语义**判据（直接读 `layout_graph`，不经过画）。这份形状要钉住三件事：
    /// 合并开新道；支线并回主线之后**它那条道腾出来**；尾巴上的空道裁掉。
    /// 旧 React 版的 `layoutGraph` 从不释放，两次合并就永久占三条道、列宽一路涨。
    #[test]
    fn a_rejoin_frees_the_lane_it_had_borrowed() {
        let rows = layout_graph(&branch_commits());
        let lanes: Vec<usize> = rows.iter().map(|r| r.lane).collect();
        assert_eq!(lanes, vec![0, 0, 0, 2, 1, 0, 0], "每行提交落在哪条车道：{rows:?}");
        let mut arms = Vec::new();
        for r in &rows {
            for s in &r.segs {
                if let Seg::Arm { from, to } = s {
                    arms.push((*from, *to));
                }
            }
        }
        assert_eq!(arms, vec![(0, 1), (0, 2)], "两次合并该留两条弯线");
        assert_eq!(
            rows[0].segs,
            vec![Seg::Tail(0), Seg::Arm { from: 0, to: 1 }],
            "顶上那条合并没开出新道：{rows:?}"
        );
        assert_eq!(
            rows[2].segs,
            vec![Seg::Stem(0), Seg::Through(1), Seg::Through(2), Seg::Tail(0)],
            "两条支线都得从主线这一行穿过去，一条都不能少"
        );
        // 最后两行（并回主线之后）只剩第 0 条道：上面那些道一个都不许还占着。
        assert_eq!(rows[5].segs, vec![Seg::Stem(0), Seg::Tail(0)], "并回那一行还占着别的道");
        assert_eq!(rows[6].segs, vec![Seg::Stem(0)], "根提交那行画了多余的东西");
        assert!(
            lanes.iter().max().map(|m| *m + 1).unwrap_or(0) <= 4,
            "用掉了 {} 条道，{COL_GRAPH}pt 只装得下 4 条",
            lanes.iter().max().unwrap_or(&0) + 1
        );
    }

    /// 整列都得待在那 49pt 里。圆点、竖线、弯线的端点一个都不许越界——越出去就是压到提交号那一列。
    #[test]
    fn the_graph_never_escapes_its_column() {
        // 窗口临时加高一格状态栏的高度：这条判的是图元越不越出列，不是一屏塞得下几条。
        let out = frame_at(&mut branched(), WIN_W, WIN_H + STATUS_H);
        let (dots, lines, arms) = graph_marks(&out);
        assert_eq!(dots.len(), 7, "七条提交该有七个点");
        assert_eq!(arms.len(), 2, "两次合并该有两条弯线");
        for x in dots
            .iter()
            .map(|d| d.x)
            .chain(lines.iter().flat_map(|p| [p[0].x, p[1].x]))
            .chain(arms.iter().flat_map(|p| [p[0].x, p[3].x]))
        {
            assert!(
                x > SIDEBAR_W && x < SIDEBAR_W + COL_GRAPH,
                "图元落在 x={x}，越出 {COL_GRAPH}pt 那一列"
            );
        }
    }

    /// 排一遍图序列是**整份提交列表**的遍历。放在帧里，500 条提交会把每帧都变成一次图算法，
    /// 所以它只在取数据那一步跑：造好数据 = 一次，之后五帧还是一次。
    #[test]
    fn the_graph_is_laid_out_once_per_snapshot_not_per_frame() {
        let before = graph_layouts();
        let mut app = loaded();
        assert_eq!(graph_layouts(), before + 1, "造快照时该排一次图序列");
        for _ in 0..5 {
            frame(&mut app);
        }
        assert_eq!(
            graph_layouts(),
            before + 1,
            "跑完五帧多排了 {} 次：图序列没跟着 snapshot 缓存",
            graph_layouts() - before - 1
        );
    }

    /// 筛过的日志**不画图序列**：行与行断了，竖线会连到一条没显示的提交上，看着像 bug。
    #[test]
    fn a_filtered_log_leaves_the_graph_column_empty() {
        let mut app = loaded();
        app.log_filter = "主题2".to_owned();
        let out = frame(&mut app);
        let (dots, lines, arms) = graph_marks(&out);
        assert!(
            dots.is_empty() && lines.is_empty() && arms.is_empty(),
            "筛过的日志里还画了 {} 个点、{} 条线、{} 条弯线",
            dots.len(),
            lines.len(),
            arms.len()
        );
        // 但文字照排——留空的是那一列，不是整块日志。
        assert!(
            texts(&out).iter().any(|(s, _)| s.starts_with("主题")),
            "筛完连提交行都没了"
        );
        // 虚拟行那枚**空心环**照画：它标的不是图序列，而是「这一行不是提交」。
        // 上面那条「一条线都没有」必须靠砍掉环下面那根线来满足，不能连环一起砍掉。
        assert_eq!(rings(&out).len(), 1, "筛过之后连虚拟行那枚环也没了");
    }

    /// 图元也必须按窗口排：提交 40 → 4000，一帧里的圆点数不许变。
    /// 这条专门补 `a_frame_paints_the_widget` 那个坑的反面——点数落在图上，所以数得出来。
    #[test]
    fn graph_marks_are_windowed_too() {
        let few = graph_marks(&frame(&mut filled(40, &[]))).0.len();
        let many = graph_marks(&frame(&mut filled(4000, &[]))).0.len();
        assert!(few > 1, "40 条提交只画出 {few} 个点，图序列根本没进帧");
        assert_eq!(few, many, "提交 40→4000，圆点 {few}→{many}：图序列没跟着虚拟化");
    }

    /// 带事件的帧：`frame()` 是它的空事件特例。键盘输入和点击都得走这条路才叫真输入。
    fn frame_events(app: &mut Gitx, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut out = ctx.run_ui(raw_input(events), |ui| app.shell(ui));
        out.textures_delta.clear();
        out
    }

    /// 同 `frame`，但把 egui 时钟拨到 `secs`——toast 的 8 秒超时判据靠它造「时间已经过去」。
    /// 每帧都是新 `Context`，时钟只来自 `RawInput::time`，state 里的 `note_at` 跨帧留存。
    fn frame_at_time(app: &mut Gitx, secs: f64) -> egui::FullOutput {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut input = raw_input(Vec::new());
        input.time = Some(secs);
        let mut out = ctx.run_ui(input, |ui| app.shell(ui));
        out.textures_delta.clear();
        out
    }

    fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
        raw_input_at(WIN_W, WIN_H, events)
    }

    fn frame_at(app: &mut Gitx, w: f32, h: f32) -> egui::FullOutput {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut out = ctx.run_ui(raw_input_at(w, h, Vec::new()), |ui| app.shell(ui));
        out.textures_delta.clear();
        out
    }

    fn raw_input_at(w: f32, h: f32, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))),
            // 写 true 只是让无头帧长得像真窗口。**实测把它改成 false，全套判据一条都不红**：
            // `TextEdit` 收不收文本看的是 memory 里的**控件焦点**（`filtered_events`），不是这个窗口位。
            // 记在这儿是因为反过来说「焦点靠它」会是假话。
            focused: true,
            events,
            ..Default::default()
        }
    }

    /// 找一块文字在窗口里的落点（按**前缀**匹配第一块）。
    fn locate(ctx: &egui::Context, app: &mut Gitx, label: &str) -> Option<Pos2> {
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let hit = out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with(label) => {
                // galley.rect 是文字自己的局部矩形，加上 text.pos 才落到窗口坐标里。
                Some(t.galley.rect.translate(t.pos.to_vec2()).center())
            }
            _ => None,
        });
        out.textures_delta.clear();
        hit
    }

    /// 同上，但按**包含**匹配。文件行前面还带着状态符号（`M  src/loose.rs`），
    /// 按前缀找就永远找不到。
    fn locate_has(ctx: &egui::Context, app: &mut Gitx, needle: &str) -> Option<Pos2> {
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let hit = out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text().contains(needle) => {
                Some(t.galley.rect.translate(t.pos.to_vec2()).center())
            }
            _ => None,
        });
        out.textures_delta.clear();
        hit
    }

    /// 一个**已经空跑过一遍**的 `Context`。egui 0.37 的坑（拿纯 egui 两三行复现过，与本项目无关）：
    /// 刚 `Context::default()` 出来的上下文，**第一遍不画 `Order::Foreground` 那一层**——同一遍里
    /// 面板的 `ui.label` 有形状，`Area::order(Foreground)` 里的 label 一个都没有；第二遍才出现。
    /// 不是「这个 Area 第一次出现」：换一个已经跑过很多遍的 Context，每遍都换全新 id 的卡照样画得出。
    /// 模态卡就画在那一层上，所以**凡是要在卡上定位文字的助手都得先空跑一遍**，否则是假红
    /// （「输入框没找到」）连带假绿（遮罩那条会以为是遮罩挡住了，其实那一遍卡根本没画）。
    fn warm(app: &mut Gitx) -> egui::Context {
        let ctx = egui::Context::default();
        style(&ctx);
        ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui))
            .textures_delta
            .clear();
        ctx
    }

    /// 在模态卡上点一项，或隔着卡点它后面那一行。必须走 `warm`，不能用 `click_text`。
    fn click_warm(app: &mut Gitx, label: &str) -> bool {
        let ctx = warm(app);
        let Some(at) = locate(&ctx, app, label) else { return false };
        click_at(&ctx, app, at);
        true
    }

    /// 卡**已经画得出来**的那一帧：先按 `warm` 空跑一遍，再取第二遍的形状。
    /// 直接拿 `frame()` 那一遍问模态卡的形状会问出「遮罩没画」这种假红——也包括假绿。
    fn warm_frame(app: &mut Gitx) -> egui::FullOutput {
        let ctx = warm(app);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        out.textures_delta.clear();
        out
    }

    /// 无头往输入框里敲字。焦点必须在**同一个 Context** 里拿到（见 `click_at`）。
    /// 喂的是 `Event::Text`：macOS 拼音输入法上屏那一刻交给控件的就是这个事件，
    /// 所以这条走的是真输入路径，不是绕过控件直接改字段。
    /// 卡里的输入框只有第二遍才画得出来，所以用 `warm` 起帧（见那儿那段实测）。
    fn type_into(app: &mut Gitx, hint: &str, text: &str) -> bool {
        let ctx = warm(app);
        let Some(at) = locate(&ctx, app, hint) else { return false };
        click_at(&ctx, app, at);
        // 点完那帧才 request_focus，所以文本得下一帧才喂。
        ctx.run_ui(
            raw_input(vec![egui::Event::PointerMoved(at), egui::Event::Text(text.to_owned())]),
            |ui| app.shell(ui),
        )
        .textures_delta
        .clear();
        true
    }

    /// ⌘/Ctrl+Enter。修饰键要**单独发一条 `ModifiersChanged`**——egui 的 `i.modifiers` 只由那条
    /// 事件更新，键盘事件自带的那份它不看（winit 也是这么发的：先修饰键、再按键）。
    fn press_mod_enter(app: &mut Gitx, m: egui::Modifiers) {
        let key = |pressed: bool| egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: m,
        };
        let mods = |e: egui::Modifiers| egui::Event::ModifiersChanged(e);
        frame_events(app, vec![mods(m), key(true)]).textures_delta.clear();
        frame_events(
            app,
            vec![key(false), mods(egui::Modifiers::NONE)],
        )
        .textures_delta
        .clear();
    }

    /// 起一个真 git 仓库：一次初始提交 + 一个**已暂存**的改动。
    /// 提交那条通路（点按钮 → core `git::commit` → 重新取快照）只有真仓库能验，
    /// 假 `Snapshot` 里的 `commit` 是会失败的。目录跑完删；断言挂了就留着看。
    /// `tag` 必须**每条判据一个**：目录名只有 pid + tag，并行跑的两条判据撞同一个 tag 会
    /// 互相 `remove_dir_all`，另一边正跑一半的 `git init` 就报「unable to get current working directory」。
    fn scratch_repo(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gitx-scratch-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("造临时目录");
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("跑 git");
            assert!(
                out.status.success(),
                "git {args:?} 失败：{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t.com"]);
        git(&["config", "user.name", "tester"]);
        std::fs::write(dir.join("a.txt"), "一行\n").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "起点"]);
        // 再来一处改动并 `git add`，这样壳一打开就有「已暂存文件（1）」可提交。
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n").unwrap();
        git(&["add", "a.txt"]);
        dir
    }

    /// 一帧一帧等到 `done` 成立；600 帧（约 3s）不够就带着 `note` 一起 panic。
    fn pump(app: &mut Gitx, done: impl Fn(&Gitx) -> bool) -> usize {
        let mut frames = 0;
        while !done(app) && frames < 600 {
            frame(app);
            frames += 1;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(done(app), "{frames} 帧还没等到。note = {:?}", app.note);
        frames
    }

    /// 打开真仓库，等到快照落地。
    fn open_repo(tag: &str) -> (Gitx, std::path::PathBuf) {
        let dir = scratch_repo(tag);
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        let mut app = blank();
        app.open(&path);
        pump(&mut app, |a| a.snap.is_some());
        (app, dir)
    }

    /// 端到端：真敲字 → 真点击 → core 真提交 → 快照回来。
    /// 上一批留的那句「按钮 → 列表真的换了新内容，app 侧没断言」到这里才闭上。
    #[test]
    fn a_typed_message_and_a_click_lands_a_real_commit() {
        let (mut app, dir) = open_repo("land");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert_eq!(
            app.snap.as_ref().unwrap().status.files.iter().filter(|f| f.staged).count(),
            1,
            "临时仓库该有一条已暂存的改动"
        );
        assert!(
            type_into(&mut app, "提交摘要", "补一条中文提交"),
            "提交框的占位文字没找到"
        );
        assert_eq!(app.commit_msg, "补一条中文提交", "敲的字没落到绑定上");
        assert!(click_text(&mut app, "提交到"), "没有「提交到 …」那颗按钮");
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));

        let snap = app.snap.as_ref().unwrap();
        assert_eq!(snap.commits.len(), before + 1, "提交没进日志");
        assert_eq!(snap.commits[0].subject, "补一条中文提交", "最新那一条不是刚提交的");
        assert!(snap.status.files.is_empty(), "提交完工作区还有文件：{:?}", snap.status.files);
        assert!(app.commit_msg.is_empty(), "提交成功后没清空那格信息");
        assert!(app.note.is_none(), "提交路上报了 note：{}", app.note.clone().unwrap_or_default());
        // 多出来的这条也得有图元——图序列是跟着 snapshot 一起重排的。
        assert_eq!(snap.graph.len(), snap.commits.len(), "提交完图序列和日志行号对不上");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 摘要 + 描述 = 一条**两段式**的提交信息。这条不能只比字符串拼得对不对：
    /// `commit_text` 拼好了、`git commit` 却用两个 `-m` 发出去的话，第二段会被 git 当成
    /// 注释丢掉。所以走真仓库，提交完用 `git log -1 --format=%B` 把它读回来问。
    #[test]
    fn the_description_box_joins_the_summary_with_a_blank_line() {
        let (mut app, dir) = open_repo("describe");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(type_into(&mut app, "提交摘要", "一行标题"), "没有摘要框");
        assert!(type_into(&mut app, "扩展描述", "第二行说清楚为什么"), "没有描述框");
        assert_eq!(app.commit_desc, "第二行说清楚为什么", "敲的字没落到描述框上");
        assert!(click_text(&mut app, "提交到"), "没有「提交到 …」那颗按钮");
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));
        let oid = app.snap.as_ref().unwrap().commits[0].id.clone();
        let msg = git::commit_message(dir.to_str().unwrap(), &oid).expect("把提交信息读回来");
        assert_eq!(msg, "一行标题\n\n第二行说清楚为什么", "两段之间少了那个空行，或第二段被吞了");
        assert!(app.commit_msg.is_empty() && app.commit_desc.is_empty(), "提交成功后两格没清空");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 一份两块改动的夹具：30 行的文件先提交，再把第 2 行和第 20 行各自改掉。
    /// 两处隔得比 git 的 3 行上下文远，`git diff` 就会切成两块——少了这个前提，
    /// 「只暂存一块」根本没有第二块可对照，判据会退化成「整份文件都暂存了」。
    fn two_blocks_repo(tag: &str) -> (Gitx, std::path::PathBuf) {
        let (mut app, dir) = open_repo(tag);
        let path = dir.join("thirty.txt");
        let base: String = (1..=30).map(|i| format!("第 {i} 行\n")).collect();
        std::fs::write(&path, &base).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("跑 git");
            assert!(out.status.success(), "git {args:?}：{}", String::from_utf8_lossy(&out.stderr));
        };
        git(&["add", "thirty.txt"]);
        git(&["commit", "-q", "-m", "三十行"]);
        let edited: String = (1..=30)
            .map(|i| match i {
                2 => "第 2 行改过了\n".to_owned(),
                20 => "第 20 行改过了\n".to_owned(),
                _ => format!("第 {i} 行\n"),
            })
            .collect();
        std::fs::write(&path, &edited).unwrap();
        // 选中那一个文件、把差异区切到它的未暂存补丁上——按钮只在这一格出现。
        app.file = Some("thirty.txt".to_owned());
        app.request(Target::Working { staged: false, path: Some("thirty.txt".to_owned()), untracked: false });
        pump(&mut app, |a| !a.diff.is_empty());
        (app, dir)
    }

    /// 一帧差异里有几块，问的是画出来的那句「块 N」——和按钮往 core 传的数是同一个。
    fn blocks(app: &mut Gitx) -> Vec<usize> {
        let t = texts(&frame(app));
        let mut v: Vec<usize> = t
            .iter()
            .filter_map(|(s, _)| {
                let rest = s.strip_prefix("块 ")?;
                rest[..rest.find('：')?].parse().ok()
            })
            .collect();
        v.sort_unstable();
        v
    }

    /// 端到端：点「暂存区块」= 只把那一块放进索引，另一块留在工作区，文件内容一个字都不动。
    /// 这条必须走真仓库：`git apply --cached` 收不收这份单块补丁，只有真 git 说了算。
    #[test]
    fn clicking_the_block_button_stages_only_that_block() {
        let (mut app, dir) = two_blocks_repo("blocks");
        let p = dir.to_str().unwrap();
        assert_eq!(blocks(&mut app), vec![1, 2], "夹具没造出两块，后面什么都没验");
        assert!(click_text(&mut app, "暂存区块"), "hunk 行上没有「暂存区块」");
        pump(&mut app, |_| {
            git::working_patch(p, true, Some("thirty.txt")).map(|s| s.contains("+第 2 行改过了")).unwrap_or(false)
        });
        let staged = git::working_patch(p, true, Some("thirty.txt")).unwrap();
        let rest = git::working_patch(p, false, Some("thirty.txt")).unwrap();
        assert_eq!(staged.lines().filter(|l| l.starts_with("@@")).count(), 1, "索引里不止一块：{staged}");
        assert!(staged.contains("+第 2 行") && !staged.contains("+第 20 行"), "暂存的不是第 1 块：{staged}");
        assert_eq!(rest.lines().filter(|l| l.starts_with("@@")).count(), 1, "工作区该还剩一块：{rest}");
        assert!(rest.contains("+第 20 行"), "剩下的该是第 2 块：{rest}");
        assert_eq!(
            std::fs::read_to_string(dir.join("thirty.txt")).unwrap(),
            (1..=30).map(|i| if i == 2 || i == 20 { format!("第 {i} 行改过了\n") } else { format!("第 {i} 行\n") }).collect::<String>(),
            "暂存一块的时候动了工作区的文件内容"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「放弃区块」要按两下。**第一下只翻按钮，仓库一个字节不许动**——这一条钉的就是这半步：
    /// 少了它，误点一下「放弃」直接把用户没提交的行删掉，而且没有任何判据会红。
    #[test]
    fn a_discard_needs_two_clicks() {
        let (mut app, dir) = two_blocks_repo("discard");
        let before = std::fs::read_to_string(dir.join("thirty.txt")).unwrap();
        assert!(click_text(&mut app, "放弃区块"), "hunk 行上没有「放弃区块」");
        assert_eq!(app.pending_discard, Some(1), "第一下没把按钮武装起来");
        assert_eq!(std::fs::read_to_string(dir.join("thirty.txt")).unwrap(), before, "第一下就删了内容");
        // 武装之后文案换成「确认放弃」，再点第二下才真丢。
        assert!(click_text(&mut app, "确认放弃"), "第一下之后按钮没换成「确认放弃」");
        pump(&mut app, |_| {
            !std::fs::read_to_string(dir.join("thirty.txt")).unwrap().contains("第 2 行改过了")
        });
        let after = std::fs::read_to_string(dir.join("thirty.txt")).unwrap();
        assert!(!after.contains("第 2 行改过了"), "第二下没丢掉第 1 块：{after}");
        assert!(after.contains("第 20 行改过了"), "把第 2 块也一起丢了：{after}");
        assert_eq!(app.pending_discard, None, "打完之后还武装着");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 两个「已跟踪、改了但没暂存」的文件。为什么不用 `scratch_repo` 那一个：只有一处改动时，
    /// 「只暂存这一个」和「暂存全部」落出去的结果一模一样，方框那两条判据就杀不死「点方框其实
    /// 把整组都暂存了」这种改坏。
    fn two_dirty_files(tag: &str) -> (Gitx, std::path::PathBuf) {
        let (mut app, dir) = open_repo(tag);
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("跑 git");
            assert!(out.status.success(), "git {args:?}：{}", String::from_utf8_lossy(&out.stderr));
        };
        std::fs::write(dir.join("b.txt"), "乙\n").unwrap();
        std::fs::write(dir.join("c.txt"), "丙\n").unwrap();
        git(&["add", "b.txt", "c.txt"]);
        git(&["commit", "-q", "-m", "两个文件"]);
        std::fs::write(dir.join("b.txt"), "乙\n乙改\n").unwrap();
        std::fs::write(dir.join("c.txt"), "丙\n丙改\n").unwrap();
        app.open(dir.to_str().expect("utf-8 路径"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.status.files.len()).unwrap_or(0) >= 2);
        (app, dir)
    }

    /// app 快照里「已暂存」那一栏都有谁。等状态变化必须等**这个**，不能只等 git 的索引：
    /// core 的动作跑完到快照回到 app 之间隔着一轮后台线程，那中间界面上还是旧的两栏——
    /// 再点一次就点到**别的行**上去了（本批的第二次点击就是这么打空的）。
    fn staged_in_app(app: &Gitx) -> Vec<String> {
        let mut v: Vec<String> = app
            .snap
            .as_ref()
            .map(|s| s.status.files.iter().filter(|f| f.staged).map(|f| f.path.clone()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    /// 点文件行的方框 = **只动那一个文件**，而且不许顺带把选中项换走。
    /// 整行仍然是「选中看差异」，两件事挤在同一行里，所以两头都要钉：
    /// 少了 `!toggled` 那一句，点方框会连带把差异区切到 b.txt —— 这条先红在「选中项被换走」。
    #[test]
    fn ticking_a_file_box_stages_only_that_file() {
        let (mut app, dir) = two_dirty_files("boxes");
        let p = dir.to_str().unwrap();
        app.file = Some("c.txt".to_owned());
        click_file_box(&mut app, "b.txt");
        pump(&mut app, |a| staged_in_app(a) == vec!["b.txt".to_owned()]);
        let staged = git::working_patch(p, true, Some("b.txt")).unwrap();
        assert!(staged.contains("+乙改"), "b.txt 没进索引：{staged}");
        assert_eq!(git::working_patch(p, true, Some("c.txt")).unwrap(), "", "c.txt 被一起暂存了");
        assert!(git::working_patch(p, false, Some("c.txt")).unwrap().contains("+丙改"), "c.txt 该还在工作区");
        assert_eq!(app.file.as_deref(), Some("c.txt"), "点方框顺带把选中项换走了");
        // 再点一下 = 取消暂存，回到起点（工作区那个文件从头到尾没被写过）。
        let work = std::fs::read_to_string(dir.join("b.txt")).unwrap();
        click_file_box(&mut app, "b.txt");
        pump(&mut app, |a| staged_in_app(a).is_empty());
        assert!(git::working_patch(p, false, Some("b.txt")).unwrap().contains("+乙改"), "退回工作区时把改动弄丢了");
        assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), work, "暂存/取消暂存动了文件内容");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 分类行那枚方框管**整组**：勾未暂存 = 全部进索引，勾已暂存 = 全部退回工作区。
    /// 这条钉的是「分类方框和文件方框不是一回事」——两枚画得几乎一样，列不同、动的范围也不同。
    #[test]
    fn the_section_box_toggles_the_whole_group() {
        let (mut app, dir) = two_dirty_files("groups");
        let p = dir.to_str().unwrap();
        click_section_box(&mut app, "未暂存文件");
        pump(&mut app, |a| staged_in_app(a) == vec!["b.txt".to_owned(), "c.txt".to_owned()]);
        assert!(git::working_patch(p, true, Some("b.txt")).unwrap().contains("+乙改"), "整组没把 b.txt 带上");
        assert!(git::working_patch(p, true, Some("c.txt")).unwrap().contains("+丙改"), "整组没把 c.txt 带上");
        click_section_box(&mut app, "已暂存文件");
        pump(&mut app, |a| staged_in_app(a).is_empty());
        assert!(git::working_patch(p, false, Some("b.txt")).unwrap().contains("+乙改"), "整组取消没把 b.txt 退回去");
        assert!(git::working_patch(p, false, Some("c.txt")).unwrap().contains("+丙改"), "取消暂存把改动弄丢了");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ⌘/Ctrl+Enter 是键盘通路：不碰鼠标也能提交（参考版 placeholder 里就写着这条）。
    #[test]
    fn cmd_enter_commits_without_the_mouse() {
        let (mut app, dir) = open_repo("enter");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(type_into(&mut app, "提交摘要", "用回车提交"), "提交框没了");
        assert_eq!(app.commit_msg, "用回车提交", "回车前那格还是空的");
        press_mod_enter(&mut app, egui::Modifiers::COMMAND);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));
        assert_eq!(app.snap.as_ref().unwrap().commits[0].subject, "用回车提交");
        assert!(app.commit_msg.is_empty(), "回车提交后没清空");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Windows/Linux 上那枚键不叫 ⌘，winit 交过来的是 `CTRL`。产品代码两种都得认，
    /// 而**只测 mac 那一半的话，删掉 `|| i.modifiers.ctrl` 不会有任何判据变红**——这条就是补那一半。
    #[test]
    fn ctrl_enter_commits_on_a_windows_keyboard() {
        let (mut app, dir) = open_repo("ctrl-enter");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(type_into(&mut app, "提交摘要", "用 Ctrl 回车提交"), "提交框没了");
        press_mod_enter(&mut app, egui::Modifiers::CTRL);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));
        assert_eq!(app.snap.as_ref().unwrap().commits[0].subject, "用 Ctrl 回车提交");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 没暂存任何东西时那颗按钮**点不动**：提交信息写得再好也不行——
    /// 禁用的判据和「什么算能提交」是同一个规则，两处必须一致（React 版 `WorkingCopy.tsx:337` 的
    /// `canCommit` 就是这个式子）。`git commit` 在空暂存上报「nothing to commit」，那是白跑一个进程。
    #[test]
    fn the_commit_rule_is_stated_in_one_place() {
        let (mut app, dir) = open_repo("disabled");
        // `before` 必须在点下去之前读：提交是异步的，点完再读可能已经落地，`pump` 会一直等下去。
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(type_into(&mut app, "提交摘要", "清空暂存区"), "提交框没了");
        assert!(click_text(&mut app, "提交到"), "「提交到」那行字不见了");
        pump(&mut app, |a| a.snap.as_ref().unwrap().commits.len() == before + 1);

        // 工作区现在干净：同样的点击不该再有第二次提交。
        assert!(type_into(&mut app, "提交摘要", "第二条"), "提交框没了");
        let painted = frame(&mut app);
        assert!(
            texts(&painted)
                .iter()
                .any(|(s, _)| s.starts_with("先勾选要提交的文件")),
            "干净的工作区没告诉用户为什么点不动"
        );
        assert!(click_text(&mut app, "提交到"));
        for _ in 0..20 {
            frame(&mut app);
        }
        assert_eq!(
            app.snap.as_ref().unwrap().commits.len(),
            before + 1,
            "没暂存也提交了：按钮的禁用没生效"
        );
        assert!(!app.commit_msg.is_empty(), "点不动却把信息清空了");

        // 另一半：有暂存但**没写字**，同样点不动，报的得是 core 那句措辞。
        std::fs::write(dir.join("b.txt"), "新文件\n").unwrap();
        let out = std::process::Command::new("git")
            .args(["add", "b.txt"])
            .current_dir(&dir)
            .output()
            .expect("git add");
        assert!(out.status.success(), "git add 失败");
        app.open(dir.to_str().unwrap());
        // 上一段故意留下的「第二条」得擦掉：这一半要测的是「有暂存但没写字」。
        app.commit_msg.clear();
        pump(&mut app, |a| a
            .snap
            .as_ref()
            .map(|s| s.status.files.iter().any(|f| f.staged))
            .unwrap_or(false));
        let painted = frame(&mut app);
        assert!(
            texts(&painted)
                .iter()
                .any(|(s, _)| s.starts_with("提交信息不能为空")),
            "没写信息时没说出原因：{:?}",
            texts(&painted)
                .iter()
                .map(|(s, _)| s.clone())
                .filter(|s| s.contains("提交"))
                .collect::<Vec<_>>()
        );
        assert!(click_text(&mut app, "提交到"));
        for _ in 0..20 {
            frame(&mut app);
        }
        let snap = app.snap.as_ref().unwrap();
        assert_eq!(snap.commits.len(), before + 1, "没写字也提交了：禁用没生效");
        assert!(
            snap.status.files.iter().any(|f| f.staged),
            "b.txt 被那次点不动的提交吃掉了"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// pre-commit 钩子拦下一次提交时，那格信息必须**还在**。
    /// 「按下提交就清空」看着合理，其实是把用户写的字丢在一个他控制不了的失败上——
    /// 所以清空只跟着 `Msg::Loaded`（真落地）走，`Msg::Failed` 不碰（见 `clear_msg_on_load`）。
    #[cfg(unix)]
    #[test]
    fn a_rejected_commit_keeps_the_message() {
        use std::os::unix::fs::PermissionsExt;
        let (mut app, dir) = open_repo("hook");
        let hook = dir.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").expect("写钩子");
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("钩子加可执行位");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(type_into(&mut app, "提交摘要", "被钩子拦下的一条"), "提交框没了");
        assert!(click_text(&mut app, "提交到"), "「提交到」那行字不见了");
        pump(&mut app, |a| a.note.is_some() && !a.busy);
        assert_eq!(app.snap.as_ref().unwrap().commits.len(), before, "钩子 exit 1 却还是提交了");
        assert_eq!(app.commit_msg, "被钩子拦下的一条", "提交失败把用户写的字弄丢了");

        // 失败之后紧接着做**别的**动作（这里「全部暂存」）也不能吃掉那格信息：
        // 「哪一次 `Loaded` 该清空」由 `act` 里那行 `matches!` 决定，每次动作重算一遍，
        // 所以一个没提交成功的动作绝不能把清空权留给下一个动作。
        app.act(Action::StageAll);
        pump(&mut app, |a| !a.busy);
        assert_eq!(app.commit_msg, "被钩子拦下的一条", "失败的提交把清空标记留给了下一个动作");

        // 拆掉钩子，同一句话再点一次就能落地——这才证明它还实在那格里。
        std::fs::remove_file(&hook).expect("删钩子");
        assert!(click_text(&mut app, "提交到"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));
        assert_eq!(app.snap.as_ref().unwrap().commits[0].subject, "被钩子拦下的一条");
        assert!(app.commit_msg.is_empty(), "这次成功了却没清空");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「层级顺序」这一枚现在是**弹出按钮**：点按钮只把第二层摊开，换排序靠菜单里那一项。
    /// 所以这条既钉「第二层真接上了排序」（点完重取列表、回来的是新顺序），也钉
    /// 「按钮上写的是当前那一项」。
    /// 结尾用真仓库跑：换完顺序得能**取回新快照**，夹具那条路会把重取变成一次失败的 git。
    #[test]
    fn the_order_menu_switches_between_the_two_orders() {
        let (mut app, dir) = open_repo("order-menu");
        // 菜单没摊开时，按钮行上只该有**当前那一档**。
        assert!(filter_text(&mut app, DATE_ORDER).is_some(), "默认那一档没写在按钮上");
        assert_eq!(filter_text(&mut app, TOPO_ORDER), None, "菜单没摊开，另一档不该出现在按钮行上");
        popup_pick(&mut app, DATE_ORDER, TOPO_ORDER);
        assert!(app.topo, "菜单里点「层级顺序」没换排序");
        assert!(app.busy && app.snap.is_none(), "换了排序没重取列表：日志会停在旧顺序上");
        pump(&mut app, |a| a.snap.is_some());
        assert!(filter_text(&mut app, TOPO_ORDER).is_some(), "换了排序，按钮上还是旧那一项");
        assert_eq!(filter_text(&mut app, DATE_ORDER), None, "按钮上两档同时出现：文案没跟着换");
        popup_pick(&mut app, TOPO_ORDER, DATE_ORDER);
        assert!(!app.topo, "点「日期顺序」没换回来");
        pump(&mut app, |a| a.snap.is_some() && !a.topo);
        let _ = dir;
    }

    /// 三枚带**第二层标记**、还没接的那枚不带。标记是哪个码位不归这条管——那是
    /// `every_rune_the_shell_paints_has_a_glyph` 的活，只有它认字形表；这条只数
    /// 「按钮文案比标签多出来几枚非空白字」。
    /// 别写成 `Some(format!("{label} {ARROW}"))`：那和 `ARROW` 常量自比，把箭头改成空串
    /// 后面还留着一个空格，改成一枚别的符号也照样绿——两头的错都抓不住。
    #[test]
    fn only_the_three_switches_carry_an_arrow() {
        let mut app = loaded();
        let mut marks = |label: &str| -> usize {
            let t = filter_text(&mut app, label)
                .unwrap_or_else(|| panic!("筛选条上没有「{label}」这枚按钮"));
            t[label.len()..].chars().filter(|c| !c.is_whitespace()).count()
        };
        for label in [ALL_BRANCHES, SHOW_REMOTE, DATE_ORDER] {
            assert_eq!(marks(label), 1, "「{label}」那枚的第二层标记不是一枚");
        }
        assert_eq!(marks("紧凑视图"), 0, "还没接的那枚不该带标记");
        // 点它给的还是那句实话，不是开一个空菜单。
        assert!(click_text(&mut app, "紧凑视图"), "「紧凑视图」没画出来");
        assert_eq!(app.note.as_deref(), Some("egui 版还没接：紧凑视图"), "点没接的那枚该说实话");
    }

    /// 「所有分支」点开列本地分支，选中一条 = `git log <那条分支>`：只有它走得到的提交才留下。
    /// 支线从**初始提交**分出去、主线再往前走一步，于是两条线各有一个对面拿不到的提交——
    /// 拿长度当判据不行（两边都是 2 条），只能比**提交主题**。
    /// 结尾用 `git rev-list --count` 复核长度，不听 app 自己的证词。
    #[test]
    fn the_scope_menu_walks_only_the_picked_branch() {
        let (mut app, dir) = open_repo("scope-side-branch");
        let base = git_in(&dir, &["branch", "--show-current"]);
        git_in(&dir, &["checkout", "-q", "-b", "side"]);
        std::fs::write(dir.join("side.txt"), "侧\n").unwrap();
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "只在支线"]);
        git_in(&dir, &["checkout", "-q", base.as_str()]);
        std::fs::write(dir.join("main.txt"), "一\n").unwrap();
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "只在主线"]);
        // 开仓那一刻这两条提交都还不存在，先让快照追上仓库。
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        app.open(&path);
        pump(&mut app, |a| line_of(a).contains(&"只在主线".to_owned()));
        assert_eq!(app.log_rev, None, "默认不该带 rev");
        assert!(!line_of(&app).contains(&"只在支线".to_owned()), "HEAD 那条线里不该有支线的提交");

        popup_pick(&mut app, ALL_BRANCHES, "side");
        assert_eq!(app.log_rev.as_deref(), Some("side"), "菜单里点了分支没落到范围上");
        pump(&mut app, |a| a.snap.is_some() && a.log_rev.as_deref() == Some("side"));
        let l = line_of(&app);
        assert!(l.contains(&"只在支线".to_owned()), "按支线筛完，列表里没有支线独有的提交：{l:?}");
        assert!(!l.contains(&"只在主线".to_owned()), "支线列表里混进了只有主线走得到的提交：{l:?}");
        let want = git_in(&dir, &["rev-list", "--count", "side"]).parse::<usize>().expect("数字");
        assert_eq!(l.len(), want, "列表长度和仓库里的对不上：app 说 {l:?}");

        // 点回「所有分支」= 范围清空，回到 HEAD 那条线。
        popup_pick(&mut app, "side", ALL_BRANCHES);
        assert_eq!(app.log_rev, None, "点「所有分支」没把范围清空");
        pump(&mut app, |a| a.snap.is_some() && a.log_rev.is_none());
        assert!(!line_of(&app).contains(&"只在支线".to_owned()), "回到「所有分支」还留着支线");
    }

    /// 「显示远程分支」那一枚**只决定远程药丸画不画**：不重取列表（`git log` 的参数一个字没动，
    /// argv 那层由核心层的 `the_rev_argument_only_adds_the_rev_before_the_pathspec` 钉住）。
    /// 远程跟踪指针用 `update-ref` 造，**不碰网络**。
    #[test]
    fn hiding_remote_branches_drops_only_the_remote_pill() {
        let (mut app, dir) = open_repo("remote-pill");
        let head = git_in(&dir, &["rev-parse", "HEAD"]);
        git_in(&dir, &["update-ref", "refs/remotes/origin/main", head.as_str()]);
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        app.open(&path);
        let remote_pill = format!("{}origin/main", pill_glyph(RefKind::Remote));
        let branch_pill = format!("{}{}", pill_glyph(RefKind::Branch), git_in(&dir, &["branch", "--show-current"]));
        // 等的是**应用状态**（新快照里真带上了远程指针），画没画下一步单独问——
        // `pump` 的判据只拿得到 `&Gitx`，帧得由 `&mut` 起。
        pump(&mut app, |a| {
            a.snap.as_ref().is_some_and(|s| s.badges.iter().any(|r| r.iter().any(|b| b.kind == RefKind::Remote)))
        });
        assert!(row_texts(&mut app).contains(&remote_pill), "远程药丸没画在行上");
        let before = app.snap.as_ref().unwrap().commits.len();

        popup_pick(&mut app, SHOW_REMOTE, HIDE_REMOTE);
        assert!(!app.show_remote, "点「隐藏远程分支」没关掉");
        assert!(app.snap.is_some() && !app.busy, "这一枚只决定画不画，不该把列表重取一遍");
        let l = row_texts(&mut app);
        assert!(!l.contains(&remote_pill), "关掉之后远程药丸还画在行上");
        assert!(l.iter().any(|t| t.starts_with(&branch_pill)), "本地分支药丸跟着没了：过滤写宽了");
        assert_eq!(app.snap.as_ref().unwrap().commits.len(), before, "提交列表被这枚动了");

        popup_pick(&mut app, HIDE_REMOTE, SHOW_REMOTE);
        assert!(app.show_remote, "点回「显示远程分支」没打开");
        assert!(row_texts(&mut app).contains(&remote_pill), "重新打开后远程药丸没回来");
    }

    /// 日志四列**全部靠左**：每一格的数据都贴住自己表头的左缘。
    /// 参考图实测（1050pt 宽那张，logical pt）：表头左缘 提交 202.5 / 描述 280 / 作者 744 /
    /// 日期 934，四条列边线分别在 198 / 275 / 739 / 929——每格数据与它的表头同一条竖线起笔。
    /// 一条判据同时管三种错法：右对齐（左缘随文字宽度漂）、`Ui::put` 的居中与两端对齐、
    /// 以及把提交号画在图列**里面**（圆点压在提交号上）。数据必须长短不齐，见 `ragged()`。
    #[test]
    fn every_log_column_starts_at_its_own_header() {
        let mut app = ragged();
        let out = frame(&mut app);
        let (head, rows) = (head_band(&out), rows_band(&out));
        let head = || head.clone();
        let rows = rows.clone();
        let x = |label: &str, y: std::ops::RangeInclusive<f32>| text_rect(&out, label, y).min.x;
        let flush = |col: &str, header: f32, cells: [(f32, &str); 3]| {
            for (got, what) in cells {
                assert!(
                    (got - header).abs() < 1.0,
                    "{col} 列里「{what}」的左缘 {got} 没贴住表头 {header}——右对齐或居中都会让它随字宽漂"
                );
            }
        };
        let (sha, subject, author) = (
            [x("a1", rows.clone()), x("b22", rows.clone()), x("c333", rows.clone())],
            [x("短", rows.clone()), x("一条长得多的", rows.clone()), x("中等长度的一条", rows.clone())],
            [x("包融", rows.clone()), x("baorong", rows.clone()), x("R", rows.clone())],
        );
        // 日期列这一格给的是**相对文案**（「今天 09:05」「昨天 23:58」），文字本身跟着本机时区漂：
        // 同一条 `2020-03-04T05:06+08:00` 换到 UTC 机器上是 03-03。所以按内容找它会假红，
        // 改成按**位置**找——一行里最靠右的那块文字就是日期，它的左缘即这一列的左缘。
        let right = |label: &str| {
            let r = text_rect(&out, label, rows.clone());
            let band = (r.center().y - ROW_H / 2.0 + 1.0)..=(r.center().y + ROW_H / 2.0 - 1.0);
            all_text_rects(&out, band, &[""])
                .into_iter()
                .map(|(_, rc)| rc.min.x)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let date = [right("a1"), right("b22"), right("c333")];
        flush("提交号", x("提交", head()), [(sha[0], "a1"), (sha[1], "b22"), (sha[2], "c333")]);
        flush("描述", x("描述", head()), [(subject[0], "短"), (subject[1], "一条长得多的…"), (subject[2], "中等长度的一条")]);
        flush("作者", x("作者", head()), [(author[0], "包融"), (author[1], "baorong <…>"), (author[2], "R")]);
        flush("日期", x("日期", head()), [(date[0], "第 1 行"), (date[1], "第 2 行"), (date[2], "第 3 行")]);
        // 绝对位置钉在**量出来的字面量**上：参考图（1050pt 宽）四条列边线 198 / 275 / 739 / 929，
        // 表头四个词的左缘 202.5 / 280 / 744 / 934，数据跟自己的表头同一条竖线起笔。
        // 拿 `COL_*` 加加减减比给自己看等于没判——这一批间距从 8 改成 5，那种写法照样绿。
        assert_eq!(sha[0], 203.0, "提交号的左缘（边线 198 + 内缩 5）");
        assert_eq!(subject[0], 280.0, "描述列的左缘（边线 275 + 内缩 5）");
        assert_eq!(author[0], 744.0, "作者列的左缘（边线 739 + 内缩 5）");
        assert_eq!(date[0], 934.0, "日期列的左缘（边线 929 + 内缩 5）");
        // 行距与表头高也照抄实测值（数据行 130→156→182 = 26，表头 79..103 = 24）：
        // 上面那些判据全是「间距 == ROW_H」，把 ROW_H 改回旧的 21 它们一个都不会红。
        assert_eq!(ROW_H, 26.0, "行距不是参考图量的 26pt");
        assert_eq!(HEADER_H, 24.0, "表头那一条不是参考图量的 24pt");
    }

    /// 描述列开头那一串药丸。参考图实测（logical pt）：药丸高 15、浅蓝 `#D5E5FD` 配 `#AAAAAA` 灰边，
    /// 「超前3个版本」是深藏青 `#003979` 白字且**紧贴**前一枚（合起来才是一颗胶囊），
    /// 三枚占 357.5..476、描述文字从 481 起笔——比最后一枚多让 5pt。
    #[test]
    fn ref_pills_sit_left_of_the_subject_in_their_row() {
        let out = frame(&mut filled(4, &[]));
        let rows = rows_band(&out);
        let y = text_rect(&out, "a0000", rows.clone()).center().y;
        let band = (y - ROW_H / 2.0 + 1.0)..=(y + ROW_H / 2.0 - 1.0);
        let word = |p: &str| {
            all_text_rects(&out, band.clone(), &[p])
                .into_iter()
                .next()
                .unwrap_or_else(|| {
                    panic!("第一条提交那行没有 {p:?}，这一行的文字是 {:?}", all_text_rects(&out, band.clone(), &[""]))
                })
                .1
        };
        let x0 = col_edges(SIDEBAR_W, WIN_W)[1];
        let (main, subject) = (word("⤴main"), word("主题0"));
        // 底色矩形按颜色认领：这一行该有一枚浅蓝 + 一枚深藏青。
        let in_row = |c: Color32| {
            let mut v: Vec<(Rect, egui::CornerRadius)> = rounded(&out)
                .into_iter()
                .filter(|(fill, r, _)| *fill == c && (r.center().y - y).abs() < 1.0)
                .map(|(_, r, k)| (r, k))
                .collect();
            v.sort_by(|a, b| a.0.min.x.total_cmp(&b.0.min.x));
            v
        };
        let (blue, navy) = (in_row(PILL_BG), in_row(PILL_AHEAD_BG));
        assert_eq!(
            (blue.len(), navy.len()),
            (1, 1),
            "第一行该有一枚浅蓝 + 一枚深藏青，实际 {blue:?} {navy:?}"
        );
        for (r, _) in blue.iter().chain(navy.iter()) {
            assert_eq!(r.height(), PILL_H, "药丸高 {r:?} 不是实测的 {PILL_H}pt");
        }
        assert_eq!(blue[0].0.min.x, x0, "第一枚药丸没顶到描述列的左缘");
        assert_eq!(navy[0].0.min.x, blue[0].0.max.x, "「超前」那枚没紧贴前一枚：中间留缝就成两截");
        // 两枚拼起来是**一颗**胶囊：只有朝外的两头是圆的，接缝那两侧收平。
        // 四角都圆会在缝上留两个缺口，参考图没有那两个缺口。
        assert_eq!((blue[0].1.nw, blue[0].1.ne), (PILL_R, 0), "浅蓝那枚：左圆、右贴缝");
        assert_eq!((navy[0].1.nw, navy[0].1.ne), (0, PILL_R), "深藏青那枚：左贴缝、右圆");
        assert_eq!(main.min.x, blue[0].0.min.x + PILL_PAD, "药丸文字没在边框内缩 {PILL_PAD}pt 处起笔");
        assert_eq!(
            subject.min.x,
            navy[0].0.max.x + PILL_AFTER,
            "描述文字没从药丸右边让出 {PILL_AFTER}pt"
        );
        assert_eq!(text_color(&out, "超前2个版本", band.clone()), Color32::WHITE, "深藏青那枚该是白字");
        assert_ne!(text_color(&out, "⤴main", band.clone()), Color32::WHITE, "浅蓝那枚是深色字，白字压在上面看不见");
        // 远程跟踪指针挂在第三行（`filled` 把 origin/main 的 target 定在提交 2），那儿没有深藏青。
        let y2 = text_rect(&out, "a0002", rows).center().y;
        let row2 = (y2 - ROW_H / 2.0 + 1.0)..=(y2 + ROW_H / 2.0 - 1.0);
        assert_eq!(text_rect(&out, "⤴origin/main", row2.clone()).min.x, x0 + PILL_PAD, "远程指针前头不该还有别的药丸");
        // 单独一枚（前头没东西可拼）就该四角都是圆的。
        let lone = rounded(&out)
            .into_iter()
            .filter(|(_, r, _)| (r.center().y - y2).abs() < 1.0)
            .map(|(_, r, k)| (r, k))
            .collect::<Vec<_>>();
        assert_eq!(lone.len(), 1, "第三行只该有一枚远程指针：{lone:?}");
        assert_eq!(lone[0].1, egui::CornerRadius::same(PILL_R), "独个儿那枚该四角都圆");
        assert!(all_text_rects(&out, row2, &["超前"]).is_empty(), "这一行没有超前数，就不该出那枚胶囊");
    }

    /// 虚拟行（工作区）那一条：整行灰底 `#DCDCDC`、图列里一枚**空心环**、环下面接到第一条提交、
    /// 日期是「今天 HH:MM」。参考图这一行**没被选中也是灰的**，所以先把选中挪走——
    /// 选中那格铺的是 SELECTED，灰底量不到了。
    #[test]
    fn the_uncommitted_row_is_a_grey_band_and_a_hollow_ring() {
        let mut app = loaded();
        // 直接挪字段，不走 `select()`：那会对着假路径 `/fixture` 开线程跑 git，判据要的不是它。
        app.sel = Sel::Commit(0);
        let out = frame(&mut app);
        let grey: Vec<Rect> = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == UNCOMMITTED_BG)
            .map(|(_, r)| r)
            .collect();
        assert_eq!(grey.len(), 1, "灰底只该铺虚拟行那一条：{grey:?}");
        // 而且就在日志第一条那一格（紧贴表头下沿）：跑到别的带子里就不是它了。
        let top = *head_band(&out).end();
        assert!(
            (grey[0].min.y - top).abs() < 1.5 && grey[0].min.y < top + ROW_H && grey[0].height() <= ROW_H,
            "灰底不在日志第一条（表头下沿 {top}）：{:?}",
            grey[0]
        );
        assert!(
            (grey[0].min.x - SIDEBAR_W).abs() < 1.5 && (grey[0].max.x - WIN_W).abs() < 1.5,
            "灰底没整行铺满：{:?}",
            grey[0]
        );
        let y = grey[0].center().y;
        assert_eq!(rings(&out), vec![pos2(lane_x(SIDEBAR_W, 0), y)], "环不在第 0 道那一格");
        // 环下面那根竖线：从环心接到行底，颜色跟着第一条提交的车道。
        let stem = out.shapes.iter().any(|c| {
            matches!(&c.shape, egui::Shape::LineSegment{points, stroke}
                if stroke.color == lane_color(0)
                    && (points[0] - pos2(lane_x(SIDEBAR_W, 0), y)).length() < 0.01
                    && (points[1].x - points[0].x).abs() < 0.01
                    && (points[1].y - (y + ROW_H / 2.0)).abs() < 0.01)
        });
        assert!(stem, "环没接到下面那条提交：缺一根从环心到行底的竖线");
        let band = (y - ROW_H / 2.0 + 1.0)..=(y + ROW_H / 2.0 - 1.0);
        let today = all_text_rects(&out, band.clone(), &["今天 "]);
        assert_eq!(today.len(), 1, "虚拟行的日期该是「今天 HH:MM」：{:?}", all_text_rects(&out, band, &[""]));
        assert_eq!(today[0].1.min.x, col_edges(SIDEBAR_W, WIN_W)[3], "日期列的左缘");
    }

    /// 图元自己的尺寸与颜色也是照参考图量死的：圆点半径 4.0（直径 8.0）、竖线宽 2.0、
    /// 环外径 10.0 / 环宽 2.5 / 中性灰 `#929291`、第 0 道 `#005FBD`。
    /// 这里**故意写字面量**而不是 `DOT_R`/`LINE_W`：判据要钉的是「量出来的那个数」，
    /// 拿常量比常量等于没判。旧 React 版那圈是 2.6 / 1.3 加紫色，整体小一圈——并排一比才看得出来。
    #[test]
    fn graph_marks_match_the_measured_reference_geometry() {
        let out = frame(&mut loaded());
        let (radii, widths): (Vec<f32>, Vec<f32>) = out.shapes.iter().fold(Default::default(), |mut acc, c| {
            match &c.shape {
                egui::Shape::Circle(d) if LANE_COLORS.contains(&d.fill) => acc.0.push(d.radius),
                egui::Shape::LineSegment { stroke, .. } if LANE_COLORS.contains(&stroke.color) => {
                    acc.1.push(stroke.width)
                }
                _ => {}
            }
            acc
        });
        assert!(radii.len() >= 3, "帧里只数到 {} 个圆点，图序列没进帧", radii.len());
        assert!(radii.iter().all(|r| *r == 4.0), "圆点半径 {radii:?}，参考图量的是 4.0");
        assert!(widths.len() >= 3, "帧里只数到 {} 条线", widths.len());
        assert!(widths.iter().all(|w| *w == 2.0), "线宽 {widths:?}，参考图量的是 2.0");
        let ring = out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Circle(d) if d.fill == Color32::WHITE && d.stroke.color == RING_GREY => {
                Some((d.radius, d.stroke.width))
            }
            _ => None,
        });
        assert_eq!(ring, Some((5.0, 2.5)), "环的外半径/环宽和参考图（外径 10.0、环宽 2.5）不符");
        assert_eq!(lane_color(0), Color32::from_rgb(0x00, 0x5f, 0xbd), "第 0 道是量出来的颜色");
        // 第 0 道**在哪**也是量的：参考图圆点 239..246.5、图列从 220 起 → 中心在列左缘 +22.75。
        // 这条专门钉 `GRAPH_PAD`。反向变异把左边距改回 1（圆点贴着侧栏边线跑）时，
        // 其余门禁全绿——它们只比 `lane_x(...)`，谁也没问这一道离边线多远。
        let lane0 = SIDEBAR_W + 22.75;
        let (dots, lines, _) = graph_marks(&out);
        assert!(
            dots.iter().all(|p| (p.x - lane0).abs() < 0.01),
            "圆点没落在第 0 道的实测位置 {lane0}：{dots:?}"
        );
        assert!(
            !dots.is_empty() && lines.iter().all(|[a, b]| (a.x - lane0).abs() < 0.01 && (b.x - lane0).abs() < 0.01),
            "竖线不在第 0 道上：{lines:?}"
        );
        assert!(rings(&out).iter().all(|p| (p.x - lane0).abs() < 0.01), "环不在第 0 道上");
    }

    /// 表头那四条列边线：1pt 宽、16pt 高、`(229,229,229)`，落在 198 / 275 / 739 / 929
    /// （1050pt 宽那张参考图）。上下各一条横线把表头那 24pt 从筛选条与日志里切出来。
    /// 这条判据是来还债的：上一批我用**整带**阈值扫表头，判定「参考图没有竖线」，
    /// 于是把并排一比就能看见的四条边线漏掉了——线只有 16pt 高，24pt 的带子按行取平均会被白盖掉。
    #[test]
    fn the_header_has_the_measured_column_rules() {
        let out = frame(&mut loaded());
        let head = head_band(&out);
        let (top, bottom) = (*head.start(), *head.end());
        let mid = (top + bottom) / 2.0;
        let mut vertical: Vec<(f32, f32)> = Vec::new();
        let mut horizontal: Vec<f32> = Vec::new();
        for c in &out.shapes {
            if let egui::Shape::LineSegment { points, stroke } = &c.shape {
                if stroke.color != RULE || stroke.width != 1.0 {
                    continue;
                }
                let (a, b) = (points[0], points[1]);
                if (a.x - b.x).abs() < 0.01 && a.y >= top && b.y <= bottom {
                    vertical.push((a.x, b.y - a.y));
                } else if (a.y - b.y).abs() < 0.01
                    && a.y >= top
                    && a.y <= bottom
                    && a.x <= SIDEBAR_W + 1.5
                    && b.x >= WIN_W - 1.5
                {
                    horizontal.push(a.y);
                }
            }
        }
        vertical.sort_by(|a, b| a.0.total_cmp(&b.0));
        // 这里**故意写字面量**：判据钉的是量出来的那四条边线，拿 `COL_*` 比 `COL_*` 等于没判。
        assert_eq!(
            vertical,
            vec![(198.0, RULE_H), (275.0, RULE_H), (739.0, RULE_H), (929.0, RULE_H)],
            "表头的列边线跟参考图不符（宽 1、高 {RULE_H}、四条在 198/275/739/929）"
        );
        assert_eq!(horizontal.len(), 2, "表头该有上下两条横线把它切出来：{horizontal:?}");
        // 横线贴着行的**边框**（rect 86..110），而 `head_band` 是从文字中线量的（97.5），
        // 差半个像素是 galley 取整——所以这里量两条线自己张开的高度，不跟带子比。
        horizontal.sort_by(|a, b| a.total_cmp(b));
        assert!(
            (horizontal[1] - horizontal[0] - HEADER_H).abs() < 1.5
                && ((horizontal[0] + horizontal[1]) / 2.0 - mid).abs() < 1.5,
            "两条横线不是贴着表头那一条的上下边框：{horizontal:?}（带 {top}..{bottom}，中线 {mid}）"
        );
    }

    /// 日志四列的字色是量出来的 `(38,38,38)`，不是浅灰。这一条把「颜色」和「列宽/字重」
    /// 分开钉：SourceTree 不靠把提交号与日期调淡来分主次（我们原来那么做过，并排一比就显出来了）。
    #[test]
    fn log_text_is_the_measured_near_black_not_grey() {
        let mut app = loaded();
        app.sel = Sel::Commit(1);
        let out = frame(&mut app);
        let rows = rows_band(&out);
        assert_eq!(INK, Color32::from_rgb(38, 38, 38), "INK 本身是参考图量的，别改成一个近似值");
        for label in ["a0000", "主题0", "包融 <", "今天 "] {
            assert_eq!(
                text_color(&out, label, rows.clone()),
                INK,
                "「{label}」没按参考图取 (38,38,38)——淡一档的字在浅色背景上根本站不住"
            );
        }
        // 表头同色：参考图那五个词也是 (39,39,39) 这一档，不是灰。
        assert_eq!(text_color(&out, "提交", head_band(&out)), INK, "表头字色");
        // 虚拟行更深一档。这一条把 `INK_STRONG` 和 `strong()` 解绑：那面只带 Regular 的
        // 字体让 `strong()` 退化成一次取色，深浅必须由这里的实测值说了算。
        let first = *rows.start()..=rows.start() + ROW_H;
        assert_eq!(
            text_color(&out, "Uncommitted", first),
            INK_STRONG,
            "虚拟行的描述没按参考图取 (33,33,33)"
        );
    }

    /// 视口追上列表尾 = 到底，该问下一页。这一格只管**扳机**：`log_more_busy` 立起来
    /// 就说明这一帧真发了请求（线程跑的 `git log` 对 `/fixture` 必然失败，但失败回复
    /// 不动那块牌子，所以断言不受线程快慢影响）。
    /// 判据把 `range.end >= total` 改成恒假就会红——那是「滚到底没反应」那条 bug 的原形。
    #[test]
    fn reaching_the_bottom_of_the_list_asks_for_the_next_page() {
        let mut app = loaded();
        app.log_more = true;
        frame(&mut app);
        assert!(app.log_more_busy, "列表画到最后一行也没问下一页");
        // 上一页还在路上：不许重发（不然滚轮一格一发，能连着起十几个 git 进程）。
        app.log_more_busy = false;
        app.log_more = false;
        frame(&mut app);
        assert!(!app.log_more_busy, "已经到底了还在问下一页");
    }

    /// 端到端：把「到底要下一页」接到**真 git** 上跑一整趟。
    /// 造四条提交，然后把列表腰斩成两条——剩下那两条就当「还没取的下一页」。
    /// 之后由帧里的扳机起线程、`git log --skip=2` 真跑一次、回复真落回列表。
    /// 这条判据红了 = 扳机、线程、`--skip`、回复落地四段里断了一段；只断在中间那段
    /// 的那两条判据（`reaching_the_bottom…` 管扳机、`a_second_page…` 管落地）都不一定红。
    #[test]
    fn the_bottom_of_the_list_pulls_the_next_page_from_real_git() {
        let dir = scratch_repo("pages");
        let commit = |i: usize| {
            std::fs::write(dir.join("a.txt"), format!("第 {i} 次改动\n")).unwrap();
            let out = std::process::Command::new("git")
                .args(["commit", "-q", "-am", &format!("翻页 {i}")])
                .current_dir(&dir)
                .output()
                .expect("跑 git");
            assert!(out.status.success(), "造提交失败：{}", String::from_utf8_lossy(&out.stderr));
        };
        for i in 1..=3 {
            commit(i);
        }
        let mut app = blank();
        app.open(dir.to_str().expect("utf-8 路径"));
        pump(&mut app, |a| a.snap.is_some());
        assert_eq!(app.snap.as_ref().unwrap().commits.len(), 4, "临时仓库该有 1+3 条提交");
        app.snap.as_mut().unwrap().commits.truncate(2);
        app.log_more = true;
        app.log_more_busy = false;
        pump(&mut app, |a| a.snap.as_ref().unwrap().commits.len() == 4);
        let s = app.snap.as_ref().unwrap();
        assert_eq!(s.graph.len(), 4, "翻页后图序列没重排");
        assert_eq!(s.badges.len(), 4, "翻页后药丸没重排");
        assert_eq!(s.dates.len(), 4, "翻页后日期没重排");
        // `--skip` 数错就会把已有那两条再接一遍：编号必须还是四个不同的。
        let ids: std::collections::HashSet<&str> = s.commits.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), 4, "接上来的页和已有的行重叠了");
        assert!(app.note.is_none(), "翻页路上报了错：{}", app.note.clone().unwrap_or_default());
        assert!(!app.log_more_busy, "在途标记没摘");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 下一页接上来时，三列「与提交一一对应」的派生数据要**一起**长齐——少一列，
    /// 新行就取到空的图/药丸/日期（画面上是那一列忽然白掉，不报错，所以只能靠断言）。
    /// 顺带钉住「旧页不许接」：切仓库/换排序之后序号会变，对不上号的整页丢掉。
    #[test]
    fn a_second_page_extends_every_parallel_column() {
        let mut app = loaded();
        let node = |i: usize| Commit {
            id: format!("{i:040}"),
            short_id: format!("p{i:03}"),
            subject: format!("第二页 {i}"),
            author: "包融".to_owned(),
            author_email: "br@example.com".to_owned(),
            date: "2026-10-19T11:41:07+08:00".to_owned(),
            parents: Vec::new(),
        };
        let tx = app.tx.clone();
        let _ = tx.send(Msg::More { repo: "/fixture".to_owned(), seq: app.log_seq, commits: vec![node(3), node(4)] });
        frame(&mut app);
        let s = app.snap.as_ref().unwrap();
        assert_eq!(s.commits.len(), 5, "两条一页没接上");
        assert_eq!(s.graph.len(), 5, "翻页后图序列少一列");
        assert_eq!(s.badges.len(), 5, "翻页后药丸少一列");
        assert_eq!(s.dates.len(), 5, "翻页后日期少一列");
        assert!(s.dates.iter().all(|d| !d.is_empty()), "新页的日期没换算");
        // 序号差一轮 = 这份页属于上一个仓库/上一种排序。
        let stale = app.log_seq + 1;
        let _ = tx.send(Msg::More { repo: "/fixture".to_owned(), seq: stale, commits: vec![node(9)] });
        frame(&mut app);
        assert_eq!(app.snap.as_ref().unwrap().commits.len(), 5, "旧序号的页也接了上去");
    }

    /// 「哪个提交上挂着什么引用」→药丸。顺序照参考图：本地分支在最前，紧跟它的超前数，
    /// 再是远程指针、标签。超前那枚只在 `ahead > 0` 时出现——没有可报的数就别占一格。
    #[test]
    fn pills_follow_the_refs_actually_attached_to_a_commit() {
        let node = |id: &str| Commit {
            id: id.to_owned(),
            short_id: id.to_owned(),
            subject: String::new(),
            author: String::new(),
            author_email: String::new(),
            date: String::new(),
            parents: Vec::new(),
        };
        let commits = vec![node("A"), node("B")];
        let at = |ahead: u32, behind: u32| Branch {
            name: "main".to_owned(),
            is_current: true,
            upstream: None,
            ahead,
            behind,
            target: "A".to_owned(),
        };
        let labels = |b: &[Branch], t: &[Tag], r: &[RemoteRef]| {
            ref_badges(&commits, b, t, r)
                .into_iter()
                .map(|row| row.iter().map(|x| (x.label.clone(), x.kind)).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(&[at(2, 0)], &[], &[]),
            vec![
                vec![("main".to_owned(), RefKind::Branch), ("超前2个版本".to_owned(), RefKind::Ahead)],
                Vec::new()
            ],
            "分支该落在它自己的那一行，且紧跟一枚超前数"
        );
        let mut off = at(0, 0);
        off.target = "B".to_owned();
        assert_eq!(labels(&[off], &[], &[])[0], Vec::new(), "B 行才是分支那一行");
        assert_eq!(
            labels(&[at(0, 3)], &[], &[])[0][1],
            ("落后3个版本".to_owned(), RefKind::Ahead),
            "只有落后数时也要报出来"
        );
        assert_eq!(labels(&[at(0, 0)], &[], &[])[0].len(), 1, "不前不后什么数都不该报");
        let stray = Branch { target: "zz".to_owned(), ..at(1, 1) };
        assert!(labels(&[stray], &[], &[]).iter().all(|r| r.is_empty()), "挂在不存在的提交上");
        assert_eq!(
            labels(&[], &[], &[RemoteRef { name: "origin/main".to_owned(), target: "B".to_owned() }])[1][0],
            ("origin/main".to_owned(), RefKind::Remote)
        );
        let all = labels(
            &[at(1, 0)],
            &[Tag { name: "v1.0".to_owned(), target: "A".to_owned(), annotated: true }],
            &[RemoteRef { name: "origin/main".to_owned(), target: "A".to_owned() }],
        );
        assert_eq!(
            all[0].iter().map(|(_, k)| *k).collect::<Vec<_>>(),
            vec![RefKind::Branch, RefKind::Ahead, RefKind::Remote, RefKind::Tag],
            "四枚同行时的先后"
        );
    }

    /// 和图序列同一个规矩：「引用→提交行」是整份列表的遍历（提交数 × 引用数遍字符串比较），
    /// 帧里一条都不许多跑。造一份快照匹配一次，之后五帧还是那一次。
    #[test]
    fn the_frame_never_matches_refs_to_rows() {
        let before = badge_layouts();
        let mut app = loaded();
        assert_eq!(badge_layouts(), before + 1, "取一份快照该匹配一次引用");
        for _ in 0..5 {
            frame(&mut app);
        }
        assert_eq!(
            badge_layouts(),
            before + 1,
            "五帧里又多匹配了 {} 次：药丸没跟着 snapshot 缓存",
            badge_layouts() - before - 1
        );
    }

    /// 字体链里没有这个码位，落出来就是一块豆腐——不是「不好看」，是那个字**没了**。
    /// 这条把一帧里每一块文字按**它自己的 FontId** 逐字查一遍：工具栏那 11 枚图标、
    /// 药丸前缀、下拉箭头全在内。曾经有 7 枚图标是这么死的，而无头状态下肉眼看不见。
    #[test]
    fn every_rune_the_shell_paints_has_a_glyph() {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut app = loaded();
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        // 这台机器没有可用的 CJK 字体时 `fonts::install` 直接 return，汉字本来就不可能绿——
        // 那是机器的事，不是外壳的事。用「修」探一下就知道装没装上。
        if !ctx.fonts_mut(|f| f.has_glyph(&FontId::proportional(12.0), '修')) {
            out.textures_delta.clear();
            eprintln!("跳过：这台机器上没有可用的 CJK 字体");
            return;
        }
        let mut missing: Vec<(char, FontId)> = Vec::new();
        for shape in &out.shapes {
            let egui::Shape::Text(t) = &shape.shape else { continue };
            for s in &t.galley.job.sections {
                let r = s.byte_range.start.0..s.byte_range.end.0;
                for ch in t.galley.job.text[r].chars() {
                    if !ctx.fonts_mut(|f| f.has_glyph(&s.format.font_id, ch))
                        && !missing.iter().any(|(m, _)| *m == ch)
                    {
                        missing.push((ch, s.format.font_id.clone()));
                    }
                }
            }
        }
        // 无头跑没有后端去应用字体纹理增量，不 clear 掉落栈时 panic（同 `fonts::install` 那条）。
        out.textures_delta.clear();
        assert!(missing.is_empty(), "外壳画出了字体链里没有的字形：{missing:?}");
    }

    /// 差异正文每一行都从**同一个 x** 起（SourceTree 的 diff 靠左）。
    /// 只比行首、不比内容：居中与两端对齐恰恰让每行的行首随文字长度漂，五行一比就露馅。
    #[test]
    fn diff_lines_share_one_left_edge() {
        let mut app = loaded();
        let out = frame(&mut app);
        let band = (WIN_H - STATUS_H - STAGING_H)..=(WIN_H - STATUS_H);
        let found = all_text_rects(&out, band, &["块", " 上下文", "-删掉", "+加上", "+又加"]);
        assert_eq!(found.len(), 6, "差异区该有六行文字（一条 hunk 头、两条上下文、一删两加）：{found:?}");
        let first = found[0].1.min.x;
        for (text, r) in &found[1..] {
            assert!(
                (r.min.x - first).abs() < 0.01,
                "差异行 {text:?} 的行首 {} 和 {first} 不齐——这一格被居中或两端对齐了",
                r.min.x
            );
        }
        // 行首要真的在两个行号栏**右边**，不然靠左靠是靠左了，却压在行号上。
        let gutter_right = SIDEBAR_W + FILES_W + NO_W * 2.0;
        assert!(first > gutter_right, "正文起点 {first} 没让出两个行号栏（右缘 {gutter_right}）");
    }

    /// 新克隆的数据库是空的，`repos` 也就是空的——这一条钉的是「空列表不是死路」：
    /// 工具栏上必须有一个能点的入口，填个路径就能开出仓库来。
    /// 填的是仓库里的**子目录**（`src`），顺带钉住「往上走到仓库根」那一步。
    #[test]
    fn an_empty_bookmark_list_still_offers_a_way_in() {
        let dir = scratch_repo("way-in");
        let sub = dir.join("src");
        std::fs::create_dir_all(&sub).expect("造子目录");
        let mut app = blank();
        assert!(click_text(&mut app, "添加仓库"), "工具栏上没有「添加仓库」，空书签列表就没有入口");
        assert!(type_into(&mut app, "仓库路径…", sub.to_str().unwrap()), "输入框没画出来");
        assert!(click_text(&mut app, "打开"), "路径填了但「打开」没出现");
        pump(&mut app, |a| a.snap.is_some());
        assert_eq!(app.repo.as_deref(), Some(dir.to_str().unwrap()), "填子目录没走到仓库根");
        assert_eq!(app.repos.len(), 1, "开起来的仓库没进书签列表");
        assert!(!app.add_open && app.add_path.is_empty(), "添加完成后那条输入没收起来");
    }

    /// 拒绝的那条路要留得下来改：给提示、不清空、不收起，更不能把仓库登记进去。
    #[test]
    fn a_path_that_is_not_a_repo_is_refused_without_touching_the_list() {
        let mut app = blank();
        assert!(click_text(&mut app, "添加仓库"));
        assert!(type_into(&mut app, "仓库路径…", "/definitely/not/a/repo-20261020"));
        assert!(click_text(&mut app, "打开"));
        assert!(
            app.note.as_deref().unwrap_or_default().contains("不是仓库"),
            "拒了一条还没给提示：{:?}",
            app.note
        );
        assert!(app.repos.is_empty() && app.repo.is_none(), "不是仓库的路径被登记进去并打开了");
        assert!(app.add_open && !app.add_path.is_empty(), "被拒后把输入清空了，用户没法改完再来");
    }

    /// 书签要**落库**：只在内存里记一份的话重启就没了，而这条通路 core 那边早就有函数，
    /// 缺的只是壳里那一次调用。
    #[test]
    fn an_added_repo_is_written_to_the_database() {
        let dir = scratch_repo("persist");
        let mut app = blank();
        let db_dir = std::env::temp_dir().join(format!("gitx-db-{}", std::process::id()));
        std::fs::create_dir_all(&db_dir).expect("造临时库目录");
        app.conn = Some(
            db::open(db_dir.join("gate.db").to_str().unwrap()).expect("临时库"),
        );
        app.add_open = true;
        app.add_path = dir.to_str().unwrap().to_owned();
        assert!(click_text(&mut app, "打开"));
        pump(&mut app, |a| a.snap.is_some());
        let listed = db::list_repos(app.conn.as_ref().unwrap()).expect("读回来");
        assert_eq!(
            listed.iter().filter(|r| r.path == dir.to_str().unwrap()).count(),
            1,
            "书签只进了内存，没进库：{listed:?}"
        );
    }

    // ------------------------------------------------------ 卡片 / 右键菜单
    //
    // 这一批是纯接线（核心层早就有 create_branch / create_tag / stash_* / delete_*），
    // 所以判据一律走端到端：**真点菜单 → 真填卡 → 真跑 git → 用另一条 git 命令复核**。
    // 只钉「界面上出现了什么」的话，接错线（卡关掉但动作没发、动作发了但打错提交）都还能绿。

    /// 无头点工具条第 `i` 枚。走**图标那一格文字**定位，不算格子：这一行是 `right_to_left`，
    /// 枚间距由 `style()` 的 `item_spacing` 说了算（实测 56，不是按钮本身的 50），
    /// 按 50 算出来的落点会点到隔壁那枚上——本批第一条判据就是这么红的。
    fn click_tool(app: &mut Gitx, i: usize) -> bool {
        let ctx = egui::Context::default();
        style(&ctx);
        let Some(at) = locate(&ctx, app, TOOLS[i].0) else {
            panic!("工具条第 {i} 枚（{}）没画出来", TOOLS[i].0);
        };
        click_at(&ctx, app, at);
        true
    }

    /// 在真仓库里跑一条 git、收回显。用来复核「app 说的」和「仓库里真的」是同一件事——
    /// 只信 app 的快照，等于让被测对象自己出证词。
    fn git_in(dir: &std::path::Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("跑 git");
        assert!(
            out.status.success(),
            "git {args:?}：{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    /// 按一个不带修饰键的键：按下、松手各一帧。
    fn press(app: &mut Gitx, k: egui::Key) {
        let key = |pressed: bool| egui::Event::Key {
            key: k,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame_events(app, vec![key(true)]);
        frame_events(app, vec![key(false)]);
    }

    /// 右键一行、再点它菜单里的某一项。**全程共用一个 `Context`**：菜单开合记在 ctx 的
    /// memory 里，换一个 Context 就等于菜单已经关了，第二步也就永远找不到那项（假绿）。
    /// 找不到行、找不到菜单项都 panic —— 静默的 `false` 会让这条在「菜单根本没画出来」下照样绿。
    fn menu_pick(app: &mut Gitx, over: &str, item: &str) {
        let ctx = egui::Context::default();
        style(&ctx);
        let Some(at) = locate(&ctx, app, over) else {
            panic!("窗口里没有「{over}」这一行");
        };
        let btn = |pressed: bool| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(at)],
            vec![egui::Event::PointerMoved(at), btn(true)],
            vec![egui::Event::PointerMoved(at), btn(false)],
            // 菜单挂在 `Order::Foreground`，松开那一帧才排进层里，所以要多跑一帧才数得到里面的字。
            vec![egui::Event::PointerMoved(at)],
        ] {
            ctx.run_ui(raw_input(events), |ui| app.shell(ui))
                .textures_delta
                .clear();
        }
        let Some(on) = locate(&ctx, app, item) else {
            panic!("「{over}」的右键菜单里没有「{item}」这一项");
        };
        click_at(&ctx, app, on);
    }

    /// 点开筛选条上那枚**弹出按钮**，再点菜单里的某一项。
    /// 两件事都得在同一个 `Context` 上做：菜单开合记在 ctx 的 memory 里，换一个 Context
    /// 就等于菜单已经关了（同 `menu_pick`）。而且它和模态卡一样落在 `Order::Foreground`
    /// （`popup.rs:148`），所以按完还得**多跑一帧**才数得到里面的字（见 `warm`）。
    /// 菜单项和按钮**可能同名**（「层级顺序」既是当前那一项、也在菜单里），所以找项时只认
    /// 按钮**下方**那块文字——不然这条会点到按钮自己，菜单开开关关，什么都没改，照样绿。
    fn popup_pick(app: &mut Gitx, button: &str, item: &str) {
        let ctx = warm(app);
        // 按钮和菜单项按同一套规矩找：中央得在侧栏**右边**（整条筛选条就在日志面板里）。
        // 用现成的 `locate` 不行——侧栏也把每条分支画成一行，而侧栏画在日志面板**之前**，
        // 按前缀「side」先撞上的是侧栏那一行：它点不出菜单，于是第二次挑项报「菜单里没这项」。
        let mut first = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let at = first
            .shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text().starts_with(button) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    (r.center().x > SIDEBAR_W + 1.0).then_some(r.center())
                }
                _ => None,
            });
        first.textures_delta.clear();
        let Some(at) = at else { panic!("筛选条上没有「{button}」这枚按钮") };
        click_at(&ctx, app, at);
        // **只认菜单里那一块**，光按「按钮下方第一块同名文字」筛不够：侧栏本来就把每条分支
        // 画在按钮下面那片区域里，于是这一下点到的是侧栏的分支行，菜单开开关关、
        // 范围一个字没改，判据照样能绿（本批实测红过一次，红的是另一处）。
        // 弹出层和按钮同在日志面板里，所以中央必须落在侧栏**右边**；同名候选取最上面那块。
        let below = |app: &mut Gitx| {
            let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
            let mut hits: Vec<(f32, Pos2)> = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) if t.galley.text() == item => {
                        let r = t.galley.rect.translate(t.pos.to_vec2());
                        (r.center().y > at.y + 4.0 && r.center().x > SIDEBAR_W + 1.0)
                            .then_some((r.center().y, r.center()))
                    }
                    _ => None,
                })
                .collect();
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            out.textures_delta.clear();
            hits.into_iter().next().map(|(_, p)| p)
        };
        below(app);
        let Some(on) = below(app) else {
            panic!("「{button}」的菜单里没有「{item}」这一项");
        };
        click_at(&ctx, app, on);
    }

    /// 日志当前列出的**主题**。范围、排序这俩开关改的都是这份列表，判据比的是内容不是行数
    /// （两条分支各 2 个提交时，行数一模一样）。
    fn line_of(app: &Gitx) -> Vec<String> {
        app.snap.as_ref().map(|s| s.commits.iter().map(|c| c.subject.clone()).collect()).unwrap_or_default()
    }

    /// 筛选条上以 `prefix` 起笔的那块文字，连箭头一起取回来——要看的是「当前哪一档」
    /// 和「第二层在不在」。
    fn filter_text(app: &mut Gitx, prefix: &str) -> Option<String> {
        let out = frame(app);
        texts(&out).into_iter().find(|(t, _)| t.starts_with(prefix)).map(|(t, _)| t)
    }

    /// 日志**数据区**那一格里画了哪些文字。表头以上、暂存带以下都筛掉：
    /// 侧栏和筛选条也有同名文字，整屏去数会把「行上的药丸」和「别处的字符串」混成一件事。
    fn row_texts(app: &mut Gitx) -> Vec<String> {
        let out = frame(app);
        let band = rows_band(&out);
        texts(&out)
            .into_iter()
            .filter(|(_, p)| band.contains(&p.y))
            .map(|(t, _)| t)
            .collect()
    }

    /// 工具条「分支」→ 卡 → 敲名字 → 回车：分支真长出来了，卡也跟着关掉。
    #[test]
    fn the_branch_card_creates_a_real_branch() {
        let (mut app, dir) = open_repo("card-new-branch");
        let before = app.snap.as_ref().unwrap().branches.len();
        click_tool(&mut app, 4);
        assert!(app.modal.is_some(), "点工具条「分支」没开出卡");
        assert!(type_into(&mut app, "分支名", "卡片长出来的"), "卡里那格输入框没找到");
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| {
            a.snap.as_ref().map(|s| s.branches.len()).unwrap_or(0) > before
        });
        assert!(app.modal.is_none(), "回车之后卡没关");
        assert!(
            git_in(&dir, &["branch", "--format=%(refname:short)"]).contains("卡片长出来的"),
            "app 说有这条分支，仓库里没有"
        );
    }

    /// 回车是**主按钮**那条路；空名字不许关卡（git 会拒，但先把该填什么留在那儿）。
    /// 这一条钉的是 `primary_action` 里那句「名字空着返回 None」。
    #[test]
    fn an_empty_name_keeps_the_card_open() {
        let (mut app, dir) = open_repo("card-empty-name");
        let before = git_in(&dir, &["branch", "--format=%(refname:short)"]);
        click_tool(&mut app, 4);
        press(&mut app, egui::Key::Enter);
        frame(&mut app);
        assert!(app.modal.is_some(), "名字还没填就把卡关了");
        assert_eq!(
            git_in(&dir, &["branch", "--format=%(refname:short)"]),
            before,
            "名字空着也把分支建出来了"
        );
        // 关卡与建分支都会改 `note`/快照；这两样都没动才说明那一下真的什么都没做。
        assert!(app.note.is_none(), "空名字那一下报了错：{}", app.note.clone().unwrap_or_default());
        assert_eq!(app.snap.as_ref().unwrap().branches.len(), before.lines().count());
    }

    /// Esc 只关卡，不动 git。少了这条，「先关卡再执行」那种写法也能混过去。
    #[test]
    fn escape_closes_the_card_without_doing_the_thing() {
        let (mut app, dir) = open_repo("card-esc");
        click_tool(&mut app, 6);
        assert!(app.modal.is_some(), "点「贮藏」没开出卡");
        assert!(type_into(&mut app, "贮藏说明", "这条不该存在"));
        press(&mut app, egui::Key::Escape);
        assert!(app.modal.is_none(), "Esc 没关卡");
        assert!(app.snap.as_ref().unwrap().stashes.is_empty(), "Esc 把贮藏也办掉了");
        assert_eq!(git_in(&dir, &["stash", "list"]), "", "仓库里真的多出一条贮藏");
    }

    /// 卡开着的时候后面那一层点不着：点「历史」不会改变选中项，只会把卡关掉
    /// （遮罩那一下就是「点外面」）。这条钉的是那格铺满整屏的遮罩矩形——**形状和后果都要问**：
    /// 只问后果，「遮罩缩小了但那一行恰好没被点到」能绿；只问形状，「遮罩画了但没接点击」也能绿。
    /// 所以再加一遍**没卡的时候同坐标点得到**的对照，少了它测的就是空气。
    #[test]
    fn a_card_swallows_everything_behind_it() {
        let (mut app, _) = open_repo("card-mask");
        app.sel = Sel::Uncommitted;
        assert!(click_warm(&mut app, "历史"), "窗口里没有侧栏「历史」那一行");
        assert!(matches!(app.sel, Sel::Commit(_)), "没卡的时候这一下都点不着，那遮罩测的是空气");
        app.sel = Sel::Uncommitted;
        click_tool(&mut app, 4);
        assert!(app.modal.is_some(), "卡没开起来，那后面测的就是空气");
        let out = warm_frame(&mut app);
        let want = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H));
        // 遮罩认「铺满整屏 + 黑色半透明」，不认具体 alpha：egui 会把已经注册过一遍的
        // `Area` 那一层整体压暗（实测写进去的 80 出来是 19，`area.rs:706` 那句
        // `ui.multiply_opacity`），钉死数值就是钉一个 egui 自己会变的数。
        let dark = rects(&out)
            .into_iter()
            .filter(|(_, r)| *r == want)
            .map(|(c, _)| c.to_tuple())
            .filter(|(r, g, b, a)| *r == 0 && *g == 0 && *b == 0 && *a > 0 && *a < 255)
            .count();
        assert!(dark > 0, "遮罩没铺满整屏（找 {want:?} 那一格的黑色半透明底）");
        assert!(click_warm(&mut app, "历史"), "卡开着的时候侧栏「历史」那一行找不到了");
        assert!(matches!(app.sel, Sel::Uncommitted), "隔着卡片点到后面那一行了");
        assert!(app.modal.is_none(), "点卡片外面应该关卡");
    }

    /// 破坏性动作的闸在**出口**那一处，不在每一张菜单里：右键「删除这个分支」之后
    /// 分支必须还在、卡必须已经摊开，按「确定」才真删。
    #[test]
    fn deleting_a_branch_asks_first_then_actually_deletes() {
        let dir = scratch_repo("menu-delete");
        git_in(&dir, &["branch", "可以删的那条"]);
        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| {
            a.snap.as_ref().map(|s| s.branches.len()).unwrap_or(0) >= 2
        });
        menu_pick(&mut app, "可以删的那条", "删除这个分支");
        assert!(app.modal.is_some(), "删分支没先问一句");
        assert!(
            app.snap.as_ref().unwrap().branches.iter().any(|b| b.name == "可以删的那条"),
            "问都没问就删了"
        );
        assert!(click_warm(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |a| {
            !a.snap.as_ref().unwrap().branches.iter().any(|b| b.name == "可以删的那条")
        });
        assert!(
            !git_in(&dir, &["branch", "--format=%(refname:short)"]).contains("可以删的那条"),
            "点了确定，仓库里那条分支还在"
        );
    }

    /// 重命名分支：右键 → 卡 → 输入新名字 → 「重命名」，ref 真的换了名字，
    /// 旧名字从 `git branch` 里消失。
    #[test]
    fn renaming_a_branch_lands_a_real_rename() {
        let dir = scratch_repo("menu-rename");
        git_in(&dir, &["branch", "旧名字"]);
        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| {
            a.snap.as_ref().map(|s| s.branches.len()).unwrap_or(0) >= 2
        });
        menu_pick(&mut app, "旧名字", "重命名分支…");
        assert!(app.modal.is_some(), "点了重命名，卡没摊开");
        assert!(type_into(&mut app, "新分支名", "新名字"), "卡里没有「新分支名」那一格");
        // 主按钮文案「重命名」和卡标题「重命名分支」同前缀，按字定位会先撞上标题，
        // 所以这里走 Enter —— 它和点主按钮是同一条 `primary_action` 出口。
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| {
            a.snap.as_ref().unwrap().branches.iter().any(|b| b.name == "新名字")
        });
        let names = git_in(&dir, &["branch", "--format=%(refname:short)"]);
        assert!(names.contains("新名字"), "仓库里没有新名字：{names:?}");
        assert!(!names.contains("旧名字"), "旧名字还在：{names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「在这条提交上打标签」：标签落点必须是**打开那一刻**那条提交，不是 HEAD。
    /// 所以夹具里补了第二条提交，打在最早那条上，再用 `rev-parse` 比出高低。
    #[test]
    fn a_tag_lands_on_the_commit_the_menu_was_opened_on() {
        let (mut app, dir) = open_repo("menu-tag");
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n第三行\n").unwrap();
        git_in(&dir, &["commit", "-aqm", "第二条"]);
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()).unwrap_or(0) >= 2);
        menu_pick(&mut app, "起点", "在这条提交上打标签");
        assert!(app.modal.is_some(), "日志行右键没开出打标签的卡");
        assert!(type_into(&mut app, "标签名", "v0.1"));
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| a.snap.as_ref().map(|s| !s.tags.is_empty()).unwrap_or(false));
        let tagged = git_in(&dir, &["rev-parse", "v0.1"]);
        assert_eq!(tagged, git_in(&dir, &["rev-parse", "HEAD~1"]), "标签打错了提交");
        assert_ne!(tagged, git_in(&dir, &["rev-parse", "HEAD"]), "标签打到了 HEAD 上");
    }

    /// 贮藏的来回：卡里存进去 → 侧栏那一栏右键丢出来。顺手钉确认闸对贮藏也生效。
    #[test]
    fn a_stash_round_trips_through_the_sidebar() {
        let (mut app, dir) = open_repo("card-stash");
        click_tool(&mut app, 6);
        assert!(type_into(&mut app, "贮藏说明", "先收起来"), "贮藏卡里那格输入框没找到");
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.len()).unwrap_or(0) == 1);
        assert!(git_in(&dir, &["stash", "list"]).contains("先收起来"));
        menu_pick(&mut app, "stash@{0}", "丢弃这条贮藏");
        assert!(app.modal.is_some(), "丢弃贮藏没先问一句");
        assert!(click_warm(&mut app, "确定"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.is_empty()).unwrap_or(false));
        assert_eq!(git_in(&dir, &["stash", "list"]), "");
    }

    /// 文件行的右键菜单只动那一个文件。
    #[test]
    fn the_file_row_menu_stages_only_that_file() {
        let (mut app, dir) = two_dirty_files("menu-stage");
        menu_pick(&mut app, "b.txt", "暂存这个文件");
        pump(&mut app, |a| staged_in_app(a) == vec!["b.txt".to_owned()]);
        assert_eq!(git_in(&dir, &["diff", "--name-only"]), "c.txt", "把 c.txt 也一起暂存了");
    }

    /// 「忽略这个文件」只改 .gitignore；未跟踪那一行点「放弃」是**删文件**，文案得说实话。
    /// 两条都拿 `git status` 复核：只看 app 的快照等于让被测对象自己出证词。
    #[test]
    fn the_file_row_menu_ignores_and_deletes() {
        let (mut app, dir) = two_dirty_files("menu-ignore");
        std::fs::write(dir.join("d.txt"), "丁\n").unwrap();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| {
            a.snap.as_ref().map(|s| s.status.files.len()).unwrap_or(0) >= 3
        });
        menu_pick(&mut app, "d.txt", "忽略这个文件（写进 .gitignore）");
        pump(&mut app, |a| !a.snap.as_ref().unwrap().status.files.iter().any(|f| f.path == "d.txt"));
        let ignore = std::fs::read_to_string(dir.join(".gitignore")).unwrap_or_default();
        assert!(ignore.lines().any(|l| l.trim() == "d.txt"), ".gitignore 里没写上：{ignore:?}");
        assert!(dir.join("d.txt").exists(), "忽略只该改 .gitignore，不该删文件");
        assert!(
            !git_in(&dir, &["status", "--porcelain"]).contains("d.txt"),
            "git 眼里 d.txt 还没被忽略"
        );
        menu_pick(&mut app, "c.txt", "放弃这个文件的改动");
        pump(&mut app, |a| !a.snap.as_ref().unwrap().status.files.iter().any(|f| f.path == "c.txt"));
        assert!(
            !git_in(&dir, &["status", "--porcelain"]).contains("c.txt"),
            "c.txt 的改动还在"
        );
        assert_eq!(std::fs::read_to_string(dir.join("c.txt")).unwrap(), "丙\n", "放弃没把文件放回原样");
    }

    // ------------------------------------------------------------------- 克隆
    //
    // 源用**本机路径**当远端：门禁不许连真远端，也不许假设这台机器上有某个仓库。
    // `git clone /路径 /另一个路径` 走的是同一条代码通路（同一份 `git::clone_repo`），
    // 换 https 只是换个字符串。

    /// 「添加仓库」展开那一条 →「克隆…」→ 两格都填 → 真 `git clone`：
    /// 书签里多一条、tab 开到它上面、HEAD 与 `origin` 都对得上源。
    #[test]
    fn the_clone_card_makes_a_working_copy() {
        let src = scratch_repo("clone-src");
        let dst = std::env::temp_dir().join(format!("gitx-clone-dst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        let mut app = blank();
        assert!(click_text(&mut app, "添加仓库"), "tab 条上没有「添加仓库」");
        assert!(click_text(&mut app, "克隆…"), "展开的那一条里没有「克隆…」");
        assert!(app.modal.is_some(), "点「克隆…」没开出卡");
        assert!(type_into(&mut app, "源仓库地址", src.to_str().unwrap()), "卡里第一格没找到");
        assert!(type_into(&mut app, "目标目录", dst.to_str().unwrap()), "卡里第二格没找到");
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| !a.repos.is_empty());
        assert_eq!(app.repos[0].path, dst.to_str().unwrap(), "书签加错了目录");
        assert_eq!(app.repo.as_deref(), Some(dst.to_str().unwrap()), "克隆完没把新仓库打开");
        assert_eq!(
            git_in(&dst, &["rev-parse", "HEAD"]),
            git_in(&src, &["rev-parse", "HEAD"]),
            "克隆出来的 HEAD 跟源对不上"
        );
        assert_eq!(git_in(&dst, &["remote", "get-url", "origin"]), src.to_str().unwrap());
        pump(&mut app, |a| a.snap.is_some());
        let _ = std::fs::remove_dir_all(&dst);
        let _ = std::fs::remove_dir_all(&src);
    }

    /// 起点仓库 + 一条真正改过 `a.txt` 的提交，再在工作区留一处未暂存改动：
    /// 文件列表里有 `a.txt` 可右键，历史里有两条可挑。
    /// `tag` 规矩同 `scratch_repo`：每条判据一个。
    fn history_repo(tag: &str) -> (Gitx, std::path::PathBuf) {
        let (mut app, dir) = open_repo(tag);
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n第三行\n").unwrap();
        git_in(&dir, &["commit", "-aqm", "第二条改 a.txt"]);
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n第三行\n第四行\n").unwrap();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()).unwrap_or(0) == 2);
        (app, dir)
    }

    /// 从 `a.txt` 的右键菜单打开历史，等到列表与第一条补丁都落地。
    fn open_history_for_test(app: &mut Gitx) {
        menu_pick(app, "a.txt", "查看文件历史");
        pump(app, |a| a.history.as_ref().is_some_and(|h| !h.log_loading && h.entries.len() == 2));
        pump(app, |a| a.history.as_ref().is_some_and(|h| !h.diff.is_empty()));
    }

    /// 面板里的提交说明和日志里的**同一串字**都用同一份字体档？不是：日志行 12.5，
    /// 面板行 11.5。按文字找会先撞上遮罩后面的日志行——点它等于点了遮罩，把面板关掉。
    fn history_click(app: &mut Gitx, subject: &str) {
        let ctx = warm(app);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let at = out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t)
                if t.galley.text() == subject
                    && t.galley
                        .job
                        .sections
                        .iter()
                        .any(|s| (s.format.font_id.size - 11.5).abs() < 0.01) =>
            {
                Some(t.galley.rect.translate(t.pos.to_vec2()).center())
            }
            _ => None,
        });
        out.textures_delta.clear();
        let at = at.unwrap_or_else(|| panic!("历史面板里没有「{subject}」这一行"));
        click_at(&ctx, app, at);
    }

    /// 右键「查看文件历史」→ `git log --follow` 的列表真的落到面板上，
    /// 并且默认选中最新那条、右侧是那条提交对这个文件的补丁。
    #[test]
    fn the_file_history_lists_the_commits_that_touched_the_file() {
        let (mut app, dir) = history_repo("hist-list");
        open_history_for_test(&mut app);
        {
            let h = app.history.as_ref().unwrap();
            assert_eq!(h.path, "a.txt");
            assert_eq!(h.entries[0].commit.subject, "第二条改 a.txt");
            assert_eq!(h.entries[1].commit.subject, "起点");
            assert_eq!(h.sel, 0);
            assert!(
                h.diff.iter().any(|l| l.text.contains("第三行")),
                "选中最新那条，右侧却不是它加的第三行：{:?}",
                h.diff.iter().map(|l| &l.text).collect::<Vec<_>>()
            );
        }
        let out = warm_frame(&mut app);
        let all: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).collect();
        assert!(all.iter().any(|t| t.starts_with("文件历史 · a.txt")), "面板标题没画出来");
        assert!(all.iter().any(|t| t == "逐行归属"), "页签没画出来");
        assert!(all.iter().any(|t| t == "第二条改 a.txt"), "历史行没画出来");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 挑更早的一条，右侧补丁跟着换。钉的是「选中项 → 新请求 → 同序号同路径才落地」
    /// 这条链，而不是补丁内容本身。
    #[test]
    fn picking_an_older_entry_swaps_the_patch() {
        let (mut app, dir) = history_repo("hist-pick");
        open_history_for_test(&mut app);
        history_click(&mut app, "起点");
        pump(&mut app, |a| {
            a.history
                .as_ref()
                .is_some_and(|h| h.sel == 1 && !h.patch_loading && !h.diff.is_empty())
        });
        {
            let h = app.history.as_ref().unwrap();
            assert!(h.diff.iter().any(|l| l.text.contains("一行")), "旧版补丁里没有它新增的那行");
            assert!(!h.diff.iter().any(|l| l.text.contains("第三行")), "旧版补丁里混进了新版的行");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 逐行归属页签走真 `git blame`：工作区那份四行，第二行必须是 tester 写的。
    #[test]
    fn the_blame_tab_attributes_each_line_to_a_commit() {
        let (mut app, dir) = history_repo("hist-blame");
        open_history_for_test(&mut app);
        assert!(click_warm(&mut app, "逐行归属"), "页签没找到");
        pump(&mut app, |a| {
            a.history
                .as_ref()
                .is_some_and(|h| h.tab == HistTab::Blame && !h.blame_loading && !h.blame.is_empty())
        });
        {
            let h = app.history.as_ref().unwrap();
            assert_eq!(h.blame.len(), 4, "工作区那份是四行");
            assert_eq!(h.blame[1].content, "第二行");
            assert_eq!(h.blame[1].author, "tester");
            assert!(
                h.blame[1].short_id.starts_with(&h.entries[0].commit.short_id),
                "第二行应该归最新那条提交：blame {} vs 历史 {}",
                h.blame[1].short_id,
                h.entries[0].commit.short_id
            );
        }
        let out = warm_frame(&mut app);
        let all: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).collect();
        assert!(all.iter().any(|t| t == "第二行"), "blame 内容没画出来");
        assert!(all.iter().any(|t| t == "tester"), "blame 作者没画出来");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「全文」看的是那个版本的原文件，不是补丁。换选中项之后全文也要跟着换。
    #[test]
    fn the_full_text_view_shows_the_file_at_that_version() {
        let (mut app, dir) = history_repo("hist-full");
        open_history_for_test(&mut app);
        assert!(click_warm(&mut app, "全文"), "「全文」按钮没找到");
        pump(&mut app, |a| {
            a.history
                .as_ref()
                .is_some_and(|h| h.full_for == Some(0) && !h.full_loading && !h.full.is_empty())
        });
        assert_eq!(app.history.as_ref().unwrap().full.join("\n"), "一行\n第二行\n第三行");
        history_click(&mut app, "起点");
        pump(&mut app, |a| {
            a.history
                .as_ref()
                .is_some_and(|h| h.sel == 1 && h.full_for == Some(1) && !h.full_loading)
        });
        assert_eq!(app.history.as_ref().unwrap().full.join("\n"), "一行");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 覆盖工作区必须先问一句；问完点「确定」才真的把旧版拉回来。
    #[test]
    fn restoring_an_old_version_asks_before_it_overwrites_the_worktree() {
        let (mut app, dir) = history_repo("hist-restore");
        open_history_for_test(&mut app);
        history_click(&mut app, "起点");
        pump(&mut app, |a| {
            a.history.as_ref().is_some_and(|h| h.sel == 1 && !h.patch_loading)
        });
        assert!(click_warm(&mut app, "恢复到这一版"), "恢复按钮没找到");
        assert!(app.modal.is_some(), "没问一句就动手了");
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "一行\n第二行\n第三行\n第四行\n",
            "确认之前工作区就被改了"
        );
        assert!(click_warm(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |_| {
            std::fs::read_to_string(dir.join("a.txt")).map(|s| s == "一行\n").unwrap_or(false)
        });
        assert!(app.history.is_some(), "恢复完把历史面板一起关了");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Esc 和点遮罩都能关面板。后者钉的是「整屏那一格可点矩形真的在面板外面」。
    #[test]
    fn the_file_history_layer_closes_on_escape_and_outside_click() {
        let (mut app, dir) = history_repo("hist-close");
        open_history_for_test(&mut app);
        press(&mut app, egui::Key::Escape);
        frame(&mut app);
        assert!(app.history.is_none(), "Esc 没关掉面板");

        open_history_for_test(&mut app);
        // 按/松必须在**同一个 Context** 里：`frame_events` 每调一次都是新 Context，
        // egui 的点击判定记在上一次 memory 里，分开发永远是「没点过」。
        let ctx = warm(&mut app);
        let at = pos2(12.0, 12.0);
        let btn = |pressed: bool| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(at)],
            vec![egui::Event::PointerMoved(at), btn(true)],
            vec![egui::Event::PointerMoved(at), btn(false)],
            vec![],
        ] {
            ctx.run_ui(raw_input(events), |ui| app.shell(ui)).textures_delta.clear();
        }
        assert!(app.history.is_none(), "点面板外面没关掉");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 只填地址、目标目录还空着：卡不许关，而且**不许留下半个目录**——`git clone` 一旦开跑
    /// 就先把目录建出来，所以这一条钉的是主按钮压根没点下去。
    #[test]
    fn a_clone_needs_both_fields() {
        let src = scratch_repo("clone-half");
        let dst = std::env::temp_dir().join(format!("gitx-clone-half-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        let mut app = blank();
        click_text(&mut app, "添加仓库");
        click_text(&mut app, "克隆…");
        assert!(type_into(&mut app, "源仓库地址", src.to_str().unwrap()));
        press(&mut app, egui::Key::Enter);
        frame(&mut app);
        assert!(app.modal.is_some(), "目标目录还空着就把卡关了");
        assert!(!dst.exists(), "主按钮按下去了：{dst:?} 被建出来");
        assert!(app.repos.is_empty(), "书签里冒出了一条不该有的仓库");
        let _ = std::fs::remove_dir_all(&src);
    }

    // ------------------------------------------------------------------- 冲突解决与继续/中止

    /// 打开一个真停在 merge 冲突里的仓库。`a.txt` 在 `side` 上是「他们的」，
    /// 在当前分支上是「我的」，工作区那份带冲突标记。
    fn conflict_repo(tag: &str) -> (Gitx, std::path::PathBuf) {
        let dir = scratch_repo(tag);
        // `scratch_repo` 留了一处已暂存改动，先落一条提交把工作区弄干净。
        git_in(&dir, &["commit", "-qm", "当前分支"]);
        let base = git_in(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
        git_in(&dir, &["checkout", "-q", "-b", "side"]);
        std::fs::write(dir.join("a.txt"), "他们的\n").unwrap();
        git_in(&dir, &["commit", "-aqm", "他们改"]);
        git_in(&dir, &["checkout", "-q", &base]);
        std::fs::write(dir.join("a.txt"), "我的\n").unwrap();
        git_in(&dir, &["commit", "-aqm", "我改"]);
        let out = std::process::Command::new("git")
            .args(["merge", "side"])
            .current_dir(&dir)
            .output()
            .expect("跑 git merge");
        assert!(!out.status.success(), "夹具的 merge 没冲突");

        let path = dir.to_str().expect("utf-8 路径").to_owned();
        let mut app = blank();
        app.open(&path);
        pump(&mut app, |a| a.snap.is_some());
        (app, dir)
    }

    /// 横幅和分组：停在合并里的仓库，文件表上方写着「未解决的合并：冲突：1」，
    /// 下面单独一组「冲突的文件（1）」，而**不再**混进「已暂存文件」。
    #[test]
    fn a_half_finished_merge_gets_its_own_banner_and_conflict_group() {
        let (mut app, dir) = conflict_repo("conflict-group");
        let snap = app.snap.as_ref().unwrap();
        assert_eq!(snap.status.repo_state, "merge");
        assert_eq!(snap.status.files.len(), 1, "冲突夹具里应该只有一个文件");
        assert!(snap.status.files[0].staged, "`u` 行在 core 里是 staged");

        let out = warm_frame(&mut app);
        let all: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).collect();
        assert!(
            all.iter().any(|t| t == "未解决的合并：冲突：1"),
            "横幅没画出来：{all:?}"
        );
        assert!(all.iter().any(|t| t == "冲突的文件（1）"), "冲突分组没画出来：{all:?}");
        assert!(
            !all.iter().any(|t| t.starts_with("已暂存文件（")),
            "冲突文件混进了已暂存那一组：{all:?}"
        );
        assert!(all.iter().any(|t| t == "继续"), "横幅上没有「继续」");
        assert!(all.iter().any(|t| t == "终止"), "横幅上没有「终止」");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 端到端：右键「使用我的版本解决」→ 确认卡 → 真 git 解决 → 冲突分组消失、
    /// 横幅还在 →「继续」→ 确认 → merge 提交落地。
    #[test]
    fn resolving_with_ours_and_continuing_finishes_the_merge() {
        let (mut app, dir) = conflict_repo("conflict-ours");
        menu_pick(&mut app, "a.txt", "使用「我的」版本解决");
        assert!(app.modal.is_some(), "覆盖工作区之前没先问一句");
        assert!(click_warm(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |a| {
            a.snap
                .as_ref()
                .is_some_and(|s| !s.status.files.iter().any(is_conflict))
        });
        // 「我的」那一侧就是 HEAD 里的内容，暂存之后索引和 HEAD 一致，porcelain 是空的；
        // 但 merge 还在进行中（MERGE_HEAD 还在），下面那条横幅判据才是关键。
        assert_eq!(git_in(&dir, &["status", "--porcelain"]), "");
        let out = warm_frame(&mut app);
        let all: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).collect();
        assert!(!all.iter().any(|t| t.starts_with("冲突的文件（")), "解决了还挂着冲突分组");
        assert!(all.iter().any(|t| t == "未解决的合并："), "横幅跟着冲突一起消失了");

        assert!(click_text(&mut app, "继续"), "横幅上没有「继续」");
        assert!(app.modal.is_some(), "继续之前没先问一句");
        assert!(click_warm(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |a| {
            a.snap.as_ref().is_some_and(|s| s.status.repo_state == "none")
        });
        assert_eq!(
            git_in(&dir, &["rev-list", "--parents", "-1", "HEAD"])
                .split_whitespace()
                .count(),
            3,
            "继续之后 HEAD 不是一条合并提交"
        );
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "我的\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「终止」必须先过确认卡，而且卡上按「取消」一个字节都不许动。
    #[test]
    fn aborting_a_merge_asks_before_it_rolls_back() {
        let (mut app, dir) = conflict_repo("conflict-abort");
        assert!(click_text(&mut app, "终止"), "横幅上没有「终止」");
        assert!(app.modal.is_some(), "终止没先问一句");
        assert!(click_warm(&mut app, "取消"), "确认卡上没有「取消」");
        frame(&mut app);
        assert_eq!(
            app.snap.as_ref().unwrap().status.repo_state,
            "merge",
            "点了取消，操作还是被终止了"
        );
        assert!(dir.join("a.txt").exists(), "取消之后工作区文件没了");

        assert!(click_text(&mut app, "终止"), "横幅上没有「终止」");
        assert!(click_warm(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |a| {
            a.snap.as_ref().is_some_and(|s| s.status.repo_state == "none")
        });
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "我的\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

