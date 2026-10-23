//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use eframe::egui::{pos2, Color32, Rect};


// ---- 从参考图量出来的几何（pt = @2x 截图的像素 / 2）----
/// 标题栏那一行的高度。**两张参考图都是 52**：底色 244,249,246 从 y=0 一路到 y=51，
/// y=52 是一条 202,206,203 的分隔线，y=53 起换成筛选条那条带（240,244,241）。
/// 原来记的 55 是错的（当时按"看着像"记的，没逐行扫底色）。
/// 这一行在 SourceTree 里**就是 macOS 标题栏**：红绿灯和仓库名和那 11 枚按钮同在一行，
/// 所以我们必须把窗口配成"内容铺满标题栏"，而不是在它下面再画一条（见 `viewport`）。
pub(crate) const TOOLBAR_H: f32 = 52.0;
/// 红绿灯：三枚 12pt 圆，中心 x=26/46/66（实测 x 20..32、40..52、60..72），y 20..32。
/// 它们是操作系统画的，盖在我们内容上面 —— 所以这一行的左边 72pt **必须留空**。
#[cfg(test)]
pub(crate) const TRAFFIC_RIGHT: f32 = 72.0;
/// 仓库名「comic (Git)」的左沿实测 **91.5pt**（红绿灯右沿 72 + 19.5 间距）。
/// 只给判据看：判据要钉的是**量来的数**，不是产品里取整后的那个常量。
#[cfg(test)]
pub(crate) const TITLE_LEFT: f32 = 91.5;
/// 上面那个 91.5 落到帧上：egui 0.37 的 `Margin` 四个边是 **i8 整点**，给不了半像素，所以取 92。
pub(crate) const TITLE_INSET: i8 = 92;
/// 最右一枚按钮（设置）的右沿实测 1099 / 窗口 1120 → 右边距 **21pt**。
pub(crate) const TOOLBAR_RIGHT: i8 = 21;
/// 仓库 tab 条。SourceTree 把书签放在**另一个窗口**里，这个壳只有一个窗口，
/// 所以书签列表摊成工具栏下面的一行 tab（2026-10-20 用户要求）。
pub(crate) const TAB_H: f32 = 24.0;
/// 顶部两条带加起来吃掉的高度。产品代码不需要它（面板自己会叠），
/// 但判据要按这条线把「tab 条以下」的带子和文字筛出来，所以只给测试看。
#[cfg(test)]
pub(crate) const TOP_H: f32 = TOOLBAR_H + TAB_H;
pub(crate) const SIDEBAR_W: f32 = 141.0;
pub(crate) const STAGING_H: f32 = 245.0;
/// 文件表底下那格提交框占的高度：摘要 26 + 描述 3 行 45.6 + 按钮行 26（行距 0，控件之间的
/// `item_spacing.y` 也是 0），合计 97.6，按 96 的预算放。**这是自己定的预算，不是参考图量来的**——
/// 参考图那张提交框只露出摘要一行，量不到三行描述该多高。它的用处只有一个：文件表在上面滚，
/// 提交框钉在带底（见 `the_commit_box_stays_inside_a_short_band`）。
/// 这 1.6pt 的溢出是换上 Inter 之后多出来的（行盒比自带字体高）：不能靠抬预算解决，
/// 抬了会把文件表最后一行顶到视口边缘、点不中；父面板的 clip 会把这点溢出切掉，看不见。
pub(crate) const COMMIT_H: f32 = 96.0;
pub(crate) const FILES_W: f32 = 435.0;
/// 窗口能缩到多小。**870 是量出来的，不是拍的**：无头扫 600→1050（步长 10），
/// 860 那一格还剩 1 处撞（仓库名压 ⊕），870 起每一格 0 处重叠，两个高度（321 / 545）同一条线。
/// 这条线从 #30 的 770 抬到了 870：**代价是标题栏合并** —— 左边 92pt 让给红绿灯、
/// 右边 21pt 是按钮余量，等于把可用宽度砍掉 104pt，而名字和 11 枚按钮谁都不肯让。
/// SourceTree 不会撞，因为 `NSToolbar` 装不下会把条目收进 » 溢出菜单、标题用「…」截断；
/// 我们还没有那套，所以先抬地板，**截断让位排在 #35**（那批要重排这一行的左组）。
/// 高 321 = 两条顶带（52+24）+ 暂存带 245：再矮，底部面板被夹扁、文件表只剩 40pt 的地板。
pub(crate) const MIN_W: f32 = 870.0;
pub(crate) const MIN_H: f32 = TOOLBAR_H + TAB_H + STAGING_H;
/// 参考图（1050pt 宽那张）数据行的行距：130→156→182→210，**26pt**。
/// 我们原来用 21，整张日志因此比 SourceTree 矮一截、一屏多塞五行。
pub(crate) const ROW_H: f32 = 26.0;
pub(crate) const DIFF_ROW_H: f32 = 15.0;
/// 表头那一条：参考图 y 79..103 = 24pt（比数据行矮 2）。
pub(crate) const HEADER_H: f32 = 24.0;
/// 表头列边上的分隔竖线：宽 1pt、高 16pt、`(229,229,229)`，四条都在列边界上。
/// 之前判定「表头没有竖线」是量错了带——线只有 16pt 高，用整带阈值扫会被背景盖掉。
pub(crate) const RULE_H: f32 = 16.0;
pub(crate) const RULE: Color32 = Color32::from_rgb(229, 229, 229);
/// 每列文字离自己列左边线的距离。参考图四个表头词的左缘：提交 202.5、描述 280、
/// 作者 744、日期 934——列边界分别 198 / 275 / 739 / 929，都差 5pt。
pub(crate) const COL_INSET: f32 = 5.0;
/// 四列的宽度全部照那张 1050pt 宽的参考图量：边线在 198 / 275 / 739 / 929。
/// 描述列（275..739 = 464）是弹性的，所以只要这三条定死，剩下那条自然对上。
pub(crate) const COL_GRAPH: f32 = 57.0;
pub(crate) const COL_SHA: f32 = 77.0;
pub(crate) const COL_AUTHOR: f32 = 190.0;
pub(crate) const COL_DATE: f32 = 121.0;
pub(crate) const NO_W: f32 = 30.0;
/// 图序列的车道几何：第 0 道的**中心**在列左缘 +22.75（参考图量圆点 239..246.5，
/// 列从 220 起），往后每 12pt 一道；57pt 的列装得下 4 道（见 `layout_graph` 裁尾巴那段）。
pub(crate) const LANE_W: f32 = 12.0;
pub(crate) const GRAPH_PAD: f32 = 16.75;
/// 参考图实测（logical pt，@2x 量的）：圆点直径 8.0 → 半径 4.0，竖线宽 2.0。
/// 之前那对 2.6 / 1.3 是旧 React 版留下的，比参考图小一圈——同一张截图并排一看就露出来。
pub(crate) const DOT_R: f32 = 4.0;
pub(crate) const LINE_W: f32 = 2.0;
/// 虚拟行那枚空心环：外直径 10.0、环宽 2.5（也就是内孔 5.0）、中性灰 `#929291`。
/// 环的半径是 `DOT_R + 1.0`：外沿比圆点大一圈，内孔正好把圆点那一格空出来。
pub(crate) const RING_W: f32 = 2.5;
pub(crate) const RING_GREY: Color32 = Color32::from_rgb(0x92, 0x92, 0x91);
/// 窗口内容区 = 参考图 2100×1090 像素 @2x。四块带的断言以此为准。
pub(crate) const WIN_W: f32 = 1050.0;
pub(crate) const WIN_H: f32 = 545.0;
/// 侧栏最底那条「筛选器」占的高度：`add_space(4)` + 那条输入框。输入框实际拿的是全局
/// `interact_size.y`，而那个值就是 `ROW_H`（见 `style()`），所以这条跟着行高走——
/// 行高从 21 改成 26 时这里没跟着改，侧栏的底色矩形就超出窗口底 6pt（`bands_match_...` 抓到）。
pub(crate) const SIDE_FILTER_H: f32 = 4.0 + ROW_H;
/// 侧栏内容框离面板左缘的距离（`Frame::inner_margin`）。下面那几个数全是**面板坐标**，
/// 画的时候减去它才是行内偏移——这样常量的名字就等于参考图上量到的那个位置。
pub(crate) const SIDE_INSET: i8 = 5;
/// 侧栏文字列的左缘。参考图四组标签——「WORKSPACE」69..86、「分支」198..207、
/// 「main」230..240、「文件状态」103..114——**全部**起在 x=41：分类不缩进，子项也不缩进，
/// 图标/圆点住在 18..38 那一格里。我们原来是「标题 5、子项 20」两档缩进，跟参考图不是一回事。
pub(crate) const SIDE_TEXT_X: f32 = 41.0;
/// 图标格 18..38 的中心。
pub(crate) const SIDE_ICON_X: f32 = 28.0;
/// 分类行（分支/标签/远端/已贮藏）的行距：参考图 202.5→275.5→322.5→369.5→416.5→463.5，
/// 五段全是 **47pt**。我们原来所有侧栏行都是 21pt 一条，整栏挤成一坨。
pub(crate) const SIDE_CAT_H: f32 = 47.0;
/// 「工作区」那一条：它到自己第一条子项是 30.5pt。
pub(crate) const SIDE_HEAD_H: f32 = 30.0;
/// 选中子项的灰色药丸：参考图 x 10..129.5、高 25.5、圆角 5。子项行高就是 `ROW_H`。
pub(crate) const SIDE_PILL_L: f32 = 10.0;
pub(crate) const SIDE_PILL_R: f32 = 129.5;
pub(crate) const SIDE_PILL_BG: Color32 = Color32::from_rgb(204, 208, 205);
/// 图标是 (69,81,108) 的深蓝灰线描，分类标签是 (164,167,165) 的浅灰，子项文字 (61,62,61)。
/// 三个都是**最暗像素**，也就是颜色的上限（细笔画在 @2x 上会被抗锯齿摊薄）。
pub(crate) const SIDE_ICON_C: Color32 = Color32::from_rgb(69, 81, 108);
pub(crate) const SIDE_LABEL_C: Color32 = Color32::from_rgb(164, 167, 165);
pub(crate) const SIDE_INK: Color32 = Color32::from_rgb(61, 62, 61);
/// 当前分支前面那颗点：参考图圆心 x≈29.5、直径 7、`(36,93,184)`。**蓝**，不是我们原来用的黑。
pub(crate) const SIDE_DOT_X: f32 = 29.5;
pub(crate) const SIDE_DOT_R: f32 = 3.5;
pub(crate) const SIDE_DOT_C: Color32 = Color32::from_rgb(36, 93, 184);
/// 描述列前面那枚药丸的几何。高 15 / 上下各留 2.5 是参考图量的（行带 129..149，药丸 131.5..146.5）；
/// 两枚浅蓝药丸（`upstream/main` 357.5..458、`origin/main` 458.5..538.5）之间只隔 2pt，
/// 而「超前 3 个版本」那枚深藏青是**贴死**在前一枚右边的（403 / 403.5）——它俩合成一颗胶囊。
/// 药丸结束后描述文字再让 5pt（药丸右缘 476、文字起笔 481）。圆角取 4。
pub(crate) const PILL_H: f32 = 15.0;
pub(crate) const PILL_PAD: f32 = 5.0;
pub(crate) const PILL_GAP: f32 = 2.0;
pub(crate) const PILL_AFTER: f32 = 5.0;
pub(crate) const PILL_R: u8 = 4;
pub(crate) const PILL_FONT: f32 = 11.0;

// 亮色是 SourceTree 的实际观感：侧栏比内容区暗一格，选中行整行蓝底白字。
pub(crate) const SIDEBAR_BG: Color32 = Color32::from_rgb(0xf1, 0xf1, 0xf3);
pub(crate) const TOOLBAR_BG: Color32 = Color32::from_rgb(0xf7, 0xf7, 0xf9);
pub(crate) const SELECTED: Color32 = Color32::from_rgb(0x1c, 0x6c, 0xf0);
pub(crate) const HOVER: Color32 = Color32::from_rgb(0xe6, 0xe9, 0xef);
/// 增/删两行的整行底色，量自 `sourcetree-4.2.19-repo-window-2026-10-20.png`
/// 的 y 488..502（删）与 504..518（增）。之前那对 `#ddf3e2` / `#fbdcdc` 是旧 React 版抄的，
/// 比参考图深一档、还偏冷。
pub(crate) const ADD_BG: Color32 = Color32::from_rgb(232, 253, 242);
pub(crate) const DEL_BG: Color32 = Color32::from_rgb(255, 238, 234);
/// hunk 头那一行的底色：同张图 y 409..438，`(240,244,241)`。原来那枚淡紫 `#e8eaf0` 是拍的。
pub(crate) const HUNK_BG: Color32 = Color32::from_rgb(240, 244, 241);
pub(crate) const MUTED: Color32 = Color32::from_rgb(0x86, 0x8a, 0x96);
/// 日志四列的字色。参考图量的：提交号、作者、日期、描述四处都是 `(38,38,38)`——**没有**
/// 我们原来给提交号/日期/作者用的那种浅灰。SourceTree 不靠颜色分主次，只靠列宽和字重。
pub(crate) const INK: Color32 = Color32::from_rgb(38, 38, 38);
/// 虚拟行（未提交那一行）的描述比其余行更深一档，参考图量的是 `(33,33,33)`。
/// 它不是从主题的 `strong_text_color` 来的——那个值跟字体字重绑在一起，而这里只有
/// Regular 一个字面，`strong()` 只能改颜色，所以颜色得自己钉。
pub(crate) const INK_STRONG: Color32 = Color32::from_rgb(33, 33, 33);
pub(crate) const LINE_NO: Color32 = Color32::from_rgb(0xa8, 0xac, 0xb6);
pub(crate) const WARN: Color32 = Color32::from_rgb(0xb0, 0x3a, 0x2e);
/// 冲突/进行中操作那条横幅的底色。**没有参考图依据**：SourceTree 那张截图里没有冲突，
/// 这是我们的增强，浅到不抢文件表的注意力。
pub(crate) const WARN_BG: Color32 = Color32::from_rgb(0xfd, 0xf0, 0xee);
/// 冲突横幅的行高。也没有参考图；和 `ROW_H` 差 4pt，塞得下 11.5 号字与两枚小按钮。
pub(crate) const CONFLICT_BANNER_H: f32 = 30.0;
/// 底部状态栏。SourceTree 的四张参考图都把窗口底裁掉了（量不到它的状态栏），
/// 高度按行节奏取 `ROW_H`；文案与几何等拿到一张带状态栏的参考图或真窗口对照再修，
/// PROCESS「还没量到的」已登记。
pub(crate) const STATUS_H: f32 = 26.0;
// 描述列那几枚 ref 药丸。三个值都是在参考图上量的（`docs/reference/…-repo-window-2026-10-20.png`，
// 第一行 `⑂ main` 的浅蓝底取 (213,229,253)、描边取 (170,170,170)、`超前3个版本` 的藏青底取 (0,57,121)）。
pub(crate) const PILL_BG: Color32 = Color32::from_rgb(0xd5, 0xe5, 0xfd);
pub(crate) const PILL_EDGE: Color32 = Color32::from_rgb(0xaa, 0xaa, 0xaa);
pub(crate) const PILL_AHEAD_BG: Color32 = Color32::from_rgb(0x00, 0x39, 0x79);
/// 标签药丸**没有**参考图依据：那张截图的仓库里一个标签都没有。这里的浅绿只是
/// 「和分支区分得开」，不是量出来的。哪天拿一张带标签的参考图来，替换掉这一行。
pub(crate) const PILL_TAG_BG: Color32 = Color32::from_rgb(0xe4, 0xf2, 0xdc);
/// 虚拟行「Uncommitted changes」那条的底色，参考图量的是 (220,220,220)。
/// 它不是选中态——选中是整行蓝，这一行是**不选也灰**。
pub(crate) const UNCOMMITTED_BG: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);

// ---- 文件状态那枚色块。三个颜色全是参考图量来的：
// 改动 = 琥珀底白 `…`(238,197,91)（repo-window 那张 680 个像素）、
// 重命名 = 玫瑰底白 `R`(163,115,122)（同张 242 个像素）、
// 未跟踪 = 紫底白 `?`(101,73,129)（只在 2026-10-20 那张里出现，302 个像素）。
// 方块 **14pt**：琥珀色块在两张图里都是 14×14（差异带头 x585..599、x665..678；
// 文件表里那枚 x190..205），紫的那枚量到 11~13——按 14 统一，宁可略大不凭印象配颜色。
// 增/删/冲突那三种**参考图里没有**，一律落灰底。
pub(crate) const BADGE_MOD: Color32 = Color32::from_rgb(238, 197, 91);
pub(crate) const BADGE_REN: Color32 = Color32::from_rgb(163, 115, 122);
pub(crate) const BADGE_UNK: Color32 = Color32::from_rgb(101, 73, 129);
pub(crate) const BADGE_NONE: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
pub(crate) const BADGE_S: f32 = 14.0;
/// 文件表那一格的四档落点。参考图（`sourcetree-4.2.19-repo-window.png`，面板左缘那条
/// 1pt 分隔线在 x=140，往右 141 起算）实测：分类行方框 **x 149**、文件行方框 **x 165**、
/// 状态图标 **x 188**、文件名 **x 208.5** —— 也就是离面板左缘 8 / 24 / 47 / 67.5。
/// 方框与图标都是 **13.5 见方**（三枚量到一模一样）。取整为 8 / 25 / 47 / 68。
///
/// 中间有一批把文件行那三档各减 8（17 / 39 / 60），**这一批改回来了**：用户 2026-10-21
/// 说「我知道为啥文件改动列表内容需要左移一点（勾选的 box 没了，但是没有移动位置）」——
/// 那句「往左移」要修的是**没有方框那一栏白留一格空位**（某条提交的改动列表），
/// 不是工作区那两栏的缩进。空位收掉之后，工作区按参考图排。
pub(crate) const CHK_S: f32 = 13.5;
pub(crate) const CHK_HEAD_X: f32 = 8.0;
pub(crate) const CHK_ROW_X: f32 = 25.0;
pub(crate) const ICON_X: f32 = 47.0;
pub(crate) const NAME_X: f32 = 68.0;
/// 勾上的方框：参考图那三枚中心色都是 (71,141,247)（边缘被抗锯齿拉到 (54,122,246)）。
pub(crate) const CHK_BLUE: Color32 = Color32::from_rgb(71, 141, 247);
/// **这一栏没有方框时**（某条提交的改动列表）两档落到哪儿：色块顶到最左边那一格
/// （和分类行的全选框同列），文字挪到**分类标题同一列**（8 + 13.5 + 7 = 28.5）。
/// 留着方框那 22pt 空档就是用户说的那句「勾选的 box 没了，但是没有移动位置」。
pub(crate) const NO_BOX_ICON_X: f32 = CHK_HEAD_X;
pub(crate) const NO_BOX_NAME_X: f32 = CHK_HEAD_X + CHK_S + 7.0;
/// 没勾上那面**没有样本可抄**——两张参考图里的方框全是勾上的。白底 + 1pt 灰边是估的，
/// 等真窗口目视（#25）再定，别把它当成量到的数。
pub(crate) const CHK_EDGE: Color32 = Color32::from_rgb(190, 190, 190);
/// 差异区那格文件标题的底色带：两张参考图都是 **(236,236,236) 高 30pt**
/// （一张 y338..368、一张 y378..408），色块离带左缘 8pt（585-577、665-657）。
pub(crate) const DIFF_HEAD_BG: Color32 = Color32::from_rgb(236, 236, 236);
pub(crate) const DIFF_HEAD_H: f32 = 30.0;
/// 色块离差异带头左缘的距离，两张图都是 8pt。
pub(crate) const DIFF_HEAD_PAD: f32 = 8.0;
/// 差异区灰带右端那格搜索框。参考图有这一条，但没量到宽高（docs/PROCESS.md 登记在
/// 「仍然没看到过的」里），`DIFF_SEARCH_W` 只按「搜索差异」四个字 + 输入余量估，
/// 高度要能落进 `DIFF_HEAD_H`（30）且不把灰带撑高。
pub(crate) const DIFF_SEARCH_W: f32 = 170.0;
pub(crate) const DIFF_SEARCH_H: f32 = 20.0;
/// 文件表里「当前文件」那一行的灰带。**(220,220,220) 量的是 2026-10-20 那张日志区
/// 第一条选中行（y109..128，横贯整行）**——文件表那两张参考图都没有选中行，所以这一格
/// 是「同一套未聚焦选中色搬到文件表」，不是直接在文件表里量到的。
/// 有焦点时 SourceTree 用蓝 (0,100,225)（同张 y547..569 量到）。焦点态留给键盘那一批。
pub(crate) const PICK_BG: Color32 = Color32::from_rgb(220, 220, 220);
/// 差异区「块 N： 行 A-B」那一行右边的两枚小按钮，全部量自
/// `sourcetree-4.2.19-repo-window-2026-10-20.png`：带子 y 409..438（**29pt**，底色
/// `(240,244,241)`，左缘 x 656），两枚按钮 x 977..1032、1041..1096，各 **55×15**、
/// 中间隔 **9pt**、右缘离带右缘 9pt（1096 vs 1105）。边框 `(228,232,229)`、字色 `(109,110,109)`。
pub(crate) const HUNK_BTN_W: f32 = 55.0;
pub(crate) const HUNK_BTN_H: f32 = 15.0;
pub(crate) const HUNK_BTN_GAP: f32 = 9.0;
pub(crate) const HUNK_BTN_EDGE: f32 = 9.0;
pub(crate) const HUNK_BTN_STROKE: Color32 = Color32::from_rgb(228, 232, 229);
pub(crate) const HUNK_BTN_FG: Color32 = Color32::from_rgb(109, 110, 109);

/// 文件历史浮层。**没有参考图依据**：SourceTree 4.2.19 的文件历史是一个独立窗口，
/// 两张截图都没拍到。这里按「能同时放下提交列表和差异」定档位，目视那一轮再调。
pub(crate) const HIST_W: f32 = 860.0;
pub(crate) const HIST_H: f32 = 500.0;
pub(crate) const HIST_HEAD_H: f32 = 34.0;
pub(crate) const HIST_TAB_H: f32 = 28.0;
pub(crate) const HIST_LIST_W: f32 = 320.0;
pub(crate) const HIST_ROW_H: f32 = 46.0;
pub(crate) const HIST_BLAME_ROW_H: f32 = 18.0;
pub(crate) const HIST_PAD: f32 = 10.0;

/// 模态卡那一格的尺寸。**没有参考图依据**：两张 SourceTree 截图都没拍到弹窗，
/// 这几个数是为了「卡片里的行全是我们手摆的，高度必须自己算才能居中」而定的固定档位，
/// 等目视那一轮（#25）再对着真弹窗调。
pub(crate) const MODAL_W: f32 = 380.0;
pub(crate) const MODAL_TITLE_H: f32 = 30.0;
pub(crate) const MODAL_ROW_H: f32 = 26.0;
pub(crate) const MODAL_FOOT_H: f32 = 34.0;
/// 提交卡里描述那一格给三行（和内联那格 `desired_rows(3)` 同一档）。
pub(crate) const DESC_ROWS: usize = 3;
/// 偏好卡那一格的大小：SourceTree 4.2.19
/// `Contents/Resources/zh_CN.lproj/PreferencesWindow.nib`（英文版同一份，数值一致）里
/// 「通用」那张窗口的**内容区实测 855×441 pt**，不是拍的。
/// 这里记一条更正：本仓库早先按 409 摆过，那是**把标题栏 32pt 从内容区里又减了一遍**
/// （441 − 32 = 409）——NSWindow 的 `contentRect(forFrameRect:)` 给出来的 441 已经是
/// 去掉标题栏与工具栏之后的那一格，不该再减。测量方法见 docs/PROCESS.md
/// 「逆向本机 SourceTree.app」那一条（nib 用 `NSNib` 实例化后读 frame）。
/// 小窗口放不下时按屏 clamp（判据见 `the_preferences_card_fits_a_minimal_window`）。
pub(crate) const PREF_W: f32 = 855.0;
pub(crate) const PREF_H: f32 = 441.0;

// ---------------------------------------------------------------- 推送卡 / 拉取卡
// 下面每一格都是从本机那两张 nib 里读出来的**内容区坐标**（NSWindow 原点在左下，
// 这里全部换算成我们用的「距内容区顶边多少 pt」：`顶 = 内容高 − y − 高`）。
// 测量方法与原始数字见 docs/PROCESS.md「逆向本机 SourceTree.app」。
// 两张卡的尺寸**不跟行数走**（`Modal::rows()` 对它们返回 0），因为那一格里每一格都量过。

/// `GitPushSheet` 内容区 650×271（窗口 frame 650×303，标题栏 32）。
pub(crate) const PUSH_W: f32 = 650.0;
pub(crate) const PUSH_H: f32 = 271.0;
/// 「推送到仓库：」那一行的顶边。三件控件（标签 / 弹出按钮 / URL 框）同属一行，
/// 行顶按那只 URL 框换算：nib 里 `{{253, 230}, {372, 22}}`，内容高 271 − 230 − 22 = 19。
pub(crate) const PUSH_ROW1_Y: f32 = 19.0;
/// 「推送到仓库：」那一行：标签 x、弹出按钮 x 与宽、右边那格显示 URL 的框。
pub(crate) const PUSH_LABEL_X: f32 = 17.0;
pub(crate) const PUSH_REMOTE_X: f32 = 141.0;
pub(crate) const PUSH_REMOTE_W: f32 = 105.0;
pub(crate) const PUSH_URL_X: f32 = 253.0;
pub(crate) const PUSH_URL_W: f32 = 372.0;
/// 「要推送的分支」那只框（NSBox，含标题那一条）。
pub(crate) const PUSH_BOX: Rect = Rect::from_min_max(pos2(17.0, 45.0), pos2(633.0, 215.0));
/// 框里的表格：列宽 38/186/260/43（表头 28pt 高、行高 16pt）——四列的宽度是 nib 里
/// `NSTableColumn` 的实测值，行高是 `rowHeight` 的实测值。
pub(crate) const PUSH_TABLE_X: f32 = 39.0;
pub(crate) const PUSH_TABLE_TOP: f32 = 70.0;
pub(crate) const PUSH_HEAD_H: f32 = 28.0;
pub(crate) const PUSH_ROW_H: f32 = 16.0;
pub(crate) const PUSH_COL_PICK: f32 = 38.0;
pub(crate) const PUSH_COL_LOCAL: f32 = 186.0;
pub(crate) const PUSH_COL_REMOTE: f32 = 260.0;
pub(crate) const PUSH_COL_TRACK: f32 = 43.0;
/// 「全选」那颗小按钮在框里的位置（nib 里 80×18）。
pub(crate) const PUSH_SELECT_ALL: Rect = Rect::from_min_max(pos2(37.0, 184.0), pos2(117.0, 202.0));
/// 框下面那两枚勾（101×18 / 91×18）。
pub(crate) const PUSH_TAGS_Y: f32 = 217.0;
pub(crate) const PUSH_FORCE_Y: f32 = 237.0;
pub(crate) const PUSH_CHECK_X: f32 = 18.0;

/// `GitPullSheet` 内容区 614×299（窗口 frame 614×331）。
pub(crate) const PULL_W: f32 = 614.0;
pub(crate) const PULL_H: f32 = 299.0;
/// 三行标签的左缘与那一列值（弹出按钮/文本框）的左缘。
pub(crate) const PULL_LABEL_X: f32 = 21.0;
pub(crate) const PULL_FIELD_X: f32 = 172.0;
pub(crate) const PULL_REMOTE_W: f32 = 425.0;
pub(crate) const PULL_BRANCH_W: f32 = 342.0;
/// 三行的顶边（从内容区往上量）。
pub(crate) const PULL_REMOTE_Y: f32 = 19.0;
pub(crate) const PULL_URL_Y: f32 = 46.0;
pub(crate) const PULL_BRANCH_Y: f32 = 75.0;
pub(crate) const PULL_INTO_Y: f32 = 102.0;
/// 「刷新」那颗按钮（87×32）——它跑一次 `git fetch <远端>`，回来后那一列分支跟着新快照变。
pub(crate) const PULL_REFRESH: Rect = Rect::from_min_max(pos2(513.0, 72.0), pos2(600.0, 104.0));
/// 「选项」那只框（580×116）。
pub(crate) const PULL_BOX: Rect = Rect::from_min_max(pos2(17.0, 127.0), pos2(597.0, 243.0));
/// 框里四行从上到下的顶边。第三行「立即提交合并的改动」是 git 的默认行为
/// （`--commit`），按「点了看不出区别的勾不如不摆」这条**故意不摆**（152 那一格空着），
/// 理由写在 `git::PullOpts` 上。
pub(crate) const PULL_OPT_LOG_Y: f32 = 172.0;
pub(crate) const PULL_OPT_NOFF_Y: f32 = 192.0;
pub(crate) const PULL_OPT_REBASE_Y: f32 = 222.0;
pub(crate) const PULL_OPT_X: f32 = 37.0;

// 参考图里那一行 hunk 头是**双倍行高**（29pt = 两条正文行）。这一版没跟上：差异区走的是
// `ScrollArea::show_rows`，它只认一个统一行距（正文行量的也是 15~16pt，见 `DIFF_ROW_H`），
// 逐行高度得自己按累计偏移做可见区间。所以按钮按 15pt 画，29 这条记在 docs/PROCESS.md 等替换。

// 一帧里**行闭包实际跑了几次**（日志行 + 差异行）。为什么产品代码里要有这个数：
// `FullOutput.shapes` 只有细分后的图形，egui 在细分阶段按 clip_rect 把屏幕外的行剪掉了，
// 于是「把 4000 条提交全排一遍」和「只排视口那 9 行」落图完全一样——布局开销才是虚拟化的
// 判据，而它不落在图上。每帧在 `shell()` 开头清零，无头门禁读它。
thread_local! {
    static ROWS_PAINTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn bump_rows_painted() {
    ROWS_PAINTED.with(|c| c.set(c.get() + 1));
}

/// 每帧开头清零（app.rs 调）；门禁读数走 [`rows_painted`]。
pub(crate) fn reset_rows_painted() {
    ROWS_PAINTED.with(|c| c.set(0));
}

#[cfg(test)]
pub(crate) fn rows_painted() -> usize {
    ROWS_PAINTED.with(|c| c.get())
}

// 图序列**在所在那个线程上**排了几次。为什么是 thread_local 而不是全局：
// 排布局的活儿归 worker 线程，帧归主线程——门禁要问的是「主线程排了几次」，
// 全局计数会把两边混成一个数（也会被并行跑的别的测试污染）。
// 生产路径的正确形状是：worker 排 1 次、主线程 0 次，而 snapshot 里已经有图序列。
thread_local! {
    static GRAPH_LAYOUTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn bump_graph_layouts() {
    GRAPH_LAYOUTS.with(|c| c.set(c.get() + 1));
}

/// 主线程上排过几次图序列。读的是产品代码里那个计数器——产品只负责加，读归门禁。
#[cfg(test)]
pub(crate) fn graph_layouts() -> usize {
    GRAPH_LAYOUTS.with(|c| c.get())
}

// 引用→提交行的匹配（`ref_badges`）排了几次。和 `GRAPH_LAYOUTS` 同一个道理：
// 那是「提交数 × 引用数」遍字符串比较，帧里每帧重算一遍是白给的开销。
// 判据 `the_frame_never_matches_refs_to_rows` 读的就是这个数——跑一帧，它必须还是 0。
thread_local! {
    static BADGE_LAYOUTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn bump_badge_layouts() {
    BADGE_LAYOUTS.with(|c| c.set(c.get() + 1));
}

/// 主线程上匹配过几次「引用→提交行」。同上一条判据的路数。
#[cfg(test)]
pub(crate) fn badge_layouts() -> usize {
    BADGE_LAYOUTS.with(|c| c.get())
}

/// 车道配色。**第 0 道是量出来的**：参考图那条线性历史全是 `#005FBD`（0,95,189），
/// 而旧 React 版那圈调色板（`src/graph.ts`，随 Tauri 层一起删了，还能在 git 历史里翻到）
/// 第一枚是紫色 `#7C5CFC`——并排一比就知道差在哪儿。后面七道**没有依据**：那张截图里
/// 只有一条道，多车道的配色顺序要等一张有分叉的参考图来钉。
/// 别随手改：按**颜色**认领图元的那条判据（`graph_marks`）认的就是这八种。
pub(crate) const LANE_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x00, 0x5f, 0xbd),
    Color32::from_rgb(0x3f, 0xb9, 0x50),
    Color32::from_rgb(0xf8, 0x51, 0x49),
    Color32::from_rgb(0xd2, 0x99, 0x22),
    Color32::from_rgb(0x5a, 0x8b, 0xff),
    Color32::from_rgb(0xdb, 0x61, 0xa2),
    Color32::from_rgb(0x2d, 0xd4, 0xbf),
    Color32::from_rgb(0xe3, 0xb3, 0x41),
];

pub(crate) fn lane_color(lane: usize) -> Color32 {
    LANE_COLORS[lane % LANE_COLORS.len()]
}

/// 工具栏：图标在上、中文在下，右对齐 11 个（顺序与说明文案照抄参考图和 4.2.19 的 nib）。
/// **图标只能是字体链里真有字形的那些**：`every_rune_the_shell_paints_has_a_glyph`
/// 这条无头判据会把每一枚落过字的码位跑一遍。之前那批箭头、带圈字母和两个彩色 emoji
/// 全都不在 Noto Sans SC 的 cmap 里，于是参考图里十一个清楚的图标，在我们这儿有七个是豆腐块——
/// 这不是配色问题，是根本没字。
pub(crate) const TOOLS: [(&str, &str, &str); 11] = [
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
    ("⊙", "设置", "GitX 设置"),
];
