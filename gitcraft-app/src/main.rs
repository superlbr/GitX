//! GitCraft 的 egui 界面层：SourceTree 主窗口的壳，接的是**真数据**。
//!
//! 这一层和 `gitcraft-core` 之间**没有 IPC 那一跳**——git 函数直接调（旧 Tauri 层才有那一跳）。
//! 布局几何照 `docs/reference/sourcetree-4.2.19-repo-window.png` 量出来的数字写
//! （见 PROCESS.md「复刻规格：SourceTree 主窗口实测几何」），常量名就是图上那四块边界。
//! 页只有一页：上面日志，下面「文件列表 | 差异」；选日志第一行那个虚拟行就是工作区。

mod fonts;

use chrono::Datelike;
use eframe::egui::{self, Align, Align2, Color32, FontId, Label, Margin, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, Vec2, pos2, vec2};
use gitcraft_core::models::{Branch, ChangeStatus, Commit, DiffEntry, FileStatus, Remote, RemoteRef, Repo, RepoStatus, Stash, Tag};
use gitcraft_core::rusqlite::Connection;
use gitcraft_core::{db, git, provider, GitcraftError};
use std::sync::mpsc::{channel, Receiver, Sender};

// ---- 从参考图量出来的几何（pt = @2x 截图的像素 / 2）----
/// 标题栏那一行的高度。**两张参考图都是 52**：底色 244,249,246 从 y=0 一路到 y=51，
/// y=52 是一条 202,206,203 的分隔线，y=53 起换成筛选条那条带（240,244,241）。
/// 原来记的 55 是错的（当时按"看着像"记的，没逐行扫底色）。
/// 这一行在 SourceTree 里**就是 macOS 标题栏**：红绿灯和仓库名和那 11 枚按钮同在一行，
/// 所以我们必须把窗口配成"内容铺满标题栏"，而不是在它下面再画一条（见 `viewport`）。
const TOOLBAR_H: f32 = 52.0;
/// 红绿灯：三枚 12pt 圆，中心 x=26/46/66（实测 x 20..32、40..52、60..72），y 20..32。
/// 它们是操作系统画的，盖在我们内容上面 —— 所以这一行的左边 72pt **必须留空**。
#[cfg(test)]
const TRAFFIC_RIGHT: f32 = 72.0;
/// 仓库名「comic (Git)」的左沿实测 **91.5pt**（红绿灯右沿 72 + 19.5 间距）。
/// 只给判据看：判据要钉的是**量来的数**，不是产品里取整后的那个常量。
#[cfg(test)]
const TITLE_LEFT: f32 = 91.5;
/// 上面那个 91.5 落到帧上：egui 0.37 的 `Margin` 四个边是 **i8 整点**，给不了半像素，所以取 92。
const TITLE_INSET: i8 = 92;
/// 最右一枚按钮（设置）的右沿实测 1099 / 窗口 1120 → 右边距 **21pt**。
const TOOLBAR_RIGHT: i8 = 21;
/// 仓库 tab 条。SourceTree 把书签放在**另一个窗口**里，这个壳只有一个窗口，
/// 所以书签列表摊成工具栏下面的一行 tab（2026-10-20 用户要求）。
const TAB_H: f32 = 24.0;
/// 顶部两条带加起来吃掉的高度。产品代码不需要它（面板自己会叠），
/// 但判据要按这条线把「tab 条以下」的带子和文字筛出来，所以只给测试看。
#[cfg(test)]
const TOP_H: f32 = TOOLBAR_H + TAB_H;
const SIDEBAR_W: f32 = 141.0;
const STAGING_H: f32 = 245.0;
/// 文件表底下那格提交框占的高度：摘要 22 + 描述 3 行 + 按钮行 22（行距 0，控件之间的
/// `item_spacing.y` 也是 0）。**这是自己定的预算，不是参考图量来的**——参考图那张
/// 提交框只露出摘要一行，量不到三行描述该多高。它的用处只有一个：文件表在上面滚，
/// 提交框钉在带底（见 `the_commit_box_stays_inside_a_short_band`）。
const COMMIT_H: f32 = 96.0;
const FILES_W: f32 = 435.0;
/// 窗口能缩到多小。**870 是量出来的，不是拍的**：无头扫 600→1050（步长 10），
/// 860 那一格还剩 1 处撞（仓库名压 ⊕），870 起每一格 0 处重叠，两个高度（321 / 545）同一条线。
/// 这条线从 #30 的 770 抬到了 870：**代价是标题栏合并** —— 左边 92pt 让给红绿灯、
/// 右边 21pt 是按钮余量，等于把可用宽度砍掉 104pt，而名字和 11 枚按钮谁都不肯让。
/// SourceTree 不会撞，因为 `NSToolbar` 装不下会把条目收进 » 溢出菜单、标题用「…」截断；
/// 我们还没有那套，所以先抬地板，**截断让位排在 #35**（那批要重排这一行的左组）。
/// 高 321 = 两条顶带（52+24）+ 暂存带 245：再矮，底部面板被夹扁、文件表只剩 40pt 的地板。
const MIN_W: f32 = 870.0;
const MIN_H: f32 = TOOLBAR_H + TAB_H + STAGING_H;
/// 参考图（1050pt 宽那张）数据行的行距：130→156→182→210，**26pt**。
/// 我们原来用 21，整张日志因此比 SourceTree 矮一截、一屏多塞五行。
const ROW_H: f32 = 26.0;
const DIFF_ROW_H: f32 = 15.0;
/// 表头那一条：参考图 y 79..103 = 24pt（比数据行矮 2）。
const HEADER_H: f32 = 24.0;
/// 表头列边上的分隔竖线：宽 1pt、高 16pt、`(229,229,229)`，四条都在列边界上。
/// 之前判定「表头没有竖线」是量错了带——线只有 16pt 高，用整带阈值扫会被背景盖掉。
const RULE_H: f32 = 16.0;
const RULE: Color32 = Color32::from_rgb(229, 229, 229);
/// 每列文字离自己列左边线的距离。参考图四个表头词的左缘：提交 202.5、描述 280、
/// 作者 744、日期 934——列边界分别 198 / 275 / 739 / 929，都差 5pt。
const COL_INSET: f32 = 5.0;
/// 四列的宽度全部照那张 1050pt 宽的参考图量：边线在 198 / 275 / 739 / 929。
/// 描述列（275..739 = 464）是弹性的，所以只要这三条定死，剩下那条自然对上。
const COL_GRAPH: f32 = 57.0;
const COL_SHA: f32 = 77.0;
const COL_AUTHOR: f32 = 190.0;
const COL_DATE: f32 = 121.0;
const NO_W: f32 = 30.0;
/// 图序列的车道几何：第 0 道的**中心**在列左缘 +22.75（参考图量圆点 239..246.5，
/// 列从 220 起），往后每 12pt 一道；57pt 的列装得下 4 道（见 `layout_graph` 裁尾巴那段）。
const LANE_W: f32 = 12.0;
const GRAPH_PAD: f32 = 16.75;
/// 参考图实测（logical pt，@2x 量的）：圆点直径 8.0 → 半径 4.0，竖线宽 2.0。
/// 之前那对 2.6 / 1.3 是旧 React 版留下的，比参考图小一圈——同一张截图并排一看就露出来。
const DOT_R: f32 = 4.0;
const LINE_W: f32 = 2.0;
/// 虚拟行那枚空心环：外直径 10.0、环宽 2.5（也就是内孔 5.0）、中性灰 `#929291`。
/// 环的半径是 `DOT_R + 1.0`：外沿比圆点大一圈，内孔正好把圆点那一格空出来。
const RING_W: f32 = 2.5;
const RING_GREY: Color32 = Color32::from_rgb(0x92, 0x92, 0x91);
/// 窗口内容区 = 参考图 2100×1090 像素 @2x。四块带的断言以此为准。
const WIN_W: f32 = 1050.0;
const WIN_H: f32 = 545.0;
/// 侧栏最底那条「筛选器」占的高度：`add_space(4)` + 那条输入框。输入框实际拿的是全局
/// `interact_size.y`，而那个值就是 `ROW_H`（见 `style()`），所以这条跟着行高走——
/// 行高从 21 改成 26 时这里没跟着改，侧栏的底色矩形就超出窗口底 6pt（`bands_match_...` 抓到）。
const SIDE_FILTER_H: f32 = 4.0 + ROW_H;
/// 工具栏那条开发期提示占的宽度。1050pt 的窗口里右侧 11 枚按钮要 473pt，
/// 左边仓库名 + 分支 + 书签下拉 + 添加仓库再要 ~280pt，留给提示的就只剩这一格。
const NOTE_W: f32 = 240.0;
/// 侧栏内容框离面板左缘的距离（`Frame::inner_margin`）。下面那几个数全是**面板坐标**，
/// 画的时候减去它才是行内偏移——这样常量的名字就等于参考图上量到的那个位置。
const SIDE_INSET: i8 = 5;
/// 侧栏文字列的左缘。参考图四组标签——「WORKSPACE」69..86、「分支」198..207、
/// 「main」230..240、「文件状态」103..114——**全部**起在 x=41：分类不缩进，子项也不缩进，
/// 图标/圆点住在 18..38 那一格里。我们原来是「标题 5、子项 20」两档缩进，跟参考图不是一回事。
const SIDE_TEXT_X: f32 = 41.0;
/// 图标格 18..38 的中心。
const SIDE_ICON_X: f32 = 28.0;
/// 分类行（分支/标签/远端/已贮藏）的行距：参考图 202.5→275.5→322.5→369.5→416.5→463.5，
/// 五段全是 **47pt**。我们原来所有侧栏行都是 21pt 一条，整栏挤成一坨。
const SIDE_CAT_H: f32 = 47.0;
/// 「工作区」那一条：它到自己第一条子项是 30.5pt。
const SIDE_HEAD_H: f32 = 30.0;
/// 选中子项的灰色药丸：参考图 x 10..129.5、高 25.5、圆角 5。子项行高就是 `ROW_H`。
const SIDE_PILL_L: f32 = 10.0;
const SIDE_PILL_R: f32 = 129.5;
const SIDE_PILL_BG: Color32 = Color32::from_rgb(204, 208, 205);
/// 图标是 (69,81,108) 的深蓝灰线描，分类标签是 (164,167,165) 的浅灰，子项文字 (61,62,61)。
/// 三个都是**最暗像素**，也就是颜色的上限（细笔画在 @2x 上会被抗锯齿摊薄）。
const SIDE_ICON_C: Color32 = Color32::from_rgb(69, 81, 108);
const SIDE_LABEL_C: Color32 = Color32::from_rgb(164, 167, 165);
const SIDE_INK: Color32 = Color32::from_rgb(61, 62, 61);
/// 当前分支前面那颗点：参考图圆心 x≈29.5、直径 7、`(36,93,184)`。**蓝**，不是我们原来用的黑。
const SIDE_DOT_X: f32 = 29.5;
const SIDE_DOT_R: f32 = 3.5;
const SIDE_DOT_C: Color32 = Color32::from_rgb(36, 93, 184);
/// 描述列前面那枚药丸的几何。高 15 / 上下各留 2.5 是参考图量的（行带 129..149，药丸 131.5..146.5）；
/// 两枚浅蓝药丸（`upstream/main` 357.5..458、`origin/main` 458.5..538.5）之间只隔 2pt，
/// 而「超前 3 个版本」那枚深藏青是**贴死**在前一枚右边的（403 / 403.5）——它俩合成一颗胶囊。
/// 药丸结束后描述文字再让 5pt（药丸右缘 476、文字起笔 481）。圆角取 4。
const PILL_H: f32 = 15.0;
const PILL_PAD: f32 = 5.0;
const PILL_GAP: f32 = 2.0;
const PILL_AFTER: f32 = 5.0;
const PILL_R: u8 = 4;
const PILL_FONT: f32 = 11.0;

// 亮色是 SourceTree 的实际观感：侧栏比内容区暗一格，选中行整行蓝底白字。
const SIDEBAR_BG: Color32 = Color32::from_rgb(0xf1, 0xf1, 0xf3);
const TOOLBAR_BG: Color32 = Color32::from_rgb(0xf7, 0xf7, 0xf9);
const SELECTED: Color32 = Color32::from_rgb(0x1c, 0x6c, 0xf0);
const HOVER: Color32 = Color32::from_rgb(0xe6, 0xe9, 0xef);
/// 增/删两行的整行底色，量自 `sourcetree-4.2.19-repo-window-2026-10-20.png`
/// 的 y 488..502（删）与 504..518（增）。之前那对 `#ddf3e2` / `#fbdcdc` 是旧 React 版抄的，
/// 比参考图深一档、还偏冷。
const ADD_BG: Color32 = Color32::from_rgb(232, 253, 242);
const DEL_BG: Color32 = Color32::from_rgb(255, 238, 234);
/// hunk 头那一行的底色：同张图 y 409..438，`(240,244,241)`。原来那枚淡紫 `#e8eaf0` 是拍的。
const HUNK_BG: Color32 = Color32::from_rgb(240, 244, 241);
const MUTED: Color32 = Color32::from_rgb(0x86, 0x8a, 0x96);
/// 日志四列的字色。参考图量的：提交号、作者、日期、描述四处都是 `(38,38,38)`——**没有**
/// 我们原来给提交号/日期/作者用的那种浅灰。SourceTree 不靠颜色分主次，只靠列宽和字重。
const INK: Color32 = Color32::from_rgb(38, 38, 38);
/// 虚拟行（未提交那一行）的描述比其余行更深一档，参考图量的是 `(33,33,33)`。
/// 它不是从主题的 `strong_text_color` 来的——那个值跟字体字重绑在一起，而这里只有
/// Regular 一个字面，`strong()` 只能改颜色，所以颜色得自己钉。
const INK_STRONG: Color32 = Color32::from_rgb(33, 33, 33);
const LINE_NO: Color32 = Color32::from_rgb(0xa8, 0xac, 0xb6);
const WARN: Color32 = Color32::from_rgb(0xb0, 0x3a, 0x2e);
// 描述列那几枚 ref 药丸。三个值都是在参考图上量的（`docs/reference/…-repo-window-2026-10-20.png`，
// 第一行 `⑂ main` 的浅蓝底取 (213,229,253)、描边取 (170,170,170)、`超前3个版本` 的藏青底取 (0,57,121)）。
const PILL_BG: Color32 = Color32::from_rgb(0xd5, 0xe5, 0xfd);
const PILL_EDGE: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);
const PILL_AHEAD_BG: Color32 = Color32::from_rgb(0x00, 0x39, 0x79);
/// 标签药丸**没有**参考图依据：那张截图的仓库里一个标签都没有。这里的浅绿只是
/// 「和分支区分得开」，不是量出来的。哪天拿一张带标签的参考图来，替换掉这一行。
const PILL_TAG_BG: Color32 = Color32::from_rgb(0xe4, 0xf2, 0xdc);
/// 虚拟行「Uncommitted changes」那条的底色，参考图量的是 (220,220,220)。
/// 它不是选中态——选中是整行蓝，这一行是**不选也灰**。
const UNCOMMITTED_BG: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);

// ---- 文件状态那枚色块。三个颜色全是参考图量来的：
// 改动 = 琥珀底白 `…`(238,197,91)（repo-window 那张 680 个像素）、
// 重命名 = 玫瑰底白 `R`(163,115,122)（同张 242 个像素）、
// 未跟踪 = 紫底白 `?`(101,73,129)（只在 2026-10-20 那张里出现，302 个像素）。
// 方块 **14pt**：琥珀色块在两张图里都是 14×14（差异带头 x585..599、x665..678；
// 文件表里那枚 x190..205），紫的那枚量到 11~13——按 14 统一，宁可略大不凭印象配颜色。
// 增/删/冲突那三种**参考图里没有**，一律落灰底。
const BADGE_MOD: Color32 = Color32::from_rgb(238, 197, 91);
const BADGE_REN: Color32 = Color32::from_rgb(163, 115, 122);
const BADGE_UNK: Color32 = Color32::from_rgb(101, 73, 129);
const BADGE_NONE: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
const BADGE_S: f32 = 14.0;
/// 文件表那一格的四档落点。参考图（`sourcetree-4.2.19-repo-window.png`，面板左缘那条
/// 1pt 分隔线在 x=140，往右 141 起算）实测：分类行方框 **x 149**、文件行方框 **x 165**、
/// 状态图标 **x 188**、文件名 **x 208.5** —— 也就是离面板左缘 8 / 24 / 47 / 67.5。
/// 方框与图标都是 **13.5 见方**（三枚量到一模一样）。
///
/// 文件行那三列我们**往左挪了两回，一共 17pt**（25→8、47→30、68→51，列间距一个没动）：
/// 用户 2026-10-21 先说「变动文件列表，内容往左移动一点」（挪了 8），看了真窗口又说
/// 「文件改动列表内容需要左移一点」，于是再挪 9，**方框这一档与分类行的方框齐平**。
/// 8pt 是地板：它就是参考图那条面板左缘分隔线到方框的距离，再小就贴到线上去了。
/// 这两次都是**明确偏离 SourceTree 的自加调整**，不是复刻，别在文档里写成"量来的"。
const CHK_S: f32 = 13.5;
const CHK_HEAD_X: f32 = 8.0;
const CHK_ROW_X: f32 = 8.0;
const ICON_X: f32 = 34.0;
const NAME_X: f32 = 51.0;
/// 勾上的方框：参考图那三枚中心色都是 (71,141,247)（边缘被抗锯齿拉到 (54,122,246)）。
const CHK_BLUE: Color32 = Color32::from_rgb(71, 141, 247);
/// 没勾上那面**没有样本可抄**——两张参考图里的方框全是勾上的。白底 + 1pt 灰边是估的，
/// 等真窗口目视（#25）再定，别把它当成量到的数。
const CHK_EDGE: Color32 = Color32::from_rgb(190, 190, 190);
/// 差异区那格文件标题的底色带：两张参考图都是 **(236,236,236) 高 30pt**
/// （一张 y338..368、一张 y378..408），色块离带左缘 8pt（585-577、665-657）。
const DIFF_HEAD_BG: Color32 = Color32::from_rgb(236, 236, 236);
const DIFF_HEAD_H: f32 = 30.0;
/// 色块离差异带头左缘的距离，两张图都是 8pt。
const DIFF_HEAD_PAD: f32 = 8.0;
/// 文件表里「当前文件」那一行的灰带。**(220,220,220) 量的是 2026-10-20 那张日志区
/// 第一条选中行（y109..128，横贯整行）**——文件表那两张参考图都没有选中行，所以这一格
/// 是「同一套未聚焦选中色搬到文件表」，不是直接在文件表里量到的。
/// 有焦点时 SourceTree 用蓝 (0,100,225)（同张 y547..569 量到）。焦点态留给键盘那一批。
const PICK_BG: Color32 = Color32::from_rgb(220, 220, 220);
/// 差异区「块 N： 行 A-B」那一行右边的两枚小按钮，全部量自
/// `sourcetree-4.2.19-repo-window-2026-10-20.png`：带子 y 409..438（**29pt**，底色
/// `(240,244,241)`，左缘 x 656），两枚按钮 x 977..1032、1041..1096，各 **55×15**、
/// 中间隔 **9pt**、右缘离带右缘 9pt（1096 vs 1105）。边框 `(228,232,229)`、字色 `(109,110,109)`。
const HUNK_BTN_W: f32 = 55.0;
const HUNK_BTN_H: f32 = 15.0;
const HUNK_BTN_GAP: f32 = 9.0;
const HUNK_BTN_EDGE: f32 = 9.0;
const HUNK_BTN_STROKE: Color32 = Color32::from_rgb(228, 232, 229);
const HUNK_BTN_FG: Color32 = Color32::from_rgb(109, 110, 109);

/// 模态卡那一格的尺寸。**没有参考图依据**：两张 SourceTree 截图都没拍到弹窗，
/// 这几个数是为了「卡片里的行全是我们手摆的，高度必须自己算才能居中」而定的固定档位，
/// 等目视那一轮（#25）再对着真弹窗调。
const MODAL_W: f32 = 380.0;
const MODAL_TITLE_H: f32 = 30.0;
const MODAL_ROW_H: f32 = 26.0;
const MODAL_FOOT_H: f32 = 34.0;
// 参考图里那一行 hunk 头是**双倍行高**（29pt = 两条正文行）。这一版没跟上：差异区走的是
// `ScrollArea::show_rows`，它只认一个统一行距（正文行量的也是 15~16pt，见 `DIFF_ROW_H`），
// 逐行高度得自己按累计偏移做可见区间。所以按钮按 15pt 画，29 这条记在 PROCESS.md 等替换。

// 一帧里**行闭包实际跑了几次**（日志行 + 差异行）。为什么产品代码里要有这个数：
// `FullOutput.shapes` 只有细分后的图形，egui 在细分阶段按 clip_rect 把屏幕外的行剪掉了，
// 于是「把 4000 条提交全排一遍」和「只排视口那 9 行」落图完全一样——布局开销才是虚拟化的
// 判据，而它不落在图上。每帧在 `shell()` 开头清零，无头门禁读它。
thread_local! {
    static ROWS_PAINTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn bump_rows_painted() {
    ROWS_PAINTED.with(|c| c.set(c.get() + 1));
}

// 图序列**在所在那个线程上**排了几次。为什么是 thread_local 而不是全局：
// 排布局的活儿归 worker 线程，帧归主线程——门禁要问的是「主线程排了几次」，
// 全局计数会把两边混成一个数（也会被并行跑的别的测试污染）。
// 生产路径的正确形状是：worker 排 1 次、主线程 0 次，而 snapshot 里已经有图序列。
thread_local! {
    static GRAPH_LAYOUTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn bump_graph_layouts() {
    GRAPH_LAYOUTS.with(|c| c.set(c.get() + 1));
}

// 引用→提交行的匹配（`ref_badges`）排了几次。和 `GRAPH_LAYOUTS` 同一个道理：
// 那是「提交数 × 引用数」遍字符串比较，帧里每帧重算一遍是白给的开销。
// 判据 `the_frame_never_matches_refs_to_rows` 读的就是这个数——跑一帧，它必须还是 0。
thread_local! {
    static BADGE_LAYOUTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn bump_badge_layouts() {
    BADGE_LAYOUTS.with(|c| c.set(c.get() + 1));
}

/// 车道配色。**第 0 道是量出来的**：参考图那条线性历史全是 `#005FBD`（0,95,189），
/// 而旧 React 版那圈调色板（`src/graph.ts`，随 Tauri 层一起删了，还能在 git 历史里翻到）
/// 第一枚是紫色 `#7C5CFC`——并排一比就知道差在哪儿。后面七道**没有依据**：那张截图里
/// 只有一条道，多车道的配色顺序要等一张有分叉的参考图来钉。
/// 别随手改：按**颜色**认领图元的那条判据（`graph_marks`）认的就是这八种。
const LANE_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x00, 0x5f, 0xbd),
    Color32::from_rgb(0x3f, 0xb9, 0x50),
    Color32::from_rgb(0xf8, 0x51, 0x49),
    Color32::from_rgb(0xd2, 0x99, 0x22),
    Color32::from_rgb(0x5a, 0x8b, 0xff),
    Color32::from_rgb(0xdb, 0x61, 0xa2),
    Color32::from_rgb(0x2d, 0xd4, 0xbf),
    Color32::from_rgb(0xe3, 0xb3, 0x41),
];

fn lane_color(lane: usize) -> Color32 {
    LANE_COLORS[lane % LANE_COLORS.len()]
}

/// 工具栏：图标在上、中文在下，右对齐 11 个（顺序与说明文案照抄参考图和 4.2.19 的 nib）。
/// **图标只能是字体链里真有字形的那些**：`every_rune_the_shell_paints_has_a_glyph`
/// 这条无头判据会把每一枚落过字的码位跑一遍。之前那批箭头、带圈字母和两个彩色 emoji
/// 全都不在 Noto Sans SC 的 cmap 里，于是参考图里十一个清楚的图标，在我们这儿有七个是豆腐块——
/// 这不是配色问题，是根本没字。
const TOOLS: [(&str, &str, &str); 11] = [
    ("⊕", "提交", "把暂存的变更提交到当前分支"),
    ("↓", "拉取", "从远端拉取提交"),
    ("↑", "推送", "推送提交到远端"),
    ("⇄", "抓取", "从远端获取对象，不动工作区"),
    ("⤴", "分支", "创建分支"),
    ("⤵", "合并", "把分支合并到当前分支"),
    ("≡", "贮藏", "把工作区暂存起来"),
    ("☁", "远程服务器", "在浏览器里打开远端"),
    ("▥", "Finder", "在 Finder 中打开仓库目录"),
    ("▣", "终端", "在终端里打开当前目录"),
    ("⊙", "设置", "GitCraft 设置"),
];

/// 日志里选中的行。虚拟行「Uncommitted changes」就是工作区页——SourceTree 只有一页。
#[derive(Clone, PartialEq)]
enum Sel {
    Uncommitted,
    Commit(usize),
}

/// 一次差异请求。带 `path` 是单文件，不带是整份。
#[derive(Clone, PartialEq)]
enum Target {
    Working { staged: bool, path: Option<String>, untracked: bool },
    Commit { oid: String, path: Option<String> },
}

enum Action {
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
struct Modal {
    title: &'static str,
    /// 标题下面那行说明（「打在：修掉 xxx」）。没有就 `None`。
    detail: Option<String>,
    /// 文本框里正在打的字。`None` = 这张卡没有输入框，只有一排现成的选择。
    text: Option<String>,
    form: Form,
}

/// 卡要办的事。**里面带的数据是打开那一刻从快照算好的**：卡开着的时候后台快照可能刷新，
/// 动作不该跟着漂（「在这条提交上打标签」不能因为日志翻页就换了一条提交）。
enum Form {
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
    fn new_branch() -> Self {
        Self::new_branch_at(None)
    }

    /// `start` = 从哪条提交长出去；从分支行/日志行的菜单里进来时带的是那条的 oid。
    fn new_branch_at(start: Option<String>) -> Self {
        Modal {
            title: "新建分支",
            detail: start.as_ref().map(|s| format!("从 {s} 长出去")),
            text: Some(String::new()),
            form: Form::NewBranch { start },
        }
    }

    fn stash_save() -> Self {
        Modal {
            title: "贮藏改动",
            detail: Some("贮藏会把工作区和暂存区都收起来，之后回到当前分支".to_owned()),
            text: Some(String::new()),
            form: Form::StashSave { include_untracked: false },
        }
    }

    fn new_tag(oid: String, subject: String) -> Self {
        Modal {
            title: "新建标签",
            detail: Some(format!("打在：{subject}")),
            text: Some(String::new()),
            form: Form::NewTag { oid },
        }
    }

    fn merge(branches: Vec<String>) -> Self {
        Modal { title: "合并到当前分支", detail: None, text: None, form: Form::Merge { branches } }
    }

    /// 克隆那张卡：第一格是源地址（`text`），第二格是目标目录（表单自己带）。
    /// 目标目录留空 = 主按钮按着不让你点，而不是替你猜一个 `~/xxx`。
    fn new_clone() -> Self {
        Modal {
            title: "克隆仓库",
            detail: Some("填相对路径时按主目录算".to_owned()),
            text: Some(String::new()),
            form: Form::Clone { dest: String::new() },
        }
    }

    fn confirm(title: &'static str, detail: String, then: Action) -> Self {
        Modal { title, detail: Some(detail), text: None, form: Form::Confirm { then: Some(then) } }
    }

    /// 文本框的占位文案，也是「这一格该填什么」的唯一说法。
    fn hint(&self) -> &'static str {
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
    fn rows(&self) -> usize {
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
    fn primary(&self) -> &'static str {
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
enum RefKind {
    Branch,
    Remote,
    Tag,
    Ahead,
}

/// 一枚药丸：`label` 是**不含前缀图标**的文字（图标由 `kind` 决定，见 `pill_glyph`）。
#[derive(Clone, PartialEq, Debug)]
struct RefBadge {
    label: String,
    kind: RefKind,
}

struct Snapshot {
    status: RepoStatus,
    commits: Vec<Commit>,
    branches: Vec<Branch>,
    tags: Vec<Tag>,
    remotes: Vec<Remote>,
    /// 远程跟踪指针（`origin/main` 那一类）。**留着**是因为下一页接上来时要把整列药丸重算，
    /// 而 `ref_badges` 没有它就认不出「超前/落后」。一份仓库也就几条，不值一次重取。
    remote_refs: Vec<RemoteRef>,
    stashes: Vec<Stash>,
    /// 与 `commits` 一一对应的图序列行。**在 worker 线程上排好**，帧里只管画。
    graph: Vec<GraphRow>,
    /// 与 `commits` 一一对应的药丸列表。同样在 worker 上算：oid 相等判断要跑
    /// 「提交数 × 引用数」遍，帧里每帧重算是白给的开销（判据：`the_frame_never_matches_refs`）。
    badges: Vec<Vec<RefBadge>>,
    /// 与 `commits` 一一对应的日期文案（「今天 12:27」这种）。参考图的日期列不是 ISO 串，
    /// 而且**当天**的提交只给时间；换算要拿「现在」当参照，那份「现在」就是 `taken_at`。
    dates: Vec<String>,
    /// 这份快照拍下的时刻。日期文案与虚拟行那个「今天 HH:MM」都以它为基准，
    /// 所以同一份快照里刷多少帧，日期都不会自己跳。
    taken_at: chrono::DateTime<chrono::Local>,
}

enum Msg {
    Loaded { repo: String, snap: Box<Snapshot> },
    /// 日志的下一页。带 `seq`：切仓库/换排序之后旧页作废，不能往新列表尾巴上接。
    More { repo: String, seq: u64, commits: Vec<Commit> },
    CommitFiles { repo: String, index: usize, files: Vec<DiffEntry> },
    Patch { repo: String, seq: u64, label: String, lines: Vec<DiffLine> },
    Failed(String),
    /// 克隆回来的只有「成了在哪」或者「为什么没成」，没有快照——成了之后由 `add_repo`
    /// 自己去开那个仓库（它会再发一次 `Msg::Loaded`）。
    Cloned(std::result::Result<String, String>),
}

pub struct Gitcraft {
    conn: Option<Connection>,
    repos: Vec<Repo>,
    /// 当前仓库路径，取自 `repos` 里的 `path`。
    repo: Option<String>,
    snap: Option<Snapshot>,
    sel: Sel,
    commit_files: Vec<DiffEntry>,
    file: Option<String>,
    /// 差异已经在工作线程排好版（行号、类型），帧里只管画。
    diff: Vec<DiffLine>,
    patch_label: String,
    /// 差异请求序号：快速连点两个文件时，只有最新那次的回复能落地。
    seq: u64,
    busy: bool,
    note: Option<String>,
    log_filter: String,
    side_filter: String,
    /// 日志排序方式，即筛选条上那枚「层级顺序」：`false` = 日期序，`true` = 拓扑序。
    topo: bool,
    /// 日志范围：`None` = 不给 git 传 rev（就走 HEAD 那条线），`Some(分支名)` = 只看那条分支。
    /// 「所有分支」那枚弹出按钮改的就是它。
    log_rev: Option<String>,
    /// 「显示远程分支」：只管日志行上**远程药丸画不画**，不动取列表的参数。
    show_remote: bool,
    /// 日志还有下一页吗。上一次取回不满一页就是到底了，别再问。
    log_more: bool,
    /// 下一页在路上。不立这个牌子，滚到底的那几帧会连着发好几遍同一个请求。
    log_more_busy: bool,
    /// 分页的序号：`open` 一次加一，回复对不上号就丢掉（切了仓库/换了排序，旧页作废）。
    log_seq: u64,
    /// 文件表下方那格提交信息。放在壳里而不是控件里有两个理由：提交成功后要由回复来清空
    /// （见 `clear_msg_on_load`），以及无头门禁要能直接读它。
    /// SourceTree 那格是**两个框**：摘要 + 描述。落进 git 是一条 `-m`，中间隔一个空行。
    commit_msg: String,
    commit_desc: String,
    /// 下一次 `Msg::Loaded` 落地时要不要清空 `commit_msg`——只有提交那一个动作会置上。
    clear_msg_on_load: bool,
    /// 顶部那格「添加仓库」的路径输入。和 `commit_msg` 一样放在壳里而不是控件里：
    /// 无头门禁要能直接读它，而且清空/收起的时机由动作决定。
    add_open: bool,
    add_path: String,
    /// 当前差异区显示的是哪一份补丁。hunk 那两枚按钮要问的就是这个：同一块在第 1 格
    /// 和第 2 格里意思不同（未暂存 → 「暂存区块」，已暂存 → 「取消暂存区块」，
    /// 某条提交 → 根本没有区块可暂存，按钮不出现）。
    target: Option<Target>,
    /// 「放弃区块」按了第一次，正在等第二下确认。放弃会**真的丢掉工作区那几行**，
    /// 而 SourceTree 的那层确认是一个我还没画过、也没目视过的弹窗，所以先不照抄它：
    /// 第一次只把按钮换成「确认放弃」，仓库一个字节都不动（判据见
    /// `a_discard_needs_two_clicks`）。换差异、换仓库、按 Esc 都会取消。
    pending_discard: Option<usize>,
    /// 那张模态卡（见 `Modal`）。`None` = 没有卡，界面层照常可点。
    modal: Option<Modal>,
    /// 卡刚打开的那一帧要把光标送进文本框；之后再抢焦点就该由用户自己决定。
    modal_focus: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Gitcraft {
    fn new() -> Self {
        let (tx, rx) = channel();
        let mut conn = db_path().and_then(|p| match db::open(&p) {
            Ok(c) => {
                let _ = db::migrate(&c);
                Some(c)
            }
            Err(e) => {
                eprintln!("gitcraft-app: 打不开数据库 {p}: {e}");
                None
            }
        });
        let repos = conn.as_mut().and_then(|c| db::list_repos(c).ok()).unwrap_or_default();
        let mut app = Gitcraft {
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
        let last = app.conn.as_ref().and_then(|c| db::get_setting(c, "gitcraft_last_repo").ok().flatten());
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
    fn add_repo(&mut self, path: &str) {
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

    fn open(&mut self, path: &str) {
        self.repo = Some(path.to_string());
        if let Some(c) = &self.conn {
            let _ = db::set_setting(c, "gitcraft_last_repo", path);
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
    fn set_topo(&mut self, on: bool) {
        if self.topo == on {
            return;
        }
        self.topo = on;
        let Some(repo) = self.repo.clone() else { return };
        self.open(&repo);
    }

    /// 「所有分支」那枚弹出按钮：换范围 = 重新取一次列表，和换排序同一个理由。
    /// 先比后写，`Some("main")` 点回同一个分支不该把快照整个丢掉。
    fn set_rev(&mut self, rev: Option<String>) {
        if self.log_rev == rev {
            return;
        }
        self.log_rev = rev;
        let Some(repo) = self.repo.clone() else { return };
        self.open(&repo);
    }

    /// 日志滚到了最底下：取下一页。两道闸——知道到底了就不问、上一页还在路上不问。
    /// 参数一次抓齐：闭包跑在 `snap` 的借用里，碰不到 `self`。
    fn ask_for_more(&mut self) {
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

    fn oid(&self, i: usize) -> Option<String> {
        self.snap.as_ref().and_then(|s| s.commits.get(i)).map(|c| c.id.clone())
    }

    /// 唯一一处「选行」：日志行、侧栏「文件状态 / 历史」都走这里。
    fn select(&mut self, sel: Sel) {
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
    fn request(&mut self, target: Target) {
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
    fn start_clone(&mut self, url: String, dest: String) {
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = git::clone_repo(&url, &url, &dest).map_err(|e| format!("{e}"));
            let _ = tx.send(Msg::Cloned(r));
        });
    }

    fn act(&mut self, action: Action) {
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
                            .map_err(|e| GitcraftError::Other(format!("打不开浏览器：{e}"))),
                        None => Err(GitcraftError::Other(format!(
                            "{url} 不是「主机/所有者/仓库」那种远端地址"
                        ))),
                    }
                }),
                // 到不了这儿：`act` 在起线程之前就把克隆单独拦下走了 `Msg::Cloned`
                // （它没有 `repo` 可用，也没有快照可重取）。这一条只为穷尽而写，
                // 真被走到就是有人挪动了那道守卫——**当场响**，别让它静默变成「点了没反应」。
                Action::Clone { .. } => {
                    Err(GitcraftError::Other("克隆不该走到这条路上（act 的守卫被挪了）".to_owned()))
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
    fn modal_layer(&mut self, ctx: &egui::Context) {
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
        egui::Area::new(egui::Id::unique("gitcraft-modal"))
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

    fn poll(&mut self, ctx: &egui::Context) {
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
    fn toolbar(&mut self, ui: &mut egui::Ui) {
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
    fn on_tool(&mut self, i: usize) {
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
    fn show_modal(&mut self, m: Modal) {
        self.modal_focus = m.text.is_some();
        self.modal = Some(m);
    }

    /// 把一帧里从各处收上来的意图落地。顺序是定死的：**先说话、再挪位置、最后才动手**。
    ///
    /// 动手之前还有一道闸：`needs_confirm` 那几类不直接执行，而是换一张确认卡。
    /// 拦在这里而不是每一处菜单里，是因为入口只会越来越多（侧栏、日志行、文件行、工具条），
    /// 逐处加「确认」迟早会漏一处——而漏的那一处正好是点错时会丢工作的那一处。
    fn land(&mut self, it: Intent) {
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
    fn tab_bar(&mut self, ui: &mut egui::Ui) {
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
    fn sidebar(&mut self, ui: &mut egui::Ui) {
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
    fn bottom_band(&mut self, ui: &mut egui::Ui) {
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

    fn file_list(&mut self, ui: &mut egui::Ui) {
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
            // 两句话分开报，措辞抄 core 那句（`gitcraft-core/src/git.rs:1082`）。
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

    fn diff_pane(&mut self, ui: &mut egui::Ui) {
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
    fn hunk_ops(&self) -> Option<(bool, String)> {
        match &self.target {
            Some(Target::Working { staged, path: Some(p), untracked: false }) => Some((*staged, p.clone())),
            _ => None,
        }
    }

    // ------------------------------------------------------------------- 日志
    fn log_band(&mut self, ui: &mut egui::Ui) {
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
    fn shell(&mut self, ui: &mut egui::Ui) {
        ROWS_PAINTED.with(|c| c.set(0));
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

impl eframe::App for Gitcraft {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell(ui);
    }
}

/// 一行要画的东西。**带所有权**：绘制时只画视口里那几十行，从 `Commit` 拷出四段字符串
/// 比把整份提交搬进闭包再处理生命周期便宜得多。
struct Cells {
    sha: String,
    subject: String,
    author: String,
    date: String,
    /// 描述列前面那几枚药丸，可以空。内容在快照时就算好了（见 `Snapshot::badges`）。
    badges: Vec<RefBadge>,
    /// 图表列那个**空心环**。只有虚拟行「Uncommitted changes」有它：提交是实心圆点，
    /// 工作区不是提交，所以借一个环占住同一格。`Some(lane)` 里那个数是**下一行**（第一条
    /// 提交）的车道，环下面那条竖线跟着它——参考图里那条线是连到下面那个圆点的。
    ring: Option<usize>,
    /// 环下面那条竖线画不画。日志被筛过的时候不画：它连的是「下面那条提交」，而那条
    /// 可能正是被筛掉的，画出来等于凭空接上一根断头线（和图列整列留空同一个理由）。
    stem: bool,
}

/// 筛选条那三枚弹出按钮**右侧**的小箭头（参考图每一枚都有）。字形不是随手挑的：
/// `every_rune_the_shell_paints_has_a_glyph` 会把每一枚被画的字对着字体表查一遍，
/// 换了个字体表里没有的就当场红——`▾`（U+25BE）就是这么死的，这份字体链里只有 `▼`。
const ARROW: &str = "▼";
const ALL_BRANCHES: &str = "所有分支";
const SHOW_REMOTE: &str = "显示远程分支";
const HIDE_REMOTE: &str = "隐藏远程分支";
const DATE_ORDER: &str = "日期顺序";
const TOPO_ORDER: &str = "层级顺序";

/// 筛选条上一枚弹出按钮：按钮上是**当前那一项** + ▼，点开列出其余的。
/// 返回被点中的那项文案，没点中就是 `None`。
/// 走 `MenuButton::from_button(...small())` 而不是 `Ui::menu_button`：后者自己造默认尺寸的
/// `Button`，`.small()` 那档行高就丢了（hunk 按钮同款问题，见 PROCESS 的 egui 事实）。
fn popup_button(ui: &mut egui::Ui, label: &str, items: &[String]) -> Option<String> {
    let mut picked: Option<String> = None;
    // 返回值里那对 `(Response, Option<InnerResponse<()>>)` 没东西要接：菜单的收拢由默认的
    // `CloseOnClick` 管，点中了哪一项记在 `picked` 上。
    egui::containers::menu::MenuButton::from_button(
        egui::Button::new(format!("{label} {ARROW}")).small(),
    )
    .ui(ui, |ui| {
        for item in items {
            if ui.button(item.as_str()).clicked() {
                picked = Some(item.clone());
            }
        }
    });
    picked
}

/// 「显示远程分支」关掉时，把远程药丸从**画**里挑出去。
/// 取列表的参数一个字不动——这一枚和「层级顺序」那枚是两件事：那枚换的是 git 的排序参数，
/// 这枚只决定看不看得到。
fn visible_badges(badges: &[RefBadge], show_remote: bool) -> Vec<RefBadge> {
    if show_remote {
        return badges.to_vec();
    }
    badges.iter().filter(|b| b.kind != RefKind::Remote).cloned().collect()
}

/// 药丸前面那枚小图标。**不能随手挑**：`⑂`（U+2482）在 Noto Sans SC 的 cmap 里根本没有，
/// 落出来是豆腐块。这里每一枚都是 `every_rune_the_shell_paints_has_a_glyph` 查过的。
fn pill_glyph(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Branch | RefKind::Remote => "⤴",
        RefKind::Tag => "◇",
        RefKind::Ahead => "",
    }
}

/// 药丸底色。分支与远程跟踪在参考图里是**同一种**浅蓝（`upstream/main` 和 `main` 一个色），
/// 差别只在名字；「超前 N 个版本」是深藏青白字。标签那一枚没有参考图依据（那张图里没标签）。
fn pill_fill(kind: RefKind) -> Color32 {
    match kind {
        RefKind::Branch | RefKind::Remote => PILL_BG,
        RefKind::Tag => PILL_TAG_BG,
        RefKind::Ahead => PILL_AHEAD_BG,
    }
}

/// 描述列开头那一串药丸。返回**下一枚该摆的 x**，调用方拿它当描述文字的起点。
///
/// 宽度只能现量（`layout_no_wrap`）：药丸要贴着文字收口，而字宽归字体管——
/// 这和「车道在 worker 上排」不是一类开销，一帧最多十枚。
fn pill_row(p: &egui::Painter, row: Rect, badges: &[RefBadge], x: f32) -> f32 {
    let font = FontId::proportional(PILL_FONT);
    let top = row.center().y - PILL_H / 2.0;
    let mut x = x;
    for (i, badge) in badges.iter().enumerate() {
        // 「超前 N 个版本」紧贴前一枚，合起来是一颗胶囊；其余之间留 2pt 缝。
        let flush = badge.kind == RefKind::Ahead;
        if i > 0 && !flush {
            x += PILL_GAP;
        }
        let text = format!("{}{}", pill_glyph(badge.kind), badge.label);
        let fg = if badge.kind == RefKind::Ahead { Color32::WHITE } else { Color32::from_rgb(0x1a, 0x1a, 0x1a) };
        let galley = p.layout_no_wrap(text, font.clone(), fg);
        let w = galley.size().x + 2.0 * PILL_PAD;
        let rect = Rect::from_min_max(pos2(x, top), pos2(x + w, top + PILL_H));
        // 被 Ahead 贴住的那一枚右边收平，Ahead 自己左边收平——不然接缝处是两道圆弧。
        let glued = matches!(badges.get(i + 1).map(|b| b.kind), Some(RefKind::Ahead));
        let corners = match (flush, glued) {
            (true, _) => egui::CornerRadius { nw: 0, ne: PILL_R, sw: 0, se: PILL_R },
            (false, true) => egui::CornerRadius { nw: PILL_R, ne: 0, sw: PILL_R, se: 0 },
            _ => egui::CornerRadius::same(PILL_R),
        };
        p.rect(rect, corners, pill_fill(badge.kind), egui::Stroke::new(1.0, PILL_EDGE), egui::StrokeKind::Inside);
        p.galley(
            pos2(rect.min.x + PILL_PAD, row.center().y - galley.size().y / 2.0),
            galley,
            fg,
        );
        x += w;
    }
    x
}

/// 日志一行的画法：整行一个可点矩形，格子里用 painter 直接落字，
/// 只有描述列走 `Label::truncate`——长提交信息要省略号收尾，不能换行撑高行。
/// `graph` 为 `None` 时那一列留空：虚拟行（工作区）没有提交可连，筛选过的日志断了行也连不上。
///
/// 交回 `Response` 而不是 `bool`：调用方还要给这一行挂右键菜单，菜单得挂在同一个
/// 命中矩形上，用 `clicked()` 折成 bool 之后那枚矩形就找不回来了。
fn log_row(ui: &mut egui::Ui, width: f32, cells: &Cells, active: bool, graph: Option<&GraphRow>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::click());
    let p = ui.painter_at(rect);
    if active {
        p.rect_filled(rect, 0.0, SELECTED);
    } else if cells.ring.is_some() {
        // 参考图里虚拟行**不选中也是灰底**（220,220,220），它表示的是「这一行不是提交」，
        // 不是「鼠标在这儿」。
        p.rect_filled(rect, 0.0, UNCOMMITTED_BG);
    }
    // 日志行**不画 hover 底色**：这一列的行本来就靠选中色区分，鼠标扫过去整条带子跟着闪，
    // 反而让用户以为「亮起来的那行才是我点到的」——而它和实际命中的行并不总是一致。
    if let Some(g) = graph {
        graph_cell(&p, rect, g);
    } else if let Some(lane) = cells.ring {
        ring_cell(&p, rect, lane, cells.stem);
    }
    // 虚拟行整行加粗（参考图：`*`、「Uncommitted changes」、「今天 14:46」三处都是粗的）。
    // 这里只能做到**颜色更深**：egui 0.37 的 `RichText::strong()` 改的是取色而不是字重，
    // 而自带的那份 Noto Sans SC 只有 Regular 一个字面——真粗体要再带一份 Bold（OFL 允许，
    // 但那是第二个 8MB）。
    let strong = active || cells.ring.is_some();
    let fg = if active { Color32::WHITE } else { INK };
    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let th = ui.text_style_height(&TextStyle::Body);
    let right = rect.max.x;
    // 四列全部**靠左**，且提交号从图列右边起（参考图量的：图表列 242、提交 284.5、描述 361.5、
    // 作者 825、日期 1017——每一格的数据都从自己列的左缘开始，没有右对齐的）。
    // 之前 sha 与日期/作者走 `RIGHT_CENTER`，还把提交列画在图列**里面**（`rect.min.x`），
    // 圆点正好压在提交号上。
    p.text(band(rect, rect.min.x + COL_GRAPH + COL_INSET, COL_SHA - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.sha, font.clone(), if active { Color32::WHITE } else { INK });
    p.text(band(rect, right - COL_DATE + COL_INSET, COL_DATE - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.date, font.clone(), if active { Color32::WHITE } else { INK });
    p.text(band(rect, right - COL_DATE - COL_AUTHOR + COL_INSET, COL_AUTHOR - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.author, font, fg);
    let x0 = rect.min.x + COL_GRAPH + COL_SHA + COL_INSET;
    // 药丸占掉描述列的开头，文字从药丸后面起笔（参考图量的：药丸 357.5..476、文字起笔 481）。
    let x = if cells.badges.is_empty() { x0 } else { pill_row(&p, rect, &cells.badges, x0) + PILL_AFTER };
    let w = (right - COL_DATE - COL_AUTHOR - x - 20.0).max(60.0);
    let mut rich = RichText::new(cells.subject.as_str()).size(12.5);
    if strong {
        rich = rich.strong();
    }
    // 参考图量的：普通行 (38,38,38)、虚拟行更深一档 (33,33,33)、选中行白字。
    // `strong()` 只改取色不改字重（自带字体只有 Regular 一个字面），所以颜色在这儿自己钉住。
    rich = rich.color(if active {
        Color32::WHITE
    } else if strong {
        INK_STRONG
    } else {
        INK
    });
    put_left(ui, band(rect, x, w, th), |ui| {
        // `selectable(false)` 不是「不许选文字」这么轻：egui 默认 `selectable_labels = true`
        // （style.rs:1599），开着文字选择的 `Label` 会给自己叠一层 `Sense::click_and_drag()`
        // （label.rs:154-170），它就压在整行那个 `Sense::click()` 矩形**上面**，
        // 按下被文字选择抢走、行什么都收不到；顺带把光标改成插入态
        // （label_text_selection.rs:645）——「鼠标移到描述上就点不中」是这一条。
        // 判据：`a_click_on_a_rows_text_selects_the_row`。
        ui.add(Label::new(rich).truncate().selectable(false));
    });
    fix_pitch(ui, rect);
    resp
}

/// 虚拟行在图表列里的那枚**空心环**：白底灰边，占住实心圆点那一格，下面接一条竖线连到
/// 第一条提交的圆点（参考图：环在 x=243、下面那条蓝线一路穿过前面的圆点）。
fn ring_cell(p: &egui::Painter, row: Rect, lane: usize, stem: bool) {
    let c = pos2(lane_x(row.min.x, 0), row.center().y);
    if stem {
        p.line_segment([c, pos2(c.x, row.max.y)], egui::Stroke::new(LINE_W, lane_color(lane)));
    }
    p.circle(c, DOT_R + 1.0, Color32::WHITE, egui::Stroke::new(RING_W, RING_GREY));
}

/// `Ui::put` 内部走 `scope_dyn`，它按**子 ui 的 min_rect**推进父游标——而子 ui 是垂直居中的，
/// min_rect 只有文字那一条（高 ≈17.6），比整行（`ROW_H` 21）矮，游标于是**倒回**行中间。
/// `ScrollArea::show_rows` 用 `cursor.min.y / row_height` 反解可见区间，倒回的游标会让每行
/// 都比上一行矮半截、窗口和落点错位。所以画完必须把游标按回整行底部。
fn fix_pitch(ui: &mut egui::Ui, row: Rect) {
    ui.advance_cursor_after_rect(row);
}

/// 行内一格的可画矩形：垂直居中、左上角 x、宽 `w`、高 `th`。
fn band(row: Rect, x: f32, w: f32, th: f32) -> Rect {
    let top = row.center().y - th / 2.0;
    Rect::from_min_max(pos2(x, top), pos2(x + w, top + th))
}

/// 在布局流里占一条**通宽、定高**的空矩形，返回它的绝对矩形（不接任何输入）。
/// 整条带子自己画底色时用这个：`ui.horizontal` 会把行高交给控件说了算，
/// 而这里要的是「这一条就是 28pt，里面爱画什么画什么」。
fn rect_of(ui: &mut egui::Ui, h: f32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    rect
}

/// 在给定矩形里**靠左**放控件。别用 `Ui::put`：它内部是/// `Layout::centered_and_justified`（egui-0.37.0/src/ui.rs:1691），短文字被居中、
/// 带空格的长文字被**拉宽对齐**，一格里的行首 x 每行都不一样。
/// SourceTree 的描述列与差异正文都是齐左的。
fn put_left<T>(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui) -> T) -> T {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| add(ui),
    )
    .inner
}

/// 卡上一行不能点的说明文字。走 `put_left` + `Label` 而不是直接落 galley：
/// 说明可能很长（「打在：修掉 xxx」那条提交标题），要 `Label` 的 `truncate` 才不溢出卡片。
fn modal_text(ui: &mut egui::Ui, card: Rect, y: f32, text: &str, fg: Color32, size: f32) {
    put_left(
        ui,
        Rect::from_min_max(pos2(card.min.x + 12.0, y), pos2(card.max.x - 12.0, y + MODAL_ROW_H)),
        |ui| {
            ui.add(Label::new(RichText::new(text).size(size).color(fg)).truncate().selectable(false));
        },
    );
}

/// 卡里可选的一行（合并挑分支用的就是它）：hover 上浅灰，点中返回 true。
fn modal_row(ui: &mut egui::Ui, card: Rect, y: f32, label: &str) -> bool {
    let r = Rect::from_min_max(pos2(card.min.x + 10.0, y), pos2(card.max.x - 10.0, y + MODAL_ROW_H));
    let resp = ui.allocate_rect(r, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, HOVER);
    }
    modal_text(ui, card, y, label, INK, 12.0);
    resp.clicked()
}

/// 主按钮（和 Enter 是同一条路）把卡上的东西变成一个动作。
/// 名字空着返回 `None` —— 关卡留着让用户补，比关掉再让他从头打一遍好。
fn primary_action(m: &mut Modal) -> Option<Action> {
    let raw = m.text.as_deref().unwrap_or("").trim().to_owned();
    match &mut m.form {
        Form::NewBranch { start } => (!raw.is_empty()).then(|| Action::CreateBranch {
            name: raw.clone(),
            start: start.clone(),
            checkout: true,
        }),
        Form::NewTag { oid } => (!raw.is_empty())
            .then(|| Action::CreateTag { name: raw.clone(), target: oid.clone() }),
        Form::StashSave { include_untracked } => Some(Action::StashSave {
            message: if raw.is_empty() { "没有说明的贮藏".to_owned() } else { raw.clone() },
            include_untracked: *include_untracked,
        }),
        Form::Confirm { then } => then.take(),
        Form::Clone { dest } => {
            let dest = dest.trim().to_owned();
            (!raw.is_empty() && !dest.is_empty()).then(|| Action::Clone { url: raw.clone(), dest })
        }
        Form::Merge { .. } => None,
    }
}

/// hunk 头右端那枚小按钮：白底、1pt 灰边、圆角 4、10.5 号灰字，**最少** 55×15
/// （参考图那两枚量的就是这个尺寸；「取消暂存区块」六个字比 55 宽，按文字撑开）。
/// 返回 `(点中了没, 实际宽度)`——宽度要交回去，左边那枚才知道该从哪儿起。
///
/// 为什么自己画而不用 `egui::Button`：Button 的高度由 `spacing.button_padding` 说了算
/// （默认在文字上下各加 3pt，10.5 号的字撑出 19pt），会溢到相邻那行上去。
/// 这一格要的就是参考图那 15pt。
fn hunk_button(ui: &mut egui::Ui, text: &str, right: f32, cy: f32) -> (bool, f32) {
    let p = ui.painter();
    let g = p.layout_no_wrap(text.to_owned(), FontId::proportional(10.5), HUNK_BTN_FG);
    let w = (g.size().x + 12.0).max(HUNK_BTN_W);
    let r = Rect::from_min_max(pos2(right - w, cy - HUNK_BTN_H / 2.0), pos2(right, cy + HUNK_BTN_H / 2.0));
    p.rect_filled(r, 4.0, Color32::WHITE);
    p.rect_stroke(r, 4.0, Stroke::new(1.0, HUNK_BTN_STROKE), egui::StrokeKind::Middle);
    p.galley(r.center() - g.size() / 2.0, g.clone(), HUNK_BTN_FG);
    let clicked = ui.allocate_rect(r, Sense::click()).clicked();
    (clicked, w)
}

fn header_row(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEADER_H), Sense::hover());
    let p = ui.painter();
    let font = &ui.style().text_styles[&TextStyle::Small];
    let right = rect.max.x;
    // 表头跟着数据走同一侧：参考图那四个词（提交 / 描述 / 作者 / 日期）离自己列的左边线
    // 都是 5pt，跟数据一模一样；只有「图表」是在列里**居中**的（159..185，列 141..198）。
    let g = band(rect, rect.min.x, COL_GRAPH, 13.0);
    p.text(g.center_top(), Align2::CENTER_TOP, "图表", font.clone(), INK);
    for (x, w, label) in [
        (rect.min.x + COL_GRAPH + COL_INSET, COL_SHA - COL_INSET, "提交"),
        (rect.min.x + COL_GRAPH + COL_SHA + COL_INSET, 200.0, "描述"),
        (right - COL_DATE - COL_AUTHOR + COL_INSET, COL_AUTHOR - COL_INSET, "作者"),
        (right - COL_DATE + COL_INSET, COL_DATE - COL_INSET, "日期"),
    ] {
        p.text(band(rect, x, w, 13.0).left_top(), Align2::LEFT_TOP, label, font.clone(), INK);
    }
    // 列边界上的四条竖线（1pt 宽、16pt 高）。参考图：198 / 275 / 739 / 929，正好是
    // 提交、描述、作者、日期四列的左缘各减 `COL_INSET`。
    let y0 = rect.center().y - RULE_H / 2.0;
    for x in [
        rect.min.x + COL_GRAPH,
        rect.min.x + COL_GRAPH + COL_SHA,
        right - COL_DATE - COL_AUTHOR,
        right - COL_DATE,
    ] {
        p.line_segment([pos2(x, y0), pos2(x, y0 + RULE_H)], Stroke::new(1.0, RULE));
    }
    // 表头上下各一条横线，把这一条从筛选条与日志里隔开（参考图 y=78、y=103）。
    p.line_segment(
        [pos2(rect.min.x, rect.min.y + 0.5), pos2(right, rect.min.y + 0.5)],
        Stroke::new(1.0, RULE),
    );
    p.line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(right, rect.max.y - 0.5)],
        Stroke::new(1.0, RULE),
    );
}

// ------------------------------------------------------------------------ 小组件
/// 图标在上、中文在下的竖按钮（参考图第 4 条）。
fn tool_button(ui: &mut egui::Ui, i: usize) -> bool {
    let (icon, label, hint) = TOOLS[i];
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(50.0, TOOLBAR_H - 12.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, HOVER);
    }
    let fg = ui.visuals().text_color();
    let p = ui.painter();
    p.text(pos2(rect.center().x, rect.min.y + 10.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), fg);
    p.text(pos2(rect.center().x, rect.max.y - 7.0), Align2::CENTER_CENTER, label, FontId::proportional(9.5), fg);
    resp.on_hover_text(hint).clicked()
}

/// 一枚仓库 tab。宽度贴着名字（`layout_no_wrap` 现量），高 = 整行：
/// 选中 = 白底 + 描边，未选 = 透明灰字、悬停泛白。
/// 不用 `selectable_label`：它的底色铺满整格且不带圆角，看着不像 tab。
fn tab_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    const TAB_PAD: f32 = 8.0;
    const TAB_R: f32 = 4.0;
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), INK);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * TAB_PAD, TAB_H - 4.0), Sense::click());
    let p = ui.painter();
    let fill = if active { Color32::WHITE } else if resp.hovered() { HOVER } else { Color32::TRANSPARENT };
    let edge = if active { SIDE_PILL_BG } else { Color32::TRANSPARENT };
    p.rect(rect, egui::CornerRadius::same(TAB_R as u8), fill, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    let fg = if active || resp.hovered() { INK } else { MUTED };
    p.galley(rect.min + vec2(TAB_PAD, (rect.height() - galley.size().y) / 2.0), galley, fg);
    resp.clicked()
}

/// 侧栏的一行：整行可点，文字**钉死**在参考图量到的那一列（`SIDE_TEXT_X`），
/// 选中时先铺一枚灰色药丸（`SIDE_PILL_L..R`），`icon` 画在图标格中心。返回有没有被点。
///
/// 不走 `ui.label` 是因为布局系统给不了「标签左缘 = 面板 x=41」这个约束：
/// 滚动区、内边距、间距每一层都会挪它，而这一栏的四个数全是从参考图上量的绝对位置。
/// 侧栏的一行。**返回 Response 而不是 bool**：分支、标签、贮藏那几行要挂右键菜单，
/// 而 `context_menu` 只能问 Response。调用方自己问 `.clicked()`。
fn side_row(
    ui: &mut egui::Ui,
    h: f32,
    text: &str,
    size: f32,
    fg: Color32,
    selected: bool,
    icon: Option<fn(&egui::Painter, Pos2)>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    // 行矩形的左边就是内容框的左边；减掉内缩才换回参考图那套面板坐标。
    let base = rect.min.x - SIDE_INSET as f32;
    let p = ui.painter();
    if selected {
        p.add(egui::Shape::rect_filled(
            Rect::from_min_max(
                pos2(base + SIDE_PILL_L, rect.center().y - ROW_H / 2.0),
                pos2(base + SIDE_PILL_R, rect.center().y + ROW_H / 2.0),
            ),
            5.0,
            SIDE_PILL_BG,
        ));
    }
    if let Some(icon) = icon {
        icon(p, pos2(base + SIDE_ICON_X, rect.center().y));
    }
    let x = base + SIDE_TEXT_X;
    // 文字放在自己那一格靠左截断（分支名可以长到放不下），点击仍然由整行的 `resp` 接。
    put_left(ui, Rect::from_min_max(pos2(x, rect.min.y), pos2(base + SIDEBAR_W - 4.0, rect.max.y)), |ui| {
        // 同日志行：文字的 `Label` 别开 `selectable_labels`，不然它压在整行的点击矩形上面，
        // 按下被文字选择抢走（见 `log_row` 里那段注释）。
        ui.add(Label::new(RichText::new(text).size(size).color(fg)).truncate().selectable(false));
    });
    resp
}

/// 一帧里从界面各处收上来的「接下来要发生的事」。行点击、右键菜单、卡片按钮都往这里放，
/// 面板收尾时统一落地。
///
/// 为什么要这一层：回调们在 `Panel::show` 的闭包里，那时候 `self` 正被借走，
/// 只能往外面递一个值。以前每个面板自己攒两三个局部变量（`checkout` / `goto` / `note`），
/// 加上卡片和确认之后就变成六个，收口处还得各写一遍顺序。
#[derive(Default)]
struct Intent {
    act: Option<Action>,
    card: Option<Modal>,
    goto: Option<Sel>,
    /// 要在日志里翻出来看的那条提交（按 oid，不是按下标 —— 侧栏那些行手上没有下标）。
    reveal: Option<String>,
    note: Option<String>,
}

impl Intent {
    fn run(&mut self, a: Action) {
        self.act = Some(a);
    }

    fn card(&mut self, m: Modal) {
        self.card = Some(m);
    }
}

/// 会丢东西的那几个动作不能点一下就跑，先摊一张确认卡（SourceTree 也是这么拦的）。
/// 拦在**出口**而不是每一处菜单里：以后多加一个入口也不会漏。
fn needs_confirm(a: &Action) -> bool {
    matches!(a, Action::DeleteBranch(_) | Action::DeleteTag(_) | Action::StashPop(_) | Action::StashDrop(_))
}

/// 确认卡的标题与那句说明。文案的重点是「丢的是哪一份、还能不能找回来」。
fn confirm_of(a: &Action) -> (&'static str, String) {
    match a {
        Action::DeleteBranch(name) => (
            "删除分支",
            format!("分支 {name} 会被删掉。没合并过的分支 git 自己会拦下来，不会静默丢提交。"),
        ),
        Action::DeleteTag(name) => ("删除标签", format!("标签 {name} 从本地删掉；远端那份要另外推一次才会没。")),
        Action::StashPop(i) => ("弹出贮藏", format!("stash@{{{i}}} 回到工作区，同时从贮藏列表里删掉。")),
        Action::StashDrop(i) => ("丢弃贮藏", format!("stash@{{{i}}} 那份改动直接丢掉，找不回来。")),
        _ => ("确认", String::new()),
    }
}

/// 右键菜单里的一行。点中就把动作交出去、并把菜单关掉（`ui.close()` 往上找最近的那层
/// `ClosableTag`，所以菜单项套在 `ui.horizontal` 里照样关得掉菜单）。
fn menu_item(ui: &mut egui::Ui, label: &str, then: impl FnOnce() -> Action, intent: &mut Intent) {
    if ui.button(label).clicked() {
        intent.run(then());
        ui.close();
    }
}

/// 菜单里那一行直接变成一个动作，交进调用方手上那个 `Option<Action>`。
/// 和 `menu_item` 的区别只是出口：文件表那一层本来就在收 out-param，不多绕一道 `Intent`。
fn menu_act(ui: &mut egui::Ui, label: &str, then: impl FnOnce() -> Action, act: &mut Option<Action>) {
    if ui.button(label).clicked() {
        *act = Some(then());
        ui.close();
    }
}

/// 菜单里那一行指向一张卡（要填名字 / 要挑东西的动作）。和 `menu_item` 分开是因为
/// 交出去的东西不一样：一个是动作，一个是「先问一句」。
fn menu_card(ui: &mut egui::Ui, label: &str, then: impl FnOnce() -> Modal, intent: &mut Intent) {
    if ui.button(label).clicked() {
        intent.card(then());
        ui.close();
    }
}

/// 菜单里「跳到那条提交」那一行。它谁也不改，只是把日志挪过去。
fn menu_go(ui: &mut egui::Ui, label: &str, oid: String, intent: &mut Intent) {
    if ui.button(label).clicked() {
        intent.reveal = Some(oid);
        ui.close();
    }
}

/// 日志一行的右键菜单。`subject` 只用来给「打标签」那张卡当说明——卡上的说明必须是
/// **打开那一刻**那条提交的话，之后日志翻页不能让它换内容。
fn commit_menu(ui: &mut egui::Ui, oid: &str, subject: &str, intent: &mut Intent) {
    let full = oid.to_owned();
    menu_item(ui, "检出这个提交", || Action::Checkout(full.clone()), intent);
    menu_card(ui, "从这里新建分支", || Modal::new_branch_at(Some(full.clone())), intent);
    menu_card(ui, "在这条提交上打标签", || Modal::new_tag(full.clone(), subject.to_owned()), intent);
    menu_item(ui, "合并到当前分支", || Action::Merge(full.clone()), intent);
    if ui.button("复制完整提交号").clicked() {
        ui.ctx().copy_text(full.clone());
        ui.close();
    }
}

/// 侧栏的一个分类：图标 + 浅灰标签占一行 47pt，下面挂筛选过的子项。
/// `on` 为真的那一条既铺药丸又带蓝点——参考图里这两件事同时发生在当前分支上。
///
/// 两个回调都不许捕获 `self`（它们跑在 `Panel::show` 的闭包里，那时 `self` 正被借走），
/// 要发生什么一律往交回去的那个 `Intent` 里放。`on_menu` 只在**这一行的右键菜单真的被打开**
/// 那一帧才被调用（egui 在里面判）。
fn side_category<T>(
    ui: &mut egui::Ui,
    title: &str,
    icon: fn(&egui::Painter, Pos2),
    filter: &str,
    items: &[T],
    mut label_of: impl FnMut(&T) -> String,
    on: impl Fn(&T) -> bool,
    mut on_click: impl FnMut(&T, &mut Intent),
    mut on_menu: impl FnMut(&mut egui::Ui, &T, &mut Intent),
    intent: &mut Intent,
) {
    let q = filter.trim().to_lowercase();
    side_row(ui, SIDE_CAT_H, title, 12.5, SIDE_LABEL_C, false, Some(icon));
    let hits: Vec<(String, &T)> = items
        .iter()
        .map(|it| (label_of(it), it))
        .filter(|(label, _)| label.to_lowercase().contains(&q))
        .collect();
    for (label, it) in hits {
        let on = on(it);
        let resp =
            side_row(ui, ROW_H, &label, 12.0, SIDE_INK, on, if on { Some(icon_dot) } else { None });
        resp.context_menu(|ui| on_menu(ui, it, intent));
        if resp.clicked() {
            on_click(it, intent);
        }
    }
}

// ---- 侧栏图标：全部是 18..38 那一格里的线描，颜色 `SIDE_ICON_C`，笔画 1.5pt ----

/// 「工作区」= 一台显示器。参考图 69..86 那格量的：屏幕 20×11，下面一截支架加一道底座。
fn icon_monitor(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    p.rect_stroke(Rect::from_min_max(pos2(c.x - 10.0, c.y - 8.0), pos2(c.x + 10.0, c.y + 3.0)), 2.0, s, egui::StrokeKind::Middle);
    p.line_segment([pos2(c.x, c.y + 3.0), pos2(c.x, c.y + 6.5)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 6.5), pos2(c.x + 5.0, c.y + 6.5)], s);
}

/// 「分支」= 标准 git-branch 形：一根主干、一道斜出去的分叉、三个端点各一枚小圈。
fn icon_branch(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    p.line_segment([pos2(c.x - 5.0, c.y - 5.5), pos2(c.x - 5.0, c.y + 5.5)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 1.0), pos2(c.x + 5.0, c.y - 5.0)], s);
    for at in [pos2(c.x - 5.0, c.y - 7.5), pos2(c.x - 5.0, c.y + 7.5), pos2(c.x + 5.0, c.y - 7.5)] {
        p.circle(at, 2.5, SIDE_ICON_C, s);
    }
}

/// 「标签」= 一枚斜放的价格牌：菱形 + 一个穿孔。
fn icon_tag(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    let (l, r, t, b) = (c.x - 9.0, c.x + 9.0, c.y - 8.0, c.y + 8.0);
    p.line_segment([pos2(l, c.y), pos2(c.x, t)], s);
    p.line_segment([pos2(c.x, t), pos2(r, c.y)], s);
    p.line_segment([pos2(r, c.y), pos2(c.x, b)], s);
    p.line_segment([pos2(c.x, b), pos2(l, c.y)], s);
    p.circle(pos2(c.x - 3.5, c.y - 3.5), 1.5, SIDE_ICON_C, Stroke::NONE);
}

/// 「远端」= 一朵云。三枚圆叠在一条底线上，圆填侧栏底色好把内部的弧线盖掉。
fn icon_cloud(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    for (dx, dy, r) in [(-4.5, 1.0, 4.0), (4.5, 1.5, 3.5), (0.0, -2.5, 5.0)] {
        p.circle(pos2(c.x + dx, c.y + dy), r, SIDEBAR_BG, s);
    }
    p.line_segment([pos2(c.x - 8.5, c.y + 5.0), pos2(c.x + 8.5, c.y + 5.0)], s);
}

/// 「已贮藏」= 一个抽屉里搁着三样东西（参考图那枚就是方框加三个小圆）。
fn icon_stash(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    p.rect_stroke(Rect::from_min_max(pos2(c.x - 9.0, c.y - 6.5), pos2(c.x + 9.0, c.y + 6.5)), 2.0, s, egui::StrokeKind::Middle);
    for at in [pos2(c.x - 4.0, c.y - 1.5), pos2(c.x + 4.0, c.y - 1.5), pos2(c.x, c.y + 3.0)] {
        p.circle(at, 2.0, SIDE_ICON_C, Stroke::NONE);
    }
}

/// 当前分支前面那颗点：参考图圆心 x≈29.5、直径 7、蓝 `(36,93,184)`。
fn icon_dot(p: &egui::Painter, c: Pos2) {
    p.circle(pos2(c.x - SIDE_ICON_X + SIDE_DOT_X, c.y), SIDE_DOT_R, SIDE_DOT_C, Stroke::NONE);
}

/// 一组文件 + 组标题复选框（参考图第 5 条：勾 = 这一组都在暂存区，取消勾就是整组 unstage）。
fn section(
    ui: &mut egui::Ui,
    title: &str,
    files: &[&FileStatus],
    staged: bool,
    current: &Option<String>,
    act: &mut Option<Action>,
    pick: &mut Option<Target>,
) {
    if files.is_empty() {
        return;
    }
    let on = staged;
    // 分类行：方框钉在面板左缘 +8、标签再往后 7pt（参考图 x 149 / 169.5，面板左缘 141）。
    // 不用 `ui.checkbox`：egui 那枚跟着字号长（11.5 号撑出 18pt 的框），缩进也由前一格说了算，
    // 三列对不齐；而且它的 `changed()` 会把「整组勾掉」和「用户点了别的」混成一个信号。
    let (hrect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let head_hit = check_box(ui, hrect.min.x + CHK_HEAD_X, hrect.center().y, on);
    put_left(
        ui,
        Rect::from_min_max(pos2(hrect.min.x + CHK_HEAD_X + CHK_S + 7.0, hrect.min.y), pos2(hrect.max.x - 84.0, hrect.max.y)),
        |ui| {
            ui.add(Label::new(RichText::new(format!("{title}（{}）", files.len())).size(11.5).strong()).selectable(false).sense(Sense::hover()));
        },
    );
    let mut discard = false;
    if !staged {
        // 「放弃整组」是我们自己加的：参考图这一格是「⋯」菜单（还没画），先钉在行尾。
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(hrect.max.x - 80.0, hrect.min.y), pos2(hrect.max.x, hrect.max.y)))
                .layout(egui::Layout::right_to_left(Align::Center)),
            |ui| {
                discard = ui.small_button("放弃整组").clicked();
            },
        );
    }
    fix_pitch(ui, hrect);
    if discard {
        *act = Some(Action::Discard(files.iter().map(|f| f.path.clone()).collect()));
    } else if head_hit {
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        *act = Some(if staged { Action::Unstage(paths) } else { Action::Stage(paths) });
    }
    for f in files {
        let (path, is_staged, untracked) = (f.path.clone(), f.staged, f.worktree_status == ChangeStatus::Untracked);
        let st = status_of(*f).clone();
        let (resp, toggled) = file_row(ui, &f.path, &st, current.as_deref() == Some(f.path.as_str()), "", Some(is_staged));
        if toggled {
            let one = vec![path.clone()];
            *act = Some(if is_staged { Action::Unstage(one) } else { Action::Stage(one) });
        }
        if resp.clicked() {
            *pick = Some(Target::Working { staged: is_staged, path: Some(path.clone()), untracked });
        }
        resp.context_menu(|ui| file_menu(ui, &path, is_staged, untracked, act));
    }
}

// -------------------------------------------------------------------- 数据/线程
/// 日志日期列的文案。参考图量到的只有「今天 12:27」这一种（那张截图里最老的一行就是当天的），
/// 「昨天」按同一形状推；年内给「MM-DD HH:MM」、跨年才补年份，是照 macOS 的惯例定的，
/// **没有参考图依据**，等一张更老的截图再校。
fn date_label(iso: &str, now: chrono::DateTime<chrono::Local>) -> String {
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
fn ref_badges(
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
const LOG_PAGE: u32 = 500;

fn snapshot_of(repo: &str, topo: bool, rev: Option<&str>) -> gitcraft_core::Result<Snapshot> {
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
fn append_page(snap: &mut Snapshot, more: Vec<Commit>) {
    snap.commits.extend(more);
    snap.graph = layout_graph(&snap.commits);
    snap.badges = ref_badges(&snap.commits, &snap.branches, &snap.tags, &snap.remote_refs);
    snap.dates = snap.commits.iter().map(|c| date_label(&c.date, snap.taken_at)).collect();
}

fn spawn_snapshot(tx: &Sender<Msg>, repo: &str, topo: bool, rev: Option<&str>) {
    let (tx, repo, rev) = (tx.clone(), repo.to_owned(), rev.map(str::to_owned));
    std::thread::spawn(move || {
        let _ = tx.send(match snapshot_of(&repo, topo, rev.as_deref()) {
            Ok(s) => Msg::Loaded { repo, snap: Box::new(s) },
            Err(e) => Msg::Failed(format!("{repo}: {e}")),
        });
    });
}

fn patch_of(repo: &str, target: &Target) -> gitcraft_core::Result<String> {
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

fn patch_label(t: &Target) -> String {
    match t {
        Target::Working { staged, path: None, .. } => if *staged { "全部已暂存变更" } else { "全部未暂存变更" }.to_owned(),
        Target::Working { staged, path: Some(p), .. } => format!("{} · {}", if *staged { "已暂存" } else { "未暂存" }, p),
        Target::Commit { oid, path: None } => format!("{} · 提交的全部改动", short(oid)),
        Target::Commit { oid, path: Some(p) } => format!("{} · {}", short(oid), p),
    }
}

/// 摘要 + 描述 → 一条 git 信息。描述为空时**不带**那个空行：
/// `git commit -m "标题\n\n"` 会把尾部空行原样留在信息里。
fn commit_text(summary: &str, desc: &str) -> String {
    let s = summary.trim();
    let d = desc.trim();
    if d.is_empty() {
        s.to_owned()
    } else {
        format!("{s}\n\n{d}")
    }
}

fn target_path(t: &Target) -> Option<&str> {
    match t {
        Target::Working { path, .. } | Target::Commit { path, .. } => path.as_deref(),
    }
}

fn short(s: &str) -> String {
    s.chars().take(7).collect()
}

fn short_date(iso: &str) -> String {
    iso.chars().take(10).collect()
}

/// 一行文件状态该显示哪个字母：已暂存的行看索引侧，未暂存的行看工作树侧。
fn status_of(f: &FileStatus) -> &ChangeStatus {
    if f.staged {
        &f.index_status
    } else {
        &f.worktree_status
    }
}

fn mark(s: &ChangeStatus) -> &'static str {
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
fn badge_of(s: &ChangeStatus) -> (Color32, &'static str) {
    match s {
        ChangeStatus::Modified => (BADGE_MOD, "…"),
        ChangeStatus::Renamed => (BADGE_REN, "R"),
        ChangeStatus::Untracked => (BADGE_UNK, "?"),
        other => (BADGE_NONE, mark(other)),
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
fn check_box(ui: &mut egui::Ui, x: f32, cy: f32, checked: bool) -> bool {
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
fn file_row(ui: &mut egui::Ui, path: &str, st: &ChangeStatus, current: bool, tail: &str, check: Option<bool>) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if current {
        ui.painter().rect_filled(rect, 5.0, PICK_BG);
    }
    let mut toggled = false;
    if let Some(on) = check {
        toggled = check_box(ui, rect.min.x + CHK_ROW_X, rect.center().y, on);
    }
    let p = ui.painter();
    let (fill, glyph) = badge_of(st);
    let badge = Rect::from_min_size(pos2(rect.min.x + ICON_X, rect.center().y - BADGE_S / 2.0), Vec2::splat(BADGE_S));
    p.rect_filled(badge, 4.0, fill);
    p.text(badge.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(10.5), Color32::WHITE);
    if !tail.is_empty() {
        p.text(rect.right_center(), Align2::RIGHT_CENTER, tail, FontId::proportional(11.0), MUTED);
    }
    let right = rect.max.x - if tail.is_empty() { 0.0 } else { 76.0 };
    let text = Rect::from_min_max(pos2(rect.min.x + NAME_X, rect.min.y), pos2(right, rect.max.y));
    put_left(ui, text, |ui| {
        ui.add(Label::new(RichText::new(path).size(12.0)).truncate().selectable(false).sense(Sense::hover()))
    });
    (resp, toggled)
}

/// 文件行的右键菜单。动作直接写进 `act`（和方框、和「放弃整组」那条按钮同一条路），
/// 不绕 `Intent`：这一层本来就在收 out-param，绕一圈只会多一套出口。
fn file_menu(ui: &mut egui::Ui, path: &str, staged: bool, untracked: bool, act: &mut Option<Action>) {
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
}

// ------------------------------------------------------------------ 提交图序列
/// 一行的图元。为什么不是直接画而是先列出来：无头判据要能问「这一行有没有一条弯到第二条
/// 车道的合并线」，而落图的形状里竖线、弯线、圆点是三种不同的 shape，问不出语义。
#[derive(Clone, PartialEq, Debug)]
enum Seg {
    /// 上面有提交一路排到了这一行：本车道从行顶连到圆点。
    Stem(usize),
    /// 别的车道下面还排着提交：这条道从行顶贯穿到底。
    Through(usize),
    /// 本行提交往下接它的第一个父提交。
    Tail(usize),
    /// 合并的另一边：从本行圆点弯到 `to` 车道在这一行的行底。
    Arm { from: usize, to: usize },
}

#[derive(Clone, Debug)]
struct GraphRow {
    lane: usize,
    segs: Vec<Seg>,
}

/// 把提交列表（新的在上）排成「一行一条道」。`git log` 保证父提交排在子提交下面
/// （gitcraft-core 的 `every_parent_is_listed_below_its_child` 钉着这条），所以从头扫一遍
/// 就能给每行定车道：
/// * 这一行的提交如果已经在某条道上排队，就沿用那条道——分支接着往下走；
/// * 否则找空道，空道不够才开新道；
/// * 第一个父提交接在本行道底下，多出来的父提交各占一条道并留下一条弯线；
/// * 尾巴上的空道一律裁掉。旧 React 版的 `layoutGraph` **从不释放车道**，
///   每合并一次就多占一条，几百次合并后列宽早爆了；裁尾巴才能把 `COL_GRAPH` 那 49pt 守住。
/// * 同一个提交被排在几条道上（日期序会把分支交错）→ 只认第一条，其余腾出来。
fn layout_graph(commits: &[Commit]) -> Vec<GraphRow> {
    bump_graph_layouts();
    let free = |lanes: &mut Vec<Option<String>>| match lanes.iter().position(|l| l.is_none()) {
        Some(k) => k,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    };
    let mut rows = Vec::with_capacity(commits.len());
    let mut lanes: Vec<Option<String>> = Vec::new();
    for c in commits {
        let queued = lanes.iter().position(|l| l.as_deref() == Some(c.id.as_str()));
        let lane = match queued {
            Some(k) => k,
            None => {
                let k = free(&mut lanes);
                lanes[k] = Some(c.id.clone());
                k
            }
        };
        for k in 0..lanes.len() {
            if k != lane && lanes[k].as_deref() == Some(c.id.as_str()) {
                lanes[k] = None;
            }
        }
        let mut segs = Vec::new();
        if queued.is_some() {
            segs.push(Seg::Stem(lane));
        }
        for k in 0..lanes.len() {
            if k != lane && lanes[k].is_some() {
                segs.push(Seg::Through(k));
            }
        }
        match c.parents.split_first() {
            Some((first, rest)) => {
                segs.push(Seg::Tail(lane));
                lanes[lane] = Some(first.clone());
                for p in rest {
                    let to = match lanes.iter().position(|l| l.as_deref() == Some(p.as_str())) {
                        Some(k) => k,
                        None => free(&mut lanes),
                    };
                    lanes[to] = Some(p.clone());
                    segs.push(Seg::Arm { from: lane, to });
                }
            }
            // 根提交：这条道到此为止，往下不再有东西排队。
            None => lanes[lane] = None,
        }
        while lanes.last().map(|l| l.is_none()).unwrap_or(false) {
            lanes.pop();
        }
        rows.push(GraphRow { lane, segs });
    }
    rows
}

/// 车道中心落在图序列列里的 x。
fn lane_x(min_x: f32, lane: usize) -> f32 {
    min_x + GRAPH_PAD + lane as f32 * LANE_W + LANE_W / 2.0
}

/// 画这一行的图元：先线后点，点盖在线头上看着才像「一个提交」。
fn graph_cell(p: &egui::Painter, row: Rect, g: &GraphRow) {
    let (top, mid, bottom) = (row.min.y, row.center().y, row.max.y);
    let stroke = |lane: usize| egui::Stroke::new(LINE_W, lane_color(lane));
    let x = |lane: usize| lane_x(row.min.x, lane);
    for s in &g.segs {
        match s {
            Seg::Stem(k) => {
                p.line_segment([pos2(x(*k), top), pos2(x(*k), mid)], stroke(*k));
            }
            Seg::Through(k) => {
                p.line_segment([pos2(x(*k), top), pos2(x(*k), bottom)], stroke(*k));
            }
            Seg::Tail(k) => {
                p.line_segment([pos2(x(*k), mid), pos2(x(*k), bottom)], stroke(*k));
            }
            Seg::Arm { from, to } => {
                let (a, b) = (pos2(x(*from), mid), pos2(x(*to), bottom));
                let d = (b.y - a.y) * 0.55;
                p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    [a, a + vec2(0.0, d), b - vec2(0.0, d), b],
                    false,
                    Color32::TRANSPARENT,
                    stroke(*to),
                ));
            }
        }
    }
    p.circle_filled(pos2(x(g.lane), mid), DOT_R, lane_color(g.lane));
}

// ------------------------------------------------------------------------ 差异排版
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Hunk,
    Add,
    Del,
    Ctx,
}

#[derive(Clone)]
struct DiffLine {
    kind: Kind,
    old: Option<u32>,
    new: Option<u32>,
    text: String,
    /// 这一块是整份补丁里的第几块（**从 1 数、跨文件不重置**，和 `text` 里那句「块 N」同一个数）。
    /// 按钮往 core 传的就是它——`git::hunk_patch` 用同一套数法，两边不用换算。
    /// `None` 只留给「还没进任何一块」那种行；文件头那几行现在整个不进列表，所以帧里画不出 `None`。
    hunk: Option<usize>,
}

/// 把补丁文本排成带行号的行。**在工作线程上跑**，帧里只管画视口那几十行。
///
/// `META` 里那些文件头一行都不留：SourceTree 的差异区第一行就是 hunk 那条带
/// （`sourcetree-4.2.19-repo-window.png`：文件名灰带 (236,236,236) 到 y=368pt 结束，
/// y=368..396 已经是 hunk 带 (240,244,241)，中间没有任何一行）。
/// **skip 不能改成"不筛"**：`+++ b/a.txt` 的首字节是 `+`、`--- ` 是 `-`，
/// 不筛就会被下面的 `match` 当成一行新增/删除画出来，还带上行号。
fn layout(patch: &str) -> Vec<DiffLine> {
    let mut out = Vec::with_capacity(patch.lines().count());
    let (mut old, mut new) = (0u32, 0u32);
    let mut hunk = 0usize;
    for line in patch.lines() {
        if META.iter().any(|m| line.starts_with(m)) {
            continue;
        }
        if line.starts_with("@@") {
            hunk += 1;
            let (a, ac, c, cc) = hunk_header(line);
            old = a;
            new = c;
            // 翻译只在这一处做：帧里画 hunk 行就是画 `line.text`。文案照参考图那句
            // 「块 1： 行 569-575」（`…-2026-10-20.png` y 409..438 那一行，x 749..852）：
            // **只报一个区间**，取旧侧那一段。新侧的行号本来就在右边那一列逐行写着；
            // 删/加不等长时（`ac != cc`）参考图没有样本可抄，就多带一句「（新 C-D）」，
            // 免得把另一侧的行号范围整个藏掉。
            let (o0, o1) = (a, a + ac.saturating_sub(1));
            let (n0, n1) = (c, c + cc.saturating_sub(1));
            let head = if (o1 - o0) == (n1 - n0) {
                format!("块 {hunk}： 行 {o0}-{o1}")
            } else {
                format!("块 {hunk}： 行 {o0}-{o1}（新 {n0}-{n1}）")
            };
            out.push(DiffLine { kind: Kind::Hunk, old: None, new: None, text: head, hunk: Some(hunk) });
            continue;
        }
        // 行号用「先取值再自增」：hunk 头说 -8,6 时第一条上下文行就是第 8 行，不是第 9 行。
        let (kind, o, n) = match line.as_bytes().first() {
            Some(b'+') => {
                let n = new;
                new += 1;
                (Kind::Add, None, Some(n))
            }
            Some(b'-') => {
                let o = old;
                old += 1;
                (Kind::Del, Some(o), None)
            }
            _ => {
                let (o, n) = (old, new);
                old += 1;
                new += 1;
                (Kind::Ctx, Some(o), Some(n))
            }
        };
        out.push(DiffLine { kind, old: o, new: n, text: line.to_owned(), hunk: Some(hunk) });
    }
    out
}

/// git 自己塞在补丁里的**文件头**，一行都不画（见 `layout`）。
/// 留着这一份是为了「筛掉」，不是「换颜色画出来」。
const META: [&str; 12] = [
    "diff --git ",
    "index ",
    "--- ",
    "+++ ",
    "new file ",
    "deleted file ",
    "old mode ",
    "new mode ",
    "similarity ",
    "rename from ",
    "rename to ",
    "Binary files ",
];

/// `@@ -8,6 +10,7 @@ 说明` → (8, 6, 10, 7)；省略 `,n` 时按 1 算。
fn hunk_header(line: &str) -> (u32, u32, u32, u32) {
    let mut it = line.trim_start_matches('@').trim().split_whitespace();
    let parse = |s: Option<&str>| -> (u32, u32) {
        let s = s.unwrap_or("0").trim_start_matches(|c| c == '-' || c == '+');
        match s.split_once(',') {
            Some((a, b)) => (a.parse().unwrap_or(0), b.parse().unwrap_or(1)),
            None => (s.parse().unwrap_or(0), 1),
        }
    };
    let (a, ac) = parse(it.next());
    let (c, cc) = parse(it.next());
    (a, ac, c, cc)
}

fn db_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    Some(format!("{home}/Library/Application Support/com.gitcraft.app/gitcraft.db"))
}

fn style(ctx: &egui::Context) {
    // 字体与样式必须在**同一个入口**里装好：无头判据走的就是这个函数，真窗口也走它。
    // 分开装过一次，结果所有量文字宽度的判据都跑在 egui 自带的字体链上——那条链没有汉字，
    // 每个汉字落到 `.notdef` 上，宽度正好是字号的一半（实测「中」@14 = 7.00pt，装完 Noto = 14.00pt）。
    // 于是判据眼里的中文列宽只有真窗口的一半，「这一列放得下」那种结论全是假的。
    fonts::install(ctx);
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

#[cfg(test)]
mod geometry_tests {
    use super::*;
    // 只有测试里那份「文字落点」表用到；放进顶层 import 会让非测试编译报 unused。
    use eframe::egui::Pos2;

    /// 主线程上排过几次图序列。读的是产品代码里那个计数器——产品只负责加，读归门禁。
    fn graph_layouts() -> usize {
        GRAPH_LAYOUTS.with(|c| c.get())
    }

    /// 主线程上匹配过几次「引用→提交行」。同上一条判据的路数。
    fn badge_layouts() -> usize {
        BADGE_LAYOUTS.with(|c| c.get())
    }

    /// 一个壳，数据可选填。填了就能钉排版，不填就是空态分支。
    fn shell(snap: Option<Snapshot>, diff: Vec<DiffLine>) -> Gitcraft {
        let (tx, rx) = channel();
        Gitcraft {
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
            modal: None,
            modal_focus: false,
            tx,
            rx,
        }
    }

    /// 没打开任何仓库的壳：各面板走空态分支，但带照样分块——几何与数据无关。
    fn blank() -> Gitcraft {
        shell(None, Vec::new())
    }

    /// 一份够用的假数据：三条提交、已暂存/未暂存/未跟踪各一条、一段带 hunk 头的补丁。
    /// 目的不是测 git，是让日志、文件表、差异区**真的有内容可排**——空态壳钉不住排版。
    fn loaded() -> Gitcraft {
        filled(3, &layout(PATCH))
    }

    /// 自己决定喂多少行。虚拟化的判据比的是「数据翻一千倍、画出来的行数不变」，
    /// 所以数据量必须是参数，不能写死在 `loaded()` 里。
    fn filled(commits: usize, diff: &[DiffLine]) -> Gitcraft {
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
    fn many_unstaged(n: usize) -> Gitcraft {
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

    fn branched() -> Gitcraft {
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
    fn ragged() -> Gitcraft {
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
    fn frame(app: &mut Gitcraft) -> egui::FullOutput {
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
        let over_log = pos2((SIDEBAR_W + WIN_W) / 2.0, (TOP_H + (WIN_H - STAGING_H)) / 2.0);
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

    /// 这一帧画出的**所有有底色的矩形**：四块带的底色、增删行的底色都是纯色矩形。
    fn rects(out: &egui::FullOutput) -> Vec<(Color32, Rect)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Rect(r) if r.fill != Color32::TRANSPARENT => Some((r.fill, r.rect)),
                _ => None,
            })
            .collect()
    }

    /// 这一帧画出的**圆角矩形**（药丸那一类底色块）：底色、矩形、四个圆角。
    /// 带圆角的矩形在 egui 里仍是 `Shape::Rect`，只是 `corner_radius` 非 0——
    /// 分成两份问是因为 `rects()` 那份要把四块带的**方角**底色认出来，混进药丸就没法数了。
    fn rounded(out: &egui::FullOutput) -> Vec<(Color32, Rect, egui::CornerRadius)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Rect(r)
                    if r.fill != Color32::TRANSPARENT
                        && r.corner_radius != egui::CornerRadius::ZERO =>
                {
                    Some((r.fill, r.rect, r.corner_radius))
                }
                _ => None,
            })
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
        let r = text_rect(out, "图表", 55.0..=(WIN_H - STAGING_H));
        (r.center().y - HEADER_H / 2.0)..=(r.center().y + HEADER_H / 2.0)
    }

    /// 日志数据区的 y 带：表头下沿一直到暂存带上沿。
    fn rows_band(out: &egui::FullOutput) -> std::ops::RangeInclusive<f32> {
        *head_band(out).end()..=(WIN_H - STAGING_H)
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
        let staging_top = WIN_H - STAGING_H;
        // 工具栏：横贯顶部的一条 55pt。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, 0.0), pos2(WIN_W, TOOLBAR_H))), TOOLBAR_BG, "工具栏带");
        // tab 条：紧贴工具栏下面那条 24pt，把书签摊成一排。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, TOOLBAR_H), pos2(WIN_W, TOP_H))), TOOLBAR_BG, "tab 条带");
        // 侧栏：从两条顶带下沿一直贯穿到窗口底。SourceTree 的左栏是全高的，
        // 这一条也正是 2026-10-15「双左栏合一」被推翻的落点。
        assert_eq!(band(&r, Rect::from_min_max(pos2(0.0, TOP_H), pos2(SIDEBAR_W, WIN_H))), SIDEBAR_BG, "侧栏带");
        // 暂存带：侧栏右边、贴着底的一条 245pt；带内再切 435pt 文件表 + 差异区。
        assert_eq!(band(&r, Rect::from_min_max(pos2(SIDEBAR_W, staging_top), pos2(WIN_W, WIN_H))), Color32::WHITE, "暂存带");
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(SIDEBAR_W, staging_top), pos2(SIDEBAR_W + FILES_W, WIN_H))),
            Color32::WHITE,
            "文件表"
        );
        assert_eq!(
            band(&r, Rect::from_min_max(pos2(SIDEBAR_W + FILES_W, staging_top), pos2(WIN_W, WIN_H))),
            Color32::WHITE,
            "差异区"
        );
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
        all_text_rects(out, TOP_H..=WIN_H, &[label])
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

    /// 提示条只能占自己那一格，而且**靠右摆**。给它固定宽度之前 `truncate()` 把整行吃掉，
    /// 后面的控件被画在它上面（2026-10-20 目视核对撞见的）。
    /// 现在它住在 tab 条右端：判据是「右缘贴着窗口」+「宽度不超预算」+「不压住左边的 tab」。
    #[test]
    fn the_note_keeps_its_own_cell_at_the_right_of_the_tab_bar() {
        let mut app = loaded();
        // tab 只在 `repos` 非空时才画，测试夹具里那份是空的。
        app.repos.push(Repo {
            id: None,
            path: "/fixture".to_owned(),
            name: "fixture".to_owned(),
            added_at: None,
            last_fetched_at: None,
        });
        app.note = Some("egui 版还没接：搜索面板（SourceTree 是带作用域条的独立面板）".to_owned());
        let out = frame(&mut app);
        let n = text_rect(&out, "egui 版还没接", TOOLBAR_H..=TOP_H);
        assert!(n.width() <= NOTE_W + 0.01, "提示条吃了整行：宽 {}，上限 {NOTE_W}", n.width());
        assert!(n.max.x >= WIN_W - 12.0, "提示条没靠右：右缘 {}", n.max.x);
        let tab = text_rect(&out, "fixture", TOOLBAR_H..=TOP_H);
        assert!(tab.max.x <= n.min.x + 0.01, "提示条压住了 tab：{} > {}", n.min.x, tab.max.x);
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
        let (left, top) = (SIDEBAR_W + FILES_W, WIN_H - STAGING_H);
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
    /// 判据因此在核心层（`gitcraft_core::git` 的 `commit_patch_carries_no_log_header`，
    /// 用真临时仓库跑真 git）。
    #[test]
    fn the_diff_pane_starts_at_the_first_hunk() {
        let out = frame(&mut loaded());
        let top = WIN_H - STAGING_H + DIFF_HEAD_H;
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
        let labels = |app: &mut Gitcraft| {
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
        let label = text_rect(&out, "src/loose.rs", (WIN_H - STAGING_H)..=WIN_H);
        assert!((label.center().y - band.center().y).abs() < 1.0, "灰带没套在文件名那一行上：带 {:.1}、字 {:.1}", band.center().y, label.center().y);
        assert!(label.min.x >= band.min.x, "文件名跑到灰带左边去了");
    }

    /// 文件行那三列画在哪儿，断的是**画出来的位置**，字面量写死在这儿（引 `CHK_*`/`ICON_X`/
    /// `NAME_X` 就成了常量比自己，改常量两边一起挪、判据永不变红——上一批 `HUNK_BTN_W` 就是这么假绿的）。
    /// 参考图实测是 24 / 47 / 67.5（面板左缘 141 起算，方框 x 165、图标 x 188、名字 x 208.5），
    /// **我们统一往左 17pt** —— 用户 2026-10-21 连着两次说「内容往左移动一点」。
    /// 所以这三档是**偏离 SourceTree 的自加调整**，出处与差值记在
    /// `the_file_row_columns_are_the_chosen_offsets`。
    ///
    /// 文件行**按文件名那一格找**，不按 x 找：方框这一档现在和分类行齐平（都在 +8），
    /// 用 x 分列分不出谁是谁，上一版那么写是因为它本来就错开一列。
    #[test]
    fn the_file_row_columns_are_17pt_left_of_the_reference() {
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
            let name = text_rect(&out, path, (WIN_H - STAGING_H)..=WIN_H);
            let row: Vec<&(Color32, Rect)> = squares
                .iter()
                .filter(|(_, r)| (r.center().y - name.center().y).abs() < 1.0)
                .collect();
            assert!(!row.is_empty(), "「{path}」这一行左边没有方框：{squares:?}");
            // 这一行该有一枚方框 + 一枚状态图标，两枚都是 13.5 见方，所以只能按 x 分档问。
            let at = |x: f32| row.iter().any(|(_, r)| near(r.min.x - SIDEBAR_W, x));
            assert!(at(8.0), "「{path}」的方框不在离面板左缘 8 这一档：{row:?}");
            assert!(at(30.0), "「{path}」的状态图标不在离面板左缘 30 这一档：{row:?}");
            assert_eq!(row.len(), 2, "「{path}」一行画了 {} 枚方框大小的东西，应该是方框 + 图标两枚", row.len());
            assert!(
                near(name.min.x - SIDEBAR_W, 51.0),
                "「{path}」离面板左缘 {}，我们要的是 51",
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
    /// 这一档现在和文件行的方框**同一列**（用户第二次要求左移的结果），所以不能再用 x 分列去认，
    /// 得按**标题文字那一格的 y** 去认。把 `CHK_HEAD_X` 挪走只会红这一条，
    /// 把 `CHK_ROW_X`/`ICON_X`/`NAME_X` 挪走红的是上面那条。
    #[test]
    fn the_section_box_is_flush_with_the_file_box_column() {
        let out = frame(&mut loaded());
        let near = |a: f32, b: f32| (a - b).abs() < 1.5;
        let squares: Vec<Rect> = rounded(&out)
            .into_iter()
            .map(|(_, r, _)| r)
            .filter(|r| near(r.width(), 13.5) && near(r.height(), 13.5) && r.min.x > SIDEBAR_W && r.min.x < SIDEBAR_W + FILES_W)
            .collect();
        for title in ["已暂存文件", "未暂存文件"] {
            let t = text_rect(&out, title, (WIN_H - STAGING_H)..=WIN_H);
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
    /// （面板左缘 141 起算：方框 x 149、165，图标 x 188，名字 x 208.5），
    /// 第一版取的是 8 / 25 / 47 / 68；**这两批把文件行那三档各减 17**，列间距一个没动。
    /// 断言右侧写成 `25.0 - 17.0` 这样的**字面量算式**：把偏移改成 `- 8.0`（= 退回上一版）
    /// 或 `- 0.0`（= 退回参考图）都会红，而它不吃 `CHK_ROW_X` 本身。
    #[test]
    fn the_file_row_columns_are_the_chosen_offsets() {
        assert_eq!(CHK_S, 13.5, "参考图三枚方框、两枚图标量到都是 13.5 见方");
        assert_eq!(CHK_HEAD_X, 8.0, "分类行的方框照参考图（x 149 → 离面板左缘 8）");
        assert_eq!((CHK_ROW_X, ICON_X, NAME_X), (25.0 - 17.0, 47.0 - 17.0, 68.0 - 17.0), "文件行三档 = 第一版各减 17pt");
        assert_eq!(CHK_BLUE, Color32::from_rgb(71, 141, 247), "勾上的方框中心色");
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
        let top = WIN_H - STAGING_H;
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
        let top = WIN_H - STAGING_H;
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
        let rows = |app: &mut Gitcraft| {
            frame(app);
            ROWS_PAINTED.with(|c| c.get())
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
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("gitcraft-app 就在仓库根下面")
            .to_str()
            .expect("utf-8 路径")
            .to_owned();
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
    fn click_text(app: &mut Gitcraft, label: &str) -> bool {
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
    fn click_box_in_row(app: &mut Gitcraft, name: &str, col: f32) {
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

    fn click_file_box(app: &mut Gitcraft, name: &str) {
        click_box_in_row(app, name, CHK_ROW_X);
    }

    fn click_section_box(app: &mut Gitcraft, title: &str) {
        click_box_in_row(app, title, CHK_HEAD_X);
    }

    /// 一次完整点击的三帧。焦点记在 `Context` 的 memory 里，所以要留住同一个 ctx 让它跨帧活著
    /// ——换个 Context 就等于换了个窗口，前面那下点的焦点就没了（`type_into` 靠这一点）。
    fn click_at(ctx: &egui::Context, app: &mut Gitcraft, at: Pos2) {
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
        let out = frame(&mut branched());
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
    fn frame_events(app: &mut Gitcraft, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = egui::Context::default();
        style(&ctx);
        let mut out = ctx.run_ui(raw_input(events), |ui| app.shell(ui));
        out.textures_delta.clear();
        out
    }

    fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
        raw_input_at(WIN_W, WIN_H, events)
    }

    fn frame_at(app: &mut Gitcraft, w: f32, h: f32) -> egui::FullOutput {
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
    fn locate(ctx: &egui::Context, app: &mut Gitcraft, label: &str) -> Option<Pos2> {
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
    fn locate_has(ctx: &egui::Context, app: &mut Gitcraft, needle: &str) -> Option<Pos2> {
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
    fn warm(app: &mut Gitcraft) -> egui::Context {
        let ctx = egui::Context::default();
        style(&ctx);
        ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui))
            .textures_delta
            .clear();
        ctx
    }

    /// 在模态卡上点一项，或隔着卡点它后面那一行。必须走 `warm`，不能用 `click_text`。
    fn click_warm(app: &mut Gitcraft, label: &str) -> bool {
        let ctx = warm(app);
        let Some(at) = locate(&ctx, app, label) else { return false };
        click_at(&ctx, app, at);
        true
    }

    /// 卡**已经画得出来**的那一帧：先按 `warm` 空跑一遍，再取第二遍的形状。
    /// 直接拿 `frame()` 那一遍问模态卡的形状会问出「遮罩没画」这种假红——也包括假绿。
    fn warm_frame(app: &mut Gitcraft) -> egui::FullOutput {
        let ctx = warm(app);
        let mut out = ctx.run_ui(raw_input(Vec::new()), |ui| app.shell(ui));
        out.textures_delta.clear();
        out
    }

    /// 无头往输入框里敲字。焦点必须在**同一个 Context** 里拿到（见 `click_at`）。
    /// 喂的是 `Event::Text`：macOS 拼音输入法上屏那一刻交给控件的就是这个事件，
    /// 所以这条走的是真输入路径，不是绕过控件直接改字段。
    /// 卡里的输入框只有第二遍才画得出来，所以用 `warm` 起帧（见那儿那段实测）。
    fn type_into(app: &mut Gitcraft, hint: &str, text: &str) -> bool {
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
    fn press_mod_enter(app: &mut Gitcraft, m: egui::Modifiers) {
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
            "gitcraft-scratch-{}-{tag}",
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
    fn pump(app: &mut Gitcraft, done: impl Fn(&Gitcraft) -> bool) -> usize {
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
    fn open_repo(tag: &str) -> (Gitcraft, std::path::PathBuf) {
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
    fn two_blocks_repo(tag: &str) -> (Gitcraft, std::path::PathBuf) {
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
    fn blocks(app: &mut Gitcraft) -> Vec<usize> {
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
    fn two_dirty_files(tag: &str) -> (Gitcraft, std::path::PathBuf) {
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
    fn staged_in_app(app: &Gitcraft) -> Vec<String> {
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
        // `pump` 的判据只拿得到 `&Gitcraft`，帧得由 `&mut` 起。
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
                } else if (a.y - b.y).abs() < 0.01 && a.x <= SIDEBAR_W + 1.5 && b.x >= WIN_W - 1.5 {
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
        let band = (WIN_H - STAGING_H)..=WIN_H;
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
        let db_dir = std::env::temp_dir().join(format!("gitcraft-db-{}", std::process::id()));
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
    fn click_tool(app: &mut Gitcraft, i: usize) -> bool {
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
    fn press(app: &mut Gitcraft, k: egui::Key) {
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
    fn menu_pick(app: &mut Gitcraft, over: &str, item: &str) {
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
    fn popup_pick(app: &mut Gitcraft, button: &str, item: &str) {
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
        let below = |app: &mut Gitcraft| {
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
    fn line_of(app: &Gitcraft) -> Vec<String> {
        app.snap.as_ref().map(|s| s.commits.iter().map(|c| c.subject.clone()).collect()).unwrap_or_default()
    }

    /// 筛选条上以 `prefix` 起笔的那块文字，连箭头一起取回来——要看的是「当前哪一档」
    /// 和「第二层在不在」。
    fn filter_text(app: &mut Gitcraft, prefix: &str) -> Option<String> {
        let out = frame(app);
        texts(&out).into_iter().find(|(t, _)| t.starts_with(prefix)).map(|(t, _)| t)
    }

    /// 日志**数据区**那一格里画了哪些文字。表头以上、暂存带以下都筛掉：
    /// 侧栏和筛选条也有同名文字，整屏去数会把「行上的药丸」和「别处的字符串」混成一件事。
    fn row_texts(app: &mut Gitcraft) -> Vec<String> {
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
        let dst = std::env::temp_dir().join(format!("gitcraft-clone-dst-{}", std::process::id()));
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

    /// 只填地址、目标目录还空着：卡不许关，而且**不许留下半个目录**——`git clone` 一旦开跑
    /// 就先把目录建出来，所以这一条钉的是主按钮压根没点下去。
    #[test]
    fn a_clone_needs_both_fields() {
        let src = scratch_repo("clone-half");
        let dst = std::env::temp_dir().join(format!("gitcraft-clone-half-{}", std::process::id()));
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
}

/// 真窗口唯一的配置。拆成函数只为一件事：窗口约束在无头画布上量不到，判据只能读这个对象
/// （见 `the_window_itself_is_floored_at_the_measured_size`）。
fn viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_inner_size([WIN_W, WIN_H])
        .with_min_inner_size([MIN_W, MIN_H])
        .with_title("GitCraft")
        // 参考图里那一行 52pt 是**标题栏本身**：红绿灯 + 仓库名 + 11 枚按钮同在一行。
        // 我们原来在它下面又画了一条工具栏，于是窗口顶部白占一条 28pt 的系统标题栏，
        // 而且仓库名排在 x=9 —— 正被红绿灯盖着。这三条开关把两条带合成一条。
        // `title_shown(false)`：仓库名由我们自己画（macOS 的标题会居中，参考图那个是左对齐的）。
        .with_titlebar_shown(false)
        .with_fullsize_content_view(true)
        .with_title_shown(false)
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: viewport(),
        ..Default::default()
    };
    eframe::run_native(
        "GitCraft",
        opts,
        Box::new(|cc| {
            style(&cc.egui_ctx);
            Ok(Box::new(Gitcraft::new()))
        }),
    )
}

