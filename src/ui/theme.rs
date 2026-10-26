//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use eframe::egui::Color32;


// ---- 几何：事实来源 = docs/DESIGN.md §2 的表 + docs/design-preview.html 的 CSS ----
// §6 的迁移表把旧的 `TOOLBAR_H 52` + `TAB_H 24` 两条带合成一条 `HEADER_H 46`，
// 顺手删掉 `TOOLBAR_BG`（顶栏和状态栏都改走 §3.1 的 `BG_WINDOW`）、
// 把 `TOOLBAR_RIGHT 21` 换成 12。这一带不再出现「参考图实测」这四个字：
// SourceTree 只是脚手架，尺寸由 spec 自己定，改了 spec 就改这里。
/// 顶部那条带（§2「HEADER 46pt，横贯整窗」；预览稿 `.header{height:46}`）。
/// 内容 = 仓库胶囊 tab + 分支名 + 右侧动作按钮组；**没有第二行 tab 条**，书签摊成 tab 后
/// 就住在这 46pt 里。
/// 这一行在 macOS 上**就是标题栏**：红绿灯画在我们内容上面，所以窗口得配成
/// "内容铺满标题栏、系统标题不画"（见 `viewport`），否则顶上白占一条。
pub(crate) const HEADER_H: f32 = 46.0;
/// 红绿灯：三枚 12pt 圆（预览稿 `.lights i{12×12}`），中心 x=26/46/66 → 右沿 72。
/// 它们是操作系统画的，盖在我们内容上面 —— 所以这一行的左边 **必须留空**。
#[cfg(test)]
pub(crate) const TRAFFIC_RIGHT: f32 = 72.0;
/// 顶栏内容的左缘 = 给红绿灯让出的 92pt（§2「红绿灯 inset 92pt 不变」）。
/// egui 0.37 的 `Margin` 四个边是 **i8 整点**，给不了半像素。
pub(crate) const TITLE_INSET: i8 = 92;
/// 顶栏右边距（预览稿 `.header{padding:0 12px}`）。旧值 21 是照着 SourceTree 那排
/// 50pt 宽的竖按钮量的；按钮换成 §4.1 的 28×28 之后，余量跟着 spec 收到 12。
pub(crate) const HEADER_RIGHT: i8 = 12;
/// 顶栏的上下内边距：`46 − 28 = 18` → 9 / 9。28 是这一行最高的控件（§4.1 图标按钮 28×28），
/// 胶囊 tab 的 26（§2）在净高里垂直居中。
pub(crate) const HEADER_VPAD: i8 = 9;
// ---- §2 HEADER 里那三组控件（预览稿 `.tab` / `.branch-chip` / `.ib`）----
/// 仓库胶囊 tab：高 26、左右内边距 11、字号 13（`.tab{padding:3px 11px;font-size:13}` ——
/// 3 + 20 行高 + 3 = 26，和 §2 那句「高 26」对上）。圆角 spec 写 999（胶囊）：
/// egui 的 `CornerRadius` 是 u8，取半高 13 画出来就是同一颗胶囊。
pub(crate) const TAB_PILL_H: f32 = 26.0;
pub(crate) const TAB_PILL_PAD: f32 = 11.0;
pub(crate) const TAB_PILL_R: u8 = 13;
pub(crate) const TAB_FONT: f32 = 13.0;
/// tab 之间的间距（`.tabs{gap:6}`）。
pub(crate) const TAB_GAP: f32 = 6.0;
/// 顶栏动作按钮：命中 28×28、圆角 6、图标 14（§4.1「图标按钮 28×28，圆角 6，hover
/// `BG_HOVER`，图标 14px」；预览稿 `.ib`）。旧的 50pt 宽「图标在上、中文在下」那种竖按钮
/// 是照 SourceTree 量的，spec 收掉了：**中文只活在 tooltip 和溢出菜单里**，不带进顶栏。
pub(crate) const ICON_BTN: f32 = 28.0;
pub(crate) const ICON_BTN_R: u8 = 6;
pub(crate) const ICON_PX: f32 = 14.0;
/// 按钮组内部间距（`.btns{gap:2}`）。
pub(crate) const ICON_BTN_GAP: f32 = 2.0;
/// 分支名那一格（`.branch-chip{font-size:12}` + `b{font-weight:600}`）：
/// 分支名 12px `TEXT_SECONDARY`，`↑n ↓m` 同字号走 `TEXT_PRIMARY` + 真字重（§3.4 数字归 600 那档）。
pub(crate) const BRANCH_FONT: f32 = 12.0;
/// 暂存带默认高：§2「STAGING 带 **240pt**，可拖」。旧 245 是按旧行距（26 × ? + 提交框）凑的，
/// 随 S6-4/S6-5 的换锚作废。带可拖的档位仍是 `120 … 窗口余量`（§2 明写），
/// 所以 240 只是**默认值**，不是下限。
pub(crate) const STAGING_H: f32 = 240.0;
/// 文件表底下那格提交框占的高度：摘要 26 + 描述 3 行 45.6 + 按钮行 26（行距 0，控件之间的
/// `item_spacing.y` 也是 0），合计 97.6，按 96 的预算放。**这是自己定的预算，不是参考图量来的**——
/// 参考图那张提交框只露出摘要一行，量不到三行描述该多高。它的用处只有一个：文件表在上面滚，
/// 提交框钉在带底（见 `the_commit_box_stays_inside_a_short_band`）。
/// 这 1.6pt 的溢出是换上 Inter 之后多出来的（行盒比自带字体高）：不能靠抬预算解决，
/// 抬了会把文件表最后一行顶到视口边缘、点不中；父面板的 clip 会把这点溢出切掉，看不见。
pub(crate) const COMMIT_H: f32 = 96.0;
/// 文件表那一栏的默认宽：§2「文件表 **330pt** │ 差异区」。旧 435 是 1050pt 参考图量的
/// （面板左缘 141 起、文件表右缘 576），随 §2 换锚作废。两栏都可拖，这一枚只是默认值。
pub(crate) const FILES_W: f32 = 330.0;
/// 窗口能缩到多小：§2 钉 `860 × 394`。
/// **860 这个数只由字面值判据钉**（`the_header_numbers_are_the_design_md_values` 里
/// `assert_eq!((MIN_W, MIN_H), (860.0, 394.0))`）。`no_visible_text_overlaps_at_the_smallest_allowed_window`
/// 守的是"地板窗口不撞字"，不守这个数——实测把 `MIN_W` 降到 **800 那条还是绿的**，
/// 到 **760 才红**（2026-10-25 变异）。也就是说眼下 860 下面还有 60–100pt 的空档：
/// 上一版注释写"往下调一个像素就红"是**没验过的夸大**，已经改成实测结论。
/// §2 还写了「窗口收窄时溢出收进 `»` 菜单」——那一档溢出**还没实现**，登记在 PROCESS。
/// 真撞车的是 760 以下：那儿顶栏的 11 枚图标 + 分支 chip 已经没有地方躲。
pub(crate) const MIN_W: f32 = 860.0;
/// 高度 = §2 钉的 **394**（= `HEADER 46 + LOG 下限 84 + STAGING 240 + STATUS 24`，S6-5 之后
/// 这四项真的对得上：`ROW_H`/`LOG_HEAD_H` 已是 28、`STAGING_H` 已是 240、`STATUS_H` 已是 24）。
/// 仍写成字面值、不写成公式：公式是**我们自己算出来的**，spec 给的是 394 这一个数；
/// 而且 §2 那条 MIN_H 判据（`the_window_itself_is_floored_at_the_spec_size`）要的正是字面值。
pub(crate) const MIN_H: f32 = 394.0;
/// 日志数据行的行距：§2「表头 28 + 行 28 × N」，预览稿 `.lr{height:28}`。
/// 旧 26 是 1050pt 那张参考图量的（130→156→182→210），§6 把它换成 28——和表头同档，
/// 4/8 网格（原则④）上 28 落得比 26 稳。
pub(crate) const ROW_H: f32 = 28.0;
/// 文件表那一档的行高：**24**（§2「STAGING 带：文件表 330pt：**行高 24**」，§6 迁移表
/// `ROW_H 26`→`28` 那行明写了「文件行 24 另设 `FILE_ROW_H`」）。
/// **这条常量的存在本身就是一条 bug 的记录**：文件行从前跟着 `ROW_H` 走，S6-4 把日志行距
/// 从 26 换成 28，三条文件行连两个分类标题一起长高 24pt，最后一条（`notes.md`）被挤出
/// `ScrollArea` 的可见框——`put_left` 里那个子 `Ui` 整块不可见时 egui **根本不落字**，
/// 于是状态色块（painter 直接画）还在、文件名没了。日志行距与文件行距是两回事，不许共用一个数。
pub(crate) const FILE_ROW_H: f32 = 24.0;
pub(crate) const DIFF_ROW_H: f32 = 15.0;
/// 日志表头那一条：**28**，§2 明写「表头 28pt」，和 `ROW_H` 同档（预览稿 `.lh{height:28}`）。
/// 原来这条叫 `HEADER_H`，和 §2 的顶栏撞名，S6-2 改名时数还是 24。
pub(crate) const LOG_HEAD_H: f32 = 28.0;
/// 表头列边上的分隔竖线高 16pt，宽 1pt，四条都在列边界上（颜色 = `BORDER_SUBTLE`，
/// §3.5「所有分隔线 1px」）。之前判定「表头没有竖线」是量错了带——线只有 16pt 高，
/// 用整带阈值扫会被背景盖掉。
pub(crate) const RULE_H: f32 = 16.0;
/// 每列文字离自己列左边线的距离。§2 只给了四列的**宽**，没给内缩；预览稿 `.lr{padding:0 12px;
/// gap:12px}` 给的是 12，也是 4/8 网格（原则④）上的一档。旧 5 是参考图四个表头词量出来的，
/// 那张图的列宽和现在不同，那个数跟着一起作废。
pub(crate) const COL_INSET: f32 = 12.0;
/// §2 的三枚定宽列：图 48 \| 作者 96 \| 日期 78，中间「提交信息」那一列弹性。
/// **`COL_SHA`（提交号那一列 77）整列删掉**——§2 的四列里没有它，预览稿 `.lr` 也只有
/// `.g / .sj / .au / .dt` 四格；提交号仍可从筛选条与右键菜单拿到。
pub(crate) const COL_GRAPH: f32 = 48.0;
pub(crate) const COL_AUTHOR: f32 = 96.0;
pub(crate) const COL_DATE: f32 = 78.0;
pub(crate) const NO_W: f32 = 30.0;
/// 图序列的车道几何。§2 把图列从 57 收到 **48**，四档车道（§3.3 有 `LANE_1..4` 四枚色）
/// 在旧档上装不下：第 0 道中心 22.75 + 3×12 = 58.75 早就越列。
/// 于是拍成**第 0 道中心离列左缘 12、道距 10**：四道落在 12/22/32/42，圆点（半径 4）
/// 最右到 46，左右各留 8 / 2pt 的余量。旧 `LANE_W 12 / GRAPH_PAD 16.75` 是参考图在
/// 57pt 列上量的（台账自己写了「LANE_W 是估的」），随这一列一起换。
pub(crate) const LANE_W: f32 = 10.0;
/// 第 0 道**中心**离图列左缘多远（不是车道间距，也不是圆点左沿）。
pub(crate) const LANE0_X: f32 = 12.0;
/// 参考图实测（logical pt，@2x 量的）：圆点直径 8.0 → 半径 4.0，竖线宽 2.0。
/// 之前那对 2.6 / 1.3 是旧 React 版留下的，比参考图小一圈——同一张截图并排一看就露出来。
pub(crate) const DOT_R: f32 = 4.0;
pub(crate) const LINE_W: f32 = 2.0;
/// 虚拟行那枚空心环：外直径 10.0、环宽 2.5（也就是内孔 5.0）、中性灰 `#929291`。
/// 环的半径是 `DOT_R + 1.0`：外沿比圆点大一圈，内孔正好把圆点那一格空出来。
pub(crate) const RING_W: f32 = 2.5;
pub(crate) const RING_GREY: Color32 = Color32::from_rgb(0x92, 0x92, 0x91);
/// 无头判据跑的那扇窗的画布尺寸（`frame*()` 造 `RawInput` 时用）。**不是 spec 值**：
/// `DESIGN.md` 只钉了地板 `860 × 394`（§2），一整帧「多大」没有设计意义，
/// 取一扇普通大小的窗（1050×545）让判据既能看到内容又不至于把溢出当成常态。
pub(crate) const WIN_W: f32 = 1050.0;
pub(crate) const WIN_H: f32 = 545.0;

// -------------------------------------------------------------- 侧栏（§2 + §2.1）
/// 侧栏宽 **188**（§2 钉的固定值，旧 141 是参考图量的）。§2.1 那条「分段容器内容宽 172」
/// 就是 `188 − 8 × 2` 的另一种说法，两处必须一起动。
pub(crate) const SIDEBAR_W: f32 = 188.0;
/// 面板横内边距 **8**（§3.5「8（图标-文字、面板横内边距）」）。下面几个数全是**面板坐标**，
/// 画的时候减去它才是行内偏移——这样常量的名字就等于 §2 那一栏里"文字落在哪"。
pub(crate) const SIDE_INSET: i8 = 8;
/// 面板**竖**内边距 **12**（§3.5「12（卡片内边距、面板竖内边距）」）。
/// 横竖分开写：§3.5 给的是两个不同的档，旧代码那一句 `Margin::same(5)` 把两档混成了一个数。
pub(crate) const SIDE_VPAD: i8 = 12;
/// 分组头文字左缘：内边距 8 + 8。§2 的分组头**没有图标**，所以它比子项还靠左半档。
pub(crate) const SIDE_GROUP_X: f32 = 16.0;
/// 子项文字左缘 = 组头 + **缩进 8**（§2.1「仓库行高 26、缩进 8」——那一档写的是仓库行，
/// 分支/标签这些子项同用一个网格，不分两档缩进）。
pub(crate) const SIDE_TEXT_X: f32 = 24.0;
/// 第三级行（远端点开之后那些分支）再往右让 **12**。§2 只给到「组头 16 / 子项 24」两列，
/// 没有第三级——这一档是自拍，登记在 `DESIGN.md` §7。取 12 而不是 8：和「组头→子项」那
/// 一档 8 拉开一点，一眼看得出是**嵌进去的一层**，而不是网格对不齐。
pub(crate) const SIDE_SUB_X: f32 = 12.0;
/// 分组头那一行的高。**§2 没给**（只给了子项 26），取 26 和子项同档：分组头靠 11px 字 +
/// `TEXT_SECONDARY` 分层，不靠行高——旧 47 是从参考图上量的，属于"照抄"时代。
pub(crate) const SIDE_GROUP_H: f32 = 26.0;
/// 子项行高 **26**（§2「子项行高 26」）。**单独一条常量、不复用 `ROW_H`**：`ROW_H` 是日志行，
/// §2 要把它升到 28（S6-4），两条节奏不是一回事，共用会让那一刀顺手改坏侧栏。
pub(crate) const SIDE_ROW_H: f32 = 26.0;
/// 选中子项那枚药丸的左右缘。§2 没钉左右（只钉了「浅蓝药丸、圆角 6」），这里取
/// **整行让 4**（4..184）——药丸比文字宽出一截才像"这一条被选了"，而不是一条色带。
pub(crate) const SIDE_PILL_L: f32 = 4.0;
pub(crate) const SIDE_PILL_R: f32 = 184.0;
/// 药丸圆角 **6**（§3.5「6 按钮/输入/行/药丸」）。
pub(crate) const SIDE_PILL_RAD: u8 = 6;
/// 计数右缘：面板右 − 16。§2 只写「右对齐计数」，**没钉离右缘多远**，自拍这一档。
pub(crate) const SIDE_COUNT_X: f32 = 172.0;
/// 字级两档（§3.4）：分组头走「辅助 11px」，子项走「正文 13px」。
pub(crate) const SIDE_GROUP_FONT: f32 = 11.0;
pub(crate) const SIDE_ITEM_FONT: f32 = 13.0;
pub(crate) const SIDE_COUNT_FONT: f32 = 11.0;
/// 当前分支那颗**蓝点**（§2「蓝点 + 中黑字」）。§2 只说了"蓝"、没说哪枚蓝，
/// 取主色 `ACCENT`（§3.1「主色：链接、主按钮、焦点」）——这是自拍，不是量出来的。
/// 圆心落在文字列左边的空槽里（`SIDE_DOT_X + 半径 3.5 = 15.5`，离子项文字 24 还有 8.5pt）。
/// 直径 7 沿用旧档：spec 没给点的尺寸。
pub(crate) const SIDE_DOT_X: f32 = 12.0;
pub(crate) const SIDE_DOT_R: f32 = 3.5;
/// 底置筛选框那一档 = 上间距 8 + 输入框 **26**（§2「底置筛选输入框 26pt」）。
/// 那个 26 **和 §4.2 那句「输入框：高 30」撞车**——就近取 §2 的侧栏行，冲突登记在
/// `DESIGN.md` §7。`SIDE_FILTER_GAP` 单独拿出来是因为 `sidebar()` 真的会画一次 `add_space`，
/// 扣掉的预算（这一条）和画出来的高度必须同源，不然又是一处"两个数各说各话"。
pub(crate) const SIDE_FILTER_GAP: f32 = 8.0;
pub(crate) const SIDE_FILTER_H: f32 = SIDE_FILTER_GAP + 26.0;
/// 描述列开头那串 ref 药丸的几何（§4.5）。**高 15 / 圆角取半高 7 是拍的**：§4.5 只给了
/// 「圆角 999、11/600」和四套颜色，没给高度；15 沿用参考图量到的那一档（行带 129..149、
/// 药丸 131.5..146.5），半高画出来就是 spec 要的那颗胶囊（同 §7 里 `TAB_PILL_R` 那条处理）。
/// 两枚浅蓝之间隔 2pt，「超前/落后」那枚**贴死**在前一枚右侧（§4.5 括弧里那句），
/// 药丸结束后描述文字再让 5pt。
pub(crate) const PILL_H: f32 = 15.0;
pub(crate) const PILL_PAD: f32 = 5.0;
pub(crate) const PILL_GAP: f32 = 2.0;
pub(crate) const PILL_AFTER: f32 = 5.0;
pub(crate) const PILL_R: u8 = 7;
pub(crate) const PILL_FONT: f32 = 11.0;

/// 侧栏灰底（§3.1 `BG_SIDEBAR` `#F7F8FA`；旧 `SIDEBAR_BG` 那枚 `#F1F1F3` 是参考图量的，
/// 迁移表 §6 里 `SIDEBAR_BG → BG_SIDEBAR` 这一行说的就是它）。
pub(crate) const BG_SIDEBAR: Color32 = Color32::from_rgb(0xF7, 0xF8, 0xFA);
/// §4.6 的选中态：**底 `ACCENT_SOFT` + 左缘 2pt `ACCENT` 竖条 + 文字保持 `TEXT_PRIMARY`**。
/// 旧的 `SELECTED`（实心蓝 `#1C6CF0` + 整行反白）是复刻期从参考图量的，§6 那条「实心蓝底白字作废」
/// 说的就是它——所以这枚常量**删掉不留别名**，选中改由「底色取 §3.1 的 `ACCENT_SOFT`、
/// 条子取 `ACCENT`」两枚现成 token 画。这条 2pt 是 §4.6 给的，预览稿 `.lr.on` 用
/// `box-shadow: inset 2px 0 0 var(--accent)` 画的是同一件事。
pub(crate) const SELECT_BAR_W: f32 = 2.0;

// ---- §3.1 色板：这一带只落**现在真有人在画**的几枚（DESIGN.md 列了 18 枚，
// 剩下的 `WARN_*` / `CONF_*` 那些
// 跟着各自那一带进来——加一批没人消费的常量只会留一地死代码）。
// 旧的 `INK` / `INK_STRONG` / `MUTED` / `HOVER` / `RULE` / `HUNK_BG` / `LINE_NO` /
// `CHK_EDGE` / `FIELD_EDGE` / `CARD_EDGE` / `BTN_EDGE` / `UNCOMMITTED_BG` 全部删掉，
// 不留别名：§6 那张迁移表说了「旧常量 → 新 token」，留着两套
// 就是让下一个改代码的人自己猜哪套是事实来源。
/// 窗口、内容区、卡片、顶栏与状态栏的底（§3.1 `BG_WINDOW`）。
/// 取代旧 `TOOLBAR_BG`(#F7F7F9)：§6 把那条带删了——顶栏 46 是白底 + 下 1px 分隔线（§2），
/// 状态栏同样白底 + 上 1px 分隔线，**"比内容区暗一格的灰条"这一档在 spec 里不存在**。
pub(crate) const BG_WINDOW: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// 正文、列表（DESIGN.md §3.1）。取代 `INK`(38,38,38) 和 `INK_STRONG`(33,33,33)：
/// **不再设「更深一档」这档颜色**，主次靠字重（`fonts::semibold_family()`）。
pub(crate) const TEXT_PRIMARY: Color32 = Color32::from_rgb(0x1F, 0x23, 0x28);
/// 作者 / 日期 / 表头 / 说明（§3.1「次要」档）。
pub(crate) const TEXT_SECONDARY: Color32 = Color32::from_rgb(0x59, 0x63, 0x6E);
/// 占位、分组头、状态栏、行号、hunk 头文字（§3.1「辅助」档）。
/// 取代 `MUTED` 里「真的是占位/辅助」那一半，以及 `LINE_NO`。
pub(crate) const TEXT_MUTED: Color32 = Color32::from_rgb(0x8B, 0x94, 0x9E);
/// 行 / 按钮悬停底色（§3.1，取代旧 `HOVER`）。
pub(crate) const BG_HOVER: Color32 = Color32::from_rgb(0xEF, 0xF1, 0xF4);
/// 输入底、hunk 头、虚拟行（§3.1，取代旧 `HUNK_BG` 与 `UNCOMMITTED_BG`）。
pub(crate) const BG_INSET: Color32 = Color32::from_rgb(0xF0, 0xF1, 0xF3);
/// 所有 1px 分隔线（§3.1，取代旧 `RULE`；表头那几条竖线同色，见 `RULE_H`）。
pub(crate) const BORDER_SUBTLE: Color32 = Color32::from_rgb(0xE5, 0xE7, 0xEB);
/// 输入框 / 按钮 / 未勾选方框的描边（§3.1，取代旧 `CHK_EDGE` / `FIELD_EDGE` / `BTN_EDGE`）。
pub(crate) const BORDER_STRONG: Color32 = Color32::from_rgb(0xD0, 0xD5, 0xDD);
/// 差异增/删两行：底 + 字（§3.1；预览稿 `.dl.a/.dl.d` 同一对）。
/// 字色也上，是因为 spec 的观感是「整行连字一起染」，不是只铺一条底色。
pub(crate) const ADD_BG: Color32 = Color32::from_rgb(0xE6, 0xF4, 0xEA);
pub(crate) const ADD_FG: Color32 = Color32::from_rgb(0x1A, 0x7F, 0x37);
pub(crate) const DEL_BG: Color32 = Color32::from_rgb(0xFC, 0xEB, 0xEA);
pub(crate) const DEL_FG: Color32 = Color32::from_rgb(0xCF, 0x22, 0x2E);
pub(crate) const WARN: Color32 = Color32::from_rgb(0xb0, 0x3a, 0x2e);
/// 冲突/进行中操作那条横幅的底色。**没有参考图依据**：SourceTree 那张截图里没有冲突，
/// 这是我们的增强，浅到不抢文件表的注意力。
pub(crate) const WARN_BG: Color32 = Color32::from_rgb(0xfd, 0xf0, 0xee);
/// 冲突横幅的行高。也没有参考图；和 `ROW_H` 差 4pt，塞得下 11.5 号字与两枚小按钮。
pub(crate) const CONFLICT_BANNER_H: f32 = 30.0;
/// 底部状态栏：§2「STATUS **24pt**，横贯整窗；左：分支 · ↑↓ · 进行中操作；右：仓库路径；11px 灰字」。
/// 旧 26 是「参考图把窗口底裁掉了、量不到，于是按行节奏取 `ROW_H`」的产物；现在 spec 给了数就照 spec。
/// 24 也让 §2 那条地板公式对得上：`46 + 84 + 240 + 24 = 394`。
pub(crate) const STATUS_H: f32 = 24.0;
// ---- §4.5 ref 药丸的四套「浅底 + 同色描边 + 深色字」----
// 全部取 §3.1 现成的 token，不再从参考图上量颜色（旧三枚 `#D5E5FD / #AAAAAA / #003979`
// 是那张截图里第一行 `⑂ main` 上取的，§6 迁移表把它们归到 §4.5 这四枚）。
// 分支/远程共用浅蓝（参考图与 spec 都是同一档）；标签一支 spec 单独给了一档浅绿，
// 它的描边 `#B7E0C2` **不在 §3.1 色板里**——§4.5 就地写了这个值，这里照写，
// 并在 §7 登记「色板缺一枚 `ADD_LINE`」。
pub(crate) const PILL_BG: Color32 = ACCENT_SOFT;
pub(crate) const PILL_EDGE: Color32 = ACCENT_LINE;
pub(crate) const PILL_INK: Color32 = ACCENT_INK;
/// 「超前/落后」那枚：**深藏青底白字**（§4.5），且贴死在前一枚右侧合起来才是一颗胶囊。
/// 藏青就是 `ACCENT_INK` `#0550AE`——spec 把同一个值写了两遍，这里不重复定义。
pub(crate) const PILL_AHEAD_BG: Color32 = ACCENT_INK;
pub(crate) const PILL_TAG_BG: Color32 = ADD_BG;
pub(crate) const PILL_TAG_EDGE: Color32 = Color32::from_rgb(0xB7, 0xE0, 0xC2);
pub(crate) const PILL_TAG_INK: Color32 = ADD_FG;
// ---- §3.2 文件状态色块：**六种状态各有自己的颜色**，白字 11px、12pt 见方、圆角 3。
// 旧那三枚（琥珀 `…` / 玫瑰 `R` / 紫 `?`）是 SourceTree 参考图量的，剩下几种落灰底；
// 新 spec 给了六枚，`badge_of` 里就不再有「没量到所以落灰」那一条出口。
pub(crate) const BADGE_MOD: Color32 = Color32::from_rgb(0xD2, 0x99, 0x22);
pub(crate) const BADGE_ADD: Color32 = Color32::from_rgb(0x1A, 0x7F, 0x37);
pub(crate) const BADGE_DEL: Color32 = Color32::from_rgb(0xCF, 0x22, 0x2E);
pub(crate) const BADGE_REN: Color32 = Color32::from_rgb(0x82, 0x50, 0xDF);
/// 未跟踪 = 辅助灰（§3.2 亮/暗两档同值）。旧的紫底白 `?`(101,73,129) 作废。
pub(crate) const BADGE_UNK: Color32 = Color32::from_rgb(0x6E, 0x76, 0x81);
/// 冲突 = 橙（§3.2）。旧代码里冲突落 `BADGE_NONE` 灰底，那枚常量随之外删。
pub(crate) const BADGE_CONF: Color32 = Color32::from_rgb(0xFB, 0x85, 0x00);
/// 色块见方：旧 14pt（两张参考图量的）→ §3.2 的 12pt。
pub(crate) const BADGE_S: f32 = 12.0;
/// 色块圆角（§3.5「`3` 状态色块」）。
pub(crate) const BADGE_R: f32 = 3.0;
/// 文件表那一格的四档落点。参考图（`sourcetree-4.2.19-repo-window.png`，面板左缘那条
/// 1pt 分隔线在 x=140，往右 141 起算）实测：分类行方框 **x 149**、文件行方框 **x 165**、
/// 状态图标 **x 188**、文件名 **x 208.5** —— 也就是离面板左缘 8 / 24 / 47 / 67.5。
///
/// 中间有一批把文件行那三档各减 8（17 / 39 / 60），**这一批改回来了**：用户 2026-10-21
/// 说「我知道为啥文件改动列表内容需要左移一点（勾选的 box 没了，但是没有移动位置）」——
/// 那句「往左移」要修的是**没有方框那一栏白留一格空位**（某条提交的改动列表），
/// 不是工作区那两栏的缩进。空位收掉之后，工作区按参考图排。
/// 方框与图标都是 **13 见方**（§2「圆角勾选框 **13pt**」）。参考图三枚方框、两枚图标
/// 量到的是 13.5，和 spec 差半像素——按 S6 的规矩以 spec 为准，半档归进 §7。
pub(crate) const CHK_S: f32 = 13.0;
pub(crate) const CHK_HEAD_X: f32 = 8.0;
pub(crate) const CHK_ROW_X: f32 = 25.0;
pub(crate) const ICON_X: f32 = 47.0;
pub(crate) const NAME_X: f32 = 68.0;
/// 勾上的方框：参考图那三枚中心色都是 (71,141,247)（边缘被抗锯齿拉到 (54,122,246)）。
pub(crate) const CHK_BLUE: Color32 = Color32::from_rgb(71, 141, 247);
/// **这一栏没有方框时**（某条提交的改动列表）两档落到哪儿：色块顶到最左边那一格
/// （和分类行的全选框同列），文字挪到**分类标题同一列**（8 + 13 + 7 = 28）。
/// 留着方框那 22pt 空档就是用户说的那句「勾选的 box 没了，但是没有移动位置」。
pub(crate) const NO_BOX_ICON_X: f32 = CHK_HEAD_X;
pub(crate) const NO_BOX_NAME_X: f32 = CHK_HEAD_X + CHK_S + 7.0;
// 没勾上那面 = 白底 + 1pt `BORDER_STRONG`（§3.1「输入框/按钮描边」同一枚灰）；
// 旧的 `CHK_EDGE`(190,190,190) 是估的，现在跟着 token 走。
/// 差异区那格文件标题的底色带。**底色 §3.1 有 token**：`BG_INSET` 那一行写的用途就是
/// 「输入底、**hunk 头**、虚拟行」，S6-5 换锚后不再用参考图上量的 (236,236,236)。
/// **高度和左边距 spec 没给**：两张参考图量到的是高 30pt（一张 y338..368、一张 y378..408）、
/// 色块离带左缘 8pt（585-577、665-657），§7 登记「自拍」。折叠时文件头那条（`Kind::File`）
/// spec 也没单列，和本带共用同一枚 token。
pub(crate) const DIFF_HEAD_BG: Color32 = BG_INSET;
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

/// 模态卡那一层的尺寸档位。**宽度只有 §5.1 的三档，高度跟着内容走**（2026-10-24 起）：
/// 这三张卡以前各自钉死在 SourceTree 那三张 nib 的窗口尺寸上（855×441 / 650×271 / 614×299），
/// 可卡里只有两三行控件，剩下的全是一片空白——目视那一轮（用户：「提交、推送、设置窗口都好丑」）
/// 就是这么抓出来的。剩下的全是**行的档位**：一行多高、脚部多高、控件多大。
///
/// 宽度三档（§5.1「确认卡 380 · 标准卡 440 · 宽卡 560」）。以前这里是一卡一号：提交/添加 420、推送 600、拉取 520、偏好 520，四个数没有一个来自 spec。
/// 归属：宽卡 = 提交/拉取/推送/偏好/添加仓库；标准卡 = 新建分支/标签/贮藏/重命名/合并/重置；
/// 确认卡 = 那道闸自己。重置卡 §5.1 没点名（它是「会丢什么」的那一句问话），拍标准档，见 §7。
pub(crate) const CARD_W_CONFIRM: f32 = 380.0;
pub(crate) const CARD_W_STD: f32 = 440.0;
pub(crate) const CARD_W_WIDE: f32 = 560.0;
/// 宽卡里内容区的右缘（`560 − 16`，§5.2「内边距 16」）。推送那张表的外框、拉取那只「选项」框
/// 和「刷新」都钉在这一格上，所以它们跟着宽度档走，不再各带一枚宽度常量。
pub(crate) const CARD_BODY_RIGHT: f32 = CARD_W_WIDE - MODAL_PAD;
pub(crate) const MODAL_TITLE_H: f32 = 42.0;
/// 卡里一行的**行距**（不是控件高）。§4.2 的输入框 30 是卡里最高的那格，行距比它大一档
/// 才不让上下两格贴住；4/8 网格（原则④）上取 32，留 2pt 缝。自定数已登记 §7。
pub(crate) const MODAL_ROW_H: f32 = 32.0;
/// 底部按钮区高（§5.2「底部按钮区高 52」）。
pub(crate) const MODAL_FOOT_H: f32 = 52.0;
/// 卡内容的左右内边距。
pub(crate) const MODAL_PAD: f32 = 16.0;
/// 卡里两组控件之间的那道缝。
pub(crate) const MODAL_GAP: f32 = 12.0;
/// 卡圆角（§3.5「圆角 12 卡片/弹窗/窗口」，旧 10 作废）。
pub(crate) const CARD_R: f32 = 12.0;
/// 遮罩（§5.1「整窗 `rgba(31,35,40,.48)`」）。**带色**，不是旧的无色黑 `from_black_alpha(120)`：
/// 0.48 × 255 = 122.4 → 122。const 那一档叫 `from_rgba_unmultiplied_const`（`ecolor-0.37.0/src/color32.rs:139`），
/// 普通那个不是 const fn。
pub(crate) const MODAL_SCRIM: Color32 = Color32::from_rgba_unmultiplied_const(31, 35, 40, 122);
/// 卡的投影（§5.1「真阴影：offset 0/8、blur 24、黑 40%」）。旧实现是**两圈空心描边冒充**的，
/// egui 0.37 有真阴影（`Shadow::as_shape` 出一条带 `blur_width` 的 `RectShape`），描边撤了。
/// 40% = 102/255。
pub(crate) const CARD_SHADOW: eframe::egui::epaint::Shadow = eframe::egui::epaint::Shadow {
    offset: [0, 8],
    spread: 0,
    blur: 24,
    color: Color32::from_black_alpha(102),
};
/// 输入框：高与圆角（§4.2「高 30，圆角 6」，旧 26 作废）。
pub(crate) const FIELD_H: f32 = 30.0;
pub(crate) const FIELD_R: f32 = 6.0;
/// 卡脚那两枚按钮：高、圆角、最小宽（§4.1「主按钮 高 28，圆角 6」，旧的 26/7 作废）。
pub(crate) const BTN_H: f32 = 28.0;
pub(crate) const BTN_R: f32 = 6.0;
pub(crate) const BTN_MIN_W: f32 = 64.0;
/// 分段选择那排页签的容器高（§4.4「容器 `BG_INSET` 圆角 8，高 30」）。
/// 和输入框同高不是巧合：卡里同一竖排上挨着的两格本来就该一样高。旧值是 `MODAL_ROW_H - 6`
/// 再 `shrink(1.0)` = 24，spec 里没有这一档。
pub(crate) const SEG_H: f32 = 30.0;
/// 卡里一枚勾选方框见方多大（同时也是它的命中矩形：egui 的 checkbox 取
/// `interact_size.y` 和 `icon_width` 里大的那一档，卡这一层把两者配成同一个数）。
pub(crate) const CHK_CARD: f32 = 18.0;
/// 提交卡里描述那一格给三行（和内联那格 `desired_rows(3)` 同一档）。
pub(crate) const DESC_ROWS: usize = 3;
/// 偏好卡按**页里最多那一条**摆高度（「推送与拉取」两枚弹出 + 两枚勾 = 四行）：
/// 换页时卡不跟着跳，这是唯一一处「不为内容收缩」的取舍，换来的是页签稳定。
pub(crate) const SETTINGS_ROWS: usize = 4;

/// 主按钮那枚蓝：底色、悬停、按下、焦点那一圈光晕。
/// §3.1 给的是 `ACCENT` #0969DA（旧值 = SourceTree 那枚 `SELECTED` 蓝，随实心选中行一起作废）。
/// 悬停/按下 spec 没给单独档位：取同色相压暗一档 / `ACCENT_INK` 那档深蓝，
/// 自定数已登记在 PROCESS「spec 没给、我们自己拍的」。
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x09, 0x69, 0xDA);
pub(crate) const ACCENT_HOVER: Color32 = Color32::from_rgb(0x08, 0x5F, 0xC4);
pub(crate) const ACCENT_DOWN: Color32 = ACCENT_INK;
/// 浅底上的主色文字（§3.1），也是按下那一档的出处。
pub(crate) const ACCENT_INK: Color32 = Color32::from_rgb(0x05, 0x50, 0xAE);
/// 选中底、胶囊 tab 底（§3.1）。S6-2 顶栏那枚选中的仓库 tab 是它的第一个消费者：
/// §2「选中 = 主色浅底深字」= `ACCENT_SOFT` 铺底 + `ACCENT_INK` 落字。
pub(crate) const ACCENT_SOFT: Color32 = Color32::from_rgb(0xE8, 0xF1, 0xFD);
/// 焦点那一圈光晕 = `ACCENT_LINE`（§3.1 那枚浅蓝，取代旧的 `ACCENT_RING`）。
/// §3.5 写的是「外圈 2px `ACCENT_SOFT`」，这里画的是 2.5px `ACCENT_LINE` 实心描边：
/// 半透明在 const 里做得了（`Color32::from_rgba_unmultiplied_const`，旧注释那句「做不了」是错的），
/// 但浅一档的 `ACCENT_SOFT` 描在**白卡**上几乎看不见，取色板里同色相那枚线才出得来预览稿的效果。
pub(crate) const ACCENT_LINE: Color32 = Color32::from_rgb(0xB9, 0xD6, 0xF9);
// 次按钮（§4.1 幽灵按钮）= 白底 + `BORDER_STRONG` 描边 + `TEXT_SECONDARY` 字，
// 三样都是 token，这里不再单设 `BTN_EDGE`。
/// 表格表头那一条的底色。
pub(crate) const TABLE_HEAD_BG: Color32 = Color32::from_rgb(0xf6, 0xf7, 0xf9);

// ---------------------------------------------------------------- 推送卡 / 拉取卡
// 这两张卡的**字段、顺序、每一格叫什么**来自本机那两张 nib
// （`GitPushSheet` / `GitPullSheet`，测量记录见 docs/PROCESS.md「逆向本机 SourceTree.app」），
// 但**格子摆在哪**是这一版自己排的：nib 那套坐标是给一张 650×271 的空窗口排的，
// 我们按内容收缩以后那些数就不再成立（2026-10-24）。

/// 推送卡：宽度走 §5.1 的宽卡档（`CARD_W_WIDE`），高度**跟分支行数走**（`push_body_h`）。
/// 「推送到仓库：」那一行的顶边（标签 / 弹出按钮 / URL 三件控件同一行）。
pub(crate) const PUSH_ROW1_Y: f32 = 14.0;
pub(crate) const PUSH_LABEL_X: f32 = 16.0;
pub(crate) const PUSH_REMOTE_X: f32 = 132.0;
pub(crate) const PUSH_REMOTE_W: f32 = 120.0;
/// 那一格显示所选远端的 URL：从弹出按钮右边一路铺到内容区右缘。
pub(crate) const PUSH_URL_X: f32 = 264.0;
pub(crate) const PUSH_URL_W: f32 = CARD_BODY_RIGHT - PUSH_URL_X;
/// 「要推送的分支」那一行：左边标题，右边「全选」。
pub(crate) const PUSH_HEAD_LABEL_Y: f32 = 48.0;
/// 表格：外框顶边（= 表头顶边）、表头高、行高、四列宽。
/// 判据里 `push_row_box` / `push_remote_cell` 按这几个数算落点，改这里它们跟着改。
pub(crate) const PUSH_TABLE_X: f32 = 16.0;
pub(crate) const PUSH_TABLE_TOP: f32 = 74.0;
pub(crate) const PUSH_HEAD_H: f32 = 26.0;
pub(crate) const PUSH_ROW_H: f32 = 22.0;
pub(crate) const PUSH_COL_PICK: f32 = 46.0;
pub(crate) const PUSH_COL_LOCAL: f32 = 172.0;
/// 远程分支那一列是**剩下那一格**：外框右缘必须钉在内容区右缘上，所以它由另外三列算出来，
/// 不自己拍数（旧 292 是 600 宽那一档量的，收进 560 档就溢出了）。
pub(crate) const PUSH_COL_REMOTE: f32 = CARD_BODY_RIGHT
    - PUSH_TABLE_X
    - PUSH_COL_PICK
    - PUSH_COL_LOCAL
    - PUSH_COL_TRACK;
pub(crate) const PUSH_COL_TRACK: f32 = 50.0;
/// 表格外框右缘 = 四列加起来。
pub(crate) const PUSH_TABLE_RIGHT: f32 = PUSH_TABLE_X
    + PUSH_COL_PICK
    + PUSH_COL_LOCAL
    + PUSH_COL_REMOTE
    + PUSH_COL_TRACK;
/// 框下面那两枚勾的顶边：框底 + 一道缝。
pub(crate) fn push_checks_y(n: usize) -> f32 {
    PUSH_TABLE_TOP + PUSH_HEAD_H + PUSH_ROW_H * n as f32 + MODAL_GAP
}
/// 推送卡正文高度（标题带与卡脚由 `card_rect` 另加）。
pub(crate) fn push_body_h(n: usize) -> f32 {
    push_checks_y(n) + MODAL_ROW_H
}

/// 偏好卡正文高度：页签那一排 + 最多那一页的行数。
pub(crate) fn settings_body_h() -> f32 {
    MODAL_ROW_H + MODAL_GAP + MODAL_ROW_H * SETTINGS_ROWS as f32
}

/// 拉取卡：宽度同样走宽卡档（§5.1），四行 + 「选项」那只框，正文高度固定（`PULL_BODY_H`）。
pub(crate) const PULL_LABEL_X: f32 = 16.0;
pub(crate) const PULL_FIELD_X: f32 = 176.0;
/// 远端那一格和它的 URL 一路铺到内容区右缘。
pub(crate) const PULL_REMOTE_W: f32 = CARD_BODY_RIGHT - PULL_FIELD_X;
/// 远程分支/本地分支那两格窄一档，右边让给「刷新」（BTN_MIN_W + 一道 8pt 缝）。
pub(crate) const PULL_BRANCH_W: f32 = CARD_BODY_RIGHT - PULL_FIELD_X - BTN_MIN_W - 8.0;
/// 四行的顶边：远端、它的 URL、远程分支、本地分支。
pub(crate) const PULL_REMOTE_Y: f32 = 14.0;
pub(crate) const PULL_URL_Y: f32 = 44.0;
pub(crate) const PULL_BRANCH_Y: f32 = 76.0;
pub(crate) const PULL_INTO_Y: f32 = 108.0;
/// 「选项」那一格标题，和它下面那只框的顶/底、框里那三行开关。
/// 第一行「立即提交合并的改动」是 git 的默认行为（`--commit`），按「点了看不出区别的勾
/// 不如不摆」这条**故意不摆**，理由写在 `git::PullOpts` 上。
pub(crate) const PULL_OPT_LABEL_Y: f32 = 142.0;
pub(crate) const PULL_BOX_TOP: f32 = 168.0;
pub(crate) const PULL_BOX_BOTTOM: f32 = 252.0;
pub(crate) const PULL_OPT_X: f32 = 26.0;
pub(crate) const PULL_OPT_LOG_Y: f32 = 178.0;
pub(crate) const PULL_OPT_NOFF_Y: f32 = 202.0;
pub(crate) const PULL_OPT_REBASE_Y: f32 = 226.0;
pub(crate) const PULL_BODY_H: f32 = 264.0;

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

/// 车道配色：§3.3 给了四枚——蓝 `#0969DA` / 绿 `#1A7F37` / 紫 `#8250DF` / 橙 `#CF6A00`。
/// 旧那八枚是复刻期从参考图 + 旧 React 调色板拼的（第 0 道 `#005FBD` 是量出来的，后面七枚
/// 台账自己写了「没有依据」），§6 迁移表「图序列 4 色 + `RING_GREY` → §3.3」说的就是这一批。
/// **第 5 道起按 4 取模循环**：spec 只给了四枚，而 `layout_graph` 裁尾巴后实际很少超过四道
/// （`the_graph_never_escapes_its_column` 钉的就是这个上限）——同色相循环比再造四枚没依据的色诚实。
/// 别随手改：按**颜色**认领图元的那条判据（`graph_marks`）认的就是这四种。
pub(crate) const LANE_1: Color32 = Color32::from_rgb(0x09, 0x69, 0xDA);
pub(crate) const LANE_2: Color32 = Color32::from_rgb(0x1A, 0x7F, 0x37);
pub(crate) const LANE_3: Color32 = Color32::from_rgb(0x82, 0x50, 0xDF);
pub(crate) const LANE_4: Color32 = Color32::from_rgb(0xCF, 0x6A, 0x00);
pub(crate) const LANE_COLORS: [Color32; 4] = [LANE_1, LANE_2, LANE_3, LANE_4];

pub(crate) fn lane_color(lane: usize) -> Color32 {
    LANE_COLORS[lane % LANE_COLORS.len()]
}

/// 顶栏动作组：11 枚，右对齐，每枚是 §4.1 那颗 28×28 的图标按钮。
/// 第二个字段（中文）**不再画在顶栏上**——它只给 tooltip 和 §2 那句「溢出收进 `»` 菜单」用。
/// **图标只能是字体链里真有字形的那些**：`every_rune_the_shell_paints_has_a_glyph`
/// 这条无头判据会把每一枚落过字的码位跑一遍。之前那批箭头、带圈字母和两个彩色 emoji
/// 全都不在 Noto Sans SC 的 cmap 里，于是有七个是豆腐块——这不是配色问题，是根本没字。
/// 换成真线描 SVG 排在 S6-6（§4 那批组件）；在那之前先用字体里确实存在的字形顶着。
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
