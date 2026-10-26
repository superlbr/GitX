//! 门禁：全部无头断言。模块名保持 geometry_tests（scripts 与 PROCESS 里的过滤串不变）。

use eframe::egui::{self, Color32, FontId, Pos2, Rect, pos2, vec2};
use gitx::models::{
    Branch, ChangeStatus, Commit, DiffEntry, FileStatus, Remote, RemoteRef, Repo, RepoStatus, Tag,
};
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
        // 先问一句「有没有数据」：`snap` 下面就要被移进结构体，那之后再 `snap.as_ref()`
        // 是借一个已经搬走的值（编译不过，不是风格问题）。
        let has_data = snap.is_some();
        Gitx {
            conn: None,
            repos: Vec::new(),
            repo: snap.as_ref().map(|_| "/fixture".to_owned()),
            snap,
            sel: Sel::Uncommitted,
            commit_files: Vec::new(),
            file: None,
            diff_rows: (0..diff.len()).collect(),
            collapsed: Default::default(),
            diff_search: String::new(),
            diff,
            diff_cols: 0,
            patch_label: "全部未暂存变更".to_owned(),
            seq: 0,
            busy: false,
            note: None,
            note_at: None,
            log_filter: String::new(),
            side_filter: String::new(),
            topo: false,
            scope: git::Scope::Current,
            show_remote: true,
            // 侧栏「远端」默认全收着：展开状态是看仓库时的临时状态，夹具不该凭空摆一个开着的。
            remote_open: Vec::new(),
            // 夹具没有数据库（`conn: None`），这一格只影响 `open()` 之后要不要再去摸上一次那本；
            // 默认按 `Gitx::new` 里 `Prefs::load(None)` 的结果填 true，免得夹具自己造一个新默认。
            restore_last: true,
            // 下面四条同样按 `Prefs::load(None)`（没有数据库那层）的结果填：夹具要测的是
            // 「卡打开时长什么样」，初值必须由判据自己摆得上来，而不是靠构造函数偷偷读库。
            push_behavior: PushBehavior::Matching,
            default_remote: String::new(),
            push_tags: false,
            allow_force: true,
            // 夹具里把「还有下一页」拧死：不然「滚到底要下一页」那格判据会在无头帧里真起线程，
            // 拿 `/fixture` 当仓库跑 git，回来的 `Msg::Failed` 会污染别的断言。
            log_more: false,
            log_more_busy: false,
            log_seq: 0,
            // 搜索那一档夹具里同样是关着的：判据要它开，得自己走那条真通路（点侧栏「搜索」），
            // 而不是构造函数偷偷摆一个开着的状态——不然「那一行点得动吗」这条判据就没东西可杀。
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
            // 夹具默认停在「全部未暂存变更」那一格：和上面 `patch_label` 对得上，
            // 也让 hunk 那两枚按钮在整份补丁上**不出现**（没有单一文件路径可打）。
            target: Some(Target::Working { staged: false, path: None, untracked: false }),
            pending_discard: None,
            history: None,
            hist_seq: 0,
            modal: None,
            modal_focus: false,
            pending_dialog: None,
            // 夹具没有真的走过 `Msg::Loaded`，这一位按「快照里那条分支就是最后一次看到的
            // 分支」填：有数据 = `main`，空态壳 = 空串（和 `repo` 同一条件）。
            last_branch: if has_data { "main".to_owned() } else { String::new() },
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
        filled(3, &layout(PATCH, false))
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
                // 夹具走的是「在分支上」那一条；分离那一档由判据自己摆上来（要钉的正是
                // 状态栏换了写法之后的样子），不在公共夹具里替它做主。
                detached_at: None,
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
        let mut app = filled(3, &layout(PATCH, false));
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

    /// 顶栏那 46pt 里**任何两块文字都不许交叠**。
    /// 上一批的重叠就是这么量出来的：`书签 ▼` 压在 `⤴` 上、`添加仓库` 压在 `≡` 上，
    /// 目视只看到「顶部栏有点乱」，说不清是谁压了谁。
    /// 仓库给到 6 个、提示给到 30 字，是为了把这一条塞满——空列表那帧什么都压不到。
    /// §2 写了「窗口收窄时溢出收进 `»` 菜单」，**那一档还没实现**；tab 组超出宽度会被面板
    /// 裁掉（预览稿 `.tabs{min-width:0}` 也是这个行为），裁掉不算压住。
    #[test]
    fn the_header_has_no_overlapping_widgets() {
        let mut app = filled(3, &layout(PATCH, false));
        for i in 0..6 {
            app.repos.push(Repo {
                id: None,
                path: format!("/fixture{i}"),
                name: format!("仓库{i}"),
                added_at: None,
                last_fetched_at: None,
                group_name: String::new(),
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
                    (r.center().y <= HEADER_H).then(|| (t.galley.text().to_owned(), r))
                }
                _ => None,
            })
            .collect();
        // 探针：这一条至少得认出 6 枚 tab + 1 枚幽灵 + 11 枚图标 + 分支名 + 计数 = 20 块文字。
        // 以前是 `> 20`：那时 11 枚按钮**各带一个中文标签**（图标在上、中文在下），整条带有 30 几块。
        // §4.1 把顶栏收成纯图标之后文字数掉了十块，阈值跟着 spec 走，不是放松判据。
        assert!(items.len() >= 20, "顶栏只认出 {} 块文字（6 tab + 幽灵 + 11 图标 + 分支 + 计数），这一帧大概什么都没画", items.len());
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

    /// 顶栏的胶囊 tab 点一枚 = 开那个仓库。这一条是 #23 的全部行为：列表摊成 tab，点就是开。
    #[test]
    fn a_repo_tab_opens_that_repo() {
        let mut app = filled(3, &layout(PATCH, false));
        app.repos.push(Repo {
            id: None,
            path: "/other".to_owned(),
            name: "comic".to_owned(),
            added_at: None,
            last_fetched_at: None,
            group_name: String::new(),
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
        let over_log = pos2((SIDEBAR_W + WIN_W) / 2.0, (HEADER_H + (WIN_H - STATUS_H - STAGING_H)) / 2.0);
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
    /// **`blur_width != 0` 的那些不算「面」**：egui 的投影（`Shadow::as_shape`）就是一条带模糊
    /// 宽的 `RectShape`，底色非透明、圆角跟着卡走——不筛掉它，`card_back` 的「面积最大那一块」
    /// 会撞上它（卡底和投影**同尺寸**，只差 offset 那 8pt，纯靠图层顺序的平局裁决赢），
    /// 「卡底是白的」那条断言就变成运气。
    fn rects(out: &egui::FullOutput) -> Vec<(Color32, Rect)> {
        let mut found = Vec::new();
        for c in &out.shapes {
            collect_rects(&c.shape, c.clip_rect, &mut found);
        }
        found
            .into_iter()
            .filter(|(r, _)| r.fill != Color32::TRANSPARENT && r.blur_width == 0.0)
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
            .filter(|(r, _)| r.fill != Color32::TRANSPARENT && r.corner_radius != egui::CornerRadius::ZERO && r.blur_width == 0.0)
            .map(|(r, _)| (r.fill, r.rect, r.corner_radius))
            .collect()
    }

    /// 卡背后那一块：这一帧里**面积最大**的圆角矩形，连底色一起给。
    /// 按面积取最大是量出来的——最小的确认卡也有 380×126，而卡里第二大的圆角块是那条输入框
    /// （宽卡那一档 528×30；提交卡描述那一格撑到 528×94，同档卡底 560×318 还是压它一头）；
    /// 改成按颜色取最大就会被淡入那一格乘灰，见 `settled_frame`。
    /// 投影那一笔（同尺寸、只是往下挪 8pt）**不在这儿竞争**：`rounded()` 把 `blur_width != 0` 筛掉了。
    fn card_back(out: &egui::FullOutput) -> Option<(Color32, Rect)> {
        rounded(out)
            .into_iter()
            .max_by_key(|(_, r, _)| (r.width() * r.height()) as i64)
            .map(|(fill, r, _)| (fill, r))
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

    /// 虚拟行那枚**空心环**的圆心：白底 + `RING_GREY` 灰边。提交是车道色实心点，两者一眼分得开。
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

    /// 这一块文字用的**字号**。§4.1「图标按钮……图标 14px」和 §3.4 那四档层级里「字号」
    /// 这一半，在形状层都问不到别的：画出来的只有一块 galley，尺寸只能回它 `font_id.size`。
    /// 为什么字号要单独钉：§3.4 的层级是「字号 + 字重 + 灰度」三个维度，只钉后两个的话，
    /// 把提交信息从 13px 偷偷改回 12px 判据不会红——而那一档正是「正文 / 次要」的分界。
    fn text_size(out: &egui::FullOutput, label: &str, y: std::ops::RangeInclusive<f32>) -> f32 {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    let in_band = t.galley.text().starts_with(label)
                        && r.center().y >= *y.start()
                        && r.center().y <= *y.end();
                    in_band.then(|| t.galley.job.sections.first().map(|s| s.format.font_id.size))
                }
                _ => None,
            })
            .flatten()
            .unwrap_or_else(|| panic!("y {y:?} 里没有以「{label}」开头的文字"))
    }

    /// 这一块文字用的**字面名**（`inter-semibold` 那类命名字族；默认字族返回 `proportional`）。
    /// §6 那条「强调改走字重、不再走更深一档的灰」要问的就是这个：只换颜色也能让
    /// `text_color` 那条绿，字面没动等于没落地。
    fn text_family(out: &egui::FullOutput, label: &str, y: std::ops::RangeInclusive<f32>) -> String {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    let in_band = t.galley.text().starts_with(label)
                        && r.center().y >= *y.start()
                        && r.center().y <= *y.end();
                    in_band.then(|| {
                        t.galley.job.sections.first().map(|s| match &s.format.font_id.family {
                            // 命名字族给出的是注册时那个名字（`inter-semibold`）；四种自带
                            // 字族退成 `Proportional` 这类调试名，判据只认前者等于「没换字面」。
                            egui::FontFamily::Name(n) => n.to_string(),
                            other => format!("{other:?}"),
                        })
                    })
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
        (r.center().y - LOG_HEAD_H / 2.0)..=(r.center().y + LOG_HEAD_H / 2.0)
    }

    /// 日志数据区的 y 带：表头下沿一直到暂存带上沿。
    fn rows_band(out: &egui::FullOutput) -> std::ops::RangeInclusive<f32> {
        *head_band(out).end()..=(WIN_H - STATUS_H - STAGING_H)
    }

    /// 日志数据区**三列**的左缘（§2 的四列里「图」那一列画的是图元，没有文字起笔位）。
    /// `COL_INSET` 只有一个来源，判据不再各抄一份 `+8.0`——那一批改间距时，抄了常量的判据绿、
    /// 硬编码的那条恰好把旧值钉死了。
    /// **提交号那一列整列删了**（§2：图 48 | 提交信息 | 作者 96 | 日期 78），所以这里从四条变三条。
    fn col_edges(min_x: f32, right: f32) -> [f32; 3] {
        [
            min_x + COL_GRAPH + COL_INSET,
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

    /// 四块带的**底色与边界**逐条对 §2 那张示意图（`docs/DESIGN.md` §2 + 预览稿 CSS）。
    /// 名字里的「measured_reference」是上一代：那批数是从 SourceTree 截图量的。
    /// S6-2 起顶栏只有一条 46pt（旧的 52+24 两条带合并，§6 迁移表第一行），
    /// 顶栏和状态栏的底色也从"比内容区暗一格的灰"改成 `BG_WINDOW` 白 + 1px 分隔线。
    /// PROCESS 的改名对照记在 S6-2 那一节，历史台账里的旧名字不动。
    #[test]
    fn bands_match_the_design_spec() {
        let r = rects(&frame(&mut blank()));
        let bottom = WIN_H - STATUS_H;
        let staging_top = bottom - STAGING_H;
        // 顶栏：横贯顶部的一条 46pt，白底（§2「HEADER 46pt … 白底 + 下 1px 分隔线」）。
        // 字面值写在这儿，不是拿 `BG_WINDOW` 自比——把顶栏涂成别的灰也照样得红。
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(0.0, 0.0), pos2(WIN_W, HEADER_H))),
            Color32::from_rgb(0xFF, 0xFF, 0xFF),
            "顶栏带（§2 BG_WINDOW）"
        );
        // 侧栏：从顶带下沿一直贯穿到状态栏顶，灰底取 §3.1 的 `BG_SIDEBAR` `#F7F8FA`
        // （旧那枚 `#F1F1F3` 是参考图量的，S6-3 起作废；这里同样写**字面值**，不比常量）。
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(0.0, HEADER_H), pos2(SIDEBAR_W, bottom))),
            Color32::from_rgb(0xF7, 0xF8, 0xFA),
            "侧栏带（§3.1 BG_SIDEBAR）"
        );
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
        // 状态栏：贴着窗底，**也是白底**（§2「STATUS：白底 + 上 1px 分隔线」）。
        // 这一条以前断言的是灰（`TOOLBAR_BG`）——那枚 token 已经随 §6 删掉了。
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(0.0, bottom), pos2(WIN_W, WIN_H))),
            Color32::from_rgb(0xFF, 0xFF, 0xFF),
            "状态栏带（§2 BG_WINDOW）"
        );
        // 日志带 = 谁都没要走的那块。
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(SIDEBAR_W, HEADER_H), pos2(WIN_W, staging_top))),
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

    /// 侧栏这一带的**数**本身：S6-3a 换锚时定下的十四枚，逐个对 §2/§3.4 的字面值。
    /// 下面那批帧判据问的是「画出来对不对」，这一条问的是「常量还是不是 spec 那两个数」——
    /// 缺了它，把 `SIDEBAR_W` 改成 200 只会让帧判据跟着一起挪，谁都不红。
    #[test]
    fn the_sidebar_numbers_are_the_design_md_values() {
        assert_eq!(SIDEBAR_W, 188.0, "§2 侧栏宽 188");
        assert_eq!((SIDE_INSET, SIDE_VPAD), (8, 12), "§3.5 面板内边距：横 8 竖 12");
        assert_eq!((SIDE_GROUP_X, SIDE_TEXT_X), (16.0, 24.0), "§2.1 两列：组头 8+8、子项再缩进 8");
        assert_eq!((SIDE_GROUP_H, SIDE_ROW_H), (26.0, 26.0), "§2 子项行高 26；组头 §2 没给，自拍同档");
        assert_eq!(
            (SIDE_PILL_L, SIDE_PILL_R, SIDE_PILL_RAD),
            (4.0, 184.0, 6),
            "§2 选中药丸：圆角 6（§3.5），左右各让 4 为自拍"
        );
        assert_eq!((SIDE_GROUP_FONT, SIDE_ITEM_FONT, SIDE_COUNT_FONT), (11.0, 13.0, 11.0), "§3.4 辅助 11 / 正文 13");
        assert_eq!((SIDE_DOT_X, SIDE_DOT_R), (12.0, 3.5), "§2「蓝点」没给尺寸，沿用旧档 7pt 直径");
        assert_eq!(SIDE_COUNT_X, 172.0, "计数右缘 = 面板右 188 − 16；§2 只说「右对齐」，离边多远是自拍");
        assert_eq!(BG_SIDEBAR, Color32::from_rgb(0xF7, 0xF8, 0xFA), "§3.1 BG_SIDEBAR");
        assert_eq!(SIDE_FILTER_H, 34.0, "§2 底置筛选框 26 + 上间距 8（和 §4.2 的 30 撞车，取 §2，见 DESIGN.md §7）");
    }

    /// §3.1 那批灰/蓝/差异 token 的**字面值**判据。上面各带的帧判据全是拿常量去比常量，
    /// 少了这一条就意味着：把 `TEXT_SECONDARY` 从 `#59636E` 挪成别的灰，全绿。
    #[test]
    fn the_grey_and_status_tokens_are_the_design_md_values() {
        assert_eq!(BG_WINDOW, Color32::from_rgb(0xFF, 0xFF, 0xFF), "§3.1 BG_WINDOW");
        assert_eq!(BG_SIDEBAR, Color32::from_rgb(0xF7, 0xF8, 0xFA), "§3.1 BG_SIDEBAR");
        assert_eq!(BG_HOVER, Color32::from_rgb(0xEF, 0xF1, 0xF4), "§3.1 BG_HOVER");
        assert_eq!(BG_INSET, Color32::from_rgb(0xF0, 0xF1, 0xF3), "§3.1 BG_INSET");
        assert_eq!(BORDER_SUBTLE, Color32::from_rgb(0xE5, 0xE7, 0xEB), "§3.1 BORDER_SUBTLE");
        assert_eq!(BORDER_STRONG, Color32::from_rgb(0xD0, 0xD5, 0xDD), "§3.1 BORDER_STRONG");
        assert_eq!(TEXT_PRIMARY, Color32::from_rgb(0x1F, 0x23, 0x28), "§3.1 TEXT_PRIMARY（取代 INK 38,38,38）");
        assert_eq!(TEXT_SECONDARY, Color32::from_rgb(0x59, 0x63, 0x6E), "§3.1 TEXT_SECONDARY");
        assert_eq!(TEXT_MUTED, Color32::from_rgb(0x8B, 0x94, 0x9E), "§3.1 TEXT_MUTED");
        assert_eq!(ACCENT, Color32::from_rgb(0x09, 0x69, 0xDA), "§3.1 ACCENT");
        assert_eq!(ACCENT_SOFT, Color32::from_rgb(0xE8, 0xF1, 0xFD), "§3.1 ACCENT_SOFT");
        assert_eq!(ACCENT_LINE, Color32::from_rgb(0xB9, 0xD6, 0xF9), "§3.1 ACCENT_LINE");
        assert_eq!(ACCENT_INK, Color32::from_rgb(0x05, 0x50, 0xAE), "§3.1 ACCENT_INK");
        assert_eq!(
            (ADD_BG, ADD_FG, DEL_BG, DEL_FG),
            (
                Color32::from_rgb(0xE6, 0xF4, 0xEA),
                Color32::from_rgb(0x1A, 0x7F, 0x37),
                Color32::from_rgb(0xFC, 0xEB, 0xEA),
                Color32::from_rgb(0xCF, 0x22, 0x2E),
            ),
            "§3.1 差异两行"
        );
    }

    /// §3.2 六枚状态色块 + 见方 12 + 圆角 3 的字面值。旧的三枚（琥珀/玫瑰/紫）是参考图量的，
    /// `badge_of` 里「没量到就落灰底」那一条出口已经跟着 `BADGE_NONE` 一起删了——所以这里
    /// 必须六枚全钉：少钉一枚，就等于让那一枚重新变成"没人看着的自拍值"。
    #[test]
    fn the_status_badges_are_the_design_md_six_colors() {
        assert_eq!(
            (BADGE_MOD, BADGE_ADD, BADGE_DEL),
            (
                Color32::from_rgb(0xD2, 0x99, 0x22),
                Color32::from_rgb(0x1A, 0x7F, 0x37),
                Color32::from_rgb(0xCF, 0x22, 0x2E),
            ),
            "§3.2 修改/新增/删除"
        );
        assert_eq!(
            (BADGE_REN, BADGE_UNK, BADGE_CONF),
            (
                Color32::from_rgb(0x82, 0x50, 0xDF),
                Color32::from_rgb(0x6E, 0x76, 0x81),
                Color32::from_rgb(0xFB, 0x85, 0x00),
            ),
            "§3.2 重命名/未跟踪/冲突"
        );
        assert_eq!((BADGE_S, BADGE_R), (12.0, 3.0), "§3.2 见方 12（旧 14 作废）、§3.5 圆角 3");
    }

    /// 差异区那格文件标题带换锚到 §3.1：底色就是 `BG_INSET`（那张表的「用途」列明写了
    /// 「输入底、**hunk 头**、虚拟行」），旧那枚从参考图上量的 `(236,236,236)` 作废。
    /// 高 30、色块离带左缘 8 **spec 没给**，仍是自拍，留在 §7 的欠账里——所以这一条钉的是
    /// 「底色跟着 token」+「那两位没静默漂移」，不是「三位都来自 spec」。
    /// 反向变异实测：`DIFF_HEAD_BG` 换回 `from_rgb(236,236,236)` → 红在本条第一句；
    /// `app.rs` 那两处 `rect_filled(.., DIFF_HEAD_BG)` 改成 `BG_HOVER` → 红在
    /// `the_staging_band_is_two_panes_wide_at_the_spec_numbers` 和
    /// `the_diff_header_is_a_grey_band_with_the_status_badge`（本条只钉常量，画出来的那一遍由那两条接手）。
    #[test]
    fn the_diff_head_band_is_on_the_spec_inset_grey() {
        assert_eq!(
            DIFF_HEAD_BG, BG_INSET,
            "§3.1 把 hunk 头归在 `BG_INSET` 的用途里，差异区不该再用自己的灰"
        );
        assert_eq!(
            DIFF_HEAD_BG,
            Color32::from_rgb(0xF0, 0xF1, 0xF3),
            "`BG_INSET` 本身也不是 §3.1 那枚 `#F0F1F3` 了"
        );
        assert_eq!(
            (DIFF_HEAD_H, DIFF_HEAD_PAD),
            (30.0, 8.0),
            "这两位 spec 没给、还从参考图来；要动就连着 §7 一起动"
        );
    }

    /// 侧栏那一栏里的一块文字（含它的落笔颜色/字号/字族）。`text_color` 那几个只按 y 带筛，
    /// 而「main」「3」这些词在顶栏、状态栏、日志里也有，所以这里 **x 也要筛**：
    /// 左缘落在 `SIDEBAR_W` 里面、y 在顶栏和状态栏之间才算侧栏的。
    /// 字族按 `text_family` 同一套退路：命名字族给出注册名（`inter-medium`），
    /// 其余打成调试名（`Proportional`），判据只认前者等于「真换了字面」。
    fn side_galley(out: &egui::FullOutput, label: &str) -> (Color32, f32, String, Rect) {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    let hit = t.galley.text().starts_with(label)
                        && r.min.x < SIDEBAR_W
                        && r.center().y > HEADER_H
                        && r.center().y < WIN_H - STATUS_H;
                    hit.then(|| {
                        let s = t.galley.job.sections.first().expect("galley 没有 section");
                        let family = match &s.format.font_id.family {
                            egui::FontFamily::Name(n) => n.to_string(),
                            other => format!("{other:?}"),
                        };
                        (s.format.color, s.format.font_id.size, family, r)
                    })
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("侧栏里没有「{label}」"))
    }

    /// 分组头右边那枚计数：和标题同一行、右缘最接近 `SIDE_COUNT_X` 的一块文字。
    /// 按**右缘**找而不是按内容找：计数就是 "0"/"1"/"2"，按内容筛会把行号、徽标数字全捞进来。
    /// 返回 `(颜色, 字号, 文字, 矩形)`——第三个是**内容**，和 `side_galley` 的位置不同：
    /// 这一条要拿它对上"这一组有几条"，字族反而没人问。
    fn side_count(out: &egui::FullOutput, cy: f32) -> (Color32, f32, String, Rect) {
        let mut best: Option<(f32, (Color32, f32, String, Rect))> = None;
        for c in &out.shapes {
            if let egui::Shape::Text(t) = &c.shape {
                let r = t.galley.rect.translate(t.pos.to_vec2());
                if r.min.x >= SIDEBAR_W || (r.center().y - cy).abs() > 6.0 {
                    continue;
                }
                let d = (r.max.x - SIDE_COUNT_X).abs();
                if d < 1.0 && best.as_ref().map_or(true, |(b, _)| d < *b) {
                    let s = t.galley.job.sections.first().expect("galley 没有 section");
                    best = Some((d, (s.format.color, s.format.font_id.size, t.galley.text().to_owned(), r)));
                }
            }
        }
        best.unwrap_or_else(|| panic!("侧栏 y={cy} 那一行右缘 {SIDE_COUNT_X} 上没有计数")).1
    }

    /// 分组头左边的**空槽**（x < `SIDE_GROUP_X`）里有没有"有形状的墨"——圆、线段、带描边的方框。
    /// §2 明写分组头**无图标**，也没有折叠三角。填充矩形（药丸）不算，那种只出现在子项行。
    fn gutter_ink(out: &egui::FullOutput, cy: f32) -> usize {
        let hit = |_x0: f32, x1: f32, y0: f32, y1: f32| {
            x1 <= SIDE_GROUP_X + 0.01 && y0 >= cy - 12.0 && y1 <= cy + 12.0
        };
        out.shapes
            .iter()
            .filter(|c| match &c.shape {
                egui::Shape::Circle(d) => hit(d.center.x - d.radius, d.center.x + d.radius, d.center.y - d.radius, d.center.y + d.radius),
                egui::Shape::LineSegment { points, .. } => hit(
                    points[0].x.min(points[1].x),
                    points[0].x.max(points[1].x),
                    points[0].y.min(points[1].y),
                    points[0].y.max(points[1].y),
                ),
                egui::Shape::Rect(q) => q.stroke.width > 0.0 && hit(q.rect.min.x, q.rect.max.x, q.rect.min.y, q.rect.max.y),
                _ => false,
            })
            .count()
    }

    /// 侧栏的两列：**分组头起在 16、子项起在 24**（§2.1 的 8 内边距 + 8 缩进）。
    /// 旧判据是「九条标签全起在同一列 x=41」——那是参考图量的，§2 换成了两列网格。
    #[test]
    fn the_sidebar_texts_sit_on_the_8_16_grid() {
        let out = frame(&mut loaded());
        for label in ["工作区", "分支", "标签", "远端", "已贮藏"] {
            assert_eq!(side_galley(&out, label).3.min.x, SIDE_GROUP_X, "「{label}」是分组头，该起在 16");
        }
        for label in ["文件状态", "历史", "搜索", "main"] {
            assert_eq!(side_galley(&out, label).3.min.x, SIDE_TEXT_X, "「{label}」是子项，该起在 24");
        }
    }

    /// §2/§3.4 的两档字级与配色：分组头 11px `TEXT_SECONDARY`、子项 13px `TEXT_PRIMARY`。
    #[test]
    fn the_sidebar_font_ranks_are_the_spec_two_steps() {
        let out = frame(&mut loaded());
        for label in ["工作区", "分支", "标签", "远端", "已贮藏"] {
            let (color, size, _, _) = side_galley(&out, label);
            assert_eq!((size, color), (11.0, Color32::from_rgb(0x59, 0x63, 0x6E)), "分组头「{label}」不是 §3.4 的 11px 次要档");
        }
        for label in ["文件状态", "历史", "搜索", "main"] {
            let (color, size, _, _) = side_galley(&out, label);
            assert_eq!((size, color), (13.0, Color32::from_rgb(0x1F, 0x23, 0x28)), "子项「{label}」不是 §3.4 的 13px 正文档");
        }
    }

    /// §2 分组头右边那枚计数：11px `TEXT_MUTED`（比组头再淡一档，颜色自拍）、右缘贴 172，
    /// **报的是这一组有几条**。夹具里给「标签」塞两条，那一档就得写 2——
    /// 写死一个数的实现对这条杀不死（假绿一族「只验一半」）。
    #[test]
    fn the_sidebar_group_headers_carry_a_right_aligned_count() {
        let mut app = loaded();
        // 夹具默认 1 分支 / 0 标签 / 0 远端 / 0 贮藏：全是 0 和 1，分不清"数出来的"和"写死的"。
        let oid = "0".repeat(40);
        app.snap.as_mut().unwrap().tags = vec![
            Tag { name: "v1.0".to_owned(), target: oid.clone(), annotated: false },
            Tag { name: "v1.1".to_owned(), target: oid, annotated: false },
        ];
        let out = frame(&mut app);
        for (title, want) in [("工作区", "3"), ("分支", "1"), ("标签", "2")] {
            let row = side_galley(&out, title).3;
            let (color, size, text, rect) = side_count(&out, row.center().y);
            assert_eq!(text.trim(), want, "「{title}」那组的计数写了「{text}」，应该是 {want} 条");
            assert_eq!((size, color), (11.0, Color32::from_rgb(0x8B, 0x94, 0x9E)), "计数不是 11px 辅助档");
            assert!((rect.max.x - SIDE_COUNT_X).abs() < 1.0, "计数右缘 {:+.1}，§2 说的右对齐落在 {SIDE_COUNT_X}", rect.max.x);
        }
    }

    /// §2：分组头**无图标**（也没有折叠三角）。上一批这条是反的——"每个分类都带一枚图标"，
    /// 图标是照着参考图那五枚线描画的；换锚之后 spec 明写不要，那批线描整个删掉。
    #[test]
    fn the_sidebar_group_headers_are_iconless_and_triangleless() {
        let out = frame(&mut loaded());
        for label in ["工作区", "分支", "标签", "远端", "已贮藏"] {
            let cy = side_galley(&out, label).3.center().y;
            assert_eq!(gutter_ink(&out, cy), 0, "「{label}」左边那格还有墨：§2 的分组头无图标、无折叠三角");
        }
    }

    /// 行距：**分组头 26 + 子项 26**（§2）。这里抄的是字面量而不是常量——上一批 M2 抓到过
    /// 「拿常量和自己比」：把 26 改成别的，比常量的那条照样绿，只有钉死数字的才会红。
    /// 分支→标签中间只有一条 `main`，所以差 52 = 26 + 26；标签→远端之间没有子项，差 26。
    #[test]
    fn the_sidebar_rows_step_by_26pt() {
        let out = frame(&mut loaded());
        let c = |l: &str| side_galley(&out, l).3.center().y;
        let near = |a: f32, b: f32| (a - b).abs() < 0.6;
        assert!(near(c("远端") - c("标签"), 26.0), "标签→远端 = {}，§2 的行高是 26", c("远端") - c("标签"));
        assert!(near(c("已贮藏") - c("远端"), 26.0), "远端→已贮藏 = {}，§2 的行高是 26", c("已贮藏") - c("远端"));
        assert!(near(c("标签") - c("分支"), 52.0), "分支→标签中间只有一条 main 子项，应该是 52：实际 {}", c("标签") - c("分支"));
    }

    /// §2 的选中态：子项选中时铺一枚**浅蓝**药丸（`ACCENT_SOFT`、圆角 6、x 4..184、高 = 行高 26），
    /// 罩在选中的那一条上。旧的那枚是灰色 `#CCD0CD`、x 10..129.5、圆角 5——参考图量的，作废。
    #[test]
    fn the_selected_sidebar_row_wears_the_accent_pill() {
        let out = frame(&mut loaded());
        let pills: Vec<_> = rounded(&out)
            .into_iter()
            .filter(|(fill, rect, _)| *fill == Color32::from_rgb(0xE8, 0xF1, 0xFD) && rect.max.x <= SIDEBAR_W + 0.5)
            .collect();
        assert_eq!(pills.len(), 1, "侧栏里该只有一枚浅蓝药丸（选中的那一条）：{pills:?}");
        let (_, rect, radius) = pills[0];
        assert_eq!((rect.min.x, rect.max.x), (SIDE_PILL_L, SIDE_PILL_R), "药丸左右缘不是 4..184");
        assert_eq!(rect.height(), SIDE_ROW_H, "药丸高 {}，应该等于 §2 的子项行高 26", rect.height());
        assert_eq!(radius.nw, SIDE_PILL_RAD, "§3.5 行/药丸的圆角是 6");
        let row = side_galley(&out, "文件状态").3;
        assert!((rect.center().y - row.center().y).abs() < 0.6, "药丸没罩住选中的那一条");
    }

    /// §2「底置筛选输入框 26pt」——**画出来**的那一格高 26、并贴着侧栏下沿。
    /// `the_sidebar_numbers_are_the_design_md_values` 钉的是 `SIDE_FILTER_H = 8 + 26` 这个数，
    /// 这一条钉的是"预算扣完真摆出一个 26 高的框"。两句都不许只靠自己：
    /// 反向变异实测（2026-10-25）——把 `app.rs` 那句 `SIDE_FILTER_H - SIDE_FILTER_GAP` 的减法去掉
    /// 只有这一条红（框成 34 高，常量还是 34，数字判据照样绿）；
    /// 把 `SIDE_FILTER_H` 里那个 26 改成 24 则**两条一起红**（这里的 26 也是字面量，不跟常量走）。
    #[test]
    fn the_sidebar_filter_box_is_26_tall_and_sits_at_the_bottom() {
        let out = frame(&mut loaded());
        let boxes: Vec<Rect> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Rect(r) if r.rect.max.x <= SIDEBAR_W && r.rect.min.y > WIN_H - STATUS_H - 60.0 => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(boxes.len(), 1, "侧栏最底下应该只有一格输入框：{boxes:?}");
        let b = boxes[0];
        assert_eq!(b.height(), 26.0, "§2 的底置筛选框高 26（实测 {}", b.height());
        assert!(
            (WIN_H - STATUS_H - SIDE_VPAD as f32 - b.max.y).abs() < 0.6,
            "框没贴在侧栏下沿的内边距上：底 {} 而窗口内边距该落在 {}",
            b.max.y,
            WIN_H - STATUS_H - SIDE_VPAD as f32
        );
    }

    /// §2「当前分支 = 蓝点 + 中黑字」：点画在文字列左边的空槽（x=12）、走主色 `ACCENT`，
    /// 文字用真字重 `inter-medium`。旧实现画的是 (36,93,184) 那颗参考图蓝、而且和灰色药丸
    /// 同时出现（选中=当前混成一档），现在两件事拆开：药丸跟选中走，蓝点跟当前分支走。
    #[test]
    fn the_current_branch_wears_the_accent_dot_and_medium_ink() {
        let out = frame(&mut loaded());
        let (_, _, family, row) = side_galley(&out, "main");
        assert_eq!(family, "inter-medium", "当前分支没走 §2 的「中黑字」（真字重 medium）");
        let dots: Vec<Pos2> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Circle(d) if d.fill == Color32::from_rgb(0x09, 0x69, 0xDA) => Some(d.center),
                _ => None,
            })
            .collect();
        assert!(
            dots.iter().any(|p| (p.x - SIDE_DOT_X).abs() < 0.01 && (p.y - row.center().y).abs() < 0.6),
            "当前分支前面没有落在 x={SIDE_DOT_X} 的主色圆点：{dots:?}"
        );
        // 非当前的子项行（「搜索」）不许也长出蓝点：一个值两条出口只验一半是假绿。
        let search = side_galley(&out, "搜索").3;
        assert!(!dots.iter().any(|p| (p.y - search.center().y).abs() < 0.6), "「搜索」那一行也画了蓝点");
        // 字重同一条路：反向变异实测——把 `side_row` 里那句 `if current` 拆掉、给每一行都上
        // `medium`，上面「main 是 inter-medium」照样绿（它只验了一条出口）。这一条补上另一半。
        let (_, _, search_family, _) = side_galley(&out, "搜索");
        assert_ne!(search_family, "inter-medium", "「搜索」不是当前分支也中了中黑字：字重没按「当前」分流");
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

    /// 状态栏贴着窗底横贯整窗；暂存带、文件表、日志带的整体底边都因为让出这 24pt
    /// 而上移。这条把「谁在窗底」钉死。
    #[test]
    fn the_status_bar_bands_the_bottom_edge() {
        // §2「STATUS **24pt**，横贯整窗」——S6-5 落地的这一档。旧注释说"等 S6-5"，现在等了。
        // 24 也是 §2 那条地板公式对得上的前提：`46 + 84 + 240 + 24 = 394`（`MIN_H` 那条判据）。
        assert_eq!(STATUS_H, 24.0, "状态栏高度取 §2 的 24（不是按行节奏凑的 26）");
        let out = frame(&mut blank());
        let bottom = WIN_H - STATUS_H;
        // 白底（§2「STATUS：白底 + 上 1px 分隔线」）。以前这里是 `TOOLBAR_BG` 那枚灰。
        assert_eq!(
            band(&rects(&out), Rect::from_min_max(pos2(0.0, bottom), pos2(WIN_W, WIN_H))),
            Color32::from_rgb(0xFF, 0xFF, 0xFF),
            "状态栏带（§2 BG_WINDOW）"
        );
        let staging_top = bottom - STAGING_H;
        assert_eq!(
            band(&rects(&out), Rect::from_min_max(pos2(SIDEBAR_W, staging_top), pos2(WIN_W, bottom))),
            Color32::WHITE,
            "暂存带底边让出了状态栏"
        );
    }

    /// 版本号常驻状态栏最右：读的是 `Cargo.toml` 那一份（`ui::VERSION`），
    /// 空态（没开仓库）也必须在——它和包名、release 标签是同一个字符串。
    #[test]
    fn the_status_bar_shows_the_package_version() {
        let band = (WIN_H - STATUS_H)..=WIN_H;
        let want = format!("v{}", crate::ui::VERSION);
        for (name, mut app) in [("有仓库", loaded()), ("空态", blank())] {
            let out = frame(&mut app);
            let r = text_rect(&out, &want, band.clone());
            assert!(
                r.max.x > WIN_W - 40.0,
                "{name}状态栏里版本号没贴右缘：{}（找的是「{want}」）",
                r.max.x
            );
        }
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

    /// 状态栏的字级只有**一档**：§2「STATUS 24pt：… 11px 灰字」，而 §3.4 的用途列
    /// （「分组头、**状态栏**、药丸、占位、行号」）和 §3.1 `TEXT_MUTED` 的用途列
    /// （「占位、辅助、**状态栏**」）各自把这一档点名给了状态栏。
    /// 旧代码是三档：分支名/↑↓ 12px `TEXT_PRIMARY`、路径 11.5px、灰字用的还不是 token。
    /// 判据问的是**带子里的每一块字**，不是一处样本——分支、`↑2`、右边路径全得是 11 + `TEXT_MUTED`。
    /// 唯一的例外是分离 HEAD 那格「回到 main」：§3.1 把 `ACCENT` 的用途写成「链接、主按钮、焦点」，
    /// 它是这条带子里唯一可点的一段（见 §7「STATUS 在分离 HEAD 时写什么」那行），
    /// 所以它跟着带子的**字号**、跟 `ACCENT` 的**颜色**。——这条判据跑的是 `loaded()`，带子里
    /// **没有**那一格，所以「例外」目前只是代码里的形状，没被这条判据认过（记 PROCESS 台账）。
    /// 反向变异实测：分支那段换回 `TEXT_PRIMARY` → 红在「main」；换回 12px → 也红在「main」
    /// （循环里一 assert 就 panic，`↑2` 那一格不再被问）；只把右边路径换成 11.5 → 红在「/fixture」。
    /// 三处杀得掉各自是因为三处是三个字面量，不是靠某一条蒙中全部。
    #[test]
    fn the_status_bar_is_one_tier_of_muted_eleven_pointer_text() {
        let out = frame(&mut loaded());
        let band = (WIN_H - STATUS_H)..=WIN_H;
        let mut n = 0;
        for c in &out.shapes {
            if let egui::Shape::Text(t) = &c.shape {
                let r = t.galley.rect.translate(t.pos.to_vec2());
                if !band.contains(&r.center().y) {
                    continue;
                }
                let s = t.galley.job.sections.first().expect("galley 没有 section");
                assert_eq!(
                    s.format.font_id.size,
                    11.0,
                    "状态栏「{}」不是 §2 的 11px：{}",
                    t.galley.text(),
                    s.format.font_id.size
                );
                assert_eq!(
                    s.format.color, TEXT_MUTED,
                    "状态栏「{}」不是 §3.1 那枚给状态栏的灰字",
                    t.galley.text()
                );
                n += 1;
            }
        }
        assert!(n >= 3, "状态栏带子里只认到 {n} 块字：分支、↑2、右边路径至少该有这三块");
    }

    /// §2「STAGING 带 **240pt**：文件表 **330pt** │ 差异区」这一条把两个数钉在**画出来的带子**上，
    /// 不钉常量——钉常量的判据遇上「把 `FILES_W` 从 330 改回 435、同时把所有引用它的判据一起改」
    /// 那种改动谁也红不了。认的是差异区顶上那条灰带：它的**左缘 = 侧栏 188 + 文件表 330 = 518**，
    /// 它的**顶 = 窗底 545 − 状态栏 24 − 暂存带 240 = 281**（带的上沿再加面板那 2pt 内边距）。
    /// 反向变异实测：`STAGING_H` 240→245 红在顶边（差 5pt），`FILES_W` 330→435 红在左缘（差 105pt）。
    #[test]
    fn the_staging_band_is_two_panes_wide_at_the_spec_numbers() {
        let mut app = loaded();
        app.file = Some("src/loose.rs".to_owned());
        app.patch_label = "未暂存 · src/loose.rs".to_owned();
        let out = frame(&mut app);
        let head = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == DIFF_HEAD_BG)
            .map(|(_, r)| r)
            .find(|r| r.max.x > WIN_W - 2.0 && r.height() == DIFF_HEAD_H)
            .expect("差异区顶上那条灰带没进帧");
        assert_eq!(head.min.x, SIDEBAR_W + 330.0, "文件表那一栏不是 §2 的 330pt：差异带从左 {} 起", head.min.x);
        assert!(
            (head.min.y - (WIN_H - 24.0 - 240.0 + 2.0)).abs() < 1.5,
            "暂存带顶边不在「窗底 − 状态栏 24 − 带 240」那一档，实测 {}",
            head.min.y
        );
        // 状态栏那 24pt 真的让出来了：带的下沿正好压在状态栏上缘。
        let bottom = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == Color32::WHITE)
            .map(|(_, r)| r)
            .filter(|r| r.min.x >= SIDEBAR_W && r.max.x >= WIN_W - 2.0)
            .fold(f32::NEG_INFINITY, |m, r| m.max(r.max.y));
        assert!((bottom - (WIN_H - 24.0)).abs() < 1.5, "白底带一直铺到 {}，没在状态栏 24pt 前收住", bottom);
    }

    /// §3.2 那六枚状态色块，**每一枚都得在帧里真画出来**。
    /// 以前只有 `the_status_badges_are_the_design_md_six_colors` 钉**常量**的颜色：把 `badge_of`
    /// 里 `Deleted` 那一支换成 `(BADGE_UNK, "D")`，六枚常量一个字没动、全套判据照样绿——
    /// 而用户看到的是删除的文件顶着一枚灰块。这条按状态各认一次，认的是**画出来的东西**：
    /// 该颜色的 12×12、圆角 3 的色块，落在自己那一行的图标档（有方框那一栏 47、
    /// 没方框那一栏 8），块里那颗字母是 **11px 白字**（§3.2 那一列的三个条件一次问全）。
    /// 六个状态分六帧跑：一帧里塞六行会把列表撑出 `ScrollArea` 的裁剪框，而
    /// `put_left` 里的子 `Ui` 整块不可见时 egui **根本不落字**（S6-4 那条 `FILE_ROW_H` 的教训），
    /// 到时候"没画出来"和"被裁掉了"分不清。
    /// 反向变异实测：`Deleted` 那支换成 `BADGE_UNK` → 红在删除那一行；`Renamed` 换成 `BADGE_MOD`
    /// → 红在改名那一行；`Unmerged` 那支换成灰 → 红在冲突那一行（连没方框那档的 x=8 一起认到）。
    /// 字母字号 11.0→12.0 → 红，但红在**第一帧**（`mod.rs`）就中止了：循环里一 assert 就 panic，
    /// 后面五帧不再跑。说"六行全红"是吹的，实际只看到一条——判据成立，别把它当覆盖面。
    #[test]
    fn every_file_state_badge_is_drawn_in_its_row() {
        let one = |path: &str, st: ChangeStatus, conflict: bool| FileStatus {
            path: path.to_owned(),
            old_path: None,
            index_status: if conflict { ChangeStatus::Unmerged } else { ChangeStatus::Unknown },
            worktree_status: if conflict { ChangeStatus::Unmerged } else { st },
            staged: false,
        };
        // (状态, 文件名, 该出现的颜色, 该出现的字母)。字母与颜色都出自 §3.2 那张表。
        let cases = [
            (ChangeStatus::Modified, "s/mod.rs", BADGE_MOD, "M"),
            (ChangeStatus::Added, "s/add.rs", BADGE_ADD, "A"),
            (ChangeStatus::Deleted, "s/del.rs", BADGE_DEL, "D"),
            (ChangeStatus::Renamed, "s/ren.rs", BADGE_REN, "R"),
            (ChangeStatus::Untracked, "s/new.rs", BADGE_UNK, "?"),
            (ChangeStatus::Unmerged, "s/con.rs", BADGE_CONF, "!"),
        ];
        let mut seen = Vec::new();
        for (st, path, color, letter) in cases {
            let conflict = st == ChangeStatus::Unmerged;
            let mut app = filled(1, &[]);
            app.snap.as_mut().unwrap().status.files = vec![one(path, st, conflict)];
            let out = frame(&mut app);
            let band = (WIN_H - STATUS_H - STAGING_H)..=(WIN_H - STATUS_H);
            let row = text_rect(&out, path, band.clone());
            // 没方框那一栏（冲突、以及某条提交的改动列表）色块顶到 +8，有方框的走图标档 +47。
            let want_x = if conflict { 8.0 } else { 47.0 };
            let badge = rounded(&out)
                .into_iter()
                .filter(|(c, _, _)| *c == color)
                .map(|(_, r, rad)| (r, rad))
                .find(|(r, _)| {
                    (r.width() - 12.0).abs() < 0.6
                        && (r.height() - 12.0).abs() < 0.6
                        && (r.min.x - (SIDEBAR_W + want_x)).abs() < 1.5
                        && (r.center().y - row.center().y).abs() < 1.5
                })
                .unwrap_or_else(|| {
                    panic!("「{path}」（{letter}）这一行没画出 §3.2 那枚 12×12 的色块（颜色 {color:?}、离面板左缘 {want_x}）")
                });
            assert_eq!(badge.1.nw, 3, "「{path}」的色块圆角不是 §3.5 的 3");
            // 块里那颗字母：**白**、**11px**、中心落在色块里。
            // 比的是**中心**而不是 `badge.contains(t.pos)`：11 号字的行盒实测 14~16pt，
            // 比 12 见方的色块还高，`Align2::CENTER_CENTER` 算出的 `t.pos`（galley 左上角）
            // 会落在色块**上沿外面**一两个像素——按 contains 判等于永远判不中。
            let glyph = out.shapes.iter().find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == letter => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    if (r.center() - badge.0.center()).length() >= 3.0 {
                        return None;
                    }
                    t.galley.job.sections.first().map(|s| (s.format.color, s.format.font_id.size))
                }
                _ => None,
            });
            assert_eq!(
                glyph,
                Some((Color32::WHITE, 11.0)),
                "「{path}」色块里那颗 {letter:?} 不是 §3.2 的白字 11px：{glyph:?}"
            );
            seen.push(letter);
        }
        assert_eq!(seen, vec!["M", "A", "D", "R", "?", "!"], "六枚状态一枚都没跑全");
    }

    /// 差异区顶上那格：参考图那条灰带 (236,236,236) 已换锚成 §3.1 的 `BG_INSET`
    /// （见 `the_diff_head_band_is_on_the_spec_inset_grey`），里面「状态色块 + 加粗、靠左的文件名」。
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
            .find(|r| head.contains_rect(*r) && near(r.width(), 12.0) && near(r.height(), 12.0))
            .expect("灰带里没有那枚状态色块（§3.2：12×12，改动那枚是琥珀底）");
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
        // 预览稿 `.dp`（差异区顶上那条）取 `--text-secondary`，不是正文黑、也不是旧那枚近黑。
        assert_eq!(
            text_color(&out, "未暂存 · src/loose.rs", head.min.y..=head.max.y),
            Color32::from_rgb(0x59, 0x63, 0x6E),
            "灰带里的文件名不是 §3.1 那枚次要灰"
        );
    }

    /// hunk 头在人眼里是「块 1： 行 8-11（新 8-12）」，不是 `@@ -8,4 +8,5 @@`。这条钉两件事：
    /// 整帧**没有** git 原始的那串（翻译只许写在 `layout` 一处，删掉它这条就红），
    /// 以及两侧区间各自按 `start,count` 算（旧 8,4 → 8-11，新 8,5 → 8-12；把 count 当末行号就是 8-4/8-5）。
    #[test]
    fn a_hunk_row_reads_as_a_block_number_and_line_range() {
        let out = frame(&mut loaded());
        let t = texts(&out);
        let hit = t.iter().find(|(s, _)| s.contains("块 1： 行 8-11（新 8-12）"));
        assert!(
            hit.is_some(),
            "hunk 行没翻译成「块 N： 行 A-B（新 C-D）」：{:?}",
            t.iter().map(|(s, _)| s).filter(|s| s.contains("行") || s.contains("@@")).collect::<Vec<_>>()
        );
        assert!(t.iter().all(|(s, _)| !s.contains("@@")), "帧里还留着原始的 @@ 头");
        // §3.1 把 hunk 头的文字记在**辅助**那一档（预览稿 `.hk` 同色）。以前这条只问内容、
        // 不问颜色：反向变异实测把 `src/ui/app.rs` 那句 `.color(TEXT_MUTED)` 换成 `TEXT_PRIMARY`，
        // 全量 156 条全绿。历史面板那一格的颜色由
        // `the_file_history_lists_the_commits_that_touched_the_file` 钉，这里钉主差异区这一格。
        // `texts()` 给的是落笔点（左上），band 往下多留一行高，免得把行中心算漏。
        let y = hit.unwrap().1.y;
        assert_eq!(
            text_color(&out, "块 1", (y - 2.0)..=(y + DIFF_ROW_H)),
            Color32::from_rgb(0x8B, 0x94, 0x9E),
            "hunk 头的文字没走 §3.1 的辅助档"
        );
    }

    /// 差异区**一行 git 文件头都不许有**。参考图 `sourcetree-4.2.19-repo-window.png`：
    /// 文件名那条灰带（`BG_INSET`，S6-5 前是量出来的 (236,236,236)）止于 y=368pt，y=368..396 直接就是 hunk 那条带
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
        let t = texts(&frame(&mut filled(3, &layout(EVEN, false))));
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
        // S6-5 把文件标题带也换成 `BG_INSET` 之后，**只按颜色筛会捞到那条 30pt 的标题带**
        // （它画得更早，`find` 拿到的就是它，里面那两枚圆角是搜索框不是按钮——一条假绿）。
        // 所以再钉一层高度：hunk 行是 15pt，字面量，理由同上。
        let row = rects(&out)
            .into_iter()
            .filter(|(c, _)| *c == Color32::from_rgb(0xF0, 0xF1, 0xF3))
            .map(|(_, r)| r)
            .find(|r| r.min.x > SIDEBAR_W + FILES_W - 1.0 && r.height() == 15.0)
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

    /// 上面那条用的字面量，得有个地方钉住它们是**量来 / 规定**的数——否则改常量把画的那头挪走，
    /// 只有几何判据红，没人知道「本来该是多少」。hunk 按钮那几枚还是参考图量的
    /// （§2 只说「命中 28pt」，那一档跟着差异带落地时再换）；`BG_INSET` 这枚现在归 DESIGN.md §3.1。
    #[test]
    fn the_hunk_button_constants_are_the_measured_reference_numbers() {
        assert_eq!((HUNK_BTN_W, HUNK_BTN_H), (55.0, 15.0), "参考图那两枚按钮 55×15");
        assert_eq!((HUNK_BTN_GAP, HUNK_BTN_EDGE), (9.0, 9.0), "两枚之间 9pt、离带右缘 9pt");
        assert_eq!(HUNK_BTN_STROKE, Color32::from_rgb(228, 232, 229), "1pt 描边");
        assert_eq!(HUNK_BTN_FG, Color32::from_rgb(109, 110, 109), "按钮上的灰字");
        assert_eq!(BG_INSET, Color32::from_rgb(240, 241, 243), "hunk 头那条带的底色（§3.1 BG_INSET）");
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
        // **字面量 24**，引 `FILE_ROW_H` 就成了常量比自己（改常量两边一起挪，判据永不变红）。
        // 24 的出处是 §2「STAGING 带：文件表 330pt：**行高 24**」——注意这里**不是** `ROW_H`：
        // `ROW_H` 28 是日志行距（§2「表头 28 + 行 28 × N」），S6-4 换它的时候曾把文件行一起
        // 顶高，最后一条文件行被挤出 `ScrollArea` 的可见框、egui 根本不落字（灰带还在、文件名没了）。
        assert_eq!(band.height(), 24.0, "灰带不是整整一行高");
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
                    && r.width() > 11.5
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
            // 这一行该有一枚方框（§2 的 13）+ 一枚状态色块（§3.2 的 12pt），两枚尺寸不同档，
            // 但都落在 11.5..15 这一筛里，所以只能按 x 分档问。
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
        // 方框那一档**单独收到 ±0.6**：上面那枚 `near` 的容差是 ±1.5，拿它认方框的话
        // `CHK_S` 从 13 改成 12（§3.2 状态色块那一档！）照样被认成方框，判据就成了摆设。
        // ±0.6 的窗口是 12.4..13.6：装得下 13 的框，装不下 12 的色块、也装不下 15 的药丸。
        let box_side = |s: f32| (s - 13.0).abs() < 0.6;
        let squares: Vec<Rect> = rounded(&out)
            .into_iter()
            .map(|(_, r, _)| r)
            .filter(|r| box_side(r.width()) && box_side(r.height()) && r.min.x > SIDEBAR_W && r.min.x < SIDEBAR_W + FILES_W)
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
    /// 描边的**颜色**以前问不到（`rects()`/`rounded()` 只看 `fill`）。DESIGN.md §3.1 给了
    /// 「控件描边」这一档之后就能问了：`stroked()` 连颜色一起返回，下面按中心点配对来问。
    /// 反向变异两条：删掉 `rect_stroke` 那行 = 第一圈断言红；把它的颜色换成别的灰 =
    /// 第二条红（旧写法换颜色全绿，那是一处登记过的假绿）。
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
        let borders: Vec<(Color32, Rect)> = stroked(&out)
            .into_iter()
            .filter(|(_, r)| is_box(r) && in_pane(r))
            .collect();
        let (white, blue) = (filled(Color32::WHITE), filled(CHK_BLUE));
        assert_eq!((white.len(), blue.len()), (3, 2), "这一帧该有三枚未勾选（未暂存分类标题 + 两行文件）和两枚勾上（已暂存分类标题 + 一行文件）的方框");
        for w in &white {
            assert!(
                borders.iter().any(|(_, b)| (b.center() - w.center()).length() < 1.0),
                "{w:?} 这枚白方框没有描边，铺在白面板上等于没画"
            );
        }
        for b in &blue {
            assert!(
                !borders.iter().any(|(_, s)| (s.center() - b.center()).length() < 1.0),
                "{b:?} 这枚勾上的方框是实心的，参考图没有边框"
            );
        }
        // §3.1「输入框 / 按钮 / 未勾选方框的描边」= `BORDER_STRONG` #D0D5DD，字面值写在这儿。
        let edge = Color32::from_rgb(0xD0, 0xD5, 0xDD);
        assert_eq!(borders.len(), white.len(), "方框描边的枚数和白底方框对不上：{borders:?}");
        for (c, r) in &borders {
            assert_eq!(*c, edge, "{r:?} 那枚未勾选方框的描边不是 §3.1 那一档");
        }
    }

    /// 上面那条用的字面量，出处钉在这儿。参考图实测是 **8 / 24 / 47 / 67.5**
    /// （面板左缘 141 起算：方框 x 149、165，图标 x 188，名字 x 208.5），取整为 8 / 25 / 47 / 68。
    /// 中间有一批各减 8 又各减 9，这一批改回参考图（缘由见常量那条的注释）。
    /// 断言右侧写成 `24.0 + 1.0` 这样的**字面量算式**：把取整改成别的（比如 `- 1.0`）就会红，
    /// 而它不吃 `CHK_ROW_X` 本身。
    #[test]
    fn the_file_row_columns_are_the_reference_offsets() {
        assert_eq!(CHK_S, 13.0, "§2「圆角勾选框 13pt」。参考图量到 13.5，差的那半像素归 §7");
        assert_eq!(CHK_HEAD_X, 8.0, "分类行的方框照参考图（x 149 → 离面板左缘 8）");
        assert_eq!((CHK_ROW_X, ICON_X, NAME_X), (24.0 + 1.0, 47.0, 67.5 + 0.5), "文件行三档 = 参考图取整（方框 165、图标 188、名字 208.5）");
        assert_eq!(CHK_BLUE, Color32::from_rgb(71, 141, 247), "勾上的方框中心色");
    }

    /// 某条提交的改动列表**没有方框**（早就落库了，勾不动索引），所以那一档不能照留空位。
    /// 用户 2026-10-21 的原话：「勾选的 box 没了，但是没有移动位置」。
    /// 断的是**画出来的位置**（28 是字面量，= 8 + §2 的 13 号方框 + 7，和分类标题那一列同一个数），不吃 `NO_BOX_*` 常量。
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
                near(name.min.x - SIDEBAR_W, 28.0),
                "「{path}」离面板左缘 {}，没有方框那一栏该收到 28（= 8 + 13 + 7，和分类标题同列）",
                name.min.x - SIDEBAR_W
            );
            // 这一行只该有一枚方块大的东西：**状态色块**（§3.2 的 12pt），在方框那一档（+8）。
            let squares: Vec<(Color32, Rect)> = rounded(&out)
                .into_iter()
                .filter(|(_, r, _)| {
                    (r.width() - r.height()).abs() < 1.0
                        && r.width() > 11.5
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

    /// 窗口缩到**地板尺寸**时，不许有任何两块看得见的文字互相压着。
    /// **它守的是"地板窗口不撞字"，不守 `MIN_W` 这个数**——2026-10-25 实测：`MIN_W` 降到 800
    /// 这一条还是绿的，760 才红（旧顶栏带中文标签时是 765 撞；S6-2 换成 §4.1 的 28pt 纯图标之后
    /// 让出了约 40pt 的空档）。所以 860 由 `the_header_numbers_are_the_design_md_values` 的
    /// 字面值钉，这一条只负责"地板那一帧是干净的"。别把这两条的职责混着写。
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
    fn the_window_itself_is_floored_at_the_spec_size() {
        let v = viewport();
        assert_eq!(v.min_inner_size, Some(vec2(MIN_W, MIN_H)), "窗口的地板没交给窗口系统：{:?}", v.min_inner_size);
        assert_eq!(v.inner_size, Some(vec2(WIN_W, WIN_H)), "窗口不是按 §2 那张预览稿的尺寸开的：{:?}", v.inner_size);
    }

    /// §2 那条 46pt 的带里同时有红绿灯、仓库 tab、分支、动作按钮 —— 它**就是标题栏**。
    /// 所以窗口必须配成"内容铺到标题栏底下、系统标题不画"，否则我们是在标题栏下面
    /// 又画一条带，窗口顶上白占一条，而且第一枚 tab 会被红绿灯压住。
    /// 这三条开关在 `egui::ViewportBuilder` 上是可读字段（eframe 把它们映射给 winit 的
    /// macOS 扩展），所以这条判据读的是配置对象，不是帧。
    #[test]
    fn the_toolbar_band_is_the_window_titlebar() {
        let v = viewport();
        assert_eq!(v.titlebar_shown, Some(false), "标题栏还单独占一条：内容没铺到标题栏底下");
        assert_eq!(v.fullsize_content_view, Some(true), "内容没开 full-size：顶栏会在标题栏下面另起一行");
        assert_eq!(v.title_shown, Some(false), "系统标题还在画：会和我们左对齐的仓库 tab 撞成两个标题");
    }

    /// 红绿灯是操作系统画的，占 x 20..72（§2 + 预览稿 `.lights`：三枚 12pt 圆、间距 8），
    /// 盖在我们内容上面。所以顶栏左边 **92pt 里不许有任何东西**，第一枚胶囊 tab 从 92 起。
    #[test]
    fn nothing_is_drawn_under_the_traffic_lights() {
        // 带高 `HEADER_H = 46` 由 `the_header_numbers_are_the_design_md_values` 钉字面值；
        // 下面所有断言拿它当尺子，所以那条不能省。
        let mut app = loaded();
        // 夹具默认**没往书签列表里放仓库**（`repos` 是空的），所以这里补一本：
        // 这一条要钉的是"左组第一枚 tab 起在 92"，没有 tab 就只剩那枚幽灵按钮，钉不到点上。
        app.repos.push(Repo {
            id: None,
            path: "/other".to_owned(),
            name: "comic".to_owned(),
            added_at: None,
            last_fetched_at: None,
            group_name: String::new(),
        });
        let out = frame(&mut app);
        let vis = visible_texts(&out);
        assert!(vis.len() > 40, "这一帧大概什么都没画");
        let under: Vec<&String> = vis
            .iter()
            .filter(|(_, r)| r.min.y < HEADER_H && r.min.x < TRAFFIC_RIGHT)
            .map(|(s, _)| s)
            .collect();
        assert!(under.is_empty(), "红绿灯底下有文字：{under:?}");
        // 左组第一项现在是**仓库胶囊 tab**（以前是「xxx (Git)」那行标题；§2 删了标题，
        // tab 自己就是仓库名）。胶囊的底从 92 起，里面的字再让 11 的内边距（`.tab{padding:3px 11px}`）。
        let tab = vis.iter().find(|(s, _)| **s == *"comic").expect("顶栏没有第一枚仓库 tab");
        assert!(
            (tab.1.min.x - 103.0).abs() < 1.0,
            "第一枚 tab 的文字左沿 {:+.1}pt，应该是 inset 92 + 内边距 11 = 103",
            tab.1.min.x
        );
    }

    /// §2 的胶囊 tab：选中那一枚**铺主色浅底 + 深字 + 真字重**，高 26、圆角 = 半高、
    /// 底从 inset 92 起；未选中的**不铺底**，只落次要灰字。
    /// 反向变异的靶子：把 `repo_tab` 的 `fill` 换回旧的 `Color32::WHITE`（顶栏同色，等于没选中）、
    /// 把 `CornerRadius::same(TAB_PILL_R)` 换成 `ZERO`（变方角）、把 `ACCENT_INK` 换成
    /// `TEXT_PRIMARY`、把 `semibold_family()` 摘掉——这四处分各红一条。
    #[test]
    fn the_active_repo_tab_is_an_accent_capsule() {
        let mut app = loaded();
        // 夹具的 `repos` 是空的（`self.repo` 只指着 `/fixture`）：补两本，第一本对上当前仓库，
        // 这样"选中那一枚"和"未选中那一枚"在同一帧里都有。
        app.repos.push(Repo { id: None, path: "/fixture".to_owned(), name: "gitx".to_owned(), added_at: None, last_fetched_at: None, group_name: String::new() });
        app.repos.push(Repo { id: None, path: "/other".to_owned(), name: "comic".to_owned(), added_at: None, last_fetched_at: None, group_name: String::new() });
        let out = frame(&mut app);
        let band = 0.0..=HEADER_H;
        let pills: Vec<(Color32, Rect, egui::CornerRadius)> = rounded(&out)
            .into_iter()
            .filter(|(_, r, _)| r.min.y >= 0.0 && r.max.y <= HEADER_H)
            .collect();
        assert_eq!(pills.len(), 1, "顶栏铺底的胶囊只该有选中那一枚（未选中不铺底）：{pills:?}");
        let (fill, rect, radius) = pills[0];
        assert_eq!(fill, Color32::from_rgb(0xE8, 0xF1, 0xFD), "选中胶囊的底色不是 §3.1 的 ACCENT_SOFT");
        assert!((rect.height() - 26.0).abs() < 1.0, "胶囊高 {}，§2 写的是 26", rect.height());
        assert!((rect.min.x - 92.0).abs() < 1.0, "第一枚胶囊的左沿 {}，§2 的红绿灯 inset 是 92", rect.min.x);
        assert_eq!(radius.nw, 13, "圆角不是半高（§2 的 999 就是胶囊）");
        assert_eq!(text_color(&out, "gitx", band.clone()), Color32::from_rgb(0x05, 0x50, 0xAE), "选中的仓库名不是 §3.1 的 ACCENT_INK");
        assert_eq!(text_family(&out, "gitx", band.clone()), "inter-semibold", "选中的仓库名没走真字重（§3.4 的 600）");
        assert_eq!(text_color(&out, "comic", band), Color32::from_rgb(0x59, 0x63, 0x6E), "未选中的仓库名不是 §3.1 的次要档");
    }

    /// §4.1：顶栏那组动作按钮是**纯图标**的 28×28、图标 14px、整组贴右沿让 12。
    /// 反向变异：把 `icon_button` 里旧的中文标签画回去 → 「顶栏还写着『提交』」红；
    /// 把 `ICON_PX` 改回 13 → 字号那条红；把 `ICON_BTN`(28) 或 `ICON_BTN_GAP`(2) 改了 →
    /// 中心距那条红；把 `HEADER_RIGHT` 从 12 改回旧的 21 → 贴右沿那条红。
    ///
    /// **标签那一条第一版是假绿，变异当场被抓**：它按 `all_text_rects` 筛带（中心点在 0..=46
    /// 才算顶栏），而 28 的按钮框正好占满内容高（`46 − 9 − 9`），标签画在框底下时中心落在 47——
    /// 越了带、屏幕上却照样看得见。现在按 `min.y < HEADER_H` 筛，并先钉「筛子收满了东西」。
    #[test]
    fn the_action_group_is_icon_only_and_flush_right() {
        let out = frame(&mut loaded());
        // 「顶栏里」= 文字矩形**起在** 46pt 那条线上面。按中心点筛会漏：28 的按钮框正好占满
        // 内容高（46 − 9 − 9），标签画在框底下时中心落在 47，越了带却照样看得见。
        let in_header: Vec<String> = visible_texts(&out)
            .iter()
            .filter(|(_, r)| r.min.y < HEADER_H)
            .map(|(s, _)| s.clone())
            .collect();
        // 先钉这条筛子**确实在收东西**（11 枚图标 + 分支名 + 计数），否则下面的空命中
        // 可能只是"什么都没筛到"的假绿。
        assert!(in_header.len() >= 12, "顶栏只筛出 {} 块文字，这条判据的筛子没生效", in_header.len());
        for (_, label, _) in TOOLS.iter() {
            assert!(
                !in_header.iter().any(|t| t.starts_with(label)),
                "顶栏还写着「{label}」：§4.1 的按钮是纯图标，中文归 tooltip"
            );
        }
        let band = 0.0..=HEADER_H;
        // ⊙ = 最右那枚（设置）。图标 14px（§4.1）。
        assert_eq!(text_size(&out, "⊙", band.clone()), 14.0, "图标的字号不是 §4.1 的 14px");
        let last = all_text_rects(&out, band.clone(), &["⊙"]).into_iter().next().expect("顶栏没有 ⊙ 那枚图标").1;
        // 28 的命中框贴右沿让 12 → 框中心在 1050 − 12 − 14 = 1024，图标画在框正中。
        assert!((last.center().x - 1024.0).abs() < 1.0, "最右一枚图标的中心 x={}，应该贴右沿让 12", last.center().x);
        // 相邻两枚的中心距 = 28 + 2（`.btns{gap:2}`）。⊙ 左边那枚是 ▣。
        let prev = all_text_rects(&out, band.clone(), &["▣"]).into_iter().next().expect("顶栏没有 ▣ 那枚图标").1;
        assert!((last.center().x - prev.center().x - 30.0).abs() < 1.0, "按钮间距不是 28+2：{}", last.center().x - prev.center().x);
    }

    /// §2 的分支那一格（`.branch-chip`）：分支名 12px 次要灰，`↑n ↓m` 同字号走正文档 + 真字重。
    /// `main` 这一帧在顶栏、侧栏、状态栏**三处**都出现，`↑2 ↓0` 也在顶栏和状态栏各一处——
    /// 所以这三条全按 y 带筛，只问顶栏那 46pt。
    #[test]
    fn the_branch_chip_reads_name_then_counts() {
        let out = frame(&mut loaded());
        let band = 0.0..=HEADER_H;
        assert_eq!(text_color(&out, "main", band.clone()), Color32::from_rgb(0x59, 0x63, 0x6E), "分支名不是 §3.1 的次要档");
        assert_eq!(text_size(&out, "main", band.clone()), 12.0, "分支名不是 §2 的 12px");
        assert_eq!(text_color(&out, "↑2 ↓0", band.clone()), Color32::from_rgb(0x1F, 0x23, 0x28), "↑↓ 计数不是 §3.1 的正文档");
        assert_eq!(text_family(&out, "↑2 ↓0", band), "inter-semibold", "↑↓ 计数没走真字重（§3.4：数字归 600 那档）");
    }

    /// §2 那句「白底 + 下 1px 分隔线」：顶栏和内容区之间只有**一条** `BORDER_SUBTLE` 的
    /// 横线，横贯整窗、贴着带的下沿。底色那条判据（`bands_match_the_design_spec`）问的是
    /// "带是不是白的"，问不到这条线——删掉 `toolbar` 末尾那句 `line_segment`，
    /// 只有这一条会红。
    #[test]
    fn the_header_divides_itself_with_a_1px_line() {
        let out = frame(&mut blank());
        let lines: Vec<f32> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::LineSegment { points, stroke }
                    if stroke.color == BORDER_SUBTLE && stroke.width == 1.0
                        && (points[0].y - points[1].y).abs() < 0.01
                        && points[0].y < HEADER_H
                        && points[0].x <= 1.5
                        && points[1].x >= WIN_W - 1.5 => Some(points[0].y),
                _ => None,
            })
            .collect();
        assert_eq!(lines.len(), 1, "顶栏下沿该有且只有一条横贯整窗的分隔线：{lines:?}");
        assert!((lines[0] - 45.5).abs() < 1.0, "分隔线画在 y={}，应该贴着 46 那条带的下沿", lines[0]);
    }

    /// 「＋ 添加仓库」那一枚是**幽灵**胶囊（预览稿 `.tab.add`）：不铺底、只描一圈
    /// `BORDER_STRONG`、字走 §3.1 的辅助档 12px。它是空书签列表里**唯一**的入口，
    /// 所以"画得轻"不能等于"没画"。
    /// 反向变异：把 `ghost_tab` 的 `Stroke::new(1.0, BORDER_STRONG)` 换成 `Stroke::NONE`
    /// （= 描边没了，`stroked()` 那一格空）；把颜色换成 `BORDER_SUBTLE`；把 12 换成 13；
    /// 把 `TEXT_MUTED` 换成 `TEXT_SECONDARY`——四条各红一次。
    /// spec 写的是 `1px dashed`：egui 0.37 的圆角矩形描边给不了虚线，这条只钉"有一圈实线"，
    /// 虚线那一档作为已知让步登记在 PROCESS。
    #[test]
    fn the_add_repo_entry_is_a_ghost_pill() {
        // 空列表那一帧：顶栏左组只有这一枚，描边不会有第二块来掺。
        let out = frame(&mut blank());
        let pills: Vec<(Color32, Rect)> = stroked(&out)
            .into_iter()
            .filter(|(_, r)| r.min.y >= 0.0 && r.max.y <= HEADER_H)
            .collect();
        assert_eq!(pills.len(), 1, "顶栏该只有一枚描边胶囊：{pills:?}");
        let (edge, rect) = pills[0];
        assert_eq!(edge, Color32::from_rgb(0xD0, 0xD5, 0xDD), "幽灵 tab 的描边不是 §3.1 的 BORDER_STRONG");
        assert!((rect.height() - 26.0).abs() < 1.0, "幽灵 tab 高 {}，§2 的胶囊都是 26", rect.height());
        assert!((rect.min.x - 92.0).abs() < 1.0, "空列表时它就该顶在 inset 92 起：{rect:?}");
        let band = 0.0..=HEADER_H;
        assert_eq!(text_color(&out, "＋ 添加仓库", band.clone()), Color32::from_rgb(0x8B, 0x94, 0x9E), "幽灵 tab 的字不是 §3.1 的辅助档");
        assert_eq!(text_size(&out, "＋ 添加仓库", band), 12.0, "幽灵 tab 的字不是 `.tab.add` 的 12px");
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

    /// §2 把计数从"不许有"改成了"必须有"（分组头 = 11px 灰字 + **右对齐计数**），所以这条
    /// 从上一代的「侧栏没有计数」翻了过来：它现在钉的是**括号计数**和**字符画的三角/黑点**
    /// 都不许回来——计数只能是指甲盖右边那枚裸数字（`the_sidebar_group_headers_carry_a_right_aligned_count`
    /// 钉它的字号、颜色和落位），当前分支只能用 §2 那颗真圆点形状。
    #[test]
    fn the_sidebar_has_no_parenthesised_counts_or_character_glyph_markers() {
        let out = frame(&mut loaded());
        let t: Vec<String> = all_text_rects(&out, HEADER_H..=WIN_H, &[""])
            .into_iter()
            .filter(|(_, r)| r.min.x < SIDEBAR_W)
            .map(|(s, _)| s)
            .collect();
        assert!(t.iter().all(|s| !s.contains('（')), "侧栏标题里还挂着括号计数「（3）」：{t:?}");
        assert!(t.iter().all(|s| !s.contains('▼') && !s.contains('●')), "侧栏还在用字符画三角/黑点：{t:?}");
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

    /// §3.1 把增/删两行记成**底 + 字一对**（预览稿 `.dl.a`/`.dl.d` 的 `background` 与 `color` 同批给出）。
    /// 上面那条只问底色；字色没人问的话，把 `ADD_FG`/`DEL_FG` 从两处画字的地方摘掉、恢复成正文黑，
    /// 全量 153 条照样绿（这一批的反向变异实测过）——绿得毫无意义。
    /// 两边都写**字面量**：拿 `ADD_FG` 自己比，改常量就是两头一起挪。
    #[test]
    fn added_and_deleted_lines_carry_their_own_text_colour() {
        let out = frame(&mut loaded());
        let top = WIN_H - STATUS_H - STAGING_H;
        // 取差异区里**第一条**那种底色的行，拿到它的 y 档；补丁夹具里增行在前、删行在后各有一条。
        let band = |bg: Color32| {
            let mut ys: Vec<f32> = rects(&out)
                .into_iter()
                .filter(|(c, rc)| *c == bg && rc.min.x > SIDEBAR_W + FILES_W - 1.0 && rc.min.y > top)
                .map(|(_, rc)| rc.center().y)
                .collect();
            ys.sort_by(|a, b| a.total_cmp(b));
            let y = *ys.first().expect("差异区里没有那一行底色带");
            (y - DIFF_ROW_H / 2.0 + 1.0)..=(y + DIFF_ROW_H / 2.0 - 1.0)
        };
        assert_eq!(
            text_color(&out, "+加上的一行", band(ADD_BG)),
            Color32::from_rgb(0x1A, 0x7F, 0x37),
            "增行的字色（§3.1 ADD_FG）"
        );
        assert_eq!(
            text_color(&out, "-删掉的一行", band(DEL_BG)),
            Color32::from_rgb(0xCF, 0x22, 0x2E),
            "删行的字色（§3.1 DEL_FG）"
        );
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
            rows(&mut filled(1, &layout(&patch(400), false))),
            rows(&mut filled(1, &layout(&patch(40_000), false))),
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
        // 图序列也必须是**这一次 snapshot 一起排好**送回来的，行数跟提交数一一对上。
        assert_eq!(snap.graph.len(), snap.commits.len(), "图序列和提交列表对不上行号");
        // `open()` 现在还会顺手取一份工作区差异（选中行得和右边对得上，见
        // `opening_a_dirty_repo_shows_its_working_diff_without_a_click`），所以快照落地
        // 这一刻 busy 可以是 true——那是第二件事还在途。等它自己清掉、标签自己落下来；
        // 清不掉就是真卡死，`pump` 600 帧会红。
        assert!(app.target.is_some(), "开仓库没给差异区派活：选中行和右边对不上");
        pump(&mut app, |a| !a.busy && !a.patch_label.is_empty());
        // 排布局的线程不是主线程：帧里一次都没排，图序列却已经画得出来了。
        assert_eq!(graph_layouts(), 0, "主线程排了 {} 次图序列：这笔开销又回到帧里了", graph_layouts());
    }

    // ------------------------------------------------------------ 图序列（提交 DAG）

    /// 图序列那一列里落出来的形状：`(圆点, 直线段, 弯线)`。
    /// 按**车道配色**认，不按图元自己的坐标认——「形状有没有越出这一列」恰恰是判据本身，
    /// 先用坐标筛掉就等于把判据筛没了。
    /// 但**要按 `clip_rect` 筛一道**：§3.3 把第 0 道钉成 `#0969DA`，那和 §3.1 的主色
    /// `ACCENT` 是同一个值，于是侧栏那颗「当前分支」蓝点（widgets.rs 的 `SIDE_DOT_R`）
    /// 会被按颜色误认成提交点（实测 3 条提交数出 4 个点、半径表里多出一格 3.5）。
    /// `clip_rect` 是**画它的那个面板**的边界，不是图元位置：图元往右越出 48pt 那一列时
    /// clip 仍然是内容区，所以 `the_graph_never_escapes_its_column` 照旧能红。
    fn graph_marks(out: &egui::FullOutput) -> (Vec<Pos2>, Vec<[Pos2; 2]>, Vec<[Pos2; 4]>) {
        let lane32 = |c: Color32| LANE_COLORS.contains(&c);
        let lane = |m: &egui::epaint::ColorMode| {
            matches!(m, egui::epaint::ColorMode::Solid(c) if lane32(*c))
        };
        let in_content = |c: &egui::epaint::ClippedShape| c.clip_rect.min.x >= SIDEBAR_W - 0.5;
        let (mut dots, mut lines, mut arms) = (Vec::new(), Vec::new(), Vec::new());
        for c in &out.shapes {
            if !in_content(c) {
                continue;
            }
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

    /// 按**整串相等**找落点。`locate` 是前缀匹配，而这一批里「搜索」同时是侧栏那一行的名字、
    /// 搜索框占位词「搜索提交（说明 / 文件名）」的开头、还有状态词「搜索中…」的开头 ——
    /// 前缀匹配会按绘制顺序撞上第一块，于是「点侧栏那一行」有可能其实点到别处，
    /// 判据还以为点中了（假绿）。侧栏那一行必须用这个。
    fn locate_exact(ctx: &egui::Context, app: &mut Gitx, label: &str) -> Option<Pos2> {
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let hit = out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(t.galley.rect.translate(t.pos.to_vec2()).center())
            }
            _ => None,
        });
        out.textures_delta.clear();
        hit
    }

    /// `click_text` 的整串相等版。找不到返回 false，和它同一条规矩。
    fn click_text_exact(app: &mut Gitx, label: &str) -> bool {
        let ctx = egui::Context::default();
        style(&ctx);
        let Some(at) = locate_exact(&ctx, app, label) else { return false };
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
        let over = pos2((SIDEBAR_W + WIN_W) / 2.0, HEADER_H + 120.0);
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

    /// §2 的图列只有 **48pt**，`theme.rs` 里那句「四道落在 12/22/32/42、圆点最右到 46」是
    /// S6-4 拍的（`LANE0_X 12` + `LANE_W 10`）。这条判据存在的全部理由：**原来的夹具一条都不
    /// 验它**——`branch_commits()` 最多同时占 3 条道，反向变异把 `LANE_W` 从 10 换成 12 时
    /// 全套 177 条只红了一条（那条还是钉字面值的），第 3 道的圆心 188+12+36=**236** 正好压在
    /// 列的右边界上、圆点右沿 240 已经越出去 4pt，而 `the_graph_never_escapes_its_column`
    /// 比的是**圆心**、且拿 `COL_GRAPH` 常量比自己，两头都看不见。
    /// 于是这里用一枚**八爪合并**（一个提交四个父亲）把四条道真占满，再按**圆点右沿**判，
    /// 边界写死字面量 48（§2 给的列宽）而不是 `COL_GRAPH`。
    #[test]
    fn four_lanes_stay_inside_the_forty_eight_point_column() {
        let node = |id: &str, parents: &[&str]| Commit {
            id: id.to_owned(),
            short_id: id.to_owned(),
            subject: format!("主题{id}"),
            author: "包融".to_owned(),
            author_email: "br@example.com".to_owned(),
            date: "2026-10-20T11:41:07+08:00".to_owned(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
        };
        let commits = vec![
            node("O", &["L", "B", "C", "D"]),
            node("D", &["L"]),
            node("C", &["L"]),
            node("B", &["L"]),
            node("L", &["R"]),
            node("R", &[]),
        ];
        let lanes: Vec<usize> = layout_graph(&commits).iter().map(|r| r.lane).collect();
        assert_eq!(
            lanes.iter().max(),
            Some(&3),
            "夹具自己没占满四条道（读到 {lanes:?}），下面那条越界判据就成了空判"
        );
        let mut app = filled(1, &[]);
        let snap = app.snap.as_mut().expect("filled 造好了快照");
        snap.graph = layout_graph(&commits);
        snap.commits = commits;
        let out = frame_at(&mut app, WIN_W, WIN_H + STATUS_H);
        let (dots, _, _) = graph_marks(&out);
        assert!(dots.len() >= 4, "六条提交只画了 {} 个点，图序列没进帧", dots.len());
        // 第 0 道到第 3 道都得有圆点落过，否则「四条道装得下」没被真画出来验过。
        assert_eq!(
            lanes_of(&dots).into_iter().collect::<std::collections::HashSet<_>>(),
            (0..4).collect::<std::collections::HashSet<_>>(),
            "落图的圆点没铺满四条道：{dots:?}"
        );
        for d in &dots {
            assert!(
                d.x - 4.0 >= SIDEBAR_W && d.x + 4.0 <= SIDEBAR_W + 48.0,
                "半径 4 的圆点落在 x={:?}，左右沿 {:?}..{:?} 越出 §2 那 48pt 的图列（{SIDEBAR_W}..{}）",
                d.x,
                d.x - 4.0,
                d.x + 4.0,
                SIDEBAR_W + 48.0
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

    /// 提交号那一整列在 §2 里没有（四列 = 图 48 | 提交信息 | 作者 96 | 日期 78），S6-4 把它删了。
    /// **删列不等于删信息**：筛选条还吃 `short_id` 前缀（`app.rs` 里 `c.short_id.starts_with(&q)`
    /// 那一挡），输一段号还能把那条提交捞出来。实测两轮变异：删掉那一挡 → 第一条红（`[]`）；
    /// 把整个谓词写成恒真 → 还是第一条红（捞出三条）。
    /// **所以第二条断言本轮没有独立杀伤路径**（任何放过 `zzzz` 的改法几乎都同时放过 `a0001`，
    /// 先红在第一条）——它留在这儿是为了把「筛过之后真的变少」这件事写进同一格，别当已验。
    #[test]
    fn the_filter_still_finds_a_commit_by_its_short_id() {
        let mut app = filled(3, &[]);
        app.log_filter = "a0001".to_owned();
        let subjects = |out: &egui::FullOutput| -> Vec<String> {
            texts(out)
                .into_iter()
                .map(|(s, _)| s)
                .filter(|s| s.starts_with("主题"))
                .collect()
        };
        let hit = subjects(&frame(&mut app));
        assert_eq!(hit, vec!["主题1：一条中文提交信息"], "按提交号前缀筛，捞出来的不是那一条：{hit:?}");
        app.log_filter = "zzzz".to_owned();
        let miss = subjects(&frame(&mut app));
        assert!(miss.is_empty(), "一个不存在的前缀捞出了 {} 条：筛选没起作用", miss.len());
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

    /// 带一个「鼠标在这儿」的帧：hover 那一档底色只能这样问（`frame()` 的输入里根本没有指针位）。
    fn frame_with_pointer(app: &mut Gitx, at: egui::Pos2) -> egui::FullOutput {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut out = ctx.run_ui(
            raw_input(vec![egui::Event::PointerMoved(at)]),
            |ui| app.shell(ui),
        );
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

    /// `warm_frame` 那一套，但**时钟在走**：同一个 Context 里连喂 8 帧，`RawInput::time`
    /// 从 0 涨到 `secs`，取最后一帧。
    /// 为什么要它：egui 的 `Area` 默认带淡入（area.rs:152 `fade_in: true`，算式在 698），
    /// 整层颜色按 `(i.time - 这层第一次出现的时间) / global_style().animation_time`（默认 0.2s）
    /// 乘一遍不透明度。`raw_input` 不给 `time`（留 `None` 时 egui 自己去取墙上时钟），两帧之间
    /// 只隔几十微秒，所以逐帧问下去那一层**停在淡入的第一格**（实测卡底 255 的白被乘成 60，
    /// 连遮罩的 80 也只剩 19）——形状在、位置对，颜色不是本来的颜色。要看「淡完那一格」
    /// 只能把钟拨起来。
    fn settled_frame(app: &mut Gitx, secs: f64) -> egui::FullOutput {
        let ctx = egui::Context::default();
        style(&ctx);
        // 头一帧就是 `warm` 那一遍空跑（`Order::Foreground` 那层要到第二遍才画，见 `warm`），
        // 时间戳从 0 起：`Area` 把「第一次出现」记在 Context 里，所以这一整套必须共用一个
        // Context、且**每一帧都给显式 `time`**——留 `None` 会让 egui 自己取墙上时钟，
        // 那和后面这些 0.05、0.10… 不在同一个刻度上，年龄算出来是负的，淡入永远走不完。
        let mut out = ctx.run_ui(raw_timed(0.0), |ui| app.shell(ui));
        out.textures_delta.clear();
        for i in 1..=8 {
            let mut next = ctx.run_ui(raw_timed(secs * i as f64 / 8.0), |ui| app.shell(ui));
            // 换掉上一帧时那份纹理增量得先 `clear`：中缝里没人消费它，留着不放会在 drop 时 panic。
            next.textures_delta.clear();
            out = next;
        }
        out
    }

    /// `raw_input` 那一格，但把 egui 的时钟钉在 `secs`（见 `settled_frame` 为什么要钉）。
    fn raw_timed(secs: f64) -> egui::RawInput {
        let mut ri = raw_input(Vec::new());
        ri.time = Some(secs);
        ri
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

    /// 敲字 + 回车，**全程共用一个 `Context`**。这一点是必须的：搜索那格的触发读的是
    /// `resp.lost_focus() && key_pressed(Enter)`，而控件焦点记在 `Context` 的 memory 里
    /// ——`press()` 每帧换一个 Context（见 `frame_events`），拿它按回车等于在没有焦点的窗口里
    /// 按回车，永远接不住。回车按 `press` 同样的两帧（按下、松手）喂。
    /// 返回 false = 那一格的占位词没画出来。
    fn type_into_and_enter(app: &mut Gitx, hint: &str, text: &str) -> bool {
        let ctx = warm(app);
        let Some(at) = locate(&ctx, app, hint) else { return false };
        click_at(&ctx, app, at);
        let mut feed = |events: Vec<egui::Event>| {
            ctx.run_ui(raw_input(events), |ui| app.shell(ui))
                .textures_delta
                .clear();
        };
        feed(vec![egui::Event::PointerMoved(at), egui::Event::Text(text.to_owned())]);
        let key = |pressed: bool| egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        feed(vec![egui::Event::PointerMoved(at), key(true)]);
        feed(vec![egui::Event::PointerMoved(at), key(false)]);
        true
    }

    // --------------------------------------------------------------- 卡上定位三件套
    //
    // 面板上的词和卡里的词撞得很凶：内联提交框的占位文字「提交摘要… (⌘/Ctrl+Enter 提交)」
    // 把卡里那一格「提交摘要」整个盖在前面，而 `locate` 是**前缀匹配、取 shapes 里第一块**，
    // 于是会把鼠标送到遮罩上、还以为点中了卡里那一格。所以凡在卡上点东西，一律先把范围
    // 收到卡片矩形里；按钮那一类再按整串相等（见 `click_card_button` 那段）。

    /// 此刻那张卡的矩形：和 `modal_layer` 用同一份 `card_rect`、同一块无头屏幕。
    /// 卡片矩形是**算出来的**（`rows()` × 那几条常量），不是从帧里量出来的——所以问尺寸的判据
    /// （`the_settings_card_is_as_tall_as_its_rows` 那三条）都不靠它，靠帧里那块白底。
    fn card_now(app: &Gitx) -> Rect {
        let m = app.modal.as_ref().expect("判据要在卡上点东西，可这会儿没有打开的卡");
        card_rect(m, Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H)))
    }

    /// 卡里按文字找落点。`exact` = 整串相等；`foot` = 只看卡脚那一条带子（`modal_layer` 把两枚
    /// 按钮钉在 `card.max.y - MODAL_FOOT_H/2`，按钮只可能在那儿）。两个开关都要：提交卡的**标题**就叫
    /// 「提交」，和主按钮同名，光按整串相等认还是会点到标题上——那一格不吃点击，判据还以为
    /// 「点了没关 = 产品错了」。
    ///
    /// **只看卡底那一笔之后的文字**（和 `body_void` 同一道筛）。以前只按 `card.contains` 裁，
    /// 那句「遮罩后面那层的同名词挡不了道」是**尺寸巧合**：卡 268 高的时候暂存带里那格内联
    /// 提交框（占位词也是「提交摘要」）还在卡外头。S6-6 把行距 28→32、卡换成 560 宽那一档之后
    /// 卡长到 318，那格文字被装进了卡的矩形里，`find` 按**图层顺序**先撞上背景那一层，
    /// 点下去落到遮罩上 = 关卡（三条判据同时红在「这会儿没有打开的卡」）。
    /// 矩形包含 ≠ 那一层，判据自己也得按图层认。
    fn locate_card(
        ctx: &egui::Context,
        app: &mut Gitx,
        label: &str,
        exact: bool,
        foot: bool,
    ) -> Option<Pos2> {
        let card = card_now(app);
        let band = (card.max.y - MODAL_FOOT_H)..=card.max.y;
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let from = out
            .shapes
            .iter()
            .position(|c| {
                matches!(&c.shape, egui::Shape::Rect(r) if r.rect == card && r.fill != egui::Color32::TRANSPARENT)
            })
            .unwrap_or(0);
        let hit = out.shapes[from..].iter().find_map(|c| match &c.shape {
            egui::Shape::Text(t) => {
                let text = t.galley.text();
                let r = t.galley.rect.translate(t.pos.to_vec2());
                let named = if exact { text == label } else { text.starts_with(label) };
                let placed = card.contains(r.center()) && (!foot || band.contains(&r.center().y));
                (named && placed).then_some(r.center())
            }
            _ => None,
        });
        out.textures_delta.clear();
        hit
    }

    /// 在卡上点一项：说明行、勾选框、页签这一类按前缀就够。
    fn click_card(app: &mut Gitx, label: &str) -> bool {
        let ctx = warm(app);
        match locate_card(&ctx, app, label, false, false) {
            Some(at) => {
                click_at(&ctx, app, at);
                true
            }
            None => false,
        }
    }

    /// 在卡上点**按钮**（`hunk_button` 那一格落的文字就是按钮名本身）。
    fn click_card_button(app: &mut Gitx, label: &str) -> bool {
        let ctx = warm(app);
        match locate_card(&ctx, app, label, true, true) {
            Some(at) => {
                click_at(&ctx, app, at);
                true
            }
            None => false,
        }
    }

    /// 往卡里那一格文本框敲字（喂 `Event::Text`，和 `type_into` 同一条真输入通路）。
    fn type_into_card(app: &mut Gitx, hint: &str, text: &str) -> bool {
        let ctx = warm(app);
        let Some(at) = locate_card(&ctx, app, hint, false, false) else { return false };
        click_at(&ctx, app, at);
        ctx.run_ui(
            raw_input(vec![egui::Event::PointerMoved(at), egui::Event::Text(text.to_owned())]),
            |ui| app.shell(ui),
        )
        .textures_delta
        .clear();
        true
    }

    /// 卡上画没画出这个词（前缀）。第二遍才有卡，所以走 `warm`。
    fn card_has(app: &mut Gitx, label: &str) -> bool {
        let ctx = warm(app);
        locate_card(&ctx, app, label, false, false).is_some()
    }

    /// 卡脚上画没画出那枚**按钮**（整串相等 + 落在卡脚那一条带子里）。
    fn card_has_button(app: &mut Gitx, label: &str) -> bool {
        let ctx = warm(app);
        locate_card(&ctx, app, label, true, true).is_some()
    }

    /// 从顶栏那枚幽灵 tab「＋ 添加仓库」端出卡片（§2：入口住在 46pt 那条带里，旧的第二行
    /// tab 条已经并掉了）。字面以 `＋ ` 开头，所以按**包含**找、并且**问一嘴在不在顶栏**。
    fn open_add_card(app: &mut Gitx) {
        let ctx = egui::Context::default();
        style(&ctx);
        let at = locate_has(&ctx, app, "添加仓库").expect("顶栏上没有「＋ 添加仓库」那枚幽灵 tab");
        assert!(at.y < HEADER_H, "「＋ 添加仓库」不在顶栏那 46pt 里：y={}", at.y);
        click_at(&ctx, app, at);
        assert!(app.modal.is_some(), "点了「＋ 添加仓库」没开出卡");
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

    /// amend：勾上「修正上一条」之后，提交按钮/⌘Enter 落地的是 `--amend`——
    /// 提交条数不变、上一条换成新信息、已暂存内容并进同一条，落地后勾自动去掉。
    #[test]
    fn ticking_amend_rewrites_head_instead_of_adding_a_commit() {
        let (mut app, dir) = open_repo("amend");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(click_text(&mut app, "修正上一条"), "提交框里没有「修正上一条」");
        assert!(app.amend, "点了勾但 app.amend 还是 false");
        assert!(type_into(&mut app, "提交摘要", "改写后的提交"), "提交摘要没找到");
        // 用按钮而不是 ⌘Enter：勾选框刚被点过，焦点还在它身上，Enter 可能被当成
        // 勾选框的激活键（把勾又按掉）。按钮和回车最终都走同一个 `Action::Amend`。
        assert!(click_text(&mut app, "修正上一条提交"), "勾上之后按钮没换成「修正上一条提交」");
        assert!(app.modal.is_some(), "amend 没先问一句");
        assert!(click_warm(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |a| {
            a.snap
                .as_ref()
                .and_then(|s| s.commits.first())
                .map(|c| c.subject == "改写后的提交")
                .unwrap_or(false)
        });

        let snap = app.snap.as_ref().unwrap();
        assert_eq!(snap.commits.len(), before, "amend 多出了一条提交");
        assert!(!app.amend, "amend 落地后勾还留着");
        assert!(
            git_in(&dir, &["show", "HEAD:a.txt"]).contains("第二行"),
            "已暂存的内容没并进 amend 后的 HEAD"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 整份差异里有多个文件时，每个文件前有一条文件头带；点它只折叠自己名下的行，
    /// 再点一次展开。整份补丁不画文件头的话，两个文件的首尾是连着的，分不出哪儿换了文件。
    #[test]
    fn a_multi_file_diff_folds_one_file_at_a_time() {
        let (mut app, dir) = open_repo("fold");
        // 造第二个未暂存文件：先入库、再改，这样 `git diff` 里才有它。
        std::fs::write(dir.join("b.txt"), "b 一行\n").unwrap();
        git_in(&dir, &["add", "b.txt"]);
        std::fs::write(dir.join("b.txt"), "b 一行\nb 第二行\n").unwrap();
        // a.txt 上一批的改动已经暂存，再改一次，让它在未暂存差异里也有一块。
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n第三行\n").unwrap();

        app.request(Target::Working { staged: false, path: None, untracked: false });
        pump(&mut app, |a| a.diff.iter().filter(|l| l.kind == Kind::File).count() == 2);

        let headers: Vec<&str> = app
            .diff
            .iter()
            .filter(|l| l.kind == Kind::File)
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(headers, ["a.txt", "b.txt"], "两个文件该各有一条文件头");

        let t = texts(&frame(&mut app));
        assert!(t.iter().any(|(s, _)| s.contains("第三行")), "a.txt 的改动没画出来");
        assert!(t.iter().any(|(s, _)| s.contains("b 第二行")), "b.txt 的改动没画出来");

        let before = app.diff_rows.len();
        assert!(click_text(&mut app, "文件  a.txt"), "找不到 a.txt 的文件头带");
        assert!(app.collapsed.contains(&1), "点了 a.txt 的文件头但它没被折叠");
        assert!(app.diff_rows.len() < before, "折叠之后可见行没变少");
        let t = texts(&frame(&mut app));
        assert!(!t.iter().any(|(s, _)| s.contains("第三行")), "折叠了 a.txt，它的内容还画着");
        assert!(t.iter().any(|(s, _)| s.contains("b 第二行")), "折叠 a.txt 把 b.txt 也藏了");
        assert!(t.iter().any(|(s, _)| s.contains("已折叠  a.txt")), "折叠后文件头没换文案");

        assert!(click_text(&mut app, "已折叠  a.txt"), "折叠后的文件头带点不到");
        assert!(!app.collapsed.contains(&1), "再点一次没有展开");
        let t = texts(&frame(&mut app));
        assert!(t.iter().any(|(s, _)| s.contains("第三行")), "展开之后 a.txt 的内容没回来");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 差异区灰带右端那格搜索：只留命中的内容行，以及它们的文件头与 hunk 头。
    /// 文件头/hunk 头不是装饰——没有它们，几段命中混在一起就不知道各自属于哪个文件、
    /// 从第几行起。宽度是估的（参考图没量到这条），所以这里只测行为不测宽高。
    #[test]
    fn the_diff_search_keeps_only_matching_lines_with_their_context() {
        let (mut app, dir) = open_repo("diff-search");
        std::fs::write(dir.join("b.txt"), "b 一行\n").unwrap();
        git_in(&dir, &["add", "b.txt"]);
        std::fs::write(dir.join("b.txt"), "b 一行\nb 第二行\n").unwrap();
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n第三行\n").unwrap();

        app.request(Target::Working { staged: false, path: None, untracked: false });
        pump(&mut app, |a| a.diff.iter().filter(|l| l.kind == Kind::File).count() == 2);

        assert!(type_into(&mut app, "搜索差异", "第三行"), "灰带上没有搜索框");
        let t = texts(&frame(&mut app));
        assert!(t.iter().any(|(s, _)| s.contains("第三行")), "命中的行被筛掉了");
        assert!(!t.iter().any(|(s, _)| s.contains("b 第二行")), "没命中的行还画着");
        assert!(t.iter().any(|(s, _)| s.contains("文件  a.txt")), "命中行的文件头没留下");
        assert!(!t.iter().any(|(s, _)| s.contains("文件  b.txt")), "整份文件都没命中，它的头还留着");
        assert!(t.iter().any(|(s, _)| s.starts_with("块 ")), "hunk 头被筛掉了，行号没了上下文");

        // 在同一格输入框里连按三次退格清空。框里此刻写着「第三行」，而差异里也有一行
        // `+第三行`——`locate` 取的是帧里**先画**的那块字，灰带在滚动区之前，所以点到的是框。
        let ctx = warm(&mut app);
        let at = locate(&ctx, &mut app, "第三行").expect("搜索框里的字找不到");
        click_at(&ctx, &mut app, at);
        let key = |pressed: bool| egui::Event::Key {
            key: egui::Key::Backspace,
            physical_key: Some(egui::Key::Backspace),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        // 点中间落下的光标不一定在行尾，先按 End 顶到末尾；退格删到行首就停了。
        let end = |pressed: bool| egui::Event::Key {
            key: egui::Key::End,
            physical_key: Some(egui::Key::End),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        for pressed in [true, false] {
            ctx.run_ui(raw_input(vec![end(pressed)]), |ui| app.shell(ui))
                .textures_delta
                .clear();
        }
        for _ in 0..3 {
            for pressed in [true, false] {
                ctx.run_ui(raw_input(vec![key(pressed)]), |ui| app.shell(ui))
                    .textures_delta
                    .clear();
            }
        }
        assert_eq!(app.diff_search, "", "三下退格没把搜索框清空");
        let t = texts(&frame(&mut app));
        assert!(t.iter().any(|(s, _)| s.contains("b 第二行")), "清空搜索之后 b.txt 没回来");
        assert!(t.iter().any(|(s, _)| s.contains("文件  b.txt")), "清空搜索之后 b.txt 的文件头没回来");
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
        assert!(!app.topo, "点「{DATE_ORDER}」没换回来");
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
        for label in [CURRENT_BRANCH, SHOW_REMOTE, DATE_ORDER] {
            assert_eq!(marks(label), 1, "「{label}」那枚的第二层标记不是一枚");
        }
        assert_eq!(marks("紧凑视图"), 0, "还没接的那枚不该带标记");
        // 点它给的还是那句实话，不是开一个空菜单。
        assert!(click_text(&mut app, "紧凑视图"), "「紧凑视图」没画出来");
        assert_eq!(app.note.as_deref(), Some("egui 版还没接：紧凑视图"), "点没接的那枚该说实话");
    }

    /// 四条官方文案**逐字**钉住。字面量写死在这条判据里、**不引用** `widgets.rs` 那几个常量：
    /// 引用常量的话把常量改成任何别的字都照样绿（方法论第 1 条「常量自比」）。
    /// 出处 = 本机 SourceTree 4.2.19 `Contents/Resources/zh_CN.lproj/Localizable.strings`
    /// （UTF-16LE，得 `iconv -f UTF-16 -t UTF-8` 才搜得到）：
    /// `LogDateOrder`=按日期排序、`JumpToRevisionLabelGit`=转到Commit的SHA-1、
    /// `PendingFiles`+`SortedByPath`=待定的文件, 已依照路径排序、`LogBranchesCurrent`=当前分支。
    /// 正向按**整串相等**比：「已依照路径排序」是「已依照路径 (反向) 排序」的子串，
    /// 反向按 `contains` 比：旧自创文案以任何形式回潮都该看得见。
    #[test]
    fn the_filter_bar_and_summary_carry_the_official_strings() {
        let (mut app, dir) = open_repo("official-strings");
        let out = frame(&mut app);
        let t = texts(&out).into_iter().map(|(s, _)| s).collect::<Vec<_>>();
        for want in [
            "按日期排序 ▼",
            "当前分支 ▼",
            "转到Commit的SHA-1",
            "待定的文件, 已依照路径排序",
        ] {
            assert!(t.iter().any(|s| s == want), "没有整串「{want}」，这一帧画的是：{t:?}");
        }
        // 我们自己的旧说法一条都不许留下（「日期顺序」是我们自己写的，官方没这四个字）。
        for gone in ["日期顺序", "跳转到:", "按路径排序"] {
            assert!(!t.iter().any(|s| s.contains(gone)), "自创文案回潮「{gone}」：{t:?}");
        }
        let _ = dir;
    }

    /// 开仓默认那档 = 当前分支。**这条读的是配置对象而不是画布**（方法论第 11 条）：
    /// 无头夹具自己填 `Gitx` 的字段，产品构造函数里那一位在帧里根本读不到。
    /// 和核心层 `commits()` == `commits_on(HEAD)` 是一对：一条钉壳的默认，一条钉核心的默认。
    #[test]
    fn the_log_opens_on_the_current_branch() {
        assert_eq!(crate::ui::app::default_scope(), git::Scope::Current, "开仓默认没走当前分支");
    }

    /// 筛选条最左那枚是**三档**：所有分支（`--all`）/ 当前分支（不传范围，走 HEAD）/
    /// 具体分支（把它当位置参数传给 `git log`）。前两档是 SourceTree 的官方两态，第三档是我们加的。
    /// 支线从**初始提交**分出去、主线再往前走一步，于是两条线各有一个对面拿不到的提交——
    /// 拿长度当判据不行（两边都是 2 条），只能比**提交主题**，每档再拿 `git rev-list` 复核一次长度。
    #[test]
    fn the_scope_menu_switches_between_all_branches_and_the_current_one() {
        let (mut app, dir) = open_repo("scope-three-arm");
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
        // 默认那一档 = 当前分支。**这一条同时钉住"没把默认偷偷换成 --all"**：
        // 换成 --all 的话支线的提交会在第一帧就出现在列表里。
        assert_eq!(app.scope, git::Scope::Current, "默认不该带范围");
        assert!(filter_text(&mut app, CURRENT_BRANCH).is_some(), "按钮上没写「当前分支」");
        assert!(!line_of(&app).contains(&"只在支线".to_owned()), "HEAD 那条线里不该有支线的提交");

        popup_pick(&mut app, CURRENT_BRANCH, ALL_BRANCHES);
        assert_eq!(app.scope, git::Scope::All, "菜单里点「所有分支」没落到范围上");
        assert!(app.busy && app.snap.is_none(), "换了范围没重取列表：日志会停在旧范围上");
        pump(&mut app, |a| a.snap.is_some() && a.scope == git::Scope::All);
        let l = line_of(&app);
        for want in ["只在主线", "只在支线"] {
            assert!(l.contains(&want.to_owned()), "「所有分支」那一档没有「{want}」：{l:?}");
        }
        let want = git_in(&dir, &["rev-list", "--all", "--count"]).parse::<usize>().expect("数字");
        assert_eq!(l.len(), want, "所有分支的条数和仓库里的对不上：app 说 {l:?}");
        assert!(filter_text(&mut app, ALL_BRANCHES).is_some(), "换了档，按钮上还是旧那一项");
        assert_eq!(filter_text(&mut app, CURRENT_BRANCH), None, "按钮上两档同时出现：文案没跟着换");

        popup_pick(&mut app, ALL_BRANCHES, "side");
        assert_eq!(app.scope, git::Scope::Rev("side".to_owned()), "菜单里点了分支没落到范围上");
        pump(&mut app, |a| a.snap.is_some() && a.scope == git::Scope::Rev("side".to_owned()));
        let l = line_of(&app);
        assert!(l.contains(&"只在支线".to_owned()), "按支线筛完，列表里没有支线独有的提交：{l:?}");
        assert!(!l.contains(&"只在主线".to_owned()), "支线列表里混进了只有主线走得到的提交：{l:?}");
        let want = git_in(&dir, &["rev-list", "--count", "side"]).parse::<usize>().expect("数字");
        assert_eq!(l.len(), want, "列表长度和仓库里的对不上：app 说 {l:?}");

        // 回到「当前分支」= 范围清空，支线的提交得再消失一次。
        popup_pick(&mut app, "side", CURRENT_BRANCH);
        assert_eq!(app.scope, git::Scope::Current, "点「当前分支」没把范围清空");
        pump(&mut app, |a| a.snap.is_some() && a.scope == git::Scope::Current);
        assert!(!line_of(&app).contains(&"只在支线".to_owned()), "回到「当前分支」还留着支线");
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
        // 提交号那一列不再画了（§2 四列 = 图 48 | 提交信息 | 作者 96 | 日期 78），所以**认行**
        // 这件事从「按提交号找」换成「按提交信息找」：这三条 subject 各自只在一行里出现。
        let subjects = ["短", "一条长得多的", "中等长度的一条"];
        let subject: Vec<f32> = subjects.iter().map(|s| x(s, rows.clone())).collect();
        let author = [x("包融", rows.clone()), x("baorong", rows.clone()), x("R", rows.clone())];
        let row_band = |label: &str| {
            let r = text_rect(&out, label, rows.clone());
            (r.center().y - ROW_H / 2.0 + 1.0)..=(r.center().y + ROW_H / 2.0 - 1.0)
        };
        // 日期列这一格给的是**相对文案**（「今天 09:05」「昨天 23:58」），文字本身跟着本机时区漂：
        // 同一条 `2020-03-04T05:06+08:00` 换到 UTC 机器上是 03-03。所以按内容找它会假红，
        // 改成按**位置**找——一行里最靠右的那块文字就是日期，它的左缘即这一列的左缘。
        let rightmost = |band: std::ops::RangeInclusive<f32>| {
            all_text_rects(&out, band, &[""])
                .into_iter()
                .map(|(_, rc)| rc.min.x)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let date: Vec<f32> = subjects.iter().map(|s| rightmost(row_band(s))).collect();
        flush("提交信息", x("提交信息", head()), [(subject[0], "短"), (subject[1], "一条长得多的…"), (subject[2], "中等长度的一条")]);
        flush("作者", x("作者", head()), [(author[0], "包融"), (author[1], "baorong <…>"), (author[2], "R")]);
        flush("日期", x("日期", head()), [(date[0], "第 1 行"), (date[1], "第 2 行"), (date[2], "第 3 行")]);
        // 绝对位置钉在**字面量**上：表头三个词的左缘、数据跟自己的表头同一条竖线起笔。
        // 拿 `COL_*` 加加减减比给自己看等于没判——那一批改间距从 8 改成 5 时，那种写法照样绿。
        //
        // S6-4 换锚：侧栏 188 + 图列 48 + 内缩 12 = **248**（提交信息起笔）；
        // 作者/日期从右缘 1050 往左定 = **888** / **984**，与侧栏宽无关。
        assert_eq!(subject[0], 248.0, "提交信息列的左缘（§2：侧栏 188 + 图 48 + 内缩 12）");
        assert_eq!(author[0], 888.0, "作者列的左缘（1050 − 日期 78 − 作者 96 + 内缩 12）");
        assert_eq!(date[0], 984.0, "日期列的左缘（1050 − 日期 78 + 内缩 12）");
        // 行距与表头高钉 §2 的 28/28：上面那些判据全是「间距 == ROW_H」的相对关系，
        // 把 ROW_H 改回旧的 26 它们一个都不会红，所以这两个字面值各钉一条。
        assert_eq!(ROW_H, 28.0, "§2「表头 28 + 行 28 × N」里的行距");
        assert_eq!(LOG_HEAD_H, 28.0, "§2 的日志表头那一条");
        // 三枚定宽列也各钉一条字面值。上面 248/888/984 那三条是**加减之后的和**：
        // 把 `COL_GRAPH` 48→50 同时把 `COL_INSET` 12→10，和不变、判据不红，但 §2 的列宽已经改了。
        // 各拆一条字面值之后，任何一枚单独动都会红。
        assert_eq!(
            (COL_GRAPH, COL_AUTHOR, COL_DATE),
            (48.0, 96.0, 78.0),
            "§2 的三枚定宽列：图 48 | 作者 96 | 日期 78（提交信息那一列弹性）"
        );
        // 下面三枚是 §2 **没给**、我们自己拍的数，出处见 DESIGN.md §7；
        // 钉在这儿是为了让「悄悄改一档」变成一次可见的红色，而不是无声的漂移。
        assert_eq!(COL_INSET, 12.0, "列内缩 12 出自预览稿 `.lr{{padding:0 12px}}`（§2 没给这一档）");
        assert_eq!((LANE0_X, LANE_W), (12.0, 10.0), "第 0 道中心 12、道距 10：48pt 装四道的唯一拍法（§7）");
        assert_eq!(FILE_ROW_H, 24.0, "§2 文件表「行高 24」——它和日志的 28 是两回事，不许共用一枚常量");
    }

    /// 提交信息列开头那一串 ref 药丸（§4.5：浅底 + 同色描边 + 深色字、圆角 999、11/600；
    /// 超前/落后那枚是深藏青 `#0550AE` 底白字、**贴死**在前一枚右侧）。
    /// 药丸高 15 / 圆角取半高 7 是 spec 没给、我们拍的（见 `theme.rs` 的 `PILL_H` 那段与 DESIGN.md §7）。
    /// 判据认行**用提交信息**，不用提交号——§2 的四列里没有提交号那一列。
    #[test]
    fn ref_pills_sit_left_of_the_subject_in_their_row() {
        let out = frame(&mut filled(4, &[]));
        let rows = rows_band(&out);
        let y = text_rect(&out, "主题0", rows.clone()).center().y;
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
        let x0 = col_edges(SIDEBAR_W, WIN_W)[0];
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
        let y2 = text_rect(&out, "主题2", rows).center().y;
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

    /// 虚拟行（工作区）那一条：整行浅灰底、图列里一枚**空心环**、环下面接到第一条提交、
    /// 日期是「今天 HH:MM」。底色是 §3.1 的 `BG_INSET` `#F0F1F3`（§4「虚拟行（未提交）浅灰底整行」）；
    /// 旧的 `#DCDCDC` 实测值随灰阶 token 一起作废。
    /// 参考图这一行**没被选中也是灰的**，所以先把选中挪走——选中那格铺的是 SELECTED，灰底量不到了。
    #[test]
    fn the_uncommitted_row_is_a_grey_band_and_a_hollow_ring() {
        let mut app = loaded();
        // 直接挪字段，不走 `select()`：那会对着假路径 `/fixture` 开线程跑 git，判据要的不是它。
        app.sel = Sel::Commit(0);
        let out = frame(&mut app);
        // 收窄到「整行铺满」的那几条：§3.1 里 `BG_INSET` 一处三色（输入底 / hunk 头 / 虚拟行共用），
        // 不按宽度筛的话 diff 面板那条 hunk 头也会计进来，「只该有一条」就成了假红。
        let grey: Vec<Rect> = rects(&out)
            .into_iter()
            .filter(|(c, r)| {
                *c == BG_INSET
                    && (r.min.x - SIDEBAR_W).abs() < 1.5
                    && (r.max.x - WIN_W).abs() < 1.5
            })
            .map(|(_, r)| r)
            .collect();
        assert_eq!(grey.len(), 1, "整行宽的灰底只该铺虚拟行那一条：{grey:?}");
        // 而且就在日志第一条那一格（紧贴表头下沿）：跑到别的带子里就不是它了。
        let top = *head_band(&out).end();
        assert!(
            (grey[0].min.y - top).abs() < 1.5 && grey[0].min.y < top + ROW_H && grey[0].height() <= ROW_H,
            "灰底不在日志第一条（表头下沿 {top}）：{:?}",
            grey[0]
        );
        // 「整行铺满」已经由上面那个 filter 的两条边负责（改成铺半行，`grey.len()` 就归 0），
        // 这里不再拿同样的两个数重断一遍。
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
        assert_eq!(today[0].1.min.x, col_edges(SIDEBAR_W, WIN_W)[2], "日期列的左缘");
    }

    /// 图元自己的尺寸与颜色钉 §3.3：圆点半径 4.0（「圆点半径 4pt 不变」）、竖线宽 2.0
    /// （「线宽 2pt 不变」）、虚拟行的环外径 10.0 / 环宽 2.5 / `RING` = `#929291`、
    /// 第 0 道 = `LANE_1` `#0969DA`。
    /// 这里**故意写字面量**而不是 `DOT_R`/`LINE_W`：判据要钉的是「spec 给的那个数」，
    /// 拿常量比常量等于没判。旧 React 版那圈是 2.6 / 1.3 加紫色，整体小一圈。
    /// 第 0 道**离边线多远**同样是拍的字面值 12（`LANE0_X`）：反向变异把左边距改成 1 时，
    /// 其余门禁全绿——它们只比 `lane_x(...)`，谁也没问这一道离边线多远。
    #[test]
    fn graph_marks_are_the_design_spec_geometry() {
        let out = frame(&mut loaded());
        let (radii, widths): (Vec<f32>, Vec<f32>) = out
            .shapes
            .iter()
            // 同 `graph_marks`：`LANE_1` 和 `ACCENT` 是同一个蓝，侧栏那颗当前分支点会混进来。
            .filter(|c| c.clip_rect.min.x >= SIDEBAR_W - 0.5)
            .fold(Default::default(), |mut acc, c| {
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
        assert!(radii.iter().all(|r| *r == 4.0), "圆点半径 {radii:?}，§3.3 钉的是 4.0");
        assert!(widths.len() >= 3, "帧里只数到 {} 条线", widths.len());
        assert!(widths.iter().all(|w| *w == 2.0), "线宽 {widths:?}，§3.3 钉的是 2.0");
        let ring = out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Circle(d) if d.fill == Color32::WHITE && d.stroke.color == RING_GREY => {
                Some((d.radius, d.stroke.width))
            }
            _ => None,
        });
        assert_eq!(ring, Some((5.0, 2.5)), "环的外半径/环宽和 §3.3（外直径 10.0、环宽 2.5）不符");
        assert_eq!(lane_color(0), Color32::from_rgb(0x09, 0x69, 0xDA), "第 0 道不是 §3.3 的蓝");
        // §3.3 只有四枚车道色，第五条道只能**回绕**（`lane_color` 里那一句 `% 4`）。
        // 这条不钉的话，把回绕写成 `LANE_COLORS[lane]` 是**越界 panic**、写成 `unwrap_or(0)`
        // 是永远拿第 0 枚——两种改法在四条道以内的夹具里全都绿。
        assert_eq!(
            (lane_color(4), lane_color(5), lane_color(7)),
            (lane_color(0), lane_color(1), lane_color(3)),
            "第五条及以后的道没回绕到那四枚色上"
        );
        // 第 0 道在哪：`LANE0_X 12` 是 S6-4 拍的（§2 的图列只有 48，四道 × 10 才装得下，
        // 见 `theme.rs` 里那段），旧 22.75 是参考图在 57pt 列上量的、随那一列作废。
        let lane0 = SIDEBAR_W + 12.0;
        let (dots, lines, _) = graph_marks(&out);
        assert!(
            dots.iter().all(|p| (p.x - lane0).abs() < 0.01),
            "圆点没落在第 0 道的位置 {lane0}：{dots:?}"
        );
        assert!(
            !dots.is_empty() && lines.iter().all(|[a, b]| (a.x - lane0).abs() < 0.01 && (b.x - lane0).abs() < 0.01),
            "竖线不在第 0 道上：{lines:?}"
        );
        assert!(rings(&out).iter().all(|p| (p.x - lane0).abs() < 0.01), "环不在第 0 道上");
    }

    /// 表头那**三条**列边线：1pt 宽、16pt 高（`RULE_H`）、`BORDER_SUBTLE`，落在 §2 四列的
    /// 边界上 236 / 876 / 972。上下各一条横线把表头那 28pt 从筛选条与日志里切出来。
    /// 这条判据是来还债的：上一批我用**整带**阈值扫表头，判定「参考图没有竖线」，
    /// 于是把并排一比就能看见的边线漏掉了——线只有 16pt 高，按行取平均会被白盖掉。
    /// 名字里的「measured」跟着 §2 换锚一起作废：数现在是 spec 给的，不是参考图量的。
    #[test]
    fn the_header_carries_the_design_spec_column_rules() {
        let out = frame(&mut loaded());
        let head = head_band(&out);
        let (top, bottom) = (*head.start(), *head.end());
        let mid = (top + bottom) / 2.0;
        let mut vertical: Vec<(f32, f32)> = Vec::new();
        let mut horizontal: Vec<f32> = Vec::new();
        for c in &out.shapes {
            if let egui::Shape::LineSegment { points, stroke } = &c.shape {
                if stroke.color != BORDER_SUBTLE || stroke.width != 1.0 {
                    continue;
                }
                let (a, b) = (points[0], points[1]);
                if (a.x - b.x).abs() < 0.01 && a.y >= top && b.y <= bottom {
                    vertical.push((a.x, b.y - a.y));
                } else if (a.y - b.y).abs() < 0.01
                    && a.y >= top - 1.5
                    && a.y <= bottom + 1.5
                    && a.x <= SIDEBAR_W + 1.5
                    && b.x >= WIN_W - 1.5
                {
                    horizontal.push(a.y);
                }
            }
        }
        vertical.sort_by(|a, b| a.0.total_cmp(&b.0));
        // 这里**故意写字面量**：判据钉的是列边线落在哪，拿 `COL_*` 比 `COL_*` 等于没判。
        //
        // **S6-4 换锚**：§2 的四列是 图 48 | 提交信息（弹性）| 作者 96 | 日期 78，
        // 提交号那一整列连同它左边那条边线一起删了，所以只剩**三条**：
        // `188 + 48 = 236`（图 | 提交信息）、`1050 − 78 − 96 = 876`（提交信息 | 作者）、
        // `1050 − 78 = 972`（作者 | 日期）。旧的四条（245/322/739/929）是参考图在
        // 图列 57 / 提交列 77 / 作者 190 / 日期 121 上量的，那一档随 §2 作废。
        assert_eq!(
            vertical,
            vec![(236.0, RULE_H), (876.0, RULE_H), (972.0, RULE_H)],
            "表头的列边线不符（宽 1、高 {RULE_H}、三条在 236/876/972；见上 §2 四列）"
        );
        assert_eq!(horizontal.len(), 2, "表头该有上下两条横线把它切出来：{horizontal:?}");
        // 横线贴着行的**边框**（rect 86..110），而 `head_band` 是从文字中线量的（97.5），
        // 差半个像素是 galley 取整——所以这里量两条线自己张开的高度，不跟带子比。
        horizontal.sort_by(|a, b| a.total_cmp(b));
        assert!(
            (horizontal[1] - horizontal[0] - LOG_HEAD_H).abs() < 1.5
                && ((horizontal[0] + horizontal[1]) / 2.0 - mid).abs() < 1.5,
            "两条横线不是贴着表头那一条的上下边框：{horizontal:?}（带 {top}..{bottom}，中线 {mid}）"
        );
    }

    /// DESIGN.md §3.4 的层级：提交信息走正文 `TEXT_PRIMARY`，提交号 / 作者 / 日期 / 表头走
    /// 次要 `TEXT_SECONDARY`。旧写法是四列同色（(38,38,38)）、「只靠列宽和字重分主次」，
    /// spec 把这条换掉了（原则②「信息靠层级，用字号字重**和灰度**拉开」）。
    /// 虚拟行**不再取更深一档的字色**（§6：`INK_STRONG` → 加粗），所以这里问两件事：
    /// 它是正文色，**并且**它真的换了字面（`inter-semibold`）——只改颜色不改字面，
    /// 强调就等于没落地，而 `text_color` 那条照样绿。
    #[test]
    fn the_log_list_tiers_its_text_by_weight_and_grey() {
        let mut app = loaded();
        app.sel = Sel::Commit(1);
        let out = frame(&mut app);
        let rows = rows_band(&out);
        let primary = Color32::from_rgb(0x1F, 0x23, 0x28);
        let secondary = Color32::from_rgb(0x59, 0x63, 0x6E);
        assert_eq!(text_color(&out, "主题0", rows.clone()), primary, "提交信息不是正文色");
        assert_eq!(text_size(&out, "主题0", rows.clone()), 13.0, "提交信息不是 §3.4 的正文档 13px");
        // 「次要」那一档现在只剩作者与日期：提交号不再画了（§2 四列没有它那一格）。
        for label in ["包融 <", "今天 "] {
            assert_eq!(
                text_color(&out, label, rows.clone()),
                secondary,
                "「{label}」没走 §3.4 的次要档——四列同色是复刻期的规则"
            );
            assert_eq!(text_size(&out, label, rows.clone()), 12.0, "「{label}」不是次要档 12px");
        }
        for label in ["图表", "提交信息", "作者", "日期"] {
            assert_eq!(text_color(&out, label, head_band(&out)), secondary, "表头「{label}」不是次要档");
            assert_eq!(text_size(&out, label, head_band(&out)), 12.0, "表头「{label}」不是次要档 12px");
        }
        let first = *rows.start()..=rows.start() + ROW_H;
        assert_eq!(
            text_color(&out, "Uncommitted", first.clone()),
            primary,
            "虚拟行还给自己更深一档 = §6 那条「INK_STRONG → 加粗」没落地"
        );
        assert_eq!(text_family(&out, "Uncommitted", first), "inter-semibold", "虚拟行的强调没走真字重");
    }

    /// §4.6 的选中态是**所有列表通用**的一条：浅蓝底 `ACCENT_SOFT` + 左缘 2pt `ACCENT` 竖条，
    /// 文字**保持 `TEXT_PRIMARY`**。旧写法（实心蓝 `#1C6CF0` 铺满 + 整行反白）在 §6 迁移表里
    /// 明写作废。这条判据把三件事分开钉：底色、那根条子、以及**选中行不许出现白字**——
    /// 只钉前两样的人会把白字留在原地，而白字压在浅蓝底上是看不见的。
    #[test]
    fn a_selected_log_row_is_a_tinted_band_with_an_accent_bar() {
        let mut app = loaded();
        app.sel = Sel::Commit(0);
        let out = frame(&mut app);
        let rows = rows_band(&out);
        let y = text_rect(&out, "主题0", rows.clone()).center().y;
        let soft = Color32::from_rgb(0xE8, 0xF1, 0xFD);
        let accent = Color32::from_rgb(0x09, 0x69, 0xDA);
        let band = rects(&out)
            .into_iter()
            .filter(|(c, r)| *c == soft && (r.center().y - y).abs() < 1.0 && (r.max.x - WIN_W).abs() < 1.5)
            .map(|(_, r)| r)
            .collect::<Vec<_>>();
        assert_eq!(band.len(), 1, "选中那一行该有一整条浅蓝底：{band:?}");
        let b = &band[0];
        assert!((b.height() - ROW_H).abs() < 0.6, "选中底不是行高：{b:?}");
        assert_eq!(b.min.x, SIDEBAR_W, "浅蓝底没从内容区左缘起");
        let bar = rects(&out)
            .into_iter()
            .filter(|(c, r)| *c == accent && (r.center().y - y).abs() < 1.0 && (r.min.x - SIDEBAR_W).abs() < 0.6)
            .map(|(_, r)| r)
            .collect::<Vec<_>>();
        assert_eq!(bar.len(), 1, "左缘该有一根主色竖条：{bar:?}");
        assert_eq!(bar[0].width(), 2.0, "§4.6 的左缘竖条是 2pt（实测 {}）", bar[0].width());
        assert!((bar[0].height() - ROW_H).abs() < 0.6, "竖条没贯穿整行：{:?}", bar[0]);
        // 第三件：**没有白字**。旧写法整行反白，白字在浅蓝底上等于把这一行抹掉。
        for label in ["主题0", "包融 <", "今天 "] {
            assert_ne!(
                text_color(&out, label, rows.clone()),
                Color32::WHITE,
                "「{label}」选中时还在反白 = §4.6 那条作废没落地"
            );
        }
        assert_eq!(text_color(&out, "主题0", rows.clone()), Color32::from_rgb(0x1F, 0x23, 0x28), "选中行的提交信息该仍是正文色");
        // 选中行加粗（§4.6「可加粗」这一半走真字重，§3.4 的 600 只对拉丁为真）。
        assert_eq!(text_family(&out, "主题0", rows.clone()), "inter-semibold", "选中行没换成真字重");
    }

    /// §2 给 LOG 那一行写的「hover 浅灰」：鼠标停在**没选中**的提交行上铺 `BG_HOVER`。
    /// 选中的那一行不吃 hover（选中色比 hover 深一档，两层底色只会让鼠标位置显得含糊），
    /// 虚拟行也不吃（它常驻一层 `BG_INSET`）。
    /// 无头帧要模拟「鼠标在这儿」得先跑一帧拿到那行的 y，再带着 `PointerMoved` 跑第二帧——
    /// 所以这里用的是 `frame_with_pointer`，不是普通的 `frame`。
    #[test]
    fn a_hovered_log_row_gets_the_grey_band() {
        let hover = Color32::from_rgb(0xEF, 0xF1, 0xF4);
        let mut app = loaded();
        app.sel = Sel::Commit(1);
        let probe = frame(&mut app);
        // 鼠标停在第一条提交那一行的中间（第一条**没被选中**，选中的是第二条）。
        let y = text_rect(&probe, "主题0", rows_band(&probe)).center().y;
        // 两个落点，各钉 `row_hot` 的一半：
        // * **x=388 落在提交信息那块 `Label` 上**（实测文字 galley 占 371.7..522.9，
        //   `put_left` 那个子 `Ui` 的矩形更宽，一直铺到作者列前）。子控件自己带 `Sense`，
        //   会把 `hovered` 抢走 → 整行那份是 false。这条就是「鼠标停在描述上，灰底不画、
        //   只有文字那一小块有反应」的原形，只有 `container_contains_pointer` 认得它。
        // * **x=212 落在图列**（那一列只有 painter 落的点，没有控件）→ 两条都是 true，
        //   它守的是「别把 hover 缩回文字那一条横带」。
        for px in [SIDEBAR_W + 200.0, SIDEBAR_W + 24.0] {
            let out = frame_with_pointer(&mut app, pos2(px, y));
            let b = rects(&out)
                .into_iter()
                .filter(|(c, r)| *c == hover && (r.center().y - y).abs() < 1.0 && (r.max.x - WIN_W).abs() < 1.5)
                .map(|(_, r)| r)
                .collect::<Vec<_>>();
            assert_eq!(b.len(), 1, "鼠标停在 x={px}、y={y} 那一行该有一整条 hover 底：{b:?}");
            assert!((b[0].height() - ROW_H).abs() < 0.6, "hover 底不是行高：{:?}", b[0]);
        }
        let out = frame_with_pointer(&mut app, pos2(SIDEBAR_W + 200.0, y));
        let rows = rows_band(&out);
        // 停在别的行上，这一条就该回落成白底：整列都染一遍等于没判。
        let other = text_rect(&probe, "主题2", rows.clone()).center().y;
        let out2 = frame_with_pointer(&mut app, pos2(SIDEBAR_W + 200.0, other));
        assert!(
            !rects(&out2)
                .into_iter()
                .any(|(c, r)| c == hover && (r.center().y - y).abs() < 1.0),
            "鼠标明明停在第三行，第一条提交还染着 hover"
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

    /// 宽度账本：ASCII 1 格、CJK 2 格、制表符 4 格（egui 不认制表位，保守按四个空格算）。
    /// 差异区能往右滚多远全靠这个数，**估窄了行尾就永远滚不到**。
    #[test]
    fn the_column_ruler_counts_double_wide_glyphs() {
        assert_eq!(columns("abc"), 3);
        assert_eq!(columns("中文字"), 6);
        assert_eq!(columns("\tab"), 6);
        assert_eq!(columns(""), 0);
    }

    /// 在差异区里横向滚一把，返回滚之前/滚停之后那一行的**落点 x**，再加上滚停之后那一帧
    /// 画出的所有文字——「区块按钮跟着内容跑出屏幕没有」只能问最后一帧。
    /// 规矩和 `scroll_log` 一模一样：`ScrollArea` 只吃**指针悬停在那一块**的滚轮，
    /// 一次离散的滚轮事件还要摊到后面几十帧才放得完。
    fn scroll_diff_x(app: &mut Gitx, needle: &str, dx: f32) -> (f32, f32, Vec<(String, Pos2)>) {
        let ctx = egui::Context::default();
        style(&ctx);
        let over_diff = pos2(
            (SIDEBAR_W + FILES_W + WIN_W) / 2.0,
            WIN_H - STATUS_H - STAGING_H + DIFF_HEAD_H + 12.0,
        );
        let hover = vec![egui::Event::PointerMoved(over_diff)];
        let raw = |time: f64, events: Vec<egui::Event>| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let at = |out: &egui::FullOutput| -> f32 {
            out.shapes
                .iter()
                .find_map(|c| match &c.shape {
                    egui::Shape::Text(t) if t.galley.text() == needle => Some(t.pos.x),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("差异区里没画出那一行（要找 {needle:?}）"))
        };
        let mut out = ctx.run_ui(raw(0.0, hover.clone()), |ui| app.shell(ui));
        let before = at(&out);
        out.textures_delta.clear();
        let mut after = before;
        let mut last = Vec::new();
        for i in 0..40 {
            let events = if i == 0 {
                vec![
                    egui::Event::PointerMoved(over_diff),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(-dx, 0.0),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Wheel,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                hover.clone()
            };
            let mut f = ctx.run_ui(raw(0.016 * (i as f64 + 1.0), events), |ui| app.shell(ui));
            after = at(&f);
            last = texts(&f);
            f.textures_delta.clear();
        }
        (before, after, last)
    }

    /// 那一行到底**落了几个字**。`galley.text()` 问不出这件事：它返回原始整句，被切掉的
    /// 尾巴照样留在字符串里（第一版判据拿它比全文，`.truncate()` 变异下去还是绿的）。
    /// 要看 `elided`（宽度不够时 epaint 会置位）和各行真有的 glyph 数。
    /// 返回 `(落点 x, 切过没有, 落笔字数, 排版出来的宽度)`。
    fn laid_out(out: &egui::FullOutput, needle: &str) -> (f32, bool, usize, f32) {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == needle => Some((
                    t.pos.x,
                    t.galley.elided,
                    t.galley.rows.iter().map(|r| r.glyphs.len()).sum(),
                    t.galley.size().x,
                )),
                _ => None,
            })
            .unwrap_or_else(|| panic!("这一帧没画出那一行（要找 {needle:?}）"))
    }

    /// 差异区要能**往右看**：一行比这一格宽的时候，行尾不许只剩三个点——整行必须落笔，
    /// 底色条要跟着最长那行铺满，横向滚一把之后尾巴进得来这一格
    /// （用户 2026-10-23：「右侧，文件改动详情不能往右查看内容（目前只有三个点省略）」）。
    /// 宽度那笔账走**真通路**算：`request()` → 后台线程 → `Msg::Patch`。夹具直接填
    /// `diff_cols` 那种写法测不到这一位，改坏生产代码也不会红。
    #[test]
    fn a_long_diff_line_can_be_scrolled_to_its_end() {
        let (mut app, dir) = open_repo("diff-scroll");
        let tail = "行尾那几个字";
        std::fs::write(dir.join("a.txt"), format!("{}\n", "宽".repeat(300) + tail)).unwrap();
        app.request(Target::Working { staged: false, path: Some("a.txt".to_owned()), untracked: false });
        pump(&mut app, |a| !a.diff.is_empty());
        let want = app
            .diff
            .iter()
            .map(|l| l.text.clone())
            .find(|s| s.ends_with(tail))
            .expect("补丁里没有那行长内容");

        // 1) 整行真的落了笔，没有被收成三个点。
        let out = frame(&mut app);
        let (x, elided, glyphs, w) = laid_out(&out, &want);
        assert!(!elided, "那一行被切掉了尾巴（{glyphs}/{} 个字落笔）", want.chars().count());
        assert_eq!(glyphs, want.chars().count(), "那一行只落笔了 {glyphs} 个字");

        // 2) 增行的底色条要铺到那一行的**排版右缘**——这就是 `diff_cols` 唯一该管的活。
        //    条子短了，滚到右边是半截白。`rects()` 拿的是和 clip 求交后的值，这里必须用
        //    没裁过的原矩形，否则尾巴本来就在格子外，判据永远只能等于视口右缘。
        let mut raw: Vec<(&egui::epaint::RectShape, Rect)> = Vec::new();
        for c in &out.shapes {
            collect_rects(&c.shape, c.clip_rect, &mut raw);
        }
        let band_right = raw
            .iter()
            .filter(|(r, _)| r.fill == ADD_BG)
            .map(|(r, _)| r.rect.max.x)
            .fold(f32::MIN, f32::max);
        assert!(
            band_right >= x + w - 1.0,
            "底色条只铺到 {band_right}，那一行的尾巴在 {}：行宽那笔账没跟着最长行撑开",
            x + w
        );

        // 3) 滚一把：整行的落点往左走那么多，行尾才进得来这一格。
        let dx = 200.0;
        let (before, after, last) = scroll_diff_x(&mut app, &want, dx);
        assert!(
            (after - (before - dx)).abs() < 1.0,
            "横向滚 {dx}pt，那一行从 {before} 只走到 {after}：内容根本没跟着滚（或压根没撑开）"
        );

        // 4) 滚过之后「只动这一块」那两枚还得在**看得见**的那一格裡：它们钉的是视口右缘，
        //    不是内容右缘。钉错了行照样滚得动，按钮却跟着跑到屏幕外、再也点不到——
        //    短补丁测不出来（内容根本没超过视口），必须拿这一行长行来测。
        let pane_left = SIDEBAR_W + FILES_W;
        let p = last
            .iter()
            .find(|(s, _)| s == "暂存区块")
            .map(|(_, p)| *p)
            .unwrap_or_else(|| panic!("滚到右边之后连「暂存区块」都没画出来：{last:?}"));
        assert!(
            p.x > pane_left && p.x < WIN_W - HUNK_BTN_EDGE,
            "区块按钮滚完落在 {}，看得见的格子是 {pane_left}..{WIN_W}：它跟着内容跑出去了",
            p.x
        );
    }

    /// 只 `open()`、一下都没点：日志第一行「Uncommitted changes」是选中态，右边就得是
    /// 工作区的差异。真窗口拍到的是「没有差异」（2026-10-23 16:16），而无头判据全绿——
    /// 它们全都先点一行再看，没人看过只开不开点的壳。这条就是补那个视角。
    #[test]
    fn opening_a_dirty_repo_shows_its_working_diff_without_a_click() {
        let dir = scratch_repo("open-diff");
        // `scratch_repo` 那处改动是**已暂存**的，未暂存差异是空的；再补一层没暂存的改动。
        std::fs::write(dir.join("a.txt"), "一行\n第二行\n这行没暂存\n").unwrap();
        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| !a.diff.is_empty());
        assert!(matches!(app.sel, Sel::Uncommitted), "选中态自己挪走了：{:?}", app.sel);
        let painted = texts(&frame(&mut app));
        assert!(
            !painted.iter().any(|(s, _)| s == "没有差异"),
            "开完仓库右边还写着「没有差异」：{painted:?}"
        );
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
        open_add_card(&mut app);
        assert!(card_has(&mut app, "打开已有"), "卡上那三档没画出来");
        assert!(card_has_button(&mut app, "添加"), "第一档「打开已有」的主按钮不是「添加」");
        assert!(
            type_into_card(&mut app, "已有仓库的路径", sub.to_str().unwrap()),
            "卡里那一格文本框没画出来"
        );
        assert!(click_card_button(&mut app, "添加"), "「添加」那枚没点到");
        pump(&mut app, |a| a.snap.is_some());
        assert_eq!(app.repo.as_deref(), Some(dir.to_str().unwrap()), "填子目录没走到仓库根");
        assert_eq!(app.repos.len(), 1, "开起来的仓库没进书签列表");
        assert!(app.modal.is_none(), "仓库都开起来了，那张卡还盖在上面");
    }

    /// 拒绝的那条路要留得下来改：给提示、不关卡、不清空，更不能把仓库登记进去。
    /// 「不关卡」这一条走的是 `land` 那个返回值 —— 填错一个字母不该从头再打一遍路径。
    #[test]
    fn a_path_that_is_not_a_repo_is_refused_without_touching_the_list() {
        let mut app = blank();
        open_add_card(&mut app);
        let bad = "/definitely/not/a/repo-20261020";
        assert!(type_into_card(&mut app, "已有仓库的路径", bad));
        assert!(click_card_button(&mut app, "添加"));
        assert!(
            app.note.as_deref().unwrap_or_default().contains("不是仓库"),
            "拒了一条还没给提示：{:?}",
            app.note
        );
        assert!(app.repos.is_empty() && app.repo.is_none(), "不是仓库的路径被登记进去并打开了");
        let m = app.modal.as_ref().expect("拒了一条就把卡关了，打错的那串路径跟着没了");
        assert!(
            matches!(m.form, Form::AddRepo { tab: AddTab::Open, .. }),
            "卡留下了，但换成了另一张"
        );
        assert_eq!(m.text.as_deref(), Some(bad), "卡留下了却把输入清空，等于让用户从头打一遍");
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
        open_add_card(&mut app);
        assert!(type_into_card(&mut app, "已有仓库的路径", dir.to_str().unwrap()));
        assert!(click_card_button(&mut app, "添加"));
        pump(&mut app, |a| a.snap.is_some());
        let listed = db::list_repos(app.conn.as_ref().unwrap()).expect("读回来");
        assert_eq!(
            listed.iter().filter(|r| r.path == dir.to_str().unwrap()).count(),
            1,
            "书签只进了内存，没进库：{listed:?}"
        );
    }

    /// 那三档不是装饰：换一档，占位文字、主按钮、卡的高度都得跟着换。
    /// 克隆那一档独有一格「目标目录」，所以它比另两档高一整行；这一条同时钉住
    /// 「高度这笔账真跟着表单走」（`rows()` 里那条 `+Clone`）。
    #[test]
    fn the_add_repo_card_tabs_change_what_the_field_asks_for() {
        let mut app = blank();
        open_add_card(&mut app);
        let open_h = card_now(&app).height();
        assert!(click_card(&mut app, "克隆到本地"), "三档里没有「克隆到本地」");
        assert!(card_has(&mut app, "源仓库地址"), "换到克隆档，第一格还问的是本地路径");
        assert!(card_has(&mut app, "目标目录"), "克隆档没端出第二格");
        assert!(card_has_button(&mut app, "克隆"), "克隆档的主按钮不是「克隆」");
        let clone_h = card_now(&app).height();
        assert_eq!(clone_h - open_h, MODAL_ROW_H, "多那一格没让卡长高：{open_h} → {clone_h}");
        assert!(click_card(&mut app, "新建仓库"), "三档里没有「新建仓库」");
        assert!(card_has(&mut app, "新仓库的目录"), "换到新建档，第一格还问的是源地址");
        assert!(card_has_button(&mut app, "创建"), "新建档的主按钮不是「创建」");
        assert_eq!(card_now(&app).height(), open_h, "新建档该和打开档一样高");
        assert!(!card_has(&mut app, "目标目录"), "换出克隆档了，第二格还赖在卡上");
    }

    /// 「新建仓库」那一档：填一个**连前导目录都还不存在**的路径 → 真 `git init` →
    /// 地上有了 `.git`、书签里多一条、tab 开到它上面。复核问的是磁盘，不是 app 的说法。
    #[test]
    fn the_new_repo_tab_initializes_a_directory_and_opens_it() {
        let root = std::env::temp_dir().join(format!("gitx-init-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let fresh = root.join("brand/new");
        let mut app = blank();
        open_add_card(&mut app);
        assert!(click_card(&mut app, "新建仓库"));
        assert!(type_into_card(&mut app, "新仓库的目录", fresh.to_str().unwrap()));
        assert!(click_card_button(&mut app, "创建"), "「创建」那枚没点到");
        pump(&mut app, |a| a.snap.is_some());
        assert!(fresh.join(".git").exists(), "落地的目录里没有 .git：{fresh:?}");
        assert_eq!(app.repo.as_deref(), Some(fresh.to_str().unwrap()), "新建完没把 tab 开上去");
        assert_eq!(app.repos.len(), 1, "新建的仓库没进书签");
        assert!(app.modal.is_none(), "仓库都建好了，卡还盖着");
        assert!(app.note.is_none(), "路上报了 note：{}", app.note.clone().unwrap_or_default());
        let _ = std::fs::remove_dir_all(&root);
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
        let out = settled_frame(&mut app, 0.4);
        let want = Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H));
        // 遮罩认「铺满整屏 + §5.1 那枚**带色**的半透明」。旧实现是 `from_black_alpha(120)`：
        // 纯黑（r=g=b=0），spec 要的是 `rgba(31,35,40,.48)` —— 0.48×255 四舍五入 = 122。
        // **只在淡完那一格量**：egui 把注册过一遍的 `Area` 整层压暗（`area.rs:706` 那句
        // `multiply_opacity`，实测淡入中 122 出来只剩 29），钉本色数值就必须先让钟走过
        // `animation_time`——`settled_frame` 干的就是这件事。
        // 未预乘那一档由 `to_srgba_unmultiplied` 给；容差 ±1 是**预乘存储**的舍入
        // （35 实测读回 36），不是给颜色留的余量。
        let probe = |out: &egui::FullOutput| -> Vec<[u8; 4]> {
            rects(out)
                .into_iter()
                .filter(|(_, r)| *r == want)
                .map(|(c, _)| c.to_srgba_unmultiplied())
                .collect()
        };
        let on_spec = probe(&out)
            .into_iter()
            .filter(|[r, g, b, a]| {
                *a == 122
                    && [(*r as i16 - 31).abs(), (*g as i16 - 35).abs(), (*b as i16 - 40).abs()]
                        .iter()
                        .all(|d| *d <= 1)
            })
            .count();
        assert!(
            on_spec > 0,
            "遮罩不是 §5.1 的 rgba(31,35,40,.48)：{want:?} 那一格实测 {:?}",
            probe(&out)
        );
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

    /// 用界面存一条贮藏（工具条「贮藏」→ 卡 → 敲说明 → 回车），等它进侧栏。
    /// 三条贮藏判据共用这一段前戏，不在每一条里各抄一遍。
    fn stash_one(tag: &str, message: &str) -> (Gitx, std::path::PathBuf) {
        let (mut app, dir) = open_repo(tag);
        click_tool(&mut app, 6);
        assert!(type_into(&mut app, "贮藏说明", message), "贮藏卡里那格输入框没找到");
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.len()).unwrap_or(0) == 1);
        (app, dir)
    }

    /// 贮藏的来回：卡里存进去 → 侧栏那一栏右键丢出来。顺手钉确认闸对贮藏也生效。
    /// 菜单挂在**说明**那一行上：`stash@{0}:` 这层包装已经从名字里拿掉了。
    #[test]
    fn a_stash_round_trips_through_the_sidebar() {
        let (mut app, dir) = stash_one("card-stash", "先收起来");
        assert!(git_in(&dir, &["stash", "list"]).contains("先收起来"));
        menu_pick(&mut app, "先收起来", "丢弃这条贮藏");
        assert!(app.modal.is_some(), "丢弃贮藏没先问一句");
        assert!(click_warm(&mut app, "确定"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.is_empty()).unwrap_or(false));
        assert_eq!(git_in(&dir, &["stash", "list"]), "");
    }

    /// 连着存两条：新那条顶上来当 `stash@{0}`，早那条退成 `stash@{1}`。
    /// 只有这种情形测得出「下标会挪」——只存一条时列表会清空，`Msg::Loaded` 的越界回退
    /// 抢先把选中改回工作区，那条判据对真正的 bug 是假绿。
    fn stash_two(tag: &str, first: &str, second: &str) -> (Gitx, std::path::PathBuf) {
        let (mut app, dir) = stash_one(tag, first);
        std::fs::write(dir.join("a.txt"), "第二版内容\n").unwrap();
        click_tool(&mut app, 6);
        assert!(type_into(&mut app, "贮藏说明", second), "贮藏卡里那格输入框没找到");
        press(&mut app, egui::Key::Enter);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.len()).unwrap_or(0) == 2);
        (app, dir)
    }

    /// 侧栏那一行**只写用户自己那句说明**：`stash@{0}: ` 是列表的包装、
    /// `On <分支>: ` 是 git 的包装，两截都不该出现在名字里
    /// （用户 2026-10-23：「stash名字去掉前缀'stash@{0}: On'」）。
    #[test]
    fn a_stash_row_is_named_by_its_own_message() {
        let (mut app, _) = stash_one("stash-label", "先看名字");
        let out = frame(&mut app);
        let t = texts(&out);
        assert!(t.iter().any(|(s, _)| s == "先看名字"), "侧栏那一行不是光秃秃的说明：{t:?}");
        let junk: Vec<&String> = t
            .iter()
            .map(|(s, _)| s)
            .filter(|s| s.contains("stash@{") || s.contains("On "))
            .collect();
        assert!(junk.is_empty(), "贮藏名字里还带着包装：{junk:?}");
    }

    /// 「点开看内容」：侧栏那一行一按，左边换出这条贮藏改了哪些文件、右边换出它的补丁。
    /// 全程走**真通路**（真仓库、真 git、后台线程），不是往字段里塞假数据。
    #[test]
    fn clicking_a_stash_row_opens_its_changes() {
        let (mut app, _) = stash_one("stash-open", "点开看内容");
        assert!(click_text(&mut app, "点开看内容"), "侧栏没有那条贮藏");
        pump(&mut app, |a| {
            matches!(a.sel, Sel::Stash(0)) && !a.commit_files.is_empty() && !a.diff.is_empty()
        });
        assert!(
            app.commit_files.iter().any(|f| f.path == "a.txt"),
            "这条贮藏的改动列表里没有 a.txt：{:?}",
            app.commit_files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()
        );
        assert!(app.patch_label.contains("贮藏的全部改动"), "右侧那格的标题不是贮藏：{}", app.patch_label);
        assert!(
            app.diff.iter().any(|l| l.text.contains("第二行")),
            "补丁里没有这条贮藏收起来的那一行：{:?}",
            app.diff.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
        );
        let t = texts(&frame(&mut app));
        assert!(
            t.iter().any(|(s, _)| s.contains("这条贮藏改了 1 个文件")),
            "文件表那格的说明还在按提交说话：{t:?}"
        );
        assert!(!t.iter().any(|(s, _)| s.contains("这个提交改了")), "贮藏那一栏套了提交的文案：{t:?}");
    }

    /// 弹出/丢弃之后那条贮藏没了，`stash@{n}` 的后一位会顶上来。继续按旧下标看
    /// 就会静默换成**另一条**贮藏，所以选中项必须回工作区。
    #[test]
    fn dropping_a_stash_does_not_leave_the_next_one_selected() {
        let (mut app, _) = stash_one("stash-drop-sel", "丢掉看选中");
        assert!(click_text(&mut app, "丢掉看选中"), "侧栏没有那条贮藏");
        pump(&mut app, |a| matches!(a.sel, Sel::Stash(0)));
        menu_pick(&mut app, "丢掉看选中", "丢弃这条贮藏");
        assert!(click_warm(&mut app, "确定"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.is_empty()).unwrap_or(false));
        assert!(matches!(app.sel, Sel::Uncommitted), "丢完之后还停在贮藏那一档：{:?}", app.sel);
    }

    /// 丢掉的**正是当前选中那一条**时，选中必须回工作区：剩下那条会挪进 `stash@{0}`，
    /// 下标没变、内容换了人，差异区于是静默展示另一条贮藏。两条才测得出挪位
    /// （一条时列表清空，回退替我把选中改掉了——那是假绿，见 `stash_two`）。
    #[test]
    fn dropping_the_selected_stash_does_not_shift_another_one_under_it() {
        let (mut app, dir) = stash_two("stash-drop-shift", "头一条", "第二条");
        assert_eq!(app.snap.as_ref().unwrap().stashes[0].message, "第二条");
        assert!(click_text(&mut app, "第二条"), "侧栏没有那条贮藏");
        pump(&mut app, |a| matches!(a.sel, Sel::Stash(0)));
        menu_pick(&mut app, "第二条", "丢弃这条贮藏");
        assert!(click_warm(&mut app, "确定"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.stashes.len()).unwrap_or(9) == 1);
        assert_eq!(git_in(&dir, &["stash", "list"]).lines().count(), 1, "丢的不是「第二条」");
        assert_eq!(app.snap.as_ref().unwrap().stashes[0].message, "头一条");
        assert!(
            matches!(app.sel, Sel::Uncommitted),
            "丢完还停在旧下标：{} 挪到了 stash@{{0}} 顶上，看着像同一条、其实换了人",
            app.snap.as_ref().unwrap().stashes[0].message
        );
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

    /// 「添加仓库」卡 →「克隆到本地」那一档 → 两格都填 → 真 `git clone`：
    /// 书签里多一条、tab 开到它上面、HEAD 与 `origin` 都对得上源。
    /// 这一条按**鼠标那一下**（「克隆」）发动，另一条（`a_clone_needs_both_fields`）按 Enter，
    /// 两条合起来才把「主按钮和 Enter 是同一个出口」钉住。
    #[test]
    fn the_clone_card_makes_a_working_copy() {
        let src = scratch_repo("clone-src");
        let dst = std::env::temp_dir().join(format!("gitx-clone-dst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        let mut app = blank();
        open_add_card(&mut app);
        assert!(click_card(&mut app, "克隆到本地"), "卡上没有「克隆到本地」那一档");
        assert!(type_into_card(&mut app, "源仓库地址", src.to_str().unwrap()), "卡里第一格没找到");
        assert!(type_into_card(&mut app, "目标目录", dst.to_str().unwrap()), "卡里第二格没找到");
        assert!(click_card_button(&mut app, "克隆"), "主按钮不是「克隆」");
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
        // 用 `settled_frame` 而不是 `warm_frame`：这一层是 `Area`（`Order::Foreground`），
        // 默认带 0.2s 淡入，`warm_frame` 的钟停在第一格——底色被 `multiply_opacity` 就地乘成预乘色
        // （实测 `ADD_BG` 的 `#E6F4EA` 在这帧里是 `#3639373C`），颜色判据全废。
        // 文字却不受影响（淡入改的是 `Shape` 上的色，`Galley` 的 section 色原样留着），
        // 所以「只问字色」的判据在 `warm_frame` 下照样绿——这条差点就是这么漏掉的。
        let out = settled_frame(&mut app, 0.5);
        let all: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).collect();
        assert!(all.iter().any(|t| t.starts_with("文件历史 · a.txt")), "面板标题没画出来");
        assert!(all.iter().any(|t| t == "逐行归属"), "页签没画出来");
        assert!(all.iter().any(|t| t == "第二条改 a.txt"), "历史行没画出来");
        // §3.1 那对「底 + 字」在**这个**画字入口也要落地。补丁同一套规则有两处画：
        // 差异面板（`show_diff_pane`）和历史面板右侧（`history_diff_row`）。
        // `added_and_deleted_lines_carry_their_own_text_colour` 只走到第一处——反向变异实测：
        // 把 `history_diff_row` 里 `BG_INSET`/`ADD_FG` 两枚摘掉，全量照样全绿，
        // 等于一个值两条出口只验了一半。
        //
        // 定位靠**文字**，不靠「差异区里第一条某色矩形」：遮罩底下主差异面板照样在画，
        // 那儿既有 `@@ -` 也有增行底色，按底色找第一条会摸到遮罩后面那一格。
        let add = all_text_rects(&out, 0.0..=WIN_H, &["+第三行"]);
        assert_eq!(add.len(), 1, "「+第三行」只该在历史面板右侧出现一次：{add:?}");
        let y = add[0].1.center().y;
        let row = |cy: f32| (cy - DIFF_ROW_H / 2.0 + 1.0)..=(cy + DIFF_ROW_H / 2.0 - 1.0);
        let band_at = |cy: f32, want: Color32| {
            let hits: Vec<(f32, Rect)> = rects(&out)
                .into_iter()
                .filter(|(c, rc)| (rc.center().y - cy).abs() < 3.0 && *c == want)
                .map(|(_, rc)| (rc.center().y, rc))
                .collect();
            hits
        };
        assert_eq!(
            text_color(&out, "+第三行", row(y)),
            Color32::from_rgb(0x1A, 0x7F, 0x37),
            "历史面板那行增行的字色（§3.1 ADD_FG）"
        );
        assert_eq!(
            band_at(y, Color32::from_rgb(0xE6, 0xF4, 0xEA)).len(),
            1,
            "那行增行没铺 §3.1 的增行底色（文字中心 y={y}，附近的增行底色带={:?}）",
            band_at(y, Color32::from_rgb(0xE6, 0xF4, 0xEA))
        );
        // hunk 头在增行上面（这格补丁是「块 1」+ 两行 + 第三行），四行以内取那一条。
        // 注意 `layout` 把补丁头**人话化**了：画出来的不是 `@@ -1,1 +1,3 @@` 而是「块 1： 行 1-1（新 1-3）」，
        // 所以前缀问的是「块 」。
        let above: Vec<f32> = all_text_rects(&out, (y - 4.0 * DIFF_ROW_H)..=(y - 1.0), &["块 "])
            .into_iter()
            .map(|(_, rc)| rc.center().y)
            .collect();
        assert_eq!(above.len(), 1, "增行上面四行里该只有一条 hunk 头：{above:?}");
        assert_eq!(
            text_color(&out, "块 ", row(above[0])),
            Color32::from_rgb(0x8B, 0x94, 0x9E),
            "hunk 头文字走 §3.1 辅助档 TEXT_MUTED"
        );
        assert_eq!(
            band_at(above[0], Color32::from_rgb(0xF0, 0xF1, 0xF3)).len(),
            1,
            "hunk 头没铺 §3.1 的 BG_INSET（hunk 中心 y={}，附近的 BG_INSET 带={:?}）",
            above[0],
            band_at(above[0], Color32::from_rgb(0xF0, 0xF1, 0xF3))
        );
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
    /// 这一条按 Enter 发动（和上面按鼠标那条合起来覆盖同一个出口）。
    #[test]
    fn a_clone_needs_both_fields() {
        let src = scratch_repo("clone-half");
        let dst = std::env::temp_dir().join(format!("gitx-clone-half-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst);
        let mut app = blank();
        open_add_card(&mut app);
        assert!(click_card(&mut app, "克隆到本地"));
        assert!(type_into_card(&mut app, "源仓库地址", src.to_str().unwrap()));
        press(&mut app, egui::Key::Enter);
        frame(&mut app);
        let m = app.modal.as_ref().expect("目标目录还空着就把卡关了");
        assert!(matches!(m.form, Form::AddRepo { tab: AddTab::Clone, .. }), "卡留下了，但换成了另一张");
        assert_eq!(m.text.as_deref(), Some(src.to_str().unwrap()), "Enter 被吞了，第一格的字也跟着没了");
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


    // -------------------------------------------- 弹窗批次：提交卡、设置卡（#47）
    //
    // 这一批的产品说法只有一句：「添加仓库/提交太简陋了，要弹窗，设置都没有」。
    // 所以判据一律从**入口那一枚**起：工具条/tab 条上点一下 → 卡真端出来 → 卡上填/勾 →
    // 复核问的是仓库、库文件和帧里的形状，不问 app 自己怎么说。

    /// 一份**只属于这条判据**的临时库。旧那几条共用 `gitx-db-{pid}`，写偏好键不能再用它：
    /// 几条判据并行，读回来的会是隔壁刚写进去的那份开关。
    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gitx-prefs-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("造临时库目录");
        let _ = std::fs::remove_file(dir.join("gate.db"));
        dir.join("gate.db")
    }

    /// 工具条那枚 ⊕ 端出来的是一张**卡**，不是「把光标丢进内联那一格」：得写明提交到哪个分支、
    /// 已暂存几个文件，得有摘要、三行描述、两枚勾和最右那枚「提交」。
    /// 说明行按**整串**认：内联那一格也报暂存数，但它的说法是另一套字。
    #[test]
    fn the_commit_button_in_the_toolbar_opens_a_commit_card() {
        let (mut app, dir) = open_repo("commit-card-shell");
        let branch = git_in(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
        assert!(click_tool(&mut app, 0), "工具条第一枚（⊕）没点到");
        assert!(app.modal.is_some(), "按 ⊕ 没端出卡");
        let painted: Vec<String> = texts(&warm_frame(&mut app)).into_iter().map(|(s, _)| s).collect();
        let want = format!("提交到 {branch} · 已暂存 1 个文件");
        assert!(painted.iter().any(|s| *s == want), "说明行不是那一句：{painted:?}");
        for label in ["提交摘要", "扩展描述", "修正上一条提交", "提交之后立刻推送"] {
            assert!(card_has(&mut app, label), "卡上少了「{label}」那一格");
        }
        assert!(card_has_button(&mut app, "提交"), "主按钮不是「提交」");
        assert!(!card_has_button(&mut app, "修正并提交"), "没勾修正，主按钮不该是修正那一句");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 端到端：卡里敲字 → 点「提交」→ core 真提交 → 日志多一条。
    /// 复核问的是 `git log -1`，不是快照自己报的数。
    #[test]
    fn a_commit_typed_in_the_card_lands_in_the_repo() {
        let (mut app, dir) = open_repo("commit-card");
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(click_tool(&mut app, 0));
        assert!(type_into_card(&mut app, "提交摘要", "弹窗里补的一条"));
        assert!(click_card_button(&mut app, "提交"), "「提交」那枚没点到");
        assert!(app.modal.is_none(), "动作都发出去了，卡还盖在界面上");
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));
        assert_eq!(git_in(&dir, &["log", "-1", "--format=%s"]), "弹窗里补的一条");
        assert_eq!(git_in(&dir, &["show", "HEAD:a.txt"]).lines().count(), 2, "已暂存那行没进提交");
        assert!(app.note.is_none(), "路上报了 note：{}", app.note.clone().unwrap_or_default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 摘要空着：主按钮按下去一个 git 都不许发，卡留着。
    /// 只填描述、摘要还空着，也算「没信息」——`commit_text` 会把空摘要和描述拼成
    /// 「\n\n描述」，那串非空、git 还真会收，落下来的是一条没有标题的提交，所以关卡看的是**摘要**。
    #[test]
    fn an_empty_message_keeps_the_commit_card_open() {
        let (mut app, dir) = open_repo("commit-card-empty");
        let head = git_in(&dir, &["rev-parse", "HEAD"]);
        assert!(click_tool(&mut app, 0));
        assert!(click_card_button(&mut app, "提交"));
        frame(&mut app);
        assert!(!app.busy, "信息还空着就发了一次 git（busy 拧上了）");
        let m = app.modal.as_ref().expect("摘要空着就把卡关了");
        assert!(matches!(m.form, Form::Commit { .. }), "卡留下了，但换成了另一张");
        assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), head, "空信息也落了一条提交");
        assert!(type_into_card(&mut app, "扩展描述", "只有正文没有标题"));
        assert!(click_card_button(&mut app, "提交"));
        frame(&mut app);
        assert!(app.modal.is_some(), "只填了描述也把卡关了，落的是没有标题的提交");
        assert!(!app.busy, "只填了描述也发了 git");
        assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), head, "没有标题的那条落进仓库了");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 卡里那枚「修正上一条提交」：勾上之后主按钮跟着换名，按下去先问一句（确认卡），
    /// 确定之后**条数不变**、上一条换成新信息、已暂存那行并进同一条。
    #[test]
    fn the_commit_card_amend_goes_through_the_confirm_card() {
        let (mut app, dir) = open_repo("commit-card-amend");
        let before = app.snap.as_ref().unwrap().commits.len();
        let head = git_in(&dir, &["rev-parse", "HEAD"]);
        assert!(click_tool(&mut app, 0));
        assert!(click_card(&mut app, "修正上一条提交"), "那枚勾没点到");
        assert!(card_has_button(&mut app, "修正并提交"), "勾上之后主按钮没换名");
        assert!(type_into_card(&mut app, "提交摘要", "弹窗里改写的"));
        assert!(click_card_button(&mut app, "修正并提交"));
        let m = app.modal.as_ref().expect("amend 没先问一句就落地了");
        assert!(matches!(m.form, Form::Confirm { .. }), "问那一句的不是确认卡");
        assert!(click_card_button(&mut app, "确定"), "确认卡上没有「确定」");
        pump(&mut app, |a| {
            a.snap.as_ref().and_then(|s| s.commits.first()).map(|c| c.subject.clone())
                == Some("弹窗里改写的".to_owned())
        });
        assert_eq!(app.snap.as_ref().unwrap().commits.len(), before, "amend 多出了一条提交");
        assert!(app.modal.is_none(), "落地之后卡还挂着：确认卡把自己又问了一遍");
        assert_ne!(git_in(&dir, &["rev-parse", "HEAD"]), head, "HEAD 还指着原来那条：改写的不是上一条");
        assert!(
            git_in(&dir, &["show", "HEAD:a.txt"]).contains("第二行"),
            "已暂存的内容没并进 amend 后的 HEAD"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「提交之后立刻推送」：一条 job 里先落提交、再推。远端用**本机的一份裸仓库**——
    /// 门禁不许连真远端，而 `git push` 走的正是同一条 `git::push`。
    /// 复核问的是裸仓库那头的 HEAD：推送真到了那边才算这条接通。
    #[test]
    fn ticking_push_after_commit_lands_on_the_remote() {
        let dir = scratch_repo("commit-card-push");
        let bare = std::env::temp_dir().join(format!("gitx-bare-{}.git", std::process::id()));
        let _ = std::fs::remove_dir_all(&bare);
        std::fs::create_dir_all(&bare).expect("造裸仓库目录");
        git_in(&bare, &["init", "-q", "--bare", "."]);
        git_in(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
        let branch = git_in(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
        git_in(&dir, &["push", "-q", "-u", "origin", &branch]);
        assert_eq!(git_in(&bare, &["log", "-1", "--format=%s"]), "起点", "起点那条没推上去");
        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| a.snap.is_some());
        let before = app.snap.as_ref().unwrap().commits.len();
        assert!(click_tool(&mut app, 0));
        assert!(click_card(&mut app, "提交之后立刻推送"), "那枚勾没点到");
        assert!(type_into_card(&mut app, "提交摘要", "带着推送的一条"));
        assert!(click_card_button(&mut app, "提交"));
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(before + 1));
        assert!(app.note.is_none(), "推送路上报了 note：{}", app.note.clone().unwrap_or_default());
        assert_eq!(
            git_in(&bare, &["log", "-1", "--format=%s"]),
            "带着推送的一条",
            "提交落了、推送没到远端"
        );
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 卡脚那条分隔线到**卡自己正文最后一格文字下沿**之间的空白（pt）。
    /// 这条数是目视那一轮（2026-10-24，用户：「提交、推送、设置窗口都好丑」）抓出来的毛病：
    /// 三张卡各自钉死在 SourceTree 那三张 nib 的窗口尺寸上，正文只有三五行，剩下的全是空白。
    /// 判据按「看得见的空白」问，不按常量问——拿常量量常量就是让被测对象自己出尺子。
    ///
    /// **只数卡底那一笔之后的文字**：模态那一层画在所有面板之后，卡自己的文字都在它后面。
    /// 不这么裁会被遮罩底下那层蒙过去——反向变异跑出来的第一条假绿：把推送卡改回 nib 那张
    /// 空窗口的尺寸，卡这一格里落着的状态条、暂存带那些文字比卡自己的正文低，空白读起来还是老样子。
    fn body_void(out: &egui::FullOutput, card: Rect) -> f32 {
        let foot = card.max.y - MODAL_FOOT_H;
        let from = out
            .shapes
            .iter()
            .position(|c| {
                matches!(&c.shape, egui::Shape::Rect(r) if r.rect == card && r.fill != Color32::TRANSPARENT)
            })
            .expect("帧里没有卡底那一笔，正文文字无从定位");
        let lowest = out.shapes[from..]
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    (card.contains(r.center()) && r.max.y <= foot + 0.5).then_some(r.max.y)
                }
                _ => None,
            })
            .fold(0.0, f32::max);
        foot - lowest
    }

    /// 偏好卡的高度**按页里最多那一页算**（四行），不是照 `PreferencesWindow.nib` 那张
    /// 855×441 的窗口摆。老尺寸在这一条下会直接红：正文只有三行，脚前面是一整片空白。
    /// 「换页不跳」这条取舍留给 `SETTINGS_ROWS`，所以空白的一档预算是「一行 + 一道缝」。
    #[test]
    fn the_settings_card_is_as_tall_as_its_rows() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10), "工具条上没有 ⊙ 那枚");
        let out = settled_frame(&mut app, 0.4);
        let (_, card) = card_back(&out).expect("卡底那块圆角矩形没画出来");
        let computed = card_rect(
            app.modal.as_ref().expect("卡没打开"),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H)),
        );
        assert_eq!(card, computed, "帧里那块白底和布局算出来的那一格不一致");
        assert!(
            card.width() < WIN_W && card.max.x <= WIN_W && card.max.y <= WIN_H,
            "卡溢出默认窗口了：{card:?}，窗口 {WIN_W}×{WIN_H}"
        );
        let void = body_void(&out, card);
        assert!(
            void <= MODAL_ROW_H + MODAL_GAP + 1.0,
            "卡脚前面空了 {void} pt：正文只有三行，卡却按一张空窗口的尺寸摆着"
        );
    }

    /// 淡入要**淡得完**。egui 的 `Area` 把整层颜色乘上一个按时间涨到 1 的不透明度，那个时间差
    /// 记在 Context 里、按 Area 的 id 存。所以这条问两头：钟停在 0 那一格还发灰（淡入确实在跑，
    /// 也正因为跑，量尺寸的判据非用 `settled_frame` 不可），拨过 `animation_time`（默认 0.2s）
    /// 之后回到本色。
    /// 会红的样子：Area 的 id 每帧算重一次 → 每帧都是「第一次出现」→ 卡在真窗口里永远半透明。
    #[test]
    fn the_modal_card_fades_in_and_settles_at_full_opacity() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10));
        let (mid, _) = card_back(&warm_frame(&mut app)).expect("淡入中的卡底没画出来");
        assert_ne!(mid, Color32::WHITE, "头一帧就全白：淡入没在跑，那头一个 `settled_frame` 是白写的");
        let mut app2 = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app2, 10));
        let (done, _) = card_back(&settled_frame(&mut app2, 0.4)).expect("淡完那格的卡底没画出来");
        assert_eq!(done, Color32::WHITE, "淡入走不完，卡停在 {done:?}");
    }

    /// 窗口收到地板尺寸时，卡不能再比屏幕大：`card_rect` 那头留了 40pt 收边。
    /// 这条钉的是「量来的尺寸」和「窗口地板」打架时谁让位。
    #[test]
    fn the_settings_card_fits_the_window_floor() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10));
        let m = app.modal.take().expect("卡没打开");
        let floor = Rect::from_min_size(pos2(0.0, 0.0), vec2(MIN_W, MIN_H));
        let card = card_rect(&m, floor);
        app.modal = Some(m);
        assert!(card.width() < MIN_W && card.height() < MIN_H, "地板窗口里卡没收边：{card:?}");
        assert!(floor.contains(card.min) && floor.contains(card.max), "卡片角跑到屏幕外：{card:?}");
    }

    /// 「通用」那一页只有三条**真的接得通**的开关。SourceTree 的偏好窗有八页，这里只摆接得通的
    /// 两页：摆一条点了没反应的开关、或一页空的壳，都比没有更糟。
    #[test]
    fn the_settings_card_lists_the_wired_switches() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10));
        assert!(card_has(&mut app, "通用"), "页签里没有「通用」");
        assert!(card_has(&mut app, "关于"), "页签里没有「关于」");
        for label in ["启动时重新打开上一次的仓库", "日志默认按层级顺序", "描述列显示远程分支那枚药丸"] {
            assert!(card_has(&mut app, label), "通用那一页少了「{label}」");
        }
        assert!(!card_has(&mut app, "账户"), "摆了一条没接的页签进来");
        assert!(!card_has(&mut app, "差异视图"), "摆了一条没接的页签进来");
    }

    /// §3.4「强调走字重，不走更深的灰」也管模态卡标题：14pt + Inter SemiBold + `TEXT_PRIMARY`。
    /// 反向变异实测：把 `src/ui/app.rs` 那句的 `.color(TEXT_PRIMARY)` 换成 `TEXT_SECONDARY`、
    /// 或把 `.family(semibold_family())` 摘掉，全量 155 条全绿——标题这一档以前只有代码、没有判据。
    /// 颜色与字面**各问一次**：只动其一也要红。
    #[test]
    fn the_modal_title_is_semibold_primary_ink() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10), "设置那一枚按钮没点到");
        let out = warm_frame(&mut app);
        // 带子按卡底那一格取，不整帧按前缀找：卡标题和别处同名文字（以后溢出菜单里的
        // 「设置」那一档）得分开。顶栏那 11 枚现在**只有图标**（§4.1），所以这一帧里
        // 「设置」只出现在卡上——但判据不该靠这个巧合活着。
        let card = card_back(&out).expect("模态卡底没画出来").1;
        let band = card.min.y..=(card.min.y + MODAL_TITLE_H);
        assert_eq!(
            text_color(&out, "设置", band.clone()),
            Color32::from_rgb(0x1F, 0x23, 0x28),
            "模态卡标题的字色（§3.1 TEXT_PRIMARY）"
        );
        assert_eq!(text_family(&out, "设置", band), "inter-semibold", "模态卡标题没走真字重（§3.4）");
    }

    /// §5.1 把弹窗宽度收成**三档**并且逐张点名：确认卡 380；提交 / 拉取 / 推送 / 偏好 / 添加仓库
    /// 走 560；其余（新建分支、贮藏、新建标签、重命名、合并、重置）走 440。
    /// 换锚前这里是 `MODAL_W = 420` 通吃，外加推送 600、拉取 520、偏好 520 三张各自为政——
    /// 五个数字里**没有一个**在 spec 里。
    /// 两头各问一次：常量字面钉「档位怎么定义的」，**帧里那块白底的宽度**钉「`card_w` 真接上了」。
    /// 反过来只问白底也能过，但档位一改（比如 560 被写成别的）白底那条跟着绿、没人报错，
    /// 所以字面这条不是冗余。一律 `settled_frame`：淡入中的白底被乘成灰，`card_back`
    /// 按面积取最大就会被别处抢走（见那儿那段实测）。
    #[test]
    fn the_modal_cards_are_the_three_spec_widths() {
        assert_eq!(
            (CARD_W_CONFIRM, CARD_W_STD, CARD_W_WIDE),
            (380.0, 440.0, 560.0),
            "§5.1 那三档宽度本身"
        );

        /// 开一张卡，量**帧里画出来**的卡底宽度。`add = false` 走顶栏那枚「＋ 添加仓库」，
        /// 它在 `TOOLS` 表之外（§2 把它并进了顶栏那 46pt）。
        fn drawn(add: bool, i: usize) -> f32 {
            let mut app = with_remotes();
            if add {
                open_add_card(&mut app);
            } else {
                click_tool(&mut app, i);
            }
            assert!(app.modal.is_some(), "那张卡根本没开出来");
            let (_, card) = card_back(&settled_frame(&mut app, 0.4)).expect("卡底那块圆角矩形没画出来");
            card.width()
        }
        for (name, add, i, want) in [
            ("提交卡", false, 0, 560.0),
            ("拉取卡", false, 1, 560.0),
            ("推送卡", false, 2, 560.0),
            ("新建分支卡", false, 4, 440.0),
            ("贮藏卡", false, 6, 440.0),
            ("偏好卡", false, 10, 560.0),
            ("添加仓库卡", true, 0, 560.0),
        ] {
            let got = drawn(add, i);
            assert!((got - want).abs() < 0.01, "「{name}」画出来 {got}，不是 §5.1 点名的那一档 {want}");
        }
        // 确认卡那条走不了 `click_tool`：它挂在 amend 之后，要一本真仓库。
        // 「按下去确实先问这一句」由 `the_commit_card_amend_goes_through_the_confirm_card` 守着，
        // 这里只把一张确认卡摆上去量它的宽。
        let mut app = with_remotes();
        app.modal = Some(Modal::confirm(
            "确定要重置吗",
            "这一步会丢掉未提交的改动".to_owned(),
            Action::Stage(vec!["a.txt".to_owned()]),
        ));
        let (_, card) = card_back(&settled_frame(&mut app, 0.4)).expect("确认卡的卡底没画出来");
        assert!(
            (card.width() - 380.0).abs() < 0.01,
            "确认卡画出来 {}，不是 §5.1 最窄那一档 380",
            card.width()
        );
    }

    /// §5.1 那张卡的四件事：圆角 12、白底、1px `BORDER_SUBTLE`、**真阴影**
    /// （offset 0/8、blur 24、黑 40%）。换锚前画的是「遮罩 + 两圈越来越淡的空心描边**冒充**投影」，
    /// 所以这里第二条判据专门让它死：卡的外缘**不该再有比卡更大的空心圆角矩形**。
    /// 阴影问的是帧里那条 `blur_width != 0` 的 `RectShape`（`Shadow::as_shape` 就产出这个，
    /// epaint-0.37 shadow.rs:48）；尺寸、模糊、颜色三条全是 §5.1 的字面数，不引用 `CARD_SHADOW`
    /// ——引用常量等于让被测对象自己出尺子。
    /// 「模糊那一笔不算表面」这件事在 `rounded()` / `rects()` 里筛掉了：以前没筛，
    /// 投影和卡底面积相同，`card_back` 取最大全靠图层顺序运气。
    #[test]
    fn the_card_is_a_round_twelve_surface_with_a_real_shadow() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10), "偏好卡没开出来");
        let out = settled_frame(&mut app, 0.4);
        let (fill, card) = card_back(&out).expect("卡底那块圆角矩形没画出来");
        assert_eq!(fill, Color32::WHITE, "§5.1 卡底是白的");
        let radius = rounded(&out)
            .into_iter()
            .find(|(_, r, _)| *r == card)
            .map(|(_, _, c)| c)
            .expect("卡底不在圆角矩形那一堆里");
        assert_eq!(radius, egui::CornerRadius::same(12), "卡片圆角不是 §3.5/§5.1 那 12");

        let shadows: Vec<&egui::epaint::RectShape> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Rect(r) if r.blur_width != 0.0 => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(shadows.len(), 1, "整帧只该有投影那一笔模糊矩形：{shadows:?}");
        let sh = shadows[0];
        assert_eq!(sh.rect, card.translate(vec2(0.0, 8.0)), "投影的落点不是「卡下移 8pt」（§5.1）");
        assert_eq!(sh.blur_width, 24.0, "投影的模糊不是 §5.1 那 24");
        assert_eq!(sh.fill, Color32::from_black_alpha(102), "投影不是黑 40%（255×0.4≈102）");
        assert_eq!(sh.corner_radius, egui::CornerRadius::same(12), "投影没收成 12 的圆角");

        for (color, r) in stroked(&out) {
            assert!(
                r.width() <= card.width() + 0.5 && r.height() <= card.height() + 0.5,
                "卡外面套了一圈空心圆角矩形（{r:?} vs {card:?}，描边 {color:?}）——那是假的投影"
            );
        }
    }

    /// §5.2 那句「标题 / 正文 / 按钮区之间**只靠间距分组，不画分隔线**」点名废掉两条线：
    /// 标题带下面那条下划线、卡脚上面那条上划线（两条换锚前都在）。
    /// 阈值取 `card.width() - 1.0` 而不是「任何横线」：卡**里面**的线是表自己的线——
    /// 推送那张表的框线横贯表宽 528（= 560 − 两道 16 内边距），它该活得好好的。
    /// 所以这条同时是校准：跑一遍推送卡，若把阈值降到 520 以下它就该红。
    #[test]
    fn the_card_groups_by_space_alone() {
        let no_full_width_line = |name: &str, out: &egui::FullOutput| {
            let card = card_back(out).expect("卡底没画出来").1;
            let full = card.width() - 1.0;
            for c in &out.shapes {
                if let egui::Shape::LineSegment { points, stroke } = &c.shape {
                    let (a, b) = (points[0], points[1]);
                    let spans = (b.x - a.x).abs() >= full;
                    let horizontal = (a.y - b.y).abs() < 0.01;
                    let inside = card.contains(a) && card.contains(b);
                    assert!(
                        !(horizontal && inside && spans),
                        "{name} 里那条横跨整张卡的分隔线没删掉（§5.2）：{a:?}→{b:?}，笔色 {:?}，卡 {card:?}",
                        stroke.color
                    );
                }
            }
        };
        for (name, i) in [("偏好卡", 10usize), ("提交卡", 0), ("新建分支卡", 4)] {
            let mut app = filled(3, &layout(PATCH, false));
            assert!(click_tool(&mut app, i), "「{name}」没开出来");
            no_full_width_line(name, &settled_frame(&mut app, 0.4));
        }
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 2), "推送卡没开出来");
        no_full_width_line("推送卡", &settled_frame(&mut app, 0.4));
    }

    /// §5.2 卡片右上那枚 ×（幽灵图标按钮：不画底，hover 才出底）。
    /// 它是偏好卡**唯一**的退路——那张卡故意没有「取消」（开关当场就落，说「取消」是谎报），
    /// 所以这里按它一下必须**只关卡、不落地**。
    /// 字形先对着字体表查一次：卡打开着的那些帧不在 `every_rune_the_shell_paints_has_a_glyph`
    /// 的取景范围里（那条跑常态壳），不查就是没人管——`ARROW` 那个 `▾` 就是这么死的。
    /// 查的是 **`CLOSE_GLYPH` 自己第一个字**，不是判据里抄的那个 `×`：反向变异把常量换成
    /// `✕`（U+2715，实测这份字体链里**没有**）时，抄字面的那一版照样绿——这就是
    /// 「判据钉自己的字面、不钉被测那枚」那一类假绿（PROCESS 反向变异表 M11）。
    #[test]
    fn the_card_closes_from_the_x_in_its_title() {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10), "偏好卡没开出来");
        // 字体表要到**第一遍 `run()` 之后**才装得上（context.rs:1387 那句 panic 就是这儿来的）；
        // 而字形要等**画过它的那一笔**跑过才进得了那本字体图集，所以这一帧必须开着卡。
        let mut probe = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let glyph = CLOSE_GLYPH.chars().next().expect("关闭那一枚的字面是空的");
        let has = ctx.fonts_mut(|f| f.has_glyph(&FontId::proportional(14.0), glyph));
        probe.textures_delta.clear();
        assert!(
            has,
            "字体链里没有「{CLOSE_GLYPH}」的字形 U+{:04X}",
            glyph as u32
        );
        assert!(
            !card_has_button(&mut app, "取消"),
            "偏好卡摆了一枚「取消」：开关是当场落的，那两个字谎报了能回退的部分"
        );
        assert!(card_has(&mut app, CLOSE_GLYPH), "标题带右端那枚 × 没画出来");
        let before = app.prefs();
        assert!(click_card(&mut app, CLOSE_GLYPH), "× 点不到");
        assert!(app.modal.is_none(), "按 × 卡没关");
        assert_eq!(app.prefs(), before, "按 × 顺带把某枚开关拨了：它应该只关卡");
        assert!(!app.busy, "按 × 起了一条动作");
    }

    /// §5.2 + §4.1 + §4.2 那三个数字**画出来**的样子：卡脚那条 52pt 带子、带子里两枚 28pt
    /// 高、圆角 6 的按钮（右对齐、间距 8），和正文里那格 30pt 高、圆角 6 的输入框。
    /// 一张新建分支卡同时有主输入格和两枚按钮，所以三档在一帧里问完。
    /// 顺带把**提交卡**的卡脚缝量一遍：那一格描述框高 = `MODAL_ROW_H × DESC_ROWS`（= 3 行），
    /// 加两枚勾、再加说明和主输入各一行 = 6 行。行距从 28 改成 32 时**只有比例没变**，
    /// `rows()` 记的那本账要是少算一行，卡就短一截、末行被卡脚压住；多算一行就空一截。
    /// 408 = 440 − 16 × 2（§5.1 标准档 + §5.2 内边距），不是抄 `card_rect` 的算式。
    #[test]
    fn the_card_footer_is_52pt_with_28pt_buttons() {
        assert_eq!(MODAL_FOOT_H, 52.0, "§5.2 底部按钮区高 52");
        assert_eq!((BTN_H, BTN_R), (28.0, 6.0), "§4.1 主按钮高 28、圆角 6");
        assert_eq!((FIELD_H, FIELD_R), (30.0, 6.0), "§4.2 输入框高 30、圆角 6");

        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 4), "新建分支卡没开出来");
        let out = settled_frame(&mut app, 0.4);
        let (_, card) = card_back(&out).expect("卡底没画出来");
        let (fill, _, btn) = foot_button(&out, card, "创建并切换").expect("卡脚上没有主按钮");
        assert!((btn.height() - 28.0).abs() < 0.01, "主按钮画出来不是 28pt 高（§4.1）");
        assert!(
            (btn.center().y - (card.max.y - 26.0)).abs() < 0.01,
            "主按钮不住在卡脚那条 52pt 带子的正中（§5.2：26 = 52 / 2）"
        );
        assert!(
            (btn.max.x - (card.max.x - 16.0)).abs() < 0.01,
            "主按钮没右对齐到 16pt 内边距（§5.2）"
        );
        let primary_r = rounded(&out)
            .into_iter()
            .find(|(_, r, _)| *r == btn)
            .map(|(_, _, c)| c)
            .expect("主按钮那格不在圆角矩形那一堆里");
        assert_eq!(primary_r, egui::CornerRadius::same(6), "主按钮圆角不是 6（§4.1）");
        assert_eq!(fill, Color32::from_rgb(0x09, 0x69, 0xDA), "主按钮不是实心蓝（§4.1）");
        let (_, _, cancel) = foot_button(&out, card, "取消").expect("卡脚上没有「取消」");
        assert!(
            ((btn.min.x - cancel.max.x).abs() - 8.0).abs() < 0.01,
            "两枚按钮之间的缝不是 §5.2 那 8pt：{}",
            btn.min.x - cancel.max.x
        );

        let fields = rounded(&out)
            .into_iter()
            .filter(|(_, r, _)| (r.height() - 30.0).abs() < 0.01 && r.width() > 100.0)
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 1, "卡里那一格 30pt 高的输入框画不出唯一一块：{fields:?}");
        let (fill, rect, radius) = &fields[0];
        assert_eq!(*fill, Color32::WHITE, "输入框底色不是白的（§4.2）");
        assert_eq!(*radius, egui::CornerRadius::same(6), "输入框圆角不是 6（§4.2）");
        assert!(
            (rect.width() - 408.0).abs() < 0.01,
            "输入框没铺满 440 档卡去掉两道 16 内边距的那一格：{}",
            rect.width()
        );

        // 提交卡那一摞最高的正文（说明 + 摘要 + 三行描述 + 两枚勾）也量一道卡脚缝。
        let mut app2 = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app2, 0), "提交卡没开出来");
        let out2 = settled_frame(&mut app2, 0.4);
        let card2 = card_back(&out2).expect("提交卡底没画出来").1;
        let void = body_void(&out2, card2);
        assert!(
            void <= MODAL_GAP + 1.0,
            "提交卡卡脚前面空了 {void} pt：`Modal::rows` 那本账多算了。这道缝实测本来 9pt（末行下沿到\
             卡脚带子之间那道 `MODAL_GAP`），阈值因此取 13 而不是三条兄弟判据用的 `行距 + 缝 + 1 = 45`：\
             `rows()` 里凭空多算一行（+32）在 45 那一档照样绿，是反向变异实测出来的。\
             少算一行不在这儿拦：那一格描述框会被卡脚裁掉，本批没目视（见 PROCESS 同日「仍然没看到过的」）"
        );
    }

    /// §4.4 那排页签：容器 `BG_INSET`、圆角 **8**、高 **30**。
    /// 换锚前这里是裸色 `SEG_BG`（#f1f2f5，不在 §3.1 那张表里）配圆角 7、再被
    /// `shrink(1.0)` 收成 24——三处都没对着 spec。高 30 这一档现在由 `SEG_H` 传给三张卡。
    /// 用**添加仓库**那张问：它是 §4.4 点名的两处之一（另一处是偏好页签，同一枚 `seg_tabs`），
    /// 而且它的容器宽是 560 档的 528，和顶栏那排（内容宽 172）不会混。
    #[test]
    fn the_segmented_control_is_the_spec_inset_box() {
        assert_eq!(BG_INSET, Color32::from_rgb(0xF0, 0xF1, 0xF3), "§3.1 BG_INSET 那枚灰");
        let mut app = with_remotes();
        open_add_card(&mut app);
        let out = settled_frame(&mut app, 0.4);
        let card = card_back(&out).expect("卡底没画出来").1;
        let boxes = rounded(&out)
            .into_iter()
            .filter(|(_, r, _)| r.width() > 300.0 && r.height() >= 20.0 && r.height() <= 32.0)
            .filter(|(c, _, _)| *c == Color32::from_rgb(0xF0, 0xF1, 0xF3))
            .collect::<Vec<_>>();
        assert_eq!(boxes.len(), 1, "卡里那一排页签的灰槽画不出唯一一块：{boxes:?}");
        let (_, rect, radius) = &boxes[0];
        assert_eq!(*radius, egui::CornerRadius::same(8), "分段选择容器圆角不是 8（§4.4）");
        assert!((rect.height() - 30.0).abs() < 0.01, "分段选择容器不是 §4.4 那 30 高：{}", rect.height());
        assert!(
            (rect.width() - 528.0).abs() < 0.01,
            "容器没铺满 560 档去掉两道 16 内边距的那一格：{}",
            rect.width()
        );
        assert!(card.contains(rect.min) && card.contains(rect.max), "容器跑到卡外面了：{rect:?}");
    }

    /// 「关于」那一页念两条**事实**：书签里几本仓库、库文件在哪儿。数字按整串认，
    /// 少写一本多写本都会红；地址只认它真是个路径（尾巴上那个文件名）。
    #[test]
    fn the_about_page_states_two_facts() {
        let mut app = blank();
        assert!(click_tool(&mut app, 10));
        assert!(click_card(&mut app, "关于"), "页签里没有「关于」");
        let painted: Vec<String> = texts(&warm_frame(&mut app)).into_iter().map(|(s, _)| s).collect();
        // 一本书签都没有的壳，卡上就得写「0 本」——写成别的数（或者干脆漏掉这一行）都算这条红。
        assert!(painted.iter().any(|s| s == "书签里的仓库：0 本"), "书签数不是那一句：{painted:?}");
        assert!(
            painted.iter().any(|s| s.starts_with('/') && s.ends_with("gitx.db")),
            "关于那一页没念出库文件的路径：{painted:?}"
        );
    }

    /// 关掉「描述列显示远程分支那枚药丸」：这一帧里 `⤴origin/main` 就该没了（屏上真变了），
    /// 库里写的是字面 `"false"`（不是内存里改一改），而 `Prefs::load` 读回来要和屏上那份
    /// 一模一样（下次启动读得到同一个开关）。三段各管一种改坏。
    #[test]
    fn the_pill_switch_takes_the_pills_off_and_persists() {
        let db_file = temp_db("pill");
        let mut app = filled(3, &layout(PATCH, false));
        app.conn = Some(db::open(db_file.to_str().unwrap()).expect("临时库"));
        assert!(app.show_remote, "夹具默认就该是开着药丸的");
        assert!(click_tool(&mut app, 10));
        assert!(click_card(&mut app, "描述列显示远程分支那枚药丸"), "那枚开关没点到");
        frame(&mut app);
        let out = frame(&mut app);
        assert!(
            !texts(&out).iter().any(|(s, _)| s.contains("⤴origin/main")),
            "关掉之后药丸还画在描述列上"
        );
        assert!(!app.show_remote, "屏上没了但开关没拧");
        assert_eq!(
            db::get_setting(app.conn.as_ref().unwrap(), Prefs::KEY_REMOTE)
                .ok()
                .flatten()
                .as_deref(),
            Some("false"),
            "开关没落库"
        );
        assert_eq!(Prefs::load(app.conn.as_ref()), app.prefs(), "读回来的和屏上的不是同一份");
        let _ = std::fs::remove_file(&db_file);
    }

    /// 「日志默认按层级顺序（而不是日期）」这条改的是**同一个** `topo`：屏上重排、筛选条那枚
    /// 按钮跟着换文案、库里落 `"true"`。它和弹出菜单那条路共用一份状态，不是第二套开关。
    #[test]
    fn the_topo_switch_resorts_and_persists() {
        let db_file = temp_db("topo");
        let (mut app, dir) = open_repo("topo-switch");
        app.conn = Some(db::open(db_file.to_str().unwrap()).expect("临时库"));
        assert!(!app.topo, "夹具默认该是日期序");
        assert!(filter_text(&mut app, DATE_ORDER).is_some(), "按钮上写的不是默认那一档");
        assert!(click_tool(&mut app, 10));
        assert!(click_card(&mut app, "日志默认按层级顺序"), "那枚开关没点到");
        assert!(app.topo, "点了开关没换排序");
        pump(&mut app, |a| a.snap.is_some());
        assert!(filter_text(&mut app, TOPO_ORDER).is_some(), "换了排序，按钮上还是旧那一项");
        assert_eq!(
            db::get_setting(app.conn.as_ref().unwrap(), Prefs::KEY_TOPO).ok().flatten().as_deref(),
            Some("true"),
            "开关没落库"
        );
        assert_eq!(Prefs::load(app.conn.as_ref()), app.prefs(), "读回来的和屏上的不是同一份");
        let _ = std::fs::remove_file(&db_file);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「启动时重新打开上一次的仓库」这条改的是**下一次**启动读不读那格 `gitx_last_repo`：
    /// 屏上这一帧看不见变化，所以判据问到两头 —— 拧完 `app.restore_last` 是新的，
    /// `Prefs::load` 读回来也是新的（`Gitx::new` 走的就是这一个函数）。
    #[test]
    fn the_restore_last_switch_is_read_back_at_startup() {
        let db_file = temp_db("restore");
        let mut app = filled(3, &layout(PATCH, false));
        app.conn = Some(db::open(db_file.to_str().unwrap()).expect("临时库"));
        assert!(app.restore_last, "夹具默认该是开着「重新打开上一次」的");
        assert!(click_tool(&mut app, 10));
        assert!(click_card(&mut app, "启动时重新打开上一次的仓库"), "那枚开关没点到");
        assert!(!app.restore_last, "点了开关没拧");
        assert_eq!(Prefs::load(app.conn.as_ref()).restore_last, false, "下次启动读回来还是开的");
        assert_eq!(Prefs::load(app.conn.as_ref()), app.prefs(), "三份开关没一起落库");
        let _ = std::fs::remove_file(&db_file);
    }

    /// 「完成」和 Enter 都只干一件事：**关卡**，不发任何动作（偏好卡的开关当场就落了，
    /// 没有「主动作」这回事）。两条各测一次，因为这条通路真的有两个入口。
    #[test]
    fn the_settings_card_closes_without_firing_anything() {
        let mut app = filled(3, &layout(PATCH, false));
        assert!(click_tool(&mut app, 10));
        assert!(card_has_button(&mut app, "完成"), "偏好卡的主按钮不是「完成」");
        assert!(!card_has_button(&mut app, "取消"), "开关当场就落了，不该谎称能取消");
        assert!(click_card_button(&mut app, "完成"));
        assert!(app.modal.is_none(), "按「完成」没关卡");
        assert!(!app.busy, "「完成」发了一条 git 动作");
        assert!(app.note.is_none(), "「完成」报了 note：{:?}", app.note);

        assert!(click_tool(&mut app, 10));
        press(&mut app, egui::Key::Enter);
        assert!(app.modal.is_none(), "Enter 没关卡");
        assert!(!app.busy, "Enter 发了一条 git 动作");
    }

    // ------------------------------------------------------- 推送卡 / 拉取卡（#38 #43）
    //
    // 这两张卡上每一格的坐标都是从本机 nib 实例化后读 frame 量出来的（常量在 theme.rs，
    // 方法见 docs/PROCESS.md）。所以下面量尺寸的几条钉的是**字面数字**，不引用那两个常量——
    // 引用常量等于让被测对象自己出尺子（反向变异表里记了这一条）。

    /// 推送/拉取两张卡要的夹具：两条本地分支（`main` 有上游、`feature/x` 没有）、
    /// 两条远端（`origin` 上两个跟踪指针、`fork` 上一个只在它那儿存在的分支）。
    /// `filled` 那一份没有远端，两张卡根本开不出来。
    fn with_remotes() -> Gitx {
        let mut app = filled(3, &layout(PATCH, false));
        let (oid0, oid1) = {
            let s = app.snap.as_ref().unwrap();
            (s.commits[0].id.clone(), s.commits[1].id.clone())
        };
        let s = app.snap.as_mut().unwrap();
        s.remotes = vec![
            Remote {
                name: "origin".to_owned(),
                url: "/tmp/gate-upstream.git".to_owned(),
                push_url: None,
            },
            Remote {
                name: "fork".to_owned(),
                url: "/tmp/gate-fork.git".to_owned(),
                push_url: Some("/tmp/gate-fork-push.git".to_owned()),
            },
        ];
        s.branches.push(Branch {
            name: "feature/x".to_owned(),
            is_current: false,
            upstream: None,
            ahead: 0,
            behind: 1,
            target: oid1.clone(),
        });
        // 这两条专为一件事：让五档「推送行为」**两两不同**。只有 `main`（上游同名）
        // 和 `feature/x`（没上游）的话，上流/简单/当前三档算出来的勾选一模一样，
        // 判据就杀不死「把简单写成 upstream.is_some()」这一类变异。
        s.branches.push(Branch {
            name: "tricky".to_owned(),
            is_current: false,
            upstream: Some("origin/other".to_owned()),
            ahead: 0,
            behind: 0,
            target: oid1.clone(),
        });
        s.branches.push(Branch {
            name: "docs".to_owned(),
            is_current: false,
            upstream: Some("origin/docs".to_owned()),
            ahead: 0,
            behind: 0,
            target: oid1.clone(),
        });
        s.remote_refs.extend([
            RemoteRef { name: "origin/feature/x".to_owned(), target: oid1 },
            RemoteRef { name: "fork/only-on-fork".to_owned(), target: oid0 },
        ]);
        app
    }

    /// 卡上那张表现在的样子：`(本地分支, 勾了没, 跟踪勾了没, 远端那一侧叫什么)`。
    /// 读的是**卡自己那份数据**（不是 `PushBehavior::picks`）——判据里再调一次 `picks`
    /// 就成了用被测对象量被测对象。
    fn push_rows(app: &Gitx) -> Vec<(String, bool, bool, String)> {
        let m = app.modal.as_ref().expect("判据要在卡上读表，可这会儿没有打开的卡");
        match &m.form {
            Form::Push { rows, .. } => rows
                .iter()
                .map(|r| (r.local.clone(), r.picked, r.track, r.remote_name.clone()))
                .collect(),
            _ => panic!("打开的不是推送卡"),
        }
    }

    /// 推送卡表格第 `i` 行**最左那一格方框**的落点。坐标按 `push_card` 那一套常量算，
    /// 用途只有一个：端到端那条判据要真的用鼠标点掉某一行，而不是直接改字段。
    fn push_row_box(card: Rect, i: usize) -> Pos2 {
        let top = pos2(card.min.x, card.min.y + MODAL_TITLE_H);
        let x = top.x + PUSH_TABLE_X + PUSH_COL_PICK / 2.0;
        let y = top.y + PUSH_TABLE_TOP + PUSH_HEAD_H + PUSH_ROW_H * i as f32 + PUSH_ROW_H / 2.0;
        pos2(x, y)
    }

    /// 在卡上某个坐标按一次（按下+松手）。和 `click_card` 同一条通路，只是这一格没有文字可找。
    fn click_card_at(app: &mut Gitx, at: Pos2) {
        let ctx = warm(app);
        click_at(&ctx, app, at);
    }

    /// 这一帧画出来的、整串等于 `word` 的文字单元有几个。
    fn word_count(out: &egui::FullOutput, word: &str) -> usize {
        out.shapes
            .iter()
            .filter(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == word))
            .count()
    }

    /// 卡上那枚弹出按钮点开之后，菜单里**有没有** `item` 这一项。
    /// 只能问增量：分支名在侧栏和日志行上也画（「origin 那一列里有没有 feature/x」光看
    /// 「这一帧有没有这个字」永远是真，白测）。所以数两次——没点开时几个、点开之后几个，
    /// 变多了才算菜单里有。
    fn menu_shows(app: &mut Gitx, trigger: &str, item: &str) -> bool {
        let before = warm_frame(app);
        let n0 = word_count(&before, item);
        let ctx = warm(app);
        let Some(at) = locate_card(&ctx, app, trigger, false, false) else {
            panic!("卡上没有那枚弹出按钮：{trigger}");
        };
        click_at(&ctx, app, at);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let n1 = word_count(&out, item);
        out.textures_delta.clear();
        n1 > n0
    }

    /// 点开弹出按钮再点菜单里那一项（真鼠标两下，不是直接改表单字段）。
    /// 菜单画在最上层，同名的那一项在 shapes 里排在最后，所以从**后**往前找。
    fn click_menu_item(app: &mut Gitx, trigger: &str, item: &str) {
        let ctx = warm(app);
        let at = locate_card(&ctx, app, trigger, false, false)
            .unwrap_or_else(|| panic!("卡上没有那枚弹出按钮：{trigger}"));
        click_at(&ctx, app, at);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let spot = out
            .shapes
            .iter()
            .rev()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == item => {
                    Some(t.galley.rect.translate(t.pos.to_vec2()).center())
                }
                _ => None,
            });
        out.textures_delta.clear();
        let Some(spot) = spot else { panic!("菜单里没有那一项：{item}") };
        click_at(&ctx, app, spot);
    }

    /// 推送卡的高**跟着分支那几行走**，不是照 `GitPushSheet.nib` 那张 650×271 的空窗口摆
    /// （那套坐标是给一张空窗口排的，行少的时候底下全是空白——2026-10-24 目视那一轮报的就是它）。
    /// 三段各挡一种回退：
    ///  · 帧里那块白底 == `card_rect` 算出来的那一格 → 挡「画的一方和量的一方分家」；
    ///  · 表里少两行，卡恰好矮两行的距离 → 挡「高不看行数」（写成常量、或 `push_body_h` 里漏了 `n`）；
    ///  · 卡脚前面那道空白 ≤ 一行 + 一道缝 → 挡「按空窗口的尺寸摆着」。
    #[test]
    fn the_push_card_is_as_tall_as_its_branches() {
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 2), "工具条上没有「推送」那枚");
        let out = settled_frame(&mut app, 0.4);
        let (fill, card) = card_back(&out).expect("推送卡的卡底没画出来");
        assert_eq!(fill, Color32::WHITE, "卡底不是白的：{fill:?}");
        let computed = card_rect(
            app.modal.as_ref().expect("卡没打开"),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H)),
        );
        assert_eq!(card, computed, "帧里那块白底和布局算出来的那一格不一致");
        assert!(
            card.min.x >= 0.0 && card.min.y >= 0.0 && card.max.x <= WIN_W && card.max.y <= WIN_H,
            "推送卡溢出默认窗口了：{card:?}，窗口 {WIN_W}×{WIN_H}"
        );
        let void = body_void(&out, card);
        assert!(
            void <= MODAL_ROW_H + MODAL_GAP + 1.0,
            "卡脚前面空了 {void} pt：分支只有四行，卡却按一张空窗口的尺寸摆着"
        );
        // 夹具是四条分支，砍掉两条 → 卡应当恰好矮 `2 * PUSH_ROW_H`。
        let four = card.height();
        if let Some(m) = app.modal.as_mut() {
            if let Form::Push { rows, .. } = &mut m.form {
                rows.truncate(2);
            }
        }
        let two = card_back(&settled_frame(&mut app, 0.4))
            .expect("改成两行之后卡底没了")
            .1
            .height();
        assert_eq!(
            four - two,
            2.0 * PUSH_ROW_H,
            "卡的高和分支那几行不搭：少两行只矮了 {:.1} pt",
            four - two
        );
    }

    /// 表里每一条本地分支都得有一行，且**远端那一侧预先填的是同名**——不同名就得由人改，
    /// 预先替你改成一个猜的名字等于替你决定远端上多出一条分支。
    /// URL 那一格念的是 `push_url`（没有才退回 `url`）：推和拉走的可能不是同一个地址，
    /// 卡上要说的是「推到哪儿」。
    #[test]
    fn the_push_card_shows_every_local_branch_and_where_it_would_land() {
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 2));
        for word in [
            "推送到仓库：",
            "要推送的分支",
            "本地分支",
            "远程分支",
            "跟踪？",
            "全选",
            "推送所有标签",
            "强制推送",
            "main",
            "feature/x",
            "tricky",
            "docs",
        ] {
            assert!(card_has(&mut app, word), "推送卡上少了这一格：{word}");
        }
        let rows = push_rows(&app);
        assert_eq!(
            rows.iter().map(|(l, _, _, r)| (l.as_str(), r.as_str())).collect::<Vec<_>>(),
            vec![("main", "main"), ("feature/x", "feature/x"), ("tricky", "tricky"), ("docs", "docs")],
            "表里每一行的远端那一侧不是预先同名"
        );
        // 「跟踪？」= 这条**还没有**上游。已有上游的三条不该带着勾开出来。
        assert_eq!(
            rows.iter().map(|(_, _, t, _)| *t).collect::<Vec<bool>>(),
            vec![false, true, false, false],
            "「跟踪？」那一列的初值不是「这条还没有上游」"
        );
        // 夹具里 `origin` 没有 push_url、`fork` 有：卡片跟着远端那一格换 URL。
        assert!(card_has(&mut app, "/tmp/gate-upstream.git"), "选 origin 时 URL 那格没念对");
        assert!(!card_has(&mut app, "/tmp/gate-fork-push.git"), "没选 fork 却画了它的 push_url");
        app.allow_force = false;
        assert!(
            !card_has(&mut app, "强制推送"),
            "偏好里关了「允许强制推送」，卡上那枚勾还摆着"
        );
    }

    /// 五档预先勾哪几条。这一档**不改 git 的实参**（卡上的推送一律带明确 refspec，
    /// 而带 refspec 的 `git push` 完全无视 `push.default`），它唯一的可见后果就在这儿。
    /// 四条分支（`main` 上游同名 / `feature/x` 没上游 / `tricky` 上游不同名 / `docs` 上游同名
    /// 但不是当前那条）是特意凑的：五档的勾选结果**两两不同**，少一条就有两档互相顶包。
    #[test]
    fn the_push_behavior_pref_picks_which_rows_start_checked() {
        for (behavior, expect) in [
            (PushBehavior::Nothing, vec![false, false, false, false]),
            (PushBehavior::Matching, vec![true, true, true, true]),
            (PushBehavior::Upstream, vec![true, false, true, true]),
            (PushBehavior::Simple, vec![true, false, false, true]),
            (PushBehavior::Current, vec![true, false, false, false]),
        ] {
            let mut app = with_remotes();
            app.push_behavior = behavior;
            assert!(click_tool(&mut app, 2), "{behavior:?} 那档开不出推送卡");
            let got: Vec<bool> = push_rows(&app).into_iter().map(|(_, p, _, _)| p).collect();
            assert_eq!(got, expect, "{behavior:?} 那档预先勾错了");
        }
    }

    /// 一条都没勾、也没勾标签 → 主按钮按下去**什么都没发生**（卡留着、不起线程）；
    /// 「全选」勾满之后同一次按才把卡交出去。两半都要：只有前半会放行「永远按不动」，
    /// 只有后半会放行「一条没勾也去跑 git」。
    #[test]
    fn the_push_card_only_fires_when_something_is_picked() {
        let mut app = with_remotes();
        app.push_behavior = PushBehavior::Nothing;
        assert!(click_tool(&mut app, 2));
        assert!(click_card_button(&mut app, "推送"), "卡脚上没有「推送」那枚");
        assert!(app.modal.is_some(), "一条都没勾还是把卡关掉了");
        assert!(!app.busy, "一条都没勾却起了 git 线程");
        assert!(click_card(&mut app, "全选"), "「全选」没点到");
        assert!(push_rows(&app).iter().all(|(_, p, _, _)| *p), "「全选」之后还有行没勾上");
        assert!(click_card_button(&mut app, "推送"));
        assert!(app.modal.is_none(), "勾满了按「推送」没把卡交出去");
        assert!(app.busy, "卡交出去了但没有动作在路上");
    }

    /// 表格第 0 行那一格方框点一下就把那一条取消——这是「只推勾着的那几条」唯一的手工入口。
    /// 用坐标而不是文字：方框没有文字可找（和文件表那几条同一套）。
    #[test]
    fn clicking_a_push_row_box_unchecks_only_that_row() {
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 2));
        let card = card_now(&app);
        click_card_at(&mut app, push_row_box(card, 0));
        let rows = push_rows(&app);
        assert_eq!(rows[0].1, false, "点掉第 0 行的方框没生效");
        assert_eq!(rows[1].1, true, "把没点的那一行也一起取消了");
        assert!(
            rows[2..].iter().all(|(_, p, _, _)| *p),
            "点一行却把后面的行一起带走了"
        );
    }

    /// 卡脚那一条带子里，**包住**「{label}」那块文字的最小圆角矩形：底色、描边色、那一格。
    /// 问帧里的形状而不是问常量，是因为「主按钮和取消一眼分不分得开」只有画出来才算数。
    /// 底色取**有底的那一笔**（`rect_filled`），描边再按同一格去别处找（`rect_stroke` 是单独一笔，
    /// 空心那枚的 `fill` 是透明的）——不按两笔分开问，「取消」那圈灰边就永远读不到。
    /// 找不到返回 `None`：偏好卡**故意没有**「取消」那一枚（开关当场就落了，说「取消」是谎报）。
    fn foot_button(out: &egui::FullOutput, card: Rect, label: &str) -> Option<(Color32, Color32, Rect)> {
        let band = (card.max.y - MODAL_FOOT_H)..=card.max.y;
        let g = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    Some((t.galley.text().to_owned(), t.galley.rect.translate(t.pos.to_vec2())))
                }
                _ => None,
            })
            .find(|(text, r)| text == label && band.contains(&r.center().y))
            .map(|(_, r)| r)?;
        let mut found: Vec<&egui::epaint::RectShape> = Vec::new();
        for c in &out.shapes {
            let mut leg = Vec::new();
            collect_rects(&c.shape, c.clip_rect, &mut leg);
            found.extend(leg.into_iter().map(|(r, _)| r));
        }
        let inside = found
            .iter()
            .filter(|r| r.corner_radius != egui::CornerRadius::ZERO && r.rect.contains(g.center()))
            .collect::<Vec<_>>();
        let btn = *inside
            .iter()
            .filter(|r| r.fill != Color32::TRANSPARENT)
            .min_by_key(|r| (r.rect.width() * r.rect.height()) as i64)?;
        let edge = inside
            .iter()
            .filter(|r| r.rect == btn.rect && r.stroke.color != Color32::TRANSPARENT)
            .map(|r| r.stroke.color)
            .next()
            .unwrap_or(Color32::TRANSPARENT);
        Some((btn.fill, edge, btn.rect))
    }

    /// 主按钮和「取消」得**一眼分得开**。目视那一轮（2026-10-24，用户：「提交、推送、设置窗口都好丑」）
    /// 里两张卡脚都是两枚白底灰边的按钮，最右边那枚一点就动 git，看着和「取消」一模一样。
    /// 三档各挡一种回退：主按钮填主题蓝（都退回白底就红这儿）、取消是白底 + 灰边、
    /// 主按钮上的字是白的（填了蓝却忘了改字色，那四个字就读不出来）。
    /// 偏好卡故意没有「取消」，所以那一枚只在前两张卡上问。
    #[test]
    fn the_primary_button_is_the_filled_one() {
        for (tool, word, has_cancel) in [(1usize, "拉取", true), (2, "推送", true), (10, "完成", false)] {
            let mut app = with_remotes();
            assert!(click_tool(&mut app, tool), "工具条上没有第 {tool} 枚");
            let out = settled_frame(&mut app, 0.4);
            let card = card_back(&out).expect("卡底没画出来").1;
            let (fill, edge, btn) =
                foot_button(&out, card, word).unwrap_or_else(|| panic!("卡脚上没有「{word}」那枚实心按钮"));
            assert_eq!(fill, ACCENT, "{word} 卡的主按钮没填成主题蓝：{fill:?}");
            assert_eq!(
                edge,
                Color32::TRANSPARENT,
                "{word} 卡的主按钮是实心一块，不该再描一圈：{edge:?}"
            );
            assert!(
                card.contains(btn.min) && card.contains(btn.max),
                "{word} 卡的主按钮跑到卡外了：{btn:?}，卡 {card:?}"
            );
            let band = (card.max.y - MODAL_FOOT_H)..=card.max.y;
            assert_eq!(
                text_color(&out, word, band),
                Color32::WHITE,
                "{word} 那枚填了蓝、字却还是深色"
            );
            if !has_cancel {
                continue;
            }
            let (cfill, cedge, cancel) = foot_button(&out, card, "取消").expect("卡脚上没有「取消」那枚");
            assert_eq!(cfill, Color32::WHITE, "「取消」不是白底：{cfill:?}");
            assert_eq!(cedge, BORDER_STRONG, "「取消」那圈灰边没画上：{cedge:?}");
            assert!(
                cancel.max.x < btn.min.x,
                "「取消」跑到主按钮右边去了：{cancel:?} vs {btn:?}"
            );
        }
    }

    /// 卡上那几枚勾的**格子**是方的、边长就是 `CHK_CARD`。
    /// 这条钉的是目视那一轮里最难看的一处：全局 `spacing.interact_size.y` 是日志行高 26，
    /// egui 的 checkbox 把格子的高度取 `max(icon_width, interact_size.y)`，于是卡上每一枚
    /// 都是一块 18×26 的竖长条（用户原话「好丑」的那一档）。`check_row` 在**卡那一层**的
    /// scope 里把这三档都压回 18，主窗口的行高一处都不碰。
    /// 两张卡各量一枚：`pref_switch` 和 `modal_switch` 是两条出口，只验一半就放行另一半。
    #[test]
    fn the_card_checkboxes_are_small_squares() {
        for (tool, label) in [(10usize, "启动时重新打开上一次的仓库"), (2, "推送所有标签")] {
            let mut app = with_remotes();
            assert!(click_tool(&mut app, tool), "工具条上没有第 {tool} 枚");
            let out = settled_frame(&mut app, 0.4);
            let card = card_back(&out).expect("卡底没画出来").1;
            let band = card.min.y..=card.max.y;
            let text = text_rect(&out, label, band.clone());
            let mut found: Vec<(&egui::epaint::RectShape, Rect)> = Vec::new();
            for c in &out.shapes {
                collect_rects(&c.shape, c.clip_rect, &mut found);
            }
            let box_rect = found
                .into_iter()
                .map(|(r, _)| r.rect)
                // 贴着那行文字左缘的那一枚小方块（`icon_spacing` 是 6pt 的缝）。
                .find(|q| {
                    q.width() < 30.0
                        && q.max.x <= text.min.x
                        && text.min.x - q.max.x < 12.0
                        && (q.center().y - text.center().y).abs() < q.height()
                })
                .unwrap_or_else(|| panic!("「{label}」左边那一格勾没画出来"));
            assert!(
                (box_rect.width() - CHK_CARD).abs() <= 1.0 && (box_rect.height() - CHK_CARD).abs() <= 1.0,
                "「{label}」那枚勾是 {:?}，不是 {CHK_CARD} 见方",
                (box_rect.width(), box_rect.height())
            );
        }
    }

    /// 拉取卡同一条规矩：高按「四行 + 选项那只框」算，不照 `GitPullSheet.nib` 那张 614×299 摆。
    /// 这张卡的行数是定的，所以挡「按空窗口摆」的正是最后那道空白：老尺寸底下会多出 58 pt，
    /// 一档预算（一行 + 一道缝）拦得住。
    #[test]
    fn the_pull_card_is_as_tall_as_its_rows() {
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 1), "工具条上没有「拉取」那枚");
        let out = settled_frame(&mut app, 0.4);
        let (fill, card) = card_back(&out).expect("拉取卡的卡底没画出来");
        assert_eq!(fill, Color32::WHITE, "卡底不是白的：{fill:?}");
        let computed = card_rect(
            app.modal.as_ref().expect("卡没打开"),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(WIN_W, WIN_H)),
        );
        assert_eq!(card, computed, "帧里那块白底和布局算出来的那一格不一致");
        assert!(
            card.min.x >= 0.0 && card.min.y >= 0.0 && card.max.x <= WIN_W && card.max.y <= WIN_H,
            "拉取卡溢出默认窗口了：{card:?}，窗口 {WIN_W}×{WIN_H}"
        );
        let void = body_void(&out, card);
        assert!(
            void <= MODAL_ROW_H + MODAL_GAP + 1.0,
            "卡脚前面空了 {void} pt：选项只有三行，卡却按一张空窗口的尺寸摆着"
        );
    }

    /// 「选项」那只框里三枚勾**逐字**取自 nib（第四句「立即提交合并的改动」是 git 的默认行为，
    /// 摆了看不出区别，所以那一格留空 —— 位置留着，控件不摆）。
    #[test]
    fn the_pull_card_shows_the_three_rows_and_the_options() {
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 1));
        for word in [
            "从仓库拉取：",
            "要拉取的远程分支：",
            "拉取到本地分支：",
            "刷新",
            "选项",
            "包括被合并提交的信息内容",
            "无论是否可以快进更新都创建新的提交",
            "用变基代替合并 (警告：请确保您还没有推送您的变更)",
        ] {
            assert!(card_has(&mut app, word), "拉取卡上少了这一格：{word}");
        }
        // 当前分支 `main` 的上游是 `origin/main` → 远端那格念 origin，远程分支那格念 main，
        // 本地那格就是当前分支。这三格是卡的初值，不是 git 的参数。
        assert!(card_has(&mut app, "origin ▼"), "远端那一格没落在当前分支的上游上");
        assert!(card_has(&mut app, "/tmp/gate-upstream.git"), "URL 那格没念所选远端");
    }

    /// 拉取卡上那三格此刻写着什么：`(远端, 远程分支, 拉到哪条本地分支)`。
    fn pull_fields(app: &Gitx) -> (String, String, String) {
        let m = app.modal.as_ref().expect("判据要读拉取卡，可这会儿没有打开的卡");
        match &m.form {
            Form::Pull { remote, branch, into, .. } => {
                (remote.clone(), branch.clone(), into.clone())
            }
            _ => panic!("打开的不是拉取卡"),
        }
    }

    /// 远程分支那一列**只列所选远端**：`fork/only-on-fork` 在选 origin 时不许出现，
    /// 把远端换成 fork 之后它才出现、origin 那两条反而没了。
    /// 换远端走的是**真鼠标**（点开那枚弹出按钮、点菜单里那一项），不是直接改表单字段——
    /// 直接改字段就测不到 `cell_popup` 那一层，而那正是这一格唯一的手。
    #[test]
    fn the_pull_card_lists_only_the_selected_remote() {
        let mut app = with_remotes();
        assert!(click_tool(&mut app, 1));
        assert!(menu_shows(&mut app, "main ▼", "feature/x"), "选了 origin，那一列里却没有 origin/feature/x");
        assert!(!menu_shows(&mut app, "main ▼", "only-on-fork"), "origin 那一列里混进了 fork 的分支");

        click_menu_item(&mut app, "origin ▼", "fork");
        assert_eq!(pull_fields(&app).0, "fork", "点了菜单里的 fork，卡上远端那一格没换");
        assert!(menu_shows(&mut app, "main ▼", "only-on-fork"), "换成 fork 后那一列没跟上");
        assert!(!menu_shows(&mut app, "main ▼", "feature/x"), "fork 那一列里还留着 origin 的分支");
        // 拉取念的是 `url`，推送才念 `push_url`——夹具里 fork 这两条不一样，正好分得开。
        assert!(card_has(&mut app, "/tmp/gate-fork.git"), "换成 fork 后 URL 那格没跟着换");
    }

    /// 推送卡那三格旗标：`(远端, 强制, 推送所有标签)`。
    fn push_flags(app: &Gitx) -> (String, bool, bool) {
        let m = app.modal.as_ref().expect("判据要读推送卡，可这会儿没有打开的卡");
        match &m.form {
            Form::Push { remote, force, tags, .. } => (remote.clone(), *force, *tags),
            _ => panic!("打开的不是推送卡"),
        }
    }

    /// 往卡上某个坐标那一格**重写**（先点它拿焦点、⌘A 全选，再喂 `Event::Text`）。
    /// 全选那一步不能省：真用户改一个已有的名字就是先选中再打，光点一下是接在后面追加。
    fn retype_at_card(app: &mut Gitx, at: Pos2, text: &str) {
        let ctx = warm(app);
        click_at(&ctx, app, at);
        let a = |pressed: bool| egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        let mods = |m: egui::Modifiers| egui::Event::ModifiersChanged(m);
        for events in [
            vec![egui::Event::PointerMoved(at), mods(egui::Modifiers::COMMAND), a(true)],
            vec![a(false), mods(egui::Modifiers::NONE)],
            vec![egui::Event::PointerMoved(at), egui::Event::Text(text.to_owned())],
        ] {
            ctx.run_ui(raw_input(events), |ui| app.shell(ui)).textures_delta.clear();
        }
    }

    /// 表格第 `i` 行**「远程分支」那一格**的中心——那一格是可编辑的 `TextEdit`。
    fn push_remote_cell(card: Rect, i: usize) -> Pos2 {
        let top = pos2(card.min.x, card.min.y + MODAL_TITLE_H);
        let x = top.x + PUSH_TABLE_X + PUSH_COL_PICK + PUSH_COL_LOCAL + PUSH_COL_REMOTE / 2.0;
        let y = top.y + PUSH_TABLE_TOP + PUSH_HEAD_H + PUSH_ROW_H * i as f32 + PUSH_ROW_H / 2.0;
        pos2(x, y)
    }

    /// 偏好卡「推送与拉取」那一页改的五条，推送卡**当场**就该读得到；闸关掉之后那枚勾
    /// 摆都不摆。远端那一格的落点也在这条里问：当前分支没有上游时它读偏好，
    /// 偏好也没设就**留空、主按钮按不动**——宁可让人挑，不替他猜 `origin`。
    #[test]
    fn the_push_preferences_change_what_the_push_card_starts_with() {
        let db_file = temp_db("push-prefs");
        let mut app = with_remotes();
        app.conn = Some(db::open(db_file.to_str().unwrap()).expect("临时库"));
        // 把「当前分支」换成 `feature/x`——它没有上游，这时「默认远端」才说了算。
        {
            let s = app.snap.as_mut().unwrap();
            s.branches[0].is_current = false;
            s.branches[1].is_current = true;
        }
        assert!(click_tool(&mut app, 2), "没有上游也该开得出推送卡");
        assert_eq!(push_flags(&app).0, "", "当前分支没上游、偏好也没设默认远端时，那一格该留空");
        assert!(click_card_button(&mut app, "推送"));
        assert!(app.modal.is_some(), "远端那一格空着还把卡交出去了（等于替你猜远端）");
        press(&mut app, egui::Key::Escape);

        assert!(click_tool(&mut app, 10));
        assert!(click_card(&mut app, "推送与拉取"), "偏好卡上没有「推送与拉取」那一页");
        click_menu_item(&mut app, "（没有） ▼", "fork");
        assert!(click_card(&mut app, "允许强制推送"), "那枚闸没点到");
        assert!(click_card(&mut app, "推送所有标签"), "那枚勾没点到");
        assert!(!app.allow_force, "点了闸但 app.allow_force 没拧");
        assert!(app.push_tags, "点了「推送所有标签」但没拧");
        assert_eq!(app.default_remote, "fork", "默认远端没落到界面上");
        // 「推送时」那一档换的是表里预先勾上哪几条（不是 git 的实参，见 `PushBehavior` 那段），
        // 所以这一条必须在**重开的卡**上问，光读 `app.push_behavior` 只验到弹出一半。
        click_menu_item(&mut app, "匹配 ▼", "当前");
        assert_eq!(app.push_behavior, PushBehavior::Current, "点了「当前」那一档，app 没跟上");
        // 那一格得把选中的这一档**念回来**：`label()` 走的是 `index()`，落库走的是 `key()`，
        // 两条路各写各的，只读字段的话「显示成简单、存成 current」这种错看不出来。
        assert!(card_has(&mut app, "当前 ▼"), "选了「当前」，那一格没念回这一档");
        assert_eq!(Prefs::load(app.conn.as_ref()), app.prefs(), "五条没一起落库");
        assert!(click_card_button(&mut app, "完成"));

        assert!(click_tool(&mut app, 2));
        let (remote, force, tags) = push_flags(&app);
        assert_eq!(remote, "fork", "默认远端没成为卡上那一格的初值");
        assert!(!force, "「强制推送」不该带着勾开出来");
        assert!(tags, "「推送所有标签」那枚初值没读到偏好");
        assert_eq!(
            push_rows(&app).iter().map(|(_, p, _, _)| *p).collect::<Vec<bool>>(),
            vec![false, true, false, false],
            "偏好选了「当前」，重开的推送卡却勾了别的几条（这会儿当前是 feature/x）"
        );
        assert!(!card_has(&mut app, "强制推送"), "闸关了，卡上那枚勾还摆着");
        assert!(card_has(&mut app, "/tmp/gate-fork-push.git"), "推送卡该念 fork 的 push_url");
        let _ = std::fs::remove_file(&db_file);
    }

    /// 端到端：真仓库 + 真裸仓库当远端。表里**取消勾选**的那条不许推上去，勾着的按
    /// 卡上「远程分支」那一格的名字推上去（refspec），没上游的那条推完把上游记下来。
    /// 这三件事全在远端那头的 ref 上验，不看 app 自己的说法。
    #[test]
    fn the_push_card_sends_only_the_checked_rows_under_the_names_it_shows() {
        let dir = scratch_repo("card-push");
        let bare = std::env::temp_dir().join(format!("gitx-bare-push-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&bare);
        std::fs::create_dir_all(&bare).expect("造裸仓库目录");
        git_in(&bare, &["init", "-q", "--bare", "."]);
        git_in(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_in(&dir, &["push", "-q", "-u", "origin", "main"]);
        // 本地再多一条（不推）、再长一条没有上游的分支：卡上那两条正是「勾掉的」和「要推的」。
        git_in(&dir, &["commit", "-q", "--allow-empty", "-m", "这条不该推上去"]);
        git_in(&dir, &["branch", "feature/x"]);
        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| a.snap.is_some());

        assert!(click_tool(&mut app, 2), "真仓库开不出推送卡");
        let card = card_now(&app);
        let i = push_rows(&app)
            .iter()
            .position(|(l, _, _, _)| l == "main")
            .expect("表里没有 main 那一行");
        click_card_at(&mut app, push_row_box(card, i));
        let j = push_rows(&app)
            .iter()
            .position(|(l, _, _, _)| l == "feature/x")
            .expect("表里没有 feature/x 那一行");
        retype_at_card(&mut app, push_remote_cell(card, j), "renamed");
        assert_eq!(
            push_rows(&app).remove(j).3,
            "renamed",
            "「远程分支」那一格敲的字没落到绑定上"
        );
        assert!(click_card_button(&mut app, "推送"));
        pump(&mut app, |a| !a.busy);

        let heads = git_in(&bare, &["for-each-ref", "--format=%(refname:short)", "refs/heads/"]);
        assert_eq!(heads, "main\nrenamed", "远端上落下来的 ref 不对：{heads:?}");
        assert_eq!(
            git_in(&bare, &["log", "-1", "--format=%s", "main"]),
            "起点",
            "取消勾选的那条也被推上去了（而且带的还是旧的 main）"
        );
        assert_eq!(
            git_in(&dir, &["config", "--get", "branch.feature/x.remote"]),
            "origin",
            "「跟踪？」勾着却没记上游"
        );
        assert!(app.note.is_none(), "推送路上报了 note：{}", app.note.clone().unwrap_or_default());
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --------------------------------------------------------------- 拉取端到端
    //
    // 和推送那条同一套规矩：真仓库 + 真裸仓库当远端，断言问的全是磁盘上的事实。

    /// 一个空裸仓库当远端。
    fn bare_remote(tag: &str) -> std::path::PathBuf {
        let bare = std::env::temp_dir().join(format!("gitx-bare-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&bare);
        std::fs::create_dir_all(&bare).expect("造裸仓库目录");
        git_in(&bare, &["init", "-q", "--bare", "."]);
        bare
    }

    /// 拉取端到端夹具：本地 main 和远端 main 从「起点」分岔——本地多了「我的」
    /// （动的就是 a.txt），远端多了「他们的」。工作区是干净的：变基不许带着未提交改动跑。
    fn divergent_repo(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = scratch_repo(tag);
        // 夹具里那条已暂存的改动就是「我的」这一步：commit 掉，工作区也跟着干净了。
        let bare = bare_remote(tag);
        git_in(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_in(&dir, &["push", "-q", "-u", "origin", "main"]);
        git_in(&dir, &["commit", "-q", "-m", "我的"]);
        git_in(&dir, &["checkout", "-q", "-b", "theirs", "HEAD~1"]);
        git_in(&dir, &["commit", "-q", "--allow-empty", "-m", "他们的"]);
        git_in(&dir, &["push", "-q", "origin", "theirs:main"]);
        git_in(&dir, &["checkout", "-q", "main"]);
        git_in(&dir, &["branch", "-q", "-D", "theirs"]);
        (dir, bare)
    }

    /// 拉取卡某一行弹出按钮的中心。「拉到哪条」那行初值多半也写着 main，两行按文字
    /// 找分不出彼此——所以这枚按量来的行坐标点，不按字找。x 只敢往**左**偏：那枚按钮
    /// 实际只有文字那么宽，单元格正中早出了它的命中矩形、点到的就是遮罩。
    fn pull_cell(card: Rect, y: f32) -> Pos2 {
        let top = pos2(card.min.x, card.min.y + MODAL_TITLE_H);
        pos2(top.x + PULL_FIELD_X + 28.0, top.y + y + 12.5)
    }

    /// `menu_shows` 的按行版：数两次（没点开时几个、点开之后几个），变多了才算菜单里有。
    fn pull_menu_shows(app: &mut Gitx, y: f32, item: &str) -> bool {
        let before = warm_frame(app);
        let n0 = word_count(&before, item);
        let ctx = warm(app);
        click_at(&ctx, app, pull_cell(card_now(app), y));
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let n1 = word_count(&out, item);
        out.textures_delta.clear();
        n1 > n0
    }

    /// `click_menu_item` 的按行版：点开那行弹出按钮、再点菜单里那一项。
    fn pull_pick(app: &mut Gitx, y: f32, item: &str) {
        let ctx = warm(app);
        click_at(&ctx, app, pull_cell(card_now(app), y));
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let spot = out
            .shapes
            .iter()
            .rev()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == item => {
                    Some(t.galley.rect.translate(t.pos.to_vec2()).center())
                }
                _ => None,
            });
        out.textures_delta.clear();
        let Some(spot) = spot else { panic!("菜单里没有那一项：{item}") };
        click_at(&ctx, app, spot);
    }

    /// 「拉取到本地分支」选的不是当前那条：壳里得先切过去再拉（`git pull` 只会拉进
    /// 当前分支）。拉完目标那条到了「那边的」、HEAD 停在那条上，main 一个字没动。
    #[test]
    fn pulling_lands_on_the_local_branch_the_card_names() {
        let dir = scratch_repo("pull-into");
        git_in(&dir, &["reset", "-q", "--hard", "HEAD"]);
        let bare = bare_remote("pull-into");
        git_in(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_in(&dir, &["push", "-q", "-u", "origin", "main"]);
        git_in(&dir, &["branch", "side"]);
        git_in(&dir, &["push", "-q", "origin", "side"]);
        git_in(&dir, &["checkout", "-q", "-b", "tmp", "side"]);
        git_in(&dir, &["commit", "-q", "--allow-empty", "-m", "那边的"]);
        git_in(&dir, &["push", "-q", "origin", "HEAD:side"]);
        git_in(&dir, &["checkout", "-q", "main"]);
        git_in(&dir, &["branch", "-q", "-D", "tmp"]);

        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| a.snap.is_some());
        assert!(click_tool(&mut app, 1));
        pull_pick(&mut app, PULL_BRANCH_Y, "side");
        pull_pick(&mut app, PULL_INTO_Y, "side");
        let (remote, branch, into) = pull_fields(&app);
        assert_eq!(
            (remote.as_str(), branch.as_str(), into.as_str()),
            ("origin", "side", "side"),
            "两格没选上"
        );
        assert!(click_card_button(&mut app, "拉取"), "卡脚上没有「拉取」那枚");
        pump(&mut app, |a| !a.busy);
        assert!(app.note.is_none(), "拉取路上报了 note：{}", app.note.clone().unwrap_or_default());
        assert_eq!(
            git_in(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "side",
            "没切到要拉进的那条上"
        );
        assert_eq!(git_in(&dir, &["log", "-1", "--format=%s", "side"]), "那边的", "side 没被拉上去");
        assert_eq!(git_in(&dir, &["log", "-1", "--format=%s", "main"]), "起点", "main 被顺手动了");
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 分岔之后拉取：不勾变基就是一次合并（HEAD 两个爹），勾了变基就是把「我的」
    /// 挪到「他们的」后面（HEAD 一个爹、还是「我的」）。两条都在磁盘上的历史上验，
    /// 且本地那步的工作（a.txt 的第二行）必须保住。
    #[test]
    fn pulling_a_diverged_remote_merges_or_rebases_as_the_switch_says() {
        for (rebase, parents, why) in
            [(false, 2usize, "没勾变基却没走合并"), (true, 1, "勾了变基却没走变基")]
        {
            let (dir, bare) =
                divergent_repo(&format!("pull-{}", if rebase { "rebase" } else { "merge" }));
            let mut app = blank();
            app.open(dir.to_str().unwrap());
            pump(&mut app, |a| a.snap.is_some());
            assert!(click_tool(&mut app, 1));
            let was = match &app.modal.as_ref().unwrap().form {
                Form::Pull { opts, .. } => opts.rebase,
                _ => panic!("打开的不是拉取卡"),
            };
            assert!(!was, "重开一张卡，那枚勾得是空的");
            if rebase {
                assert!(click_card(&mut app, "用变基代替合并"), "变基那枚勾没点到");
                let now = match &app.modal.as_ref().unwrap().form {
                    Form::Pull { opts, .. } => opts.rebase,
                    _ => panic!("打开的不是拉取卡"),
                };
                assert!(now, "点了那枚勾，表单里没变");
            }
            assert!(click_card_button(&mut app, "拉取"), "卡脚上没有「拉取」那枚");
            pump(&mut app, |a| !a.busy);
            assert!(
                app.note.is_none(),
                "拉取路上报了 note：{}",
                app.note.clone().unwrap_or_default()
            );
            let n = git_in(&dir, &["log", "-1", "--format=%P"]).split_whitespace().count();
            assert_eq!(n, parents, "{why}：HEAD 的爹有 {n} 个");
            if rebase {
                assert_eq!(
                    git_in(&dir, &["log", "-1", "--format=%s"]),
                    "我的",
                    "变基完 HEAD 不是挪过去的那条"
                );
            }
            assert!(git_in(&dir, &["log", "--format=%s"]).contains("他们的"), "远端那一步没进来");
            assert_eq!(
                std::fs::read_to_string(dir.join("a.txt")).unwrap(),
                "一行\n第二行\n",
                "本地那一步的工作没保住"
            );
            let _ = std::fs::remove_dir_all(&bare);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// 「刷新」那枚：外面再克隆一本、推一条卡上还没有的新分支，点完刷新之后那一列
    /// 才出现它。负半截先证它**不在**——不然门禁放行的是「本来就有」。
    #[test]
    fn the_refresh_button_brings_the_remote_branch_column_up_to_date() {
        let dir = scratch_repo("pull-refresh");
        git_in(&dir, &["reset", "-q", "--hard", "HEAD"]);
        let bare = bare_remote("pull-refresh");
        git_in(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_in(&dir, &["push", "-q", "-u", "origin", "main"]);

        let mut app = blank();
        app.open(dir.to_str().unwrap());
        pump(&mut app, |a| a.snap.is_some());
        assert!(click_tool(&mut app, 1));

        // 第二个克隆在外面推：本仓库的 refs/remotes/origin 只能靠 fetch 才看得见它。
        let other = std::env::temp_dir().join(format!("gitx-other-{}-pull-refresh", std::process::id()));
        let _ = std::fs::remove_dir_all(&other);
        git_in(
            &std::env::temp_dir(),
            &["clone", "-q", bare.to_str().unwrap(), other.to_str().unwrap()],
        );
        git_in(&other, &["config", "user.email", "t@t.com"]);
        git_in(&other, &["config", "user.name", "tester"]);
        git_in(&other, &["commit", "-q", "--allow-empty", "-m", "别人推的"]);
        git_in(&other, &["push", "-q", "origin", "HEAD:refs/heads/from-afar"]);

        assert!(
            !pull_menu_shows(&mut app, PULL_BRANCH_Y, "from-afar"),
            "还没刷新那一列就有它了"
        );
        // 「刷新」按 nib 量在**正文右上**（`PULL_REFRESH`），不是卡脚那两枚，
        // 所以走 `click_card`（整张卡里按文字找）而不是只看脚部的 `click_card_button`。
        assert!(click_card(&mut app, "刷新"), "卡上没有「刷新」那枚");
        pump(&mut app, |a| {
            a.snap.as_ref().is_some_and(|s| {
                s.remote_refs.iter().any(|r| r.name == "origin/from-afar")
            })
        });
        assert!(
            pull_menu_shows(&mut app, PULL_BRANCH_Y, "from-afar"),
            "刷新完了那一列还是旧的"
        );
        assert!(
            app.note.is_none(),
            "刷新路上报了 note：{}",
            app.note.clone().unwrap_or_default()
        );
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }

    // ---------------------------------------------------- 归档成 zip / 原生选目录面板
    //
    // 这两条是同一类接线：点一下**不当场跑**，只立一块 `pending_dialog`，面板在 `ui()` 里、
    // egui 那一帧之外才跑（`rfd` 的原生存盘/选目录面板是嵌套事件循环）。于是判据分成三段，
    // 每段都得能单独红掉：菜单/按钮 → 牌子；牌子 → 面板之后那段；那段 → 真 git。

    /// 卡上那几枚「浏览…」的中心，按 y 从上往下。克隆那一档有**两枚**（主输入一枚、目标目录
    /// 一枚），按字定位只会撞上第一枚，所以先数出来再挑。
    fn browse_spots(ctx: &egui::Context, app: &mut Gitx) -> Vec<Pos2> {
        let card = card_now(app);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        let mut hits: Vec<(f32, Pos2)> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == "浏览…" => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    card.contains(r.center()).then_some((r.center().y, r.center()))
                }
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.into_iter().map(|(_, p)| p).collect()
    }

    /// 日志一行的右键菜单里有「归档成 zip」，点它只立一块牌子，牌子带的是**打开菜单那一行**
    /// 那条提交的完整号，不是 HEAD。建议文件名钉字面量（`fixture-00000000.zip`）：拿
    /// `repo_name()` 现算一遍再和它自己比，函数写坏了两边一起坏。
    #[test]
    fn the_history_menu_arms_an_archive_for_the_row_it_was_opened_on() {
        let mut app = loaded();
        let want = app.snap.as_ref().unwrap().commits[1].id.clone();
        menu_pick(&mut app, "主题1", "归档成 zip");
        assert_eq!(
            app.pending_dialog,
            Some(Dialog::Archive {
                repo: "/fixture".to_owned(),
                oid: want,
                name: "fixture-00000000.zip".to_owned(),
            }),
            "右键第二行点「归档成 zip」，立起来的牌子不是那一条提交"
        );
        assert!(app.modal.is_none(), "归档只读对象库、不动工作区，不该先问一句确认卡");
        assert!(!app.busy, "面板还没落地就把 busy 立起来：用户在面板里按取消，就再没人把它落回去");
        assert_eq!(app.sel, Sel::Uncommitted, "点归档顺带把选中的那行挪了");
        assert!(app.note.is_none(), "点归档当场就报了 note：{:?}", app.note);
    }

    /// 「跑测试不会弹真面板」那道边界：牌子**不在** `shell()` 里消费。无头判据一律直接调
    /// `shell()`，所以这一位不动面板，测试就一辈子碰不到 `rfd`。
    #[test]
    fn a_headless_frame_never_consumes_the_pending_dialog() {
        let mut app = loaded();
        let want = Some(Dialog::PickFolder { field: FolderField::Main });
        app.pending_dialog = want.clone();
        for _ in 0..3 {
            frame(&mut app);
        }
        assert_eq!(app.pending_dialog, want, "无头帧把牌子消费掉了：判据会替真窗口去跑原生面板");
        assert!(!app.busy, "帧里消费牌子还顺手立了 busy");
    }

    /// 「添加仓库」三档都有「浏览…」（三档填的全是磁盘路径）。枚数按两枚/一枚分档钉：
    /// 只数得出某一档有，就测不出克隆那档的第二枚被删掉。
    #[test]
    fn every_add_repo_tab_has_a_browse_button() {
        let mut app = blank();
        open_add_card(&mut app);
        for (tab, want) in [("打开已有", 1), ("克隆到本地", 2), ("新建仓库", 1)] {
            assert!(click_card(&mut app, tab), "卡上没有「{tab}」那一档");
            let ctx = warm(&mut app);
            let n = browse_spots(&ctx, &mut app).len();
            assert_eq!(n, want, "「{tab}」那档的「浏览…」枚数不对（克隆档主输入 + 目标目录各一枚）");
        }
    }

    /// 两枚「浏览…」交回的是**两个不同的格子**——合成一个枚举值就会把源地址填成目标目录。
    /// 上面那枚 = 主输入，下面那枚 = 目标目录；点完卡不许关（面板回来要往它里面填）。
    #[test]
    fn the_browse_button_arms_the_picker_for_the_field_it_sits_next_to() {
        let mut app = blank();
        open_add_card(&mut app);
        let ctx = warm(&mut app);
        let spots = browse_spots(&ctx, &mut app);
        assert_eq!(spots.len(), 1, "「打开已有」那档该只有一枚「浏览…」");
        click_at(&ctx, &mut app, spots[0]);
        assert_eq!(
            app.pending_dialog,
            Some(Dialog::PickFolder { field: FolderField::Main }),
            "主输入旁边那枚「浏览…」交的不是主输入"
        );
        assert!(app.modal.is_some(), "点「浏览…」把卡关了：面板回来没格子可填");
        assert!(!app.busy, "选目录只是填一个字，不该把界面冻成在跑 git");

        let mut app = blank();
        open_add_card(&mut app);
        assert!(click_card(&mut app, "克隆到本地"), "切不到克隆那一档");
        let ctx = warm(&mut app);
        let spots = browse_spots(&ctx, &mut app);
        assert_eq!(spots.len(), 2, "克隆档该有两枚「浏览…」，点不到第二枚");
        click_at(&ctx, &mut app, spots[1]);
        assert_eq!(
            app.pending_dialog,
            Some(Dialog::PickFolder { field: FolderField::CloneDest }),
            "目标目录旁边那枚「浏览…」交的不是目标目录（两枚交同一格就是填错格子）"
        );
    }

    /// 面板选回来的目录落到**哪一格**。`rfd` 那一层无头跑不到，所以判据直接问它后面那一步。
    #[test]
    fn a_picked_folder_lands_in_the_field_the_button_belongs_to() {
        let mut app = blank();
        open_add_card(&mut app);
        app.fill_folder(FolderField::Main, "/选里的目录");
        assert_eq!(
            app.modal.as_ref().unwrap().text.as_deref(),
            Some("/选里的目录"),
            "主输入那一格没收到面板选回来的目录"
        );

        let mut app = blank();
        open_add_card(&mut app);
        assert!(click_card(&mut app, "克隆到本地"), "切不到克隆那一档");
        app.fill_folder(FolderField::CloneDest, "/目标目录");
        let m = app.modal.as_ref().unwrap();
        let Form::AddRepo { dest, .. } = &m.form else { panic!("卡不是「添加仓库」了") };
        assert_eq!(dest, "/目标目录", "目标目录那一格没收到面板选回来的目录");
        assert!(m.text.as_deref().unwrap_or_default().is_empty(), "填错了：目标目录跑到主输入（源地址）那一格");

        // 牌子立起来之后用户可能把档切走：那一格已经从卡上消失，就不该往它里面写字。
        let mut app = blank();
        open_add_card(&mut app);
        click_card(&mut app, "克隆到本地");
        assert!(click_card(&mut app, "打开已有"), "切不回「打开已有」");
        app.fill_folder(FolderField::CloneDest, "/不该出现");
        let m = app.modal.as_ref().unwrap();
        let Form::AddRepo { dest, .. } = &m.form else { panic!("卡不是「添加仓库」了") };
        assert!(dest.is_empty(), "都切出克隆那一档了，还往目标目录里写字");
    }

    /// 面板之后那一步交给 core 的三个参数。`git::archive` 在 core 侧自有三条判据，这一条钉的是
    /// **UI 交的是哪一条提交**：反向变异把 `archive_to` 里的 `&oid` 换成 `"HEAD"`，第一条断言就红
    /// ——那正是用户右键旧提交、拿回最新代码的那种错。
    /// 第二遍导 HEAD 是给这条判据自己出夹具：文件名压根不出现在 zip 里时 `!contains` 会假绿。
    #[test]
    fn archiving_a_row_hands_core_that_rows_oid_not_head() {
        let (mut app, dir) = open_repo("archive-ui");
        let path = dir.to_str().unwrap().to_owned();
        std::fs::write(dir.join("new.txt"), "第二条才有的文件\n").unwrap();
        git_in(&dir, &["add", "new.txt"]);
        git_in(&dir, &["commit", "-q", "-m", "第二条"]);
        app.open(&path);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()).unwrap_or(0) == 2);
        let snaps = app.snap.as_ref().unwrap();
        let (head, first) = (snaps.commits[0].id.clone(), snaps.commits[1].id.clone());
        let tmp = std::env::temp_dir().join(format!("gitx-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let at_first = tmp.join("first.zip");
        let at_head = tmp.join("head.zip");

        app.archive_to(&path, &first, at_first.to_str().unwrap());
        assert!(app.busy, "归档发出去了但 busy 没立起来");
        let want = format!("已导出 {}", at_first.display());
        pump(&mut app, |a| a.note.as_deref() == Some(want.as_str()));
        let bytes = String::from_utf8_lossy(&std::fs::read(&at_first).unwrap()).into_owned();
        assert!(bytes.contains("PK"), "导出来的不是 zip：{bytes:?}");
        assert!(bytes.contains("a.txt"), "第一条提交里就有的文件没进快照");
        assert!(!bytes.contains("new.txt"), "右键旧提交拿回了最新那一版：快照里有第二条才有的文件");

        app.archive_to(&path, &head, at_head.to_str().unwrap());
        let want = format!("已导出 {}", at_head.display());
        pump(&mut app, |a| a.note.as_deref() == Some(want.as_str()));
        let bytes = String::from_utf8_lossy(&std::fs::read(&at_head).unwrap()).into_owned();
        assert!(bytes.contains("new.txt"), "夹具自己没生效：连 HEAD 的快照都读不出 new.txt，上一条 !contains 是假绿");
        assert!(!app.busy, "归档回来 busy 没落下：进度条会一直转");
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 归档成功的回复：落下 `busy`、报一句落在哪儿。
    #[test]
    fn the_archive_reply_clears_busy_and_names_the_file() {
        let mut app = loaded();
        app.busy = true;
        let tx = app.tx.clone();
        let _ = tx.send(Msg::Archived("/tmp/gitx-00000000.zip".to_owned()));
        frame(&mut app);
        assert!(!app.busy, "归档回复没落下 busy");
        assert_eq!(app.note.as_deref(), Some("已导出 /tmp/gitx-00000000.zip"), "回复没报落在哪儿");
    }

    /// 「分离时日志走宽口径」这一条规则的**四条出口**都得钉，只钉一条等于没钉：
    /// ① 分离 + 「当前分支」→ 换成「所有引用」，否则上面那些提交直接从日志里消失
    ///    （这就是用户报的那件事）；② 在分支上 → 不许偷偷把别人的分支也拉进来；
    ///    ③ 已经是「所有引用」→ 保持；④ 用户在日志栏点了某条分支的历史（`Rev`）→
    ///    **必须**保持，那是他明确挑的口径，分离不分离都不能替他改回去。
    ///
    /// 翻页那一处（`ask_for_more`）也过这一位，但无头里没有 500+ 提交的夹具，
    /// 「那里直接传 `self.scope`」这种改坏杀不掉 —— 已记进台账，不当作业已做完。
    #[test]
    fn a_detached_repo_widens_the_log_scope_and_a_wider_one_never_narrows() {
        use git::Scope;
        assert_eq!(
            eff_scope(&Scope::Current, true),
            Scope::All,
            "分离时按 HEAD 的可达链取数 = 检出的那条提交之上的历史全没了"
        );
        assert_eq!(
            eff_scope(&Scope::Current, false),
            Scope::Current,
            "在分支上时不许顺手把整个仓库的历史倒进日志"
        );
        assert_eq!(eff_scope(&Scope::All, true), Scope::All, "已经是最宽口径，别再收窄");
        assert_eq!(
            eff_scope(&Scope::Rev("side".to_owned()), true),
            Scope::Rev("side".to_owned()),
            "用户点了「只看 side 的历史」，分离不许把这一格改成全仓库"
        );
    }

    /// 端到端：真仓库、真检出到**上一条**提交，然后回答用户那句「后面的历史都看不到了」。
    /// 日志必须还认得出上面那条更新的提交。杀伤路径是把 `snapshot_of` 里那句
    /// `eff_scope` 撤掉 —— 那样取回来的就是 `HEAD~1` 的可达链，只剩一条。
    #[test]
    fn a_detached_checkout_still_shows_the_commits_above() {
        // 提交**先**造好再开仓库：`open_repo` 之后没人会自己去重读磁盘，
        // 往临时仓库里再 commit 一条，快照里那条永远等不来（pump 只会白跑 600 帧）。
        let dir = scratch_repo("detached-log");
        // 夹具带着一处**已暂存**的改动；带着它 checkout 会被 git 拒（would be overwritten），
        // 报出来的就是 checkout 的错，而不是我要问的那件事。
        git_in(&dir, &["reset", "-q", "--hard", "HEAD"]);
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        git_in(&dir, &["add", "b.txt"]);
        git_in(&dir, &["commit", "-q", "-m", "后一条"]);
        let head = git_in(&dir, &["rev-parse", "HEAD"]);
        let first = git_in(&dir, &["rev-parse", "HEAD~1"]);
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        let mut app = blank();
        app.open(&path);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(2));
        let snap = app.snap.as_ref().unwrap();
        assert_eq!(snap.commits[0].id, head, "夹具自己没生效：日志顶上就不是最新那条");
        assert!(snap.status.detached_at.is_none(), "夹具自己得先在分支上");
        let before = snap.taken_at;
        assert!(app.act(Action::Checkout(first.clone())), "检出没被接住");
        // 等的是「**新的一份快照**落地」，不是 `!busy`：`busy` 是一面共用的旗子，
        // `Msg::Patch`（开仓库时那一格差异的回复）也在落它 —— 检出还在路上，差异先回来，
        // 旗子就落了，`!busy` 那一刻读到的仍是**检出之前**那份快照。
        // （这本身是个界面缺陷，已记台账；这里先把判据的等待条件写对。）
        pump(&mut app, |a| a.snap.as_ref().is_some_and(|s| s.taken_at > before));
        let snap = app.snap.as_ref().unwrap();
        assert_eq!(snap.commits.len(), 2, "检出上一条之后，上面那条从日志里消失了");
        assert_eq!(snap.commits[0].id, head, "日志最上面还得是那条更新的提交");
        assert_eq!(
            snap.status.detached_at.as_deref(),
            Some(first.as_str()),
            "core 认不出分离，界面就没有「分离 HEAD」那一格可写"
        );
        // 只信快照等于让被测对象自己出证词。
        assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), first, "仓库里其实没分离");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 状态栏在分离时说的两句话：HEAD 现在**在哪条提交**上，以及**怎么回去**。
    /// 完整号只印前 8 位（那一排还要放 ↑2 和右边的路径）；两条出口都要钉——
    /// 记得住分支名就给「回到 main」，一开始就没在分支上（空仓库、或刚 clone 就检出）
    /// 就不许给一条点了没反应的假链接。
    #[test]
    fn the_detached_status_bar_names_the_commit_and_offers_a_way_back() {
        let oid = "b43eb9bcd42c1ec86b17875fbea452a0ce0683f9";
        let band = (WIN_H - STATUS_H)..=WIN_H;
        let mut app = loaded();
        let snap = app.snap.as_mut().unwrap();
        snap.status.branch = String::new();
        snap.status.detached_at = Some(oid.to_owned());
        let out = frame(&mut app);
        let det = text_rect(&out, "分离 HEAD · b43eb9bc", band.clone());
        assert!(det.min.x < SIDEBAR_W + 20.0, "分离那一段没贴左：{}", det.min.x);
        let back = text_rect(&out, "回到 main", band.clone());
        assert!(back.min.x > det.max.x, "「回到 main」没接在分离那一段后面");
        // 按**包含**问，不按前缀：那一格的文字是「分离 HEAD · b43eb9bc」，就算把完整号
        // 印进去，整串也不是以号开头的 —— 前缀匹配杀不掉「多印了 32 位」这种改坏。
        assert!(
            texts(&out).into_iter().all(|(s, _)| !s.contains(oid)),
            "不许把 40 位完整号印进状态栏"
        );
        assert!(
            texts(&out).into_iter().all(|(s, _)| !s.contains("(detached)")),
            "git 的内部词汇 `(detached)` 漏进了界面"
        );

        // 第二条出口：没有「最后一次在分支上」那条名字时，那一格整段不画。
        let mut app = loaded();
        app.last_branch = String::new();
        let snap = app.snap.as_mut().unwrap();
        snap.status.branch = String::new();
        snap.status.detached_at = Some(oid.to_owned());
        let out = frame(&mut app);
        assert!(
            text_rect_or_none(&out, "回到", band.clone()).is_none(),
            "认不出该回哪条分支，就别画一条点了没反应的回链"
        );

        // 第三条出口：正常在分支上时也不许出现「回到」——那一格只属于分离。
        let out = frame(&mut loaded());
        assert!(
            text_rect_or_none(&out, "回到", band).is_none(),
            "在分支上却挂着「回到」，用户会以为走错了地方"
        );
    }

    /// 端到端把那一格点掉：真分离 → 帧里出现「回到 main」→ 真点 → 仓库回到分支上、
    /// 快照不再是分离、状态栏那一帧也不再写「分离 HEAD」。少了最后那半截，
    /// 「数据回来了但界面还挂着旧说法」这种改坏杀不掉。
    #[test]
    fn going_back_from_a_detached_head_reattaches_to_the_branch() {
        let dir = scratch_repo("detached-back");
        git_in(&dir, &["reset", "-q", "--hard", "HEAD"]);
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        git_in(&dir, &["add", "b.txt"]);
        git_in(&dir, &["commit", "-q", "-m", "后一条"]);
        let first = git_in(&dir, &["rev-parse", "HEAD~1"]);
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        let mut app = blank();
        app.open(&path);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(2));
        // 分支名按快照里的来，不写死 `main`：不同机器上 `git init` 的默认分支名不一样。
        let branch = app.snap.as_ref().unwrap().status.branch.clone();
        assert!(!branch.is_empty(), "夹具自己得在一条分支上");
        let before = app.snap.as_ref().unwrap().taken_at;
        assert!(app.act(Action::Checkout(first.clone())), "检出没被接住");
        // 等「新快照落地」而不是 `!busy`：同一个理由，见上一条判据里那段注释。
        pump(&mut app, |a| a.snap.as_ref().is_some_and(|s| s.taken_at > before));
        assert!(
            app.snap.as_ref().unwrap().status.detached_at.is_some(),
            "没先分离，下面点的那一格就不存在"
        );

        let label = format!("回到 {branch}");
        let before = app.snap.as_ref().unwrap().taken_at;
        assert!(click_text(&mut app, &label), "状态栏里点不到「{label}」");
        pump(&mut app, |a| a.snap.as_ref().is_some_and(|s| s.taken_at > before));
        let snap = app.snap.as_ref().unwrap();
        assert!(snap.status.detached_at.is_none(), "点了回去，快照还挂着那条号");
        assert_eq!(snap.status.branch, branch, "回去之后分支名要还是原来那条");
        assert_eq!(
            git_in(&dir, &["symbolic-ref", "--short", "-q", "HEAD"]),
            branch,
            "仓库里 HEAD 并没有挂回分支"
        );
        let out = frame(&mut app);
        assert!(
            text_rect_or_none(&out, "分离 HEAD", (WIN_H - STATUS_H)..=WIN_H).is_none(),
            "界面还写着分离 HEAD"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 日志一行的右键菜单把那一步的**代价**写在字里：「检出这条提交（分离 HEAD）」。
    /// 只写「检出这个提交」的话，用户点完发现上面那截没了，只会以为是软件吞了他的历史 ——
    /// 这一格是唯一一处**事前**说清的地方（状态栏那句「回到 main」是事后的）。
    ///
    /// 走一次真点击（不是 `app.act`）：挑菜单里那一项本身就把文案钉住了（换了写法就 panic
    /// 「菜单里没有这项」），而 `intent.run` 那一位要是把 `full` 换成 `HEAD`，这里落下去
    /// 的就还是最新那条 —— 只断「菜单里有没有这行字」杀不掉它。
    #[test]
    fn the_history_menu_says_what_checking_out_a_commit_costs() {
        let dir = scratch_repo("menu-detach");
        git_in(&dir, &["reset", "-q", "--hard", "HEAD"]);
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        git_in(&dir, &["add", "b.txt"]);
        git_in(&dir, &["commit", "-q", "-m", "上面那条"]);
        let first = git_in(&dir, &["rev-parse", "HEAD~1"]);
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        let mut app = blank();
        app.open(&path);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(2));
        assert!(
            app.snap.as_ref().unwrap().status.detached_at.is_none(),
            "夹具自己得先在分支上"
        );
        let before = app.snap.as_ref().unwrap().taken_at;
        menu_pick(&mut app, "起点", "检出这条提交（分离 HEAD）");
        pump(&mut app, |a| a.snap.as_ref().is_some_and(|s| s.taken_at > before));
        let snap = app.snap.as_ref().unwrap();
        assert_eq!(
            snap.status.detached_at.as_deref(),
            Some(first.as_str()),
            "点的是最下面那一行，落下来的却不是那一条提交"
        );
        assert_eq!(snap.commits.len(), 2, "从菜单检出，上面那条一样会消失");
        assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), first, "仓库里没换过去");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ================================================================= 历史页搜索增强
    //
    // 2026-10-26：用户明确说「不需要 SourceTree 那个搜索面板，建议增强提交历史页的搜索，
    // 可以搜文件名」。所以这一批的每一条都写着「这是我们的增强」，不写「补 SourceTree 有的」
    // （PROCESS 2026-10-21 规则②）。SourceTree 那块面板本机没有 app、也没参考图，控件一个数
    // 都没量到 —— 这句在侧栏那段注释里已经写过一次，判据这层再钉一次：文案与档位全是自定。

    /// 侧栏那一行「搜索」点得动，而且点下去换的是**日志条右边那一格**：跳框让位给搜索框。
    ///
    /// 这一行以前是个死桩（点下去只写一句「还没接」的 note）。桩改成真的唯一证据是
    /// note 不再被写、那一格换了人，两个方向都得断：少了「note 没变」，把桩留着也算接上；
    /// 少了「跳框消失」，两格并排画也算接上。
    #[test]
    fn the_sidebar_search_row_swaps_the_jump_box_for_the_search_field() {
        let mut app = loaded();
        let out = frame(&mut app);
        assert!(
            texts(&out).iter().any(|(s, _)| s == JUMP_TO_SHA),
            "开搜之前那格该是跳框「{JUMP_TO_SHA}」"
        );
        assert!(
            !texts(&out).iter().any(|(s, _)| s == SEARCH_HINT),
            "还没点就已经画出搜索框"
        );
        assert!(
            click_text_exact(&mut app, "搜索"),
            "侧栏没有整串写着「搜索」的那一行（`click_text` 的前缀匹配在这里不算证据：\
             「搜索提交…」「搜索中…」都以它开头）"
        );
        assert!(app.search_open, "点了侧栏「搜索」，搜索那一档却没开");
        assert!(
            app.note.is_none(),
            "点下去只写了一句 note：那一行还是个桩 —— {}",
            app.note.clone().unwrap_or_default()
        );
        let out = frame(&mut app);
        assert!(
            texts(&out).iter().any(|(s, _)| s == SEARCH_HINT),
            "开着却没画出搜索框（占位词「{SEARCH_HINT}」）"
        );
        assert!(
            !texts(&out).iter().any(|(s, _)| s == JUMP_TO_SHA),
            "跳框没让位：两格同时挂在日志条右边"
        );
        // 再点一次那一行不该把命中列表抹掉。命中得由线程填，而夹具的 `/fixture` 跑不了 git，
        // 所以这一位只能直接摆上字段——被验的是「第二次点击动不动它」，不是「能不能摆上去」。
        let first = app.snap.as_ref().unwrap().commits[0].clone();
        app.search_hits = vec![SearchHit {
            commit: first,
            badges: Vec::new(),
            date: "今天 10:00".to_owned(),
        }];
        assert!(click_text_exact(&mut app, "搜索"), "第二次点不到那一行");
        assert!(app.search_open, "又点一下那一行，搜索反而关了");
        assert_eq!(
            app.search_hits.len(),
            1,
            "又点一下那一行，上一次的结果被清空了"
        );
    }

    /// 按**改过的文件名**搜到那条提交：整条通路走真的（真敲字 → 真回车 → 真 git → 结果换掉日志）。
    ///
    /// 核心层那四条（`src/git.rs`）杀不掉「壳把结果画错了地方」：比如命中画进了日志那一档，
    /// 于是分页、虚拟行、图序列全跟着来。这一条断的正是画面 —— 命中的在、没命中的不在、
    /// 而且**图序列整个空掉**（结果行与行之间断开了，竖线会连到一条没显示的提交上）。
    #[test]
    fn a_history_search_finds_the_commit_by_the_file_it_touched() {
        let (mut app, dir) = open_repo("search-file");
        // 加一条**只碰 deploy.yaml**、说明里一个「deploy」都没有的提交。
        // 少了这个设计，「搜到了」可能只是拿说明搜到的，文件名那一路根本没生效。
        // `-- deploy.yaml`：夹具里还挂着一条已暂存的 `a.txt` 改动，普通 `git commit` 会把它
        // 一起带走，「只碰 deploy.yaml」就成了假话（同 `clicking_a_search_hit…` 那段）。
        std::fs::write(dir.join("deploy.yaml"), "image: v2\n").unwrap();
        git_in(&dir, &["add", "deploy.yaml"]);
        git_in(&dir, &["commit", "-q", "-m", "改配置", "--", "deploy.yaml"]);
        // `open_repo` 取快照在提交之前，重开一次让夹具自己先有两条（不然「没命中的那条不在」
        // 是空话：日志里本来就没有它）。
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        app.open(&path);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(2));
        assert_eq!(app.snap.as_ref().unwrap().commits[0].subject, "改配置");
        // 夹具生效的最后一眼：说明不含这个词。
        assert!(
            !app.snap.as_ref().unwrap().commits[0]
                .subject
                .to_lowercase()
                .contains("deploy"),
            "夹具自己漏了：说明里就含着搜索词"
        );
        assert!(
            click_text_exact(&mut app, "搜索"),
            "侧栏没有「搜索」那一行，搜索档开不了"
        );
        assert!(
            type_into_and_enter(&mut app, SEARCH_HINT, "deploy"),
            "搜索框没画出来（占位词「{SEARCH_HINT}」）"
        );
        assert_eq!(app.search_q, "deploy", "敲的字没落到绑定上");
        assert!(app.search_open, "回车把搜索档关了");
        pump(&mut app, |a| !a.search_busy && !a.search_hits.is_empty());
        let ids: Vec<String> = app.search_hits.iter().map(|h| h.commit.subject.clone()).collect();
        assert_eq!(ids, vec!["改配置".to_owned()], "命中的不是那条只改了文件的提交：{ids:?}");
        assert!(app.note.is_none(), "搜索路上报了 note：{}", app.note.clone().unwrap_or_default());
        let out = frame(&mut app);
        let painted: Vec<String> = texts(&out).iter().map(|(s, _)| s.clone()).collect();
        assert!(
            painted.iter().any(|s| s.contains("改配置")),
            "命中没画进列表：{painted:?}"
        );
        assert!(
            !painted.iter().any(|s| s.starts_with("起点")),
            "没命中的那条（「起点」）还留在列表里：日志没被命中列表换掉"
        );
        assert!(
            !painted.iter().any(|s| s.starts_with("Uncommitted changes")),
            "搜索开着还画虚拟行：那些改动没进任何提交，按文件名搜出来是骗人的"
        );
        assert!(painted.iter().any(|s| s == "1 条命中"), "状态那一格没报命中数：{painted:?}");
        // 命中行**带着药丸**：分支「main」就停在那条提交上。这一位是在落地那一刻算的账
        // （`Msg::Search` 那一档现算 = 拿「命中数 × 引用数」遍字符串换一次滚动），
        // 画面能问的只有「算出来的药丸有没有跟着画出来」。
        // 只看**日志那一块**：顶栏那枚分支名、侧栏的分支列表、暂存带的「提交到 main」都写着
        // 「main」，光按 x 筛等于没筛（第一版按整帧问、第二版只按 x 问，两版都被 M5 反证是假绿）。
        // 而且药丸的 galley 是「⤴main」——图标和名字在同一块字里，按 `starts_with("main")`
        // 找永远找不到（M5 第一轮就是被这一条糊过去的）。和 3720 行那条判据同一个写法。
        let branch_pill = format!("{}main", pill_glyph(RefKind::Branch));
        let pills: Vec<String> = visible_texts(&out)
            .into_iter()
            .filter(|(_, r)| {
                r.min.x >= SIDEBAR_W + 1.0
                    && r.min.y >= HEADER_H
                    && r.max.y <= WIN_H - STATUS_H - STAGING_H
            })
            .map(|(s, _)| s)
            .filter(|s| s == &branch_pill)
            .collect();
        assert!(
            !pills.is_empty(),
            "命中行没画出引用药丸（落地那一刻没算 `ref_badges`，或药丸没进 `Cells`）"
        );
        let (dots, lines, arms) = graph_marks(&out);
        assert!(
            dots.is_empty() && lines.is_empty() && arms.is_empty(),
            "搜索结果里还画图序列：点 {} 条 / 线 {} 条 / 分叉 {} 条",
            dots.len(),
            lines.len(),
            arms.len()
        );
        // 环单独问一次。第一版只问了 `graph_marks`，反向变异 M3（命中行 `ring: Some(0)`）
        // **照样绿** —— 那枚环是白底 `RING_GREY` 灰边，不在车道色里，`graph_marks` 不收它。
        assert!(rings(&out).is_empty(), "搜索结果里画了虚拟行那枚环：{} 枚", rings(&out).len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 零命中要说出口，而且那一格不许自己关掉。
    ///
    /// 为什么单独一条：列表空了之后 `show_rows` 一个字都不画，画面看着和「搜索坏了」一模一样。
    /// 空态那句写明搜的是**说明和文件名**两路，不然人会怀疑自己敲错了地方。
    /// 「不许自己关」是另一回事：命中为空 ≠ 用户不再想搜了，自动关掉等于把他的词吞了。
    #[test]
    fn a_history_search_with_no_hits_says_so_and_stays_open() {
        let (mut app, dir) = open_repo("search-none");
        assert!(
            click_text_exact(&mut app, "搜索"),
            "侧栏没有「搜索」那一行"
        );
        assert!(
            type_into_and_enter(&mut app, SEARCH_HINT, "这个词一定不存在"),
            "搜索框没画出来"
        );
        pump(&mut app, |a| !a.search_busy);
        assert!(
            app.search_hits.is_empty(),
            "零命中却有了结果：{:?}",
            app.search_hits.iter().map(|h| h.commit.subject.clone()).collect::<Vec<_>>()
        );
        assert!(app.search_open, "零命中把搜索档自动关了");
        assert_eq!(app.search_q, "这个词一定不存在", "零命中把框里的词吞了");
        let out = frame(&mut app);
        assert!(
            texts(&out)
                .iter()
                .any(|(s, _)| s.contains("没有提交命中") && s.contains("这个词一定不存在")),
            "没画出空态那句：{:?}",
            texts(&out).iter().map(|(s, _)| s.clone()).take(40).collect::<Vec<_>>()
        );
        // ✕ 是这一档唯一的出口：点下去搜索整个收回去，跳框回到原位。
        assert!(
            click_text_exact(&mut app, CLOSE_GLYPH),
            "搜索框旁边没有 ✕ 那个出口"
        );
        assert!(!app.search_open, "点了 ✕ 没关");
        assert!(app.search_q.is_empty(), "点了 ✕ 框里的词没清：{}", app.search_q);
        assert!(!app.search_busy, "点了 ✕ 还挂着在途");
        let out = frame(&mut app);
        let painted: Vec<String> = texts(&out).iter().map(|(s, _)| s.clone()).collect();
        assert!(
            painted.iter().any(|s| s == JUMP_TO_SHA),
            "关掉搜索后跳框没回到原位"
        );
        assert!(!painted.iter().any(|s| s == SEARCH_HINT), "搜索框没跟着收回去");
        assert!(!painted.iter().any(|s| s.contains("没有提交命中")), "空态那句还留在画面里");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 点一条命中，打开的是**那条命中**，不是日志里同下标的那一行。
    ///
    /// 两份列表共用一个数字空间（`Sel::Commit(i)` 与 `Sel::Search(i)`、`push_id(r)`），
    /// 撞上时的表现是「点了最下面那行，右边却是最新那条的补丁」—— 一个字都不报错，
    /// 只有人对得上内容才发现。所以这条把下标**故意岔开**：命中第 0 格 = 「起点」，
    /// 日志第 0 格 = 「改配置」。取文件的通路要是指望 `Sel` 那一档，就得靠
    /// `Msg::CommitFiles.search` 那一位带回原样，这里验的就是它。
    #[test]
    fn clicking_a_search_hit_opens_that_commit_not_the_log_row_with_the_same_index() {
        let (mut app, dir) = open_repo("search-index");
        // 那条提交**只碰 deploy.yaml**：`-- deploy.yaml` 是必须的，夹具里还留着一条已暂存的
        // `a.txt` 改动，普通 `git commit` 会把它一起带走，于是「a.txt 只有最旧那条碰过」
        // 这个前提就没了（实测过：命中变成两条）。路径得先 `add`，否则 pathspec 都不认识它。
        std::fs::write(dir.join("deploy.yaml"), "image: v2\n").unwrap();
        git_in(&dir, &["add", "deploy.yaml"]);
        git_in(&dir, &["commit", "-q", "-m", "改配置", "--", "deploy.yaml"]);
        let path = dir.to_str().expect("utf-8 路径").to_owned();
        app.open(&path);
        pump(&mut app, |a| a.snap.as_ref().map(|s| s.commits.len()) == Some(2));
        assert_eq!(
            app.snap.as_ref().unwrap().commits[0].subject, "改配置",
            "夹具：日志第 0 格得是「改配置」"
        );
        // 「a.txt」只有最旧那条（起点）碰过 —— 命中列表第 0 格因此是它。
        assert!(
            click_text_exact(&mut app, "搜索"),
            "侧栏没有「搜索」那一行，搜索档开不了"
        );
        assert!(
            type_into_and_enter(&mut app, SEARCH_HINT, "a.txt"),
            "搜索框没画出来"
        );
        pump(&mut app, |a| !a.search_busy && !a.search_hits.is_empty());
        assert_eq!(app.search_hits.len(), 1, "文件名那一路该只命中「起点」");
        assert_eq!(app.search_hits[0].commit.subject, "起点");
        assert_eq!(
            app.sel,
            Sel::Uncommitted,
            "回车之前选中行是虚拟行，这一步是岔开下标的前提"
        );
        assert!(
            click_text(&mut app, "起点"),
            "命中列表里点不到「起点」那一行"
        );
        assert_eq!(app.sel, Sel::Search(0), "点命中行没落到搜索那一档");
        pump(&mut app, |a| !a.commit_files.is_empty());
        let files: Vec<String> = a_paths(&app);
        assert_eq!(
            files,
            vec!["a.txt".to_owned()],
            "右边文件表不是那条命中的（拿到的是日志第 0 格「改配置」的）：{files:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `commit_files` 里的路径，按原序。判据里到处要「右边那格到底是哪些文件」，
    /// 直接写 `app.commit_files.iter().map(|f| f.path.clone()).collect()` 一行放不下。
    fn a_paths(app: &Gitx) -> Vec<String> {
        app.commit_files.iter().map(|f| f.path.clone()).collect()
    }

    /// 搜索回复那三条闸各钉一次：**号不对 / 仓库不对 / 词改了** 必须丢，三位都对必须落。
    /// 少了任何一条的表现都是「搜 A 出 B」：上一次那一下回车的结果画成这一次的答案，
    /// 或者上一本的命中盖到这一本上。
    ///
    /// 为什么直接往 `tx` 里塞回复而不是真跑两次 git：在途竞态无头钉不住（谁先落地不由人），
    /// 但闸读的就是回复自带的那三位 —— 塞进去和 worker 发回来走的是同一条 `land` 码路。
    #[test]
    fn a_stale_search_reply_is_dropped_and_a_matching_one_lands() {
        let (mut app, dir) = open_repo("search-stale");
        app.search_q = "配置".to_owned();
        // 「在途」这块牌子上一次是 `run_search` 立的，这里手动摆上（`open_repo` 没搜过东西）。
        // 摆它是因为**丢掉**的回复不许把牌子摘掉 —— 那会让状态格在真正的回复落地之前
        // 就从「搜索中…」跳到「没搜到」。
        app.search_busy = true;
        let ok = app.snap.as_ref().unwrap().commits[0].clone();
        let here = app.repo.clone().unwrap();
        let reply = |seq: u64, repo: &str, query: &str| Msg::Search {
            repo: repo.to_owned(),
            seq,
            query: query.to_owned(),
            commits: vec![ok.clone()],
        };
        app.tx.send(reply(app.search_seq + 1, &here, "配置")).unwrap();
        frame(&mut app);
        assert!(app.search_hits.is_empty(), "对不上号的回复被采纳了（又回车一次就该丢旧的）");
        app.tx.send(reply(app.search_seq, "/另一本仓库", "配置")).unwrap();
        frame(&mut app);
        assert!(app.search_hits.is_empty(), "别的仓库的在途回复盖到了这一本上");
        app.tx.send(reply(app.search_seq, &here, "配置改")).unwrap();
        frame(&mut app);
        assert!(app.search_hits.is_empty(), "框里词已经改了，旧词的回复还被当成答案画出来");
        assert!(app.search_busy, "丢掉的回复把「在途」那一位解了：状态格会提前不转");
        app.tx.send(reply(app.search_seq, &here, "配置")).unwrap();
        frame(&mut app);
        assert_eq!(app.search_hits.len(), 1, "三位都对的那一条反而被丢了");
        assert!(!app.search_busy, "落地了却还挂在在途");
        let _ = std::fs::remove_dir_all(&dir);
    }



