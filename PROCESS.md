# GitCraft 开发进度与 TODO

> 这份台账只留**最近两批的全文** + 可复用的结论。更早的逐批叙述一行一条，全文原样搬进了
> [`docs/process-archive/`](file:///Users/baorong/Developer/superlbr/gitx/docs/process-archive)（精简时逐字节搬的，
> 一行没改；再早的版本也在 `git log -p PROCESS.md` 里）。
> **`docs/process-archive/react-tauri.md`** 写的是 React + Tauri 那一层（2026-10-08 ~ 10-20，那层已删，
> 最后一份代码在提交 `9a3077c`）——它仍然是「哪条判据为什么存在、哪个数字是从哪里量出来的」的记录，
> 捡功能回 egui 时从那儿取源码、照抄判据；里面的 t8y2/dbx、`highlight.js`、vitest、localStorage 是当时的栈。
> **`docs/process-archive/egui-shell.md`** 是 egui 壳第一批到「文件表方框列」那一批的全文。
>
> 每批四段：**决定与依据 / 改了什么 / 判据 + 反向变异表 / 仍然没看到过的**。最后一那段不许省。

## 契约：复刻 SourceTree，「更好」= 性能与功能

用户拍板（2026-10-20）：「**复刻要求界面完全一样（除了软件名），更好是指要超越 sourcetree，性能/功能**」。

- **布局照抄 + 交互语义照抄**，不是「功能对齐就行」。美观、主题、动效**不在**比较轴上。
- 「更好」收成两条可量的轴：① 性能（大仓库、28 875 行级别的 diff 不卡，每帧的活只跟窗口大小有关）；
  ② 功能（它有的我们都有，再加它做不到的：命令层可被 Agent/MCP 驱动、命令日志回查、作用域搜索）。
- 2026-10-15 的「双左栏合一（删侧栏）」**就此作废**：SourceTree 有一条贯穿全高的左侧大纲栏。
- **2026-10-21 再拍板：复刻是底线，不是天花板**（用户原话「总要比 sourcetree 更好的」）。
  于是分成三条可判的规则，别再拿「完全一致」四个字当挡箭牌：
  ① **不许重排它已有的布局**；② 它**没有**的地方可以自加，但必须在文档里标成「我们的增强」，
  不许写成「补 SourceTree 有的」；③ 它**有**而我们缺的，一律按它的文案与结构照抄。
  按这三条重排本轮的十条诉求：`#34 标题栏合并`、`#35 分支名落点`、`#31 筛选条`、`#38 推送弹窗`、
  `#37 分组项目列表` = ③；`#36 推送/拉取角标` = ②（**注意**：参考图证实「提交」那枚**官方就有**角标，
  所以角标本身不是自加，自加的只是"只留推送/拉取"这个取舍）。
- **不凭印象复刻**：界面事实只有两个来源——本机 4.2.19 应用包（`.nib` + `zh_CN.lproj/Localizable.strings`
  888 条）和 `docs/reference/` 那四张真截图（**Atlassian 专有素材**，再分发必须去掉）。
  逐块的「它有什么 / 我们缺什么」全量对照表在归档里（react-tauri.md「SourceTree 界面对照表」）。
- **读本机应用包的正确姿势**（2026-10-21 摸清，之前一直低估了它能给的）：
  `Localizable.strings` 是 **UTF-16LE**，要 `iconv -f UTF-16 -t UTF-8`；`.nib` 是**目录**
  （两份 keyed archive）。keyed archive **不是** plist：`plutil -convert xml1` 直接拒、
  `ibtool --dump` 这个选项不存在 —— 所以**数字**（窗口最小尺寸、`NSToolbarDisplayMode` 取值）拿不到。
  但**结构与文案拿得到**：把 nib 的字节按 utf-8 / utf-16-le 各解一遍、用正则扫中文串，
  就能读出每张卡上有哪些控件、按钮写什么、单元类叫什么。本轮靠它定了四件事：
  推送弹窗的字段（`GitPushSheet`：推送到仓库：/要推送的分支/本地分支/远程分支/跟踪/全选/推送所有标签/强制推送）、
  筛选条的真身（`FileStatusView2`：仅显示·待定/冲突/未跟踪/已忽略·清除·排序方式·平面列表/单列/多列/树状视图）、
  项目列表带分组在**另一个窗口**（`RepoBrowserWindow`：新建仓库组/全部展开/折叠所有/按名称排序/按类型排序，
  类名 `BookmarkCellView`/`FolderCellView`）、以及**没有 tab**（全二进制搜不到 tab 相关类）。
  唯一翻车的地方：**「搜不到字符串」不等于「没有」**——`badge` 只在偏好设置里命中，我就判了
  「工具栏没有角标」，而参考图上「提交」图标明明白白挂着「24」的蓝角标。见 #34 那节。

### 复刻规格：SourceTree 主窗口实测几何（证据 = 真截图 `docs/reference/sourcetree-4.2.19-repo-window.png`）

1050×545 逻辑 pt @2x，聚焦窗口的选中蓝 `(41,98,217)`。下面每条都能在图上指认。

**几何**
- 侧栏右边界 x=282 → **141 pt**，从工具栏下方一直贯到底。
- 工具栏带 y≈0..110 → **55 pt**（含红绿灯与「gitx (Git)」标题）；其下内容筛选条约 40pt。
  **2026-10-21 重量：这条带是 52pt，不是 55。** 两张参考图逐行扫底色都是
  `244,249,246` 从 y=0 到 y=51、y=52 一条 `202,206,203` 分隔线、y=53 起换 `240,244,241`（筛选条带）。
  55 是把分隔线和它下面两三个像素一起算进去了。同批量出的三个数，#34 全用上了：
  **红绿灯** = 三枚 12pt 圆，x 20..32 / 40..52 / 60..72（右沿 **72**）、y 20..32；
  **仓库名左沿 = 91.5**（红绿灯右沿 +19.5）；**最右一枚按钮右沿 1099 / 窗口 1120 → 右边距 21**。
  → 这三条把「顶栏」的真实结构钉死了：**那一行就是 macOS 标题栏本身**，
  我们之前在它下面又画一条带，等于顶上白占 28~32pt，而且仓库名排在 x=9 —— 正被红绿灯压住。
- 日志列边线 x **198 / 275 / 739 / 929** → 图表 **57**、提交 **77**、作者 **190**、日期 **121**，列内缩 **5pt**。
- 数据行行距 **26pt**（中心 130→156→182→210），表头那条 **24pt**（比数据行矮 2），表头**有** 1pt 竖线。
- 第 0 道圆点中心 = 图列左缘 **+22.75**；`LANE_W = 12` **是估的**（手上所有样本只有一道有东西）。
- 日志带 / 底部带分界 y≈600 → **300 pt = 窗口高的 55%**；底部竖分界 x≈1152 → 文件列表 **435 pt**、差异 **474 pt**。
- 差异区顶栏带 **(236,236,236) 高 30pt**，色块离带左缘 **8pt**、**14×14**；改动 (238,197,91)、重命名 (163,115,122)、未跟踪 (101,73,129)。
- hunk 头右端两枚按钮 **55×15**、中间 9、离右缘 9；hunk 那条带在参考图里是 **29pt 双倍行高**。
- 文件表列（从面板左缘 141 起算）：分类行方框 **+8**、**13.5 见方**、标签离框 **7**；
  文件行方框 **+25**、状态图标 **+47**（13.5 见方）、文件名 **+68**、行末「⋯」x 519..534.5。文件行距量到 **27**（**未采纳**，`ROW_H=26` 与日志共用）。
  **2026-10-21 重量 + 拍板**：实测是 8 / **24** / 47 / **67.5**（方框 x 149、165，图标 188，名字 208.5），
  旧记录那三个数是四舍五入来的；文件表**画出来的**一直正是这三档（旧判据绿的）。
  用户「内容往左移动一点」→ 文件行那三档各减 8（**17 / 39 / 60**），分类行留在 +8。
  这条是**偏离 SourceTree 的自加调整**，见 #39 那节。
- 日志字色：普通 (38,38,38)、虚拟行 (33,33,33)；侧栏文字列左缘 **x=41**（四组标签同列）、图标格 18..38（线描 (69,81,108)）、
  分类行距 **47pt**、「工作区」30pt、子项 26pt、选中灰药丸 x 10..129.5 高 26 圆角 ~5 底色 (204,208,205)、当前分支蓝点圆心 x 29.5 直径 7 (36,93,184)。
- 勾上的蓝中心 (71,141,247)；**未勾选的方框两张参考图里一枚都没有**，`CHK_EDGE` (190,190,190) 是估的。

**结构事实（比几何更要命）**
> 这条标题原来写的是「从 nib 字符串读不出来」。2026-10-21 更正：**结构读得出来**（把 keyed archive
> 按 utf-8 / utf-16-le 各解一遍扫中文，控件与按钮文案全在），**数字读不出来**（keyed archive 不是
> plist，`plutil` 拒、`ibtool --dump` 不存在）。下面这些是当年靠眼睛看出来的，现在有了第二条路。
1. **没有「工作区 / 历史」两个 tab**：就一页，上面日志、下面文件+差异；选虚拟行「Uncommitted changes」= 工作区，
   选某个提交 = 那个提交的改动。这一度是最大结构性差异，egui 版已按它做。
2. 虚拟行常驻日志第一行，提交列与作者列都是 `*`，日期本地化（「今天 12:08」），选中整行蓝底白字。
3. 侧栏是**大纲**：`WORKSPACE`（灰字 + 显示器图标）下挂 文件状态 / 历史 / 搜索，再往下 分支 / 标签 / 远端 / 已贮藏 /
   子模块 / 子树 各自成组，**最底一条「筛选器」输入框**（不是顶部搜索）。
4. 工具栏是**图标在上、中文在下**的竖按钮，右对齐 11 枚：提交 / 拉取 / 推送 / 抓取 / 分支 / 合并 / 贮藏 /
   显示远程服务器 / 在 Finder 中显示 / 终端 / 设置；左侧只有「仓库名 (Git)」标题。
   **左侧那个标题右边还跟着当前分支**（真窗口里 SourceTree 自己就显示成「comic (Git)  main」这一串），
   所以 #35 要的"分支名挪到仓库名之后"是**它本来就有**的位置。
   **而且「提交」图标左上角挂着一枚蓝色数字角标**（参考图上读数是 24，实测蓝像素包围盒
   x 527.5..543.5 / y 10.5..27.5，正好压在图标左上）—— 角标是**官方有的**，
   别信「strings 里搜不到 badge 所以没有」那种推理（见上一节最后一条）。
5. 文件列表分组标题自带**复选框**（整组一键切换），列表上方一条「待定的文件, 已依照路径排序」下拉 + 视图模式图标 + 「⋯」。
6. 差异区顶栏 = 图标 + 文件名 + 「…」；**灰带下面第一行就是 hunk 那条带**
   （2026-10-21 逐行数出来的：灰带 (236,236,236) 止于 y=368pt，y=368..396 已是 (240,244,241)）——
   `diff --git` / `index` / `---` / `+++` 这些 git 的文件头**一行都不画**，
   `git show` 自带的 `commit` / `Author:` / `Date:` / 提交说明也不画（核心层 `--format=` 掐掉）。
   hunk 头「块 1： 行 8-14」（**只报旧侧一个区间**），
   右侧 **暂存区块 / 放弃区块**（已暂存那格换成「取消暂存区块」且没有放弃那枚），行号两列，增删整行淡红/淡绿底。
7. 书签是**另一个窗口**：本地/远端/新建 三段 + 「过滤仓库」+ 按文件夹分组的树（头像 + 仓库名 + 分支芯片）。
   → 我们的 tab 条 24pt **没有参考依据**，是自己定的。

**它有一整块我们还没碰的**：git flow 全套、submodule + subtree、内置三向合并、LFS track/初始化、
Create/Apply Patch、Custom Actions、Move/Revert to Revision、冲突 Mark Resolved / Using Mine·Theirs、
作用域搜索面板、命令历史窗口（每条 git 的 argv/stdout/耗时）、CreatePullRequest / CreateRemoteRepo / Pipelines、
内置 SSH 密钥、WelcomeWizard、RepoSettings/Preferences、ArchiveSheet。
工作区页另有**六种模板**（Tree / Flat 单列 / Flat 多列 × No staging / Split view staging / Fluid staging，
两个正交维度可选），历史页有图遍历开关（仅首个父级 / 所有父级 / 所有分支）与「隐藏远程分支」这类显示过滤。

**仍然没量过的**：`Fluid staging` 的交互、`Split view staging` 是上下还是左右、各右键菜单的项序与图标、
深色模式、搜索面板作用域条、四个下拉各自的选项、窗口宽度 ≠ 1050 时的行为、侧栏滚动后的样子。

## 技术栈与门禁（2026-10-20 起）

- 核心 `gitcraft-core`：纯 Rust，git CLI（不用 libgit2）+ SQLite（`rusqlite`），**零 UI 依赖**。
- 界面 `gitcraft-app`：egui 0.37 / eframe，直连核心、**没有 IPC 那一跳**，自带一份 CJK 字体（不入库，见 README）。
- 判据：`gitcraft-core` 83 例 + `gitcraft-app` 90 例（无头，不需要屏幕），四条门禁串在
  [`scripts/gates.sh`](file:///Users/baorong/Developer/superlbr/gitx/scripts/gates.sh)；
  第四条是 `scripts/license-audit.sh`（许可证与素材审计，本地和 CI 调同一份实现）。
- **egui 构建零警告是门禁**：任何 warning（含弃用 API）都算失败。
- 窄门禁：`cd gitcraft-app && cargo test geometry_tests::<名字> -- --exact`（**必须带模块路径**，
  只给叶子名字匹配到 0 条 = 假绿）。整批改完再统一跑一次全量。
- 一次性探针（`uiprobe/`、临时调试例）跑完就删，结论写进本台账。
- **#25 那次真窗口目视已经补做**（2026-10-21 下午，用户截图 4 张：GitCraft 弹层打开 / SourceTree 参考 /
  文件状态 / 拉窄）。逐条缺陷已经分流到上面「任务单上欠的」和 #30 那节，不再有单独的待办；
  截图本身不在库里（`~/Downloads`，跑完就没了）——**这份台账里记的是量到的结论，图没了也就没了**，
  下次要复核得重新截。

---

## 累计的 egui 0.37 / 无头脚手架事实（下次别再猜）

**渲染与样式**
- `Context::default()` 是 **Dark**；只改 `Visuals::light()` 会静默失效 → 必须 `ctx.set_theme(Theme::Light)`。
- **新建的 `Context` 第一遍不画 `Order::Foreground` 层**（纯 egui 两三行复现过，与本项目无关）。模态卡就画在那一层，
  所以凡在卡上定位文字的助手都得先空跑一遍（harness 里的 `warm` / `warm_frame` / `click_warm`）。
  面板那一遍正常 → **只测遮罩会假绿**（那一遍卡根本没画，却以为是遮罩挡住了）。
- egui 把已注册 Area 的**整层透明度乘一遍**（`containers/area.rs:706` `ui.multiply_opacity`）：
  `Color32::from_black_alpha(80)` 实际落成 alpha 19。遮罩判据只能问「全屏 + 黑 + 0<alpha<255」，别钉死数字。
- 1pt 线落在半像素上会被抗锯齿摊开 → 实拍图上找不到我们的表头竖线；那种判据只能在 `Shape::LineSegment` 层。
- `RichText::strong()` 没带 Bold face 时只改颜色不改字面（Bold 至今没有）。
- 字体：eframe 默认 feature 含 `system_fonts`，**一个字体都不注册也能出中文和 emoji**（推翻 10-16 的论据），
  但兜底按字符零敲碎打——同一个窗口里 U+FF1A 全角冒号在 Proportional 家族是豆腐块、Monospace 正常，
  所以产品必须自带全覆盖 CJK。`.ttc` 是字体集合，ab_glyph 解析不了，只收单面 TTF/OTF。
  字体解析失败 / face index 越界 → panic 落在不可 unwind 的闭包里 → **进程 abort**
  （`family.rs:51`、`font_definitions.rs:249`），注册链里只放成功的面、缺文件降级。
  `PingFang.ttc` 在 macOS 27 不在 `/System/Library/Fonts/`，按 `PrivateFrameworks/.../PingFangUI.ttc` 注册
  能加载、认 CJK 码位但**画出来是空白**（比豆腐块更糟，不触发回退）→ 别按路径吃 PingFang。
  缺字体时 `.notdef` 的 advance 正好是 **0.5×字号**，所以按宽度下的断言在没字体的机器上量的是产品里看不到的字体。
- 箭头字形：`▾`(U+25BE) **不在**这条字体链里，画出来是豆腐块——不是「不好看」，那个字直接没了。
  实测有的：`▼ ˇ ↓ ﹀ ⤵ ▲ △`（外壳用 `▼`）。缺字形由 `every_rune_the_shell_paints_has_a_glyph` 用**它自己的 FontId**
  逐字查出来（`fonts_mut(|f| f.has_glyph(..))`）——先跑一帧才能查字体（`Context::run` 之前报「No fonts available」）。
- macOS 拼音：egui 0.37 的 IME 是一等公民（`ImeEvent::Preedit/Commit`、`Visuals.ime_composition`、
  `set_ime_cursor_area`），脚本注入的合成态实测正确；**真人打拼音那一半至今没结案**（辅助功能权限没开）。

**布局与交互**
- `Ui::put` = `scope_builder(max_rect, Layout::centered_and_justified)`（`ui.rs:1691`）：**不是靠左的**，
  短文字居中、带空格的长文字两端对齐。靠左走 `UiBuilder::new().max_rect(r)` + `Layout::left_to_right`（本层的 `put_left`）。
- `scope_dyn`（`ui.rs:2338`）按**子 ui 的 min_rect** 推进父游标 → `Ui::put` 把游标倒回行中间，
  而 `show_rows` 用 `cursor.min.y / row_height` 反解可见区间，几千行下来彻底对不上。
  → 每行画完补 `fix_pitch`（公开的 `Ui::advance_cursor_after_rect`，`ui.rs:1389`）。
- `Interaction::selectable_labels` 默认 **true**（`style.rs:1599`）：可选文字的 `Label` 自己叠 `Sense::click_and_drag()`
  并把光标设成 `Text`，压在整行 `Sense::click()` 上面 → **鼠标落在文字上点不中**，落在空白才中。
  `Label::selectable` 只管拖选、≠ 选中样式；0.37 没有 `Label::selected`，选中态自己铺底；`Sense::hover()` 的 `clicked()` 恒 false。
- 滚轮的下一帧就点，整个点击会丢：松开那帧在注册表里找不到按下的 Id，`potential_click_id` 直接清空
  （`interaction.rs:117-123`）。`show_rows` 的槽位 Id 跨帧不稳（`skip_ahead_auto_ids`，`scroll_area.rs:1113`，
  **成因未读通**）→ 行的 Id 由「这一行代表哪条提交」`push_id` 发。
- `max_click_duration` 默认 0.8s，超时转成拖拽 → `style()` 放到 3.0（这个值是我定的，无参考依据）。
- `ScrollArea` 只把滚轮给**指针悬停的那一块**（`scroll_area.rs:1324`）；一次离散滚轮事件被**摊平到后面几帧**
  （`wheel_state.rs`，`SCROLL_ACTION_TIMEOUT` 150ms），`smooth_scroll_delta` 逐帧衰减 → 无头喂滚动要
  每帧带 `PointerMoved`、往前推 `RawInput.time`、跑够帧数（40 帧才落满 52pt）。
- `Frame::side_top_panel` 自带 8×2 内边距（把文件表整体右移 8pt）→ 显式 `Frame::NONE`。
  全局 `interact_size.y = ROW_H` 会**漏进**侧栏底部那个输入框 → `SIDE_FILTER_H = 4 + ROW_H`。
- `Ui::menu_button`（`ui.rs:4050`）自己 new 一枚**默认尺寸**的 `Button` → 链上的 `.small()` 静默丢了。
  要带控件样式的菜单按钮走 `egui::containers::menu::MenuButton::from_button(button).ui(ui, content)`
  （`menu.rs:394`/`417`），它返回 `(Response, Option<InnerResponse<R>>)`；那对返回值没东西要接，
  点中了哪一项自己记在闭包捕获的变量上。
- `PopupCloseBehavior` 默认就是 `CloseOnClick`（`menu.rs:268`）→ 点完自己收，不需要 `ui.close()`。
- 弹出菜单、模态卡这一类 `Popup`（含 `Popup::menu`）都挂在 **`Order::Foreground`**（`popup.rs:148`）
  → 弹层上的文字要下一帧才数得到（harness 里 `click_warm` 那一遍，`popup_pick` 也空跑一次）。
- `FullOutput.shapes` 是 `ClippedShape`，**只有 `clip_rect`、没有 `order`** → 无头想按层筛形状筛不了，
  只能按几何认领（「在按钮下方 + x 落在日志带里 + 取最靠上的」）。
- `egui::Button` 的高度由 `spacing.button_padding` 说了算（10.5 号字撑出 19pt），钉不到 15pt
  → 只能自己画：`layout_no_wrap` + `rect_filled` + `rect_stroke` + `galley` + `allocate_rect`。
- `Ui::fonts(|f| …)` 给的 `&FontsView`，里面 `layout*` 要 `&mut self`；`Painter::layout_no_wrap` 是 `&self` → 量字宽走 painter。
- `Id::new` 在 0.37 **已弃用**（零警告门禁会红）→ `Id::unique`/`IdSalt`，或 `ui.allocate_rect(rect, Sense::click())` 走 auto id。
- `Rect + Vec2` 在 0.37 没实现 → `galley.rect.translate(pos.to_vec2())`。
- `Context::viewport_rect()` **存在**（`context.rs:3420`）；`InputState` 默认 10000×10000（`input_state/mod.rs:347`），
  `begin_pass` 会从 `RawInput::screen_rect` 覆写它（`:387`）。
- `FullOutput.shapes` 是 `ClippedShape`（`epaint-0.37.0/src/lib.rs:128`），**没有 `order`/`pass` 可筛**，
  但**有 `clip_rect`**：ScrollArea 把滚出界外的行**剪掉但形状照发**。所以数文字有两个口径，
  必须分开用——`all_text_rects`（未裁，看「这一格排版排到哪」）与 `visible_texts`（交集后，看「屏幕上有没有」）。
  拿未裁的矩形去求「压住」= 把剪掉的行当成撞车的行（本批第一版就这么误判过一次，见 #30）。
- 反向也成立：`cut_texts` 判「这帧确实溢出了」时**只算被切掉一部分的**（交集非负矩形）。
  整块在界外的行交集是**负矩形**、高度能到 -800，任何宽松的阈值都会被它蒙混过关 = 假证据。
- 暂存带是 `Panel::bottom("staging").default_size(STAGING_H)`：**带高写死 245**，所以带内的排版和
  窗口高度**无关**（545 / 656 / 900 量到同一组相对矩形，只是整带往下挪）。窗口高度真正会动的只有
  「带被压到 245 以下」那一档（高 < 55+24+245=324 时 `Panel::bottom` 被夹扁）。
- 窗口**宽度**才是撞车的那一维：无头扫 500→1050，**770 起 0 处重叠，765 起顶栏胶囊压 ⊕（1.9pt），
  755 起日志作者列压进描述列（11.9pt）**。修法不是重排提交框，是给窗口一条地板 `MIN_W/MIN_H`。
- 布局系统给不了「标签左缘 = 面板 x=41」这种约束（滚动区、内边距、item 间距每层都会挪它）→
  侧栏自己排行：`allocate_exact_size` + 绝对坐标，`CollapsingHeader` 撤掉。
- 行与行**没有死带**（26pt 一格接得上）；一行右端 14pt 归竖滚动条，点不中。

**无头 harness**
- 一帧 = `Context::run_ui` + `root_ui`，不必伪造 `eframe::Frame`；`RawInput.screen_rect` 就是内容区 1050×545。
- `FullOutput` **drop 前必须 `out.textures_delta.clear()`**，否则 epaint `textures.rs:333` 的 `debug_assert!` 崩
  （「Dropped TexturesDelta with 1 unapplied deltas」）。
- 焦点在 `Context` 的 memory 里 → 点击与敲字**必须共用同一个 Context**；`raw_input` 里 `focused: true` 不承重（改 false 一条都不红）。
- 修饰键：`InputState.modifiers` **只由 `Event::ModifiersChanged` 更新**（`input_state/mod.rs:434`），
  键盘事件自带的那份不看。真窗口 winit 先发修饰键，所以人手没问题、**是无头测试在骗自己**。
- 输入法上屏那一刻控件收到的是 `Event::Text`，所以 `type_into` 先点出焦点再发 `Text`。
- 每个 `frame()` 新建 Context 要重新解析 8.3MB 字体：无头每帧 +8.6ms（真窗口一进程只解析一次）。
  `style(ctx)` 第一行就 `fonts::install(ctx)`，让窗口与无头共用同一条装配路径。
- `locate()` 按 `starts_with` 认文字、取**第一个**命中 → 往界面加文案前先想「这串字在帧里唯一吗」。
  撞过三次：「添加」被「添加仓库」抢走（确认按钮改叫「打开」）；模态卡 detail 行「目标目录填相对路径按主目录算」抢在输入框前面；
  **侧栏先于日志带落笔**，于是 `locate("所有分支")` 这类操作条助手会命中侧栏里同名的分支行。
- 操作条上的弹出按钮一律用 `popup_pick(app, 按钮前缀, 菜单项)` 点：找按钮和找菜单项都加  `center().x > SIDEBAR_W + 1.0`，菜单项只认**按钮下方 4pt 以上**、精确同文、取最靠上那块。
  弹层在 Foreground 层，所以中间空跑一帧再数——那一帧就是 `every_rune…` 之外唯一能见到菜单文字的地方。
- `scratch_repo(tag)` 的目录名是 `pid + tag`，**每条判据一个 tag**，撞车会互相 `remove_dir_all`。
- `pump` 必须等**应用状态**（`staged_in_app`、`snap.*`、`repos`），不能等 git 状态：
  索引在 worker 回快照之前就先变，等到的那帧 `Snapshot` 还是旧的，第二下点在一行已经不存在的行上。
- 高负载下的偶发红要认出来：并行跑全量时 600 帧泵不够，是 CPU 让给了别的测试，单跑必过。
- 机器上没有 PIL/numpy，量参考图走 `uiprobe/pngprobe.py`（stdlib 解 PNG：IHDR/IDAT + 五种 scanline filter）。

---

## 逆向本机 SourceTree.app：怎么读、读到的是什么

只用于**量文案和确认交互语义**，不抄资源、不抄代码（许可边界见 #18 那批）。三条踩过的坑：

- `Contents/Resources/zh_CN.lproj/Localizable.strings` 是 **UTF-16LE**（2699 行）。直接 `grep` 一个都搜不到，
  先 `iconv -f UTF-16 -t UTF-8`。文案键是现成的语义对照表（`LogBranchesAll`/`LogAncestorOrder`/`StageHunk`…）。
- `.nib` **是目录**，里面才是 `keyedobjects-101500.nib` / `keyedobjects-110000.nib`。
  `strings -a LogView.nib` 报 「can't map file (Invalid argument)」不是权限问题，是指到了目录。
- keyed archive **不是 property list**，`plutil -convert xml1` 直接拒绝。所以 nib 只能读到 outlet 名和类名
  （`jumpButton`、`historyDiffActionPopUpButtonID`、`compactViewEnabled`、`NSPopUpButton`），
  **读不到数值**——窗口的最小尺寸这类量不出来，`MIN_W/MIN_H` 只能按我们自己的撞车实测来定。

---

## 判据方法论：上面这些「绿」是假的

每条新判据必须**反向变异**验一遍（改坏 → 只该红的那条红）。改一次跑一次，还原走 `/tmp` 快照 + `cmp`，不用 `git checkout`。

1. **常量跟自己比**：断言写 `near(r.width(), HUNK_BTN_W)`，把常量 55 改 60 两头一起挪 → 恒绿。
   判据里钉**绝对字面量**，另开一条去钉常量的出处（哪张图、哪段 y）。两层不冗余。
2. **「A 不压住 B」必须连 A 的尺寸一起钉**：旧写法把整行吃掉时后面的控件被推走，「不重叠」照样成立。
3. **整带阈值会淹掉细特征**：1pt 宽、16pt 高的浅灰竖线在 24pt 带里求平均，过不了阈值 = 「不存在」。错在探针，不在图。
4. **探针看不见空心**：`rects()` 按「底色非透明」筛，空心方框的描边改了也全绿 → 补 `stroked()`（fill 透明 + stroke 非透明 + 圆角非 0）。
5. **删掉也不红的两样东西要分清**：可能是**死代码**（`file_row` 里 `&& !toggled` 那类，0.37 的命中裁决早就把点击只给面积小的那枚）
   —— 删代码，不是补判据；也可能是**契约锁**（`is_repo` 复核、`git::push` 签名里没有 `node` 这一位、
   `clear_msg_on_load` 只挡不经 `act` 的那次 `Loaded`）—— 就地写明它是契约锁而不是判别门。
6. **数「画了几个形状」测不出虚拟化**：egui 在细分阶段按 `clip_rect` 剪掉屏外形状，关掉虚拟化照样绿；
   必须数**行闭包真的跑了几次**（`ROWS_PAINTED`，thread_local `Cell`）。图元的认领同理：按 x 窗口筛会把侧栏分隔线
   算成图元（改成按**车道颜色**认领），计数器必须是 thread_local（全局 `AtomicUsize` 被并行夹具污染）。
7. **整组文案的数法不能退回逐词 `contains`**：漏红过一次（多长出的两枚按钮只有「sort 后整体比对」那条看得见）。
8. **界面类判据优先端到端**：假快照骗不了真 git（amend 位改 `true` → 端到端四条全红，而只钉「界面上出现了什么」的一条都没红）。
9. **法律文本没有机器可判的正确性**：素材审计判的是「字串在不在」，把条款换成反义照样绿；只能靠人读。
   `grep -F` 认整行，排版换行会让断言恒红（第一次写完就是这样红的，改的是排版不是判据）。
10. **求交叠必须用裁剪后的矩形，反证「溢出」必须只算切掉一半的**：本批第一次扫尺寸矩阵拿未裁的
    `galley.rect` 求交，报「文件行压住提交框」——那些行整块被 ScrollArea 剪在带里，屏幕上一个像素都没有。
    反过来 `cut_texts` 用「交集比原矩形矮」当溢出证据时，**整块在界外的负矩形**（高可到 -800）会让任何
    阈值都成立 = 假证据；必须 `!v.is_negative()` 先排掉。两条都在变异表里红过。
11. **「窗口约束」这类判据读不到画布，只能读配置对象**：把 `ViewportBuilder` 那条链拆成 `viewport()` 函数，
    判据读它的公开字段（`min_inner_size`/`inner_size`）。否则「加了地板」这件事删掉也不会红。

---

## 还没接上的 / 已知缺口

**任务单上欠的**
- **#26** 文件行末那枚「⋯」弹层（现在用分类行头的「放弃整组」临时顶着，判据钉的是那枚的文字）。
- **「紧凑视图」要按两态接**：strings 里 `LogCompactView`=紧凑视图 / `LogExtendedView`=扩展视图，
  它**不是**「还没接的一枚按钮」，是有的第二层 → 该带箭头（现在 `only_the_three_switches_carry_an_arrow`
  钉的是「不带箭头」，#31 一起翻）。
- **「所有分支」这一档今天 = 不给 git 传 rev**（走 HEAD 那一条线）。strings 里这是**两态**：
  `LogBranchesAll`=所有分支 ⇄ `LogBranchesCurrent`=当前分支 → 我们这档的语义和标签**正好说反**：
  「不传 rev」= 当前分支。#31 要改成两态切换，`所有分支` 那一态传 `--all`。
- **文案与 strings 对不上的**（#31/#32，逐条抄自本机 `zh_CN.lproj/Localizable.strings`）：
  排序那一档我们写「日期顺序」，官方是**按日期排序**；路径排序那行我们写「按路径排序」，官方是
  **已依照路径排序**（反向：**已依照路径 (反向) 排序**）；跳转框 hint 我们写「跳转到:」，官方是
  **转到Commit的SHA-1**；区块文案 **暂存区块 / 取消暂存区块 / 放弃区块 / 回滚区块**，
  差异头 **块 %d ： 行 %d-%d**；领先落后 **超前%d个版本 / 落后%d个版本**；
  侧栏还有**子模块**、**子树**两组（我们的侧栏没有）。**「侧栏没有折叠三角/计数」这条结论只对着
  那张参考截图**，不代表 SourceTree 全部——`#32` 会连这条一起复核。
- **还没量到的**：`LogFollowFirst`=仅首个父级 / `LogAllParents`=所有父级 —— 一个我们完全没有的
  图遍历维度；提交图标上的**暂存数角标**；`upstream/HEAD` 那枚药丸。

**功能面**：设置卡（工具栏 11 枚里只剩 index 10「设置」没接，点了是显式提示）、
push/pull 端到端判据（fetch 有，push/pull 仍无 remote 样本）、命令层 + 命令面板（⌘K 那半在 egui 版还没搬）、
多选、文件溯源/blame、拖拽与「扫描文件夹」入口、原生目录选择器（要引 `rfd`）、深色模式、
按**行**暂存（`line.hunk` 已经给每一行留了值）、差异区右上角那条「搜索」、
`COL_GRAPH` 超过 4 道目前是 **assert 报错**（没做列宽自适应）、`--depth`/bare/submodule、
克隆进度与 stderr 流式（现在只有结果一条 note）、`.gitignore` 编辑器、amend / `--no-verify` / `-S` 签名、
上面「它有一整块」那一段里的全部。
对照表里这六条现在只是**文档**、没有判据：大补丁必须在 `diff --git` 边界折叠、窄列路径省略换头部、
UI 不许出现 emoji、提交行高度只有一个来源、没有上游时绝不显示 `↑0 ↓0`、push 实参里不得出现加速节点串。

**几何/视觉待复核**：`NO_W = 30`（行号栏）**没有参考图依据**，只出现在 `diff_lines_share_one_left_edge` 的右缘计算里；
`NOTE_W = 240` 是算出来的预算不是量的；`TAB_H = 24`、`COMMIT_H = 96`、`PICK_BG` 同样无参考；
文件行距量到 27 未采纳；hunk 带 15pt 而参考图 29pt（`show_rows` 只认统一行距，等按行虚拟化一起换）；
当前文件那枚灰带没分「窗口有没有焦点」（SourceTree 失焦灰 (220,220,220)、有焦点蓝 (0,100,225)）；
虚拟行未选中时的背景仍未定（分不清 (220,220,220) 是失焦选中还是常驻灰带）；
侧栏图标「像不像」没有判据（`icon_ink` 只判那格里有墨）；「工作区」参考图里是没翻译的大写 WORKSPACE，我们译了。

**规矩 / 安全**
- `Action::Discard` **不在** `needs_confirm` 里，菜单里点「放弃」不弹卡（和已有的按钮/复选框一致，是刻意的）；
  「放弃区块」用的是两击内联确认，不是模态卡。
- 克隆/推送的地址目前**走 argv**，凭据内嵌在 URL 里会进进程表和 git 输出。应当改成走凭据 helper —— 已知缺口。
- 测试**不连真远端**、不假设本机存在某个仓库；只用临时仓库。字体不入库，CI 上没有那份 8.3MB 字体
  （workflow 只打警告不失败）——装了字体的绿和没装字体的绿不是同一种绿。

---

## 批次台账（一行一批；全文在归档）

### React + Tauri 那一层 → [`docs/process-archive/react-tauri.md`](file:///Users/baorong/Developer/superlbr/gitx/docs/process-archive/react-tauri.md)

- 布局与结构、差异视图（unified diff 分组、按扩展名高亮、词级 diff）、GitHub 加速克隆（节点探测 + 克隆后 origin 改回真地址）
- 参照 dbx 的交互优化：`ConfirmDialog` + `requestConfirm()`、`ContextMenu`（仓库/提交/文件/分支/贮藏）、稳定色点、⌘Enter 提交、`reveal_path`
- 键盘优先体系：`src/shortcuts.ts` 命令表 + 可覆盖键位（两段式 chord `g h`，800ms 超时）
- UX 重构（左右分栏 + 行首复选框）、文件溯源 FileHistory 分栏弹窗、amend 改写提交信息、快捷键总开关、Provider 插件机制、更新检查（直连 Releases）
- SourceTree 风格布局（分组导航 + 分支三 tab + 底部状态栏 + 标签页恢复）、环境一体化（拖拽/预览/扫描/SSH/git 引导/LFS/分组）
- 中文正确性包、国内网络闭环、顶栏合并进原生标题栏、WebView 性能三刀
- `window.prompt` 在真实窗口里从来不通 → 自建输入弹窗；渲染兜底 `ErrorBoundary`（崩屏不等于白屏）
- 视觉打磨第一~四批（真截图驱动）、历史页改版 + 仓库标签页、「点提交 → 整屏空白」缺陷、按路径筛选没反应、
  push 报 `missing required key setUpstream`、应用图标、P0 三刀、默认亮色 + 双左栏合一（**已被 10-20 复刻拍板作废**）
- 框架评估（egui/eframe 那条：结论「不换」，理由后来被 spike 换掉一半）+ **Spike 结果：两条判据实测**
  （`gitcraft-app/src/fonts.rs:5` 与 `uiprobe/` 的注释仍指向这一节）
- 项目更名 GitX → gitCraft：显示层 `GitCraft`、标识符小写 `gitcraft`、工作区目录 `~/Developer/superlbr/gitx` 不动；
  顺带量到两条存储事实（SQLite 按 bundle id 分家、WKWebView 的 localStorage 按**可执行文件名**分家）
- 命令层 `src/commands.ts`（76 条、`runCommand` 单一入口、破坏性确认收进这一层、覆盖率闸门）+ 命令面板 UI（⌘K，两阶段选目标）

### egui 壳第一批 → 「文件表方框列」 → [`docs/process-archive/egui-shell.md`](file:///Users/baorong/Developer/superlbr/gitx/docs/process-archive/egui-shell.md)

- **egui 壳落地**：eframe 0.37 直连核心、无 IPC、四块带按实测几何摆、日志与差异 `show_rows` 虚拟化；
  几何门禁换成不依赖屏幕的形式，当场抓出 4 个真 bug（主题只改 Light、工具栏撑到 74pt、侧栏筛选器被挤出窗口、`side_top_panel` 内边距右移 8pt）
- **CJK 字体落地**：`raw.githubusercontent` ~1KB/s 且反复截断（截断的 sfnt 会让 epaint abort 进程），换 jsDelivr 一次拿全 8 331 336 字节；
  `fonts.rs::complete()` 校表目录；字体不入库，`OFL.txt` 入库
- **「面板里有真数据」变成断言**：行距钉死、`fix_pitch`、虚拟化靠 `ROWS_PAINTED` 数、真实仓库那条端到端（等 17 帧，中间每帧都画得出东西）
- **49pt 空列填上提交图**：Rust 版 `layout_graph` 会**释放车道**（`src/graph.ts` 从不释放，历史越翻越宽）；
  拓扑排序只差 `--topo-order` 一个 flag（有条 argv 逐字节判据）；`click_text` 进 harness
- **按下提交按钮真仓库多一条提交**：5 条端到端 + 8 次变异；查实 ⌘Enter 的修饰键来源、`clear_msg_on_load` 是契约锁、ctrl 半边原本没判据
- **日志四列与差异正文改成齐左**：`Ui::put` 居中/两端对齐 → `put_left`；新增 `text_rect` 系列
  （**此前没有任何一条量过对齐**，四列全右对齐时 23 例照样全绿）；顺带发现无头帧一直在用产品里看不到的字体
- **删 Tauri 之前先补「添加仓库」**：3 条判据 + `find_repo_root`/`is_repo` 那两步规矩；撞出 tag 冲突与 `starts_with` 前缀抢命中
- **删掉 React + Tauri 那一层**：判据只有「三条门禁全绿 + 只剩一行 Tauri 字样」；诚实记下 226 例前端判据消失、其中四条彻底没人守
- **许可证 MIT OR Apache-2.0 + 第四条门禁素材审计**：四种条款要分清（原创代码 / Atlassian 截图 / OFL 字体 / 名字与图标是商标）；
  留两条许可证的理由是 Apache-2.0 与 GPLv2 不相容；刻意不写 CLA（Apache-2.0 §5 天然管授权）
- **对照参考图把外壳几何全部重量一遍**（行距 26 / 表头 24 / 四条竖线 / 内缩 5 / 第 0 道 +22.75）：推翻「表头没有竖线」；
  M2 常量自比的教训；**补上第一次真窗口目视**
- **侧栏照参考图重画**（图标 18..38 线描、47pt 分类、同列 x=41、灰药丸）：撤掉折叠三角 / 计数 / 两档缩进（参考图上不存在）；
  M8 抓到「A 不压 B」那类假判据
- **日志行去 hover + 修「有时候点不中」**：滚动后第一帧丢点击（`push_id` 稳定行 Id）、`max_click_duration` 0.8→3.0
- **更正：真正的根因是 selectable label**（用户报的「光标变成插入状态」）：日志/侧栏/文件行三处 `selectable(false)`，
  文件表那处曾让**整列一个都点不动**
- **日志滚到底接下一页**：根因是取数写死 `--max-count=500`（`comic` 有 4400 条），不是渲染；
  `log_seq` 挡旧页、`append_page` 整列重算、`Snapshot` 多存一份 `remote_refs`；500 这个数仍然是拍的
- **顶栏重排 + 仓库 tab + 提交框/差异区**：重叠是无头收控件矩形两两求交量出来的；提交框两个框、隔一个空行；
  差异头带 30pt / 色块 14×14 / 内缩 8（改掉三个拍出来的数）；抓到 hunk 文案有两份翻译、提交框被列表顶出窗口
- **hunk 级暂存/放弃**：核心层 `hunk_patch` / `stage_hunk` / `unstage_hunk` / `discard_hunk`；
  两条故意偏离（15pt 而非 29pt、两击内联确认而非模态卡）；8 次变异 + 一次「漏红」记录
- **文件表方框列、状态图标列、分类全选框**：`check_box` 自己画（egui 那枚跟着字号长，三列对不齐）；
  **判据必须问应用状态**这条在这批查实；M4 判定为死代码删掉、M8 假判据补 `stroked()`

### 最近三批：全文在下面

- **#30** 窗口地板 770×324：**高度不背锅，撞车的是宽度**；顺带把「裁剪后可见」做成 harness 的一等公民
- **#34** 标题栏合并：那一行 52pt **就是** macOS 标题栏（红绿灯 / 仓库名 / 11 枚按钮同排），
  代价是地板从 770 抬到 **870**
- **#39** 差异区去噪（git 的文件头 + `git show` 的日志头一行不画，**靠拢**）+
  文件表三档各左移 8pt（**偏离**，用户拍板的密度）

---
## 接通分支 / 标签 / 贮藏 / 忽略 / 远端：模态卡 + 右键菜单（任务 #27，2026-10-21 凌晨）

**这批是纯接线**：`create_branch`、`delete_branch`、`create_tag`、`stash_*`、`ignore_path`、
`fetch`、`remote_url` 在核心层早就有，缺的是界面上没有一条路能走到它们。所以判据一律走端到端：
**真点菜单 → 真填卡 → 真跑 git → 用另一条 git 命令复核**。只钉「界面上出现了什么」的话，
接错线（卡关掉但动作没发、动作发了但打错提交）都还能绿。

**决定与依据**

- 右键菜单用 egui 原生 `Response::context_menu`，模态卡自己画（`Area` + `Order::Foreground`
  + 一格铺满整屏的遮罩）。不用 `egui::Window`：它自带标题栏、阴影、拖动条，跟参考图那种
  macOS 表单对不上，而且它和面板各占一层，卡后面的行照样点得着。遮罩一物两用：
  挡住后面那一层 + 「点外面 = 关卡」，于是**不需要**给每个面板各写一份「有卡就别响应」。
- 破坏性动作的闸只放在**出口**一处（`needs_confirm` + `land`），不摊在每一张菜单里：
  以后多加一个入口也不会漏。
- 每一处（工具条、侧栏分类、日志行、文件行）都只往一个 `Intent` 里放意图，帧末统一由
  `land()` 落地，顺序定死：**先说话（note）、再挪位置（goto/reveal）、最后才动手**。
  回调不许捕获 `self`（它们跑在 `Panel::show` 的闭包里，那时 `self` 正被借走）。
- 远端 URL → 可浏览网页这件事，一开始我在 `git.rs` 里另写了 `web_url` + `split_host_path`，
  查到 `provider::parse_remote` 早就把 `git@host:o/r.git`、`https://host/o/r.git`、
  带凭据那三种都吃下了 —— **删掉自己那份**，不给同一条规则留第二份实现。
  代价只是把 `git::remote_url` 改成 `pub`（核心层唯一的改动，3 行）。
- `BrowseRemote` 走 `act` 而不是在帧里直接 `Command::new("open")`：`git remote get-url`
  也是一次子进程，在帧里等它就把整帧冻住——和其余每一条 git 调用同一个理由。
- 新增动作：`Checkout`、`Fetch`、`FetchRemote(name)`、`CreateBranch{name,start,checkout}`、
  `DeleteBranch`、`Merge`、`CreateTag{name,target}`、`DeleteTag`、`StashSave{message,
  include_untracked}`、`StashApply/Pop/Drop`、`Ignore`、`BrowseRemote`。工具条第 4/5/6/7/9 枚
  分别接上分支卡 / 合并卡 / 贮藏卡 / 浏览器打开远端 / 终端；侧栏四个分类各带自己的右键菜单
  （分支 4 项、标签 4 项、远端 3 项、已贮藏 3 项），日志行 5 项、文件行 4 项。
- `log_row` 改成返回 `egui::Response`、`file_row` 返回 `(Response, bool)`：菜单得挂在
  **同一个命中矩形**上，不然「点得中那一行」和「右键得开那一行的菜单」是两套坐标。

**脚手架事实（下次别再撞）**

1. 工具条那一行是 `right_to_left` + `item_spacing`，**枚间距实测 56**，不是按钮自己的 50。
   按 50 手算落点会点到隔壁那枚——本批第一条判据就是这么红的。结论：**别手算控件坐标，
   按文字定位**（`click_tool` 定位 `TOOLS[i].0` 那枚图标字）。
2. **一个刚 `Context::default()` 出来的上下文，第一遍不画 `Order::Foreground` 那一层。**
   拿纯 egui 复现过（跟本项目无关）：同一遍里面板的 `ui.label` 有形状，`Area::order(Foreground)`
   里的 label 第 0 遍 0 个、第 1 遍才 1 个。**不是**「这个 Area 第一次出现」——换一个已经跑过
   很多遍的 Context，每遍都换全新 id 的 Area 照样画得出。所以判据侧加了 `warm()` /
   `warm_frame()` / `click_warm()`：先空跑一遍再问卡。它同时制造过一条**假绿**：
   `a_card_swallows_everything_behind_it` 在卡根本没画的时候也能绿（那一行点不着是因为
   那一遍压根没有卡）。
3. `egui::Id::new` 在 0.37 已经 deprecated，零警告构建会直接红，用 `Id::unique` / `IdSalt`。
4. egui 会把**已经注册过一遍**的 `Area` 那一层整体压暗（`containers/area.rs:706` 那句
   `ui.multiply_opacity`）：实测写进去的 `from_black_alpha(80)` 出来是 `#00000013`（alpha 19）。
   所以遮罩那条判据按「满屏那一格 + 黑色半透明」问，不钉具体 alpha——钉了就是钉一个 egui
   自己会变的数。
5. 更正上一条批次的记录：`Context::viewport_rect()` **是存在的**（`context.rs:3420`，
   就是 `input(|i| i.viewport_rect()).round_ui()`），默认值 10000×10000（`input_state/mod.rs:347`），
   `begin_pass` 用 `RawInput::screen_rect` 覆盖它（`:387`）。之前写的「0.37 没有这个 API」是错的。

**9 条新判据 + 10 次反向变异逐条对红**

| 改坏什么 | 哪条红 |
| --- | --- |
| M1 `primary_action` 去掉「名字空着返回 None」 | `an_empty_name_keeps_the_card_open`（「名字还没填就把卡关了」） |
| M2 遮罩 `Sense::click` → `Sense::hover` | `a_card_swallows_everything_behind_it`（「点卡片外面应该关卡」） |
| M3 遮罩从整屏缩成 380×300 | 同一条，红在**形状**那一格（「遮罩没铺满整屏」） |
| M4 `needs_confirm` 去掉 `DeleteBranch` | `deleting_a_branch_asks_first_then_actually_deletes`（「删分支没先问一句」） |
| M5 打标签的卡改成打在 `HEAD` | `a_tag_lands_on_the_commit_the_menu_was_opened_on`（`rev-parse` 比出打错提交） |
| M6 文件行菜单「暂存这个文件」→ `StageAll` | `the_file_row_menu_stages_only_that_file`（600 帧等不到） |
| M7 「忽略这个文件」→ `Discard` | `the_file_row_menu_ignores_and_deletes`（note 里是 git 的 pathspec 报错） |
| M8 删掉 `if esc { close = true }` | `escape_closes_the_card_without_doing_the_thing`（「Esc 没关卡」） |
| M9 回车那一读改成常量 `false` | `the_branch_card_creates_a_real_branch`（600 帧等不到分支长出来） |
| M10 「丢弃这条贮藏」→ `StashApply` | `a_stash_round_trips_through_the_sidebar`（「丢弃贮藏没先问一句」） |

M2/M3 各只红一格，正是这条判据要**形状和后果都问**的原因：只问后果，「遮罩缩小了但那一行
恰好没被点到」能绿；只问形状，「遮罩画了但没接点击」也能绿。另外这条判据自带一个对照：
**没卡的时候同一坐标必须点得到**（选中项真的变），否则测的是空气。

**自己写错的那两条，红的是我的断言不是代码**（记下来是因为这类错不会自己现形）：
`an_empty_name_keeps_the_card_open` 原本断 `git status --porcelain` 为空——`scratch_repo`
故意留了一处已暂存改动，这条永远红；换成比对 `git branch --format` 前后一致。
`the_file_row_menu_ignores_and_deletes` 从隔壁那条复制过来一句
`staged_in_app(a) == ["b.txt"]`，可这个夹具里 b.txt 从没被暂存过；改成问
「c.txt 从列表里没了 + `git status` 里没了 + 文件内容回到原样」。

**仍然没看到过的**：真窗口目视照旧欠着（#25）——02:19 再试，`screencapture` 整屏仍然
是一张纯黑（2880×1800，逐像素全 (0,0,0)），显示器睡这件事还没解决，所以**卡片宽度 380、
行高 26、底部 34 这几个数只有无头证据**；合并卡只列本地分支（远程分支要先抓取）；
`Action::Discard` **不在** `needs_confirm` 里，菜单发起的「放弃」和既有按钮/方框一样不弹卡，
这是刻意的策略缺口，等「放弃」整组改成先摊改动清单再确认那一批一起收；
卡里的 Tab 焦点切换、上下键选行没做；「检出这个提交/标签」会进 detached HEAD，没提示；
侧栏「远端」那一栏的点击只报一条 note，抓取全部远端还没进度；文件行末那枚「⋯」仍然没有
可见的把手（右键能用，但参考图那枚要点出来才知道有，#26）。

app 80 例、核心 80 例全绿，`scripts/gates.sh` 四条门禁全绿。

## 接通「克隆仓库」：两格模态卡 + 后台 clone（任务 #28，2026-10-21 凌晨）

**为什么先做它**：接完 #27 之后，这个壳能改、能提交、能建分支打标签贮藏，但**没有一条路把
一台机器上还不存在的仓库弄进来**——「添加仓库」只吃已有路径。对准备开源的 git 客户端来说
这是最大的一个功能洞。

**决定与依据**

- 入口挂在既有那枚「添加仓库」展开出来的那一条里（`[路径输入][打开][克隆…]`），不往工具条加：
  工具条那 11 枚是按参考图量的，自己加一枚就是破坏复刻契约。展开时才出现 = 收起来零宽度成本，
  所以那条「顶部两条带不许交叠」的判据不受影响（它跑的是收起那一帧）。
- 卡有两格文本框。`Modal` 的约定是「一张卡一格文本框」（`text`），第二格由表单自己带：
  `Form::Clone { dest }`。不给 `Modal` 加一个通用第二字段——五种卡里只有一种要它。
- `git::clone_repo(clone_url, remote_url, dest)` 那个双地址是给「用加速节点抓、origin 指真地址」
  留的口子。这个壳没有节点概念，于是**两个传同一个**，不自己编一个中间态。
- 克隆是**唯一一条手上还没有仓库也要能跑**的动作：`act` 那道 `let Some(repo) = ... else { return }`
  守卫在它之前放行不了，所以单开一条 `start_clone` → `Msg::Cloned`；成了以后交给 `add_repo`
  （它自己会再发一次 `Msg::Loaded`），失败只落 `note`。`match &action` 里给 `Clone` 写的那一条
  返回 `Err("克隆不该走到这条路上")`——只为穷尽，真被走到说明有人挪了守卫，当场响。
- 目标目录留空 = 主按钮按不下去（不替你猜一个 `~/xxx`）。

**本批自己踩到的那条假绿（不是变异抓到的，记下来因为这类错不会自己现形）**：
`locate` 按**前缀**匹配帧里**第一块**文字。卡里那行说明原本写「目标目录填相对路径按主目录算」，
于是 `type_into(app, "目标目录", …)` 定位到的是那行**不能输入的说明**——点在它上面不会拿到焦点，
`Event::Text` 落空，但 `type_into` 照样返回 `true`。文案改成「填相对路径时按主目录算」，
别让说明拿会被前缀抢走的字开头。判据侧的通用教训：**凡是按文字定位的助手，都要问一句
"这串字在帧里唯一吗"**。

**两条新判据 + 两次反向变异**

| 改坏什么 | 哪条红 |
| --- | --- |
| N1 `primary_action` 的 Clone 去掉「两格都非空」守卫 | `a_clone_needs_both_fields`（「目标目录还空着就把卡关了」） |
| N2 `Msg::Cloned(Ok)` 不再 `add_repo` | `the_clone_card_makes_a_working_copy`（600 帧等不到书签） |

端到端那条**源用本机路径当远端**（`git clone /tmp/… /tmp/…`）：门禁不许连真远端，
也不许假设这台机器上有某个仓库，而 `git clone` 走的是同一条代码通路，换 https 只是换个字符串。
断的是四样：书签里那条路径就是目标目录、`app.repo` 切过去了、`rev-parse HEAD` 与源相同、
`remote get-url origin` 指回源。第二条判据额外断 `!dst.exists()`——`git clone` 一旦开跑就先把
目录建出来，所以「没留下半个目录」才是「主按钮压根没点下去」的证据。

**仍然没看到过的**：真窗口目视照旧欠着（#25，02:19 再试整屏仍纯黑）；克隆**没有进度**，
`git clone` 的 stderr 一行行回流这件事没做，用户只能等那枚 busy；目标目录**没有目录选择器**
（原生 open dialog 那一跳还没接），只能手打；`--depth` / bare / 子模块这些克隆选项没有；
带凭据的 https 地址会原样进 `git clone` 的命令行（同机器上的别的进程 `ps` 看得见），
「交给凭据助手 / keychain」那条路还没走——这是**已知缺口**，不是没想起来。

app 82 例、核心 80 例全绿，`scripts/gates.sh` 四条门禁全绿。


## 筛选条改成带箭头的弹出按钮：分支范围 / 远程药丸 / 排序（任务 #29，原 #19，2026-10-21 中午）

**为什么先做它**：#27/#28 接完，日志区能翻、能提交、能建分支，但**筛选条那四枚按钮里只有排序真的能用**，
而且它是一枚「按一下换一次」的开关——参考图上那一排是**带箭头的下拉**，第二层里才有选项。
更要紧的是「按分支看日志」这条最常用的筛选从来没有过：列表只能看 HEAD 那一条线。

**决定与依据**

- 三枚弹出按钮**落点各不相同**，这是本批唯一「看着同类、其实不同类」的地方，所以分开接、分开钉：
  换排序 = 换 `git log` 的参数，得重取；换分支范围 = 多传一个位置参数（rev），也得重取；
  「显示远程分支」= **只决定药丸画不画**，`git log` 一个字没动，**绝不重取**。
- 「所有分支」这一档今天 = **不给 git 传 rev**（`git log` 走 HEAD）。参考图上这一档大概率是 `--all`，
  但两种语义在只有本地分支的临时仓库里**看不出区别**，没有参考图依据前先按 HEAD 走，缺口登记在下面。
  核心侧补一句 `commits()` 默认 == `commits_on(HEAD)`，钉的是「别把默认悄悄换成 `--all`」。
- 控件走 `egui::containers::menu::MenuButton::from_button(Button::new(..).small())`，不用 `Ui::menu_button`：
  后者自己 new 一枚默认尺寸的按钮，链上的 `.small()` 静默丢，操作条那条带会被撑高（事实全在上面的清单里）。
- 箭头用 `▼`：`▾`(U+25BE) 不在字体链里，`every_rune_the_shell_paints_has_a_glyph` 当场把它抓成豆腐块。
- 「紧凑视图」**不给箭头**，点了还是那句「egui 版还没接」。没有第二层就不该画箭头——
  造一个空菜单假装像下拉，比承认没接更糟；`only_the_three_switches_carry_an_arrow` 钉的就是这个分别。
- `snapshot_of` 里那三条取数路收敛成一条 `commits_page(repo, 0, ..)`：核心层在 `skip = 0` 时**不发**
  `--skip`（`the_page_offset_only_adds_skip` 钉过），所以发给 git 的 argv 逐字节相同——A9 把 skip 改成 1，
  十条判据同时红，就是这条收敛没改行为的证据。

**本批自己踩到的三条假绿**（都不是变异抓的，但它们比变异值钱）

1. **常量自比**：排序/箭头两条判据起初写 `filter_text(..) == Some(format!("{label} {ARROW}"))`。
   把 `ARROW` 改成空串，标签和那个空格还在，照样绿；改成 `▲` 也照样绿——两头的错都抓不住。
   换成「数标签之后还有几枚非空白字」（`only_the_three…`）+ 「按钮上两档不许同时出现」（`the_order_menu…`）。
2. **探针点错了地方**：`popup_pick` 前两版红在判据自己身上——先只按「在按钮下方」筛菜单项，
   点到**侧栏**里同名的分支行；换成 `locate("所有分支")` 又被侧栏那行**前缀**抢走（侧栏先于日志带落笔）。
   最后加 `center().x > SIDEBAR_W + 1.0` + 取最靠上那块。想按绘制层筛（`order == Foreground`）走不通：
   `ClippedShape` 没有 `order` 字段，只能按几何认领。
3. **`pump` 超时不算判据**：A4（每次点远程那枚都 `open()`）报的红是「600 帧还没等到」——
   它只说明取数路被搅了，说不断哪条契约断了。把变异收窄成 A4b 才落到那句写明的
   「这一枚只决定画不画，不该把列表重取一遍」上。同一个理由改了 `the_scope_menu…`：
   两处 `pump` 的谓词从「列表里出现了支线的提交」改成**应用状态**（`snap.is_some() && log_rev == ..`），
   于是 A6（`open()` 丢掉 rev）和 A6b（`set_rev` 落了 rev 不重取）都红在
   「按支线筛完，列表里没有支线独有的提交：["只在主线", "起点"]」这一句上，用时 0.4s 而不是 8.3s 超时。

**六条新判据 + 十四次反向变异**

| 改坏什么 | 哪条红，报的哪句 |
| --- | --- |
| A1 `ARROW` 改成空串 | `only_the_three_switches_carry_an_arrow`：「所有分支」那枚的第二层标记不是一枚 |
| A2 `visible_badges` 无视开关、原样返回 | `hiding_remote_branches…`：关掉之后远程药丸还画在行上 |
| A3 过滤条件写宽成 `kind != Branch` | 同一条：本地分支药丸跟着没了：过滤写宽了 |
| A4 远程那枚每次都 `open()` | 同一条，但红在 pump 超时——**判据没写这件事**，见上面第 3 条 |
| A4b 只在关掉那一次多 `open()` | 同一条：这一枚只决定画不画，不该把列表重取一遍 |
| A5 菜单点中不落 `log_rev` | `the_scope_menu…`：菜单里点了分支没落到范围上 |
| A6 `open()` 里把 rev 丢掉 | 同一条：按支线筛完，列表里没有支线独有的提交（旧版这里是 600 帧超时） |
| A6b `set_rev` 落 rev 但不重取 | 同一条、同一句（0.4s） |
| A7 `popup_pick` 找按钮去掉 x 过滤 | 同一条：「side」的菜单里没有「所有分支」这一项——红在探针，正是它该红的地方 |
| A8 排序按钮文案写死不跟状态 | `the_order_menu…`：换了排序，按钮上还是旧那一项 |
| A9 `snapshot_of` 第一页 `skip` 传 1 | **十条**一起红（分页、提交、克隆、药丸那批端到端全断），`85 → 75 passed` |
| C1 rev 追加在 `--` 之后 | 核心两条：`rev 落在 -- 后面就被当成路径了` + `支线列表串了：[]`（空列表，正是那种「看着像正常」） |
| C2 `commits_on` 吃掉 rev | 核心 `scoping_to_a_rev…`：支线列表串了：["只在主线", "init"] |
| C3 空白 rev 不再丢弃 | 核心 `the_rev_argument_only_adds…`：多出来的不止那一个参数 |

还原全部走 `/tmp/g29_app_green.rs` + `/tmp/g29_core_green.rs` 的 `cp`/`cmp`，没有用过 `git checkout`。

**仍然没看到过的**：真窗口目视照旧欠着（#25，这批又欠一次——弹出菜单**展开的样子**从没在屏幕上见过，
无头只数得到它摊开那一帧的文字）；三枚按钮的**宽度、间距、箭头字号**没有参考图依据，只判了「有没有第二层」；
菜单摊开时它盖住了什么、点外面收不收，没有判据（`PopupCloseBehavior` 默认 `CloseOnClick` 是读源码读来的，不是测的）；
「所有分支」到底是 HEAD 还是 `--all` 仍未结案；远程药丸的开关只作用于日志行，工作区/侧栏那些远程分支
该不该一起藏，没查；`紧凑视图` 那档参考图上具体折叠什么，没量。

app 85 例、核心 82 例全绿，`scripts/gates.sh` 四条门禁全绿。

## 窗口地板：撞车的是宽度不是高度（任务 #30，2026-10-21 傍晚）

**开头那句任务标题是错的。** 立的是「窗口高度 ≠ 545 时文件列表压到提交框」，来源是 2026-10-21 那批
真窗口目视里「拉窄」那张截图。动手前先量旧代码，量完发现前提不成立：

- **高度不背锅**：暂存带是 `Panel::bottom("staging").default_size(245)`，带高写死，所以带内排版和窗口
  高度无关——545 / 656 / 720 / 900 量到**同一组相对矩形**（「待定的文件」永远在带顶下 9.5pt，摘要框永远
  在它下面固定的地方），只是整带往下挪。只有窗口矮到 324（= 55+24+245）以下带才被夹扁，那是另一回事。
- **第一次量出来「文件行压住提交框」是探针的锅**：`ScrollArea` 把滚出界外的行**剪掉但形状照发**，
  `ClippedShape.shape` 里那块文字的 `galley.rect` 还在原位置。拿没裁的矩形求交，就把「一行都看不见的行」
  判成撞车（`notes.md` 794..808 vs 摘要 803：屏幕上是 794..802，压根没碰）。
- **真正会撞的是宽度**：无头扫 500→1050（步长 10，裁剪后求交）——**770 起 0 处重叠**；765 起顶栏那枚
  `main ↑2 ↓0` 胶囊压住 ⊕（1.9×11pt）；755 起日志作者列的文字压进描述列（8.6×13.9pt）；再窄连
  「没有仓库 (Git)」都盖到工具栏图标上。原因和 #22 那次顶栏重叠同一路数：右侧靠右排 + 列宽写死，谁都不让。

**决定与依据**：不重排提交框（它没坏），给窗口加地板 `MIN_W = 770` / `MIN_H = 324`。
`MIN_W` 是上面那把梯子量出来的，`MIN_H` 是「两条顶带 + 暂存带」都不被夹扁的最小值。SourceTree 自己的
最小窗口尺寸**没量到**（nib 是 keyed archive，`plutil` 读不出数值，见上面那节），所以这两个数是按**我们的
布局什么时候开始撞**定的，不是照抄参考——这点如实记在这儿。

**改了什么**
- `MIN_W` / `MIN_H` 两个常量 + `viewport()` 函数（原来 `ViewportBuilder` 那条链直接写在 `main` 里，判据读不到）。
- harness：`frame_at(app, w, h)` / `raw_input_at`（`screen_rect` 参数化，之前钉死 1050×545）、
  `visible_texts`（**裁剪后**的文字矩形）、`cut_texts`（被切掉一半的文字数，用来证明「这帧真溢出了」）。
- 三处 push 循环合并成 `many_unstaged(n)`；`the_commit_box_stays_inside_a_short_band` 改用它（同一段代码抄了三份）。

**判据 + 反向变异**（app 87 例，核心 82 例不变）
1. `no_visible_text_overlaps_at_the_smallest_allowed_window`：地板尺寸起，扫 5 档尺寸，
   要求每帧 >40 块可见文字（防空帧）、`cut_texts > 0`（防「拿屏幕外的矩形比一比」）、
   无出界、无 1×3pt 以上的两两交叠。
2. `the_window_itself_is_floored_at_the_measured_size`：读 `viewport()` 的 `min_inner_size` / `inner_size`。

| 变异 | 结果 |
| --- | --- |
| `MIN_W` 770→755 | **红**：`755×324："main  ↑2 ↓0" 压住 "⊕"（11.9×11.0pt）` |
| `MIN_W` 770→765 | **红**：同一对，只有 1.9pt 宽 → 证明 1×3 的阈值不是摆设 |
| 删 `.with_min_inner_size(..)` | **红**（第 2 条）：`窗口的地板没交给窗口系统：None` |
| 删 `.with_inner_size(..)` | **红**（第 2 条）：第二条断言 |
| `visible_texts` 去掉 `.intersect(c.clip_rect)` | **红**（第 1 条）：`"+加上的一行" 压住 "今天 11:41"` 这类**剪掉的行**被当成撞车 |
| `COL_AUTHOR` 190→230（真回归） | **红**：`"包融 <br@example.com>" 压住 "主题0…"` |
| `cut_texts` 阈值 1pt→500pt | 第一版**存活**——因为整块在界外的负矩形高度能到 -800，任何阈值都过；补 `!v.is_negative()` 后同一变异**红** |
| 删掉 `cut_texts(&tight) > 0` 这行前置断言 | **存活**（判据照绿）。它是**防探针空转的守卫**，不是判别门——按方法论第 5 条就地写明，别当成覆盖率 |

那条「补 `!v.is_negative()` 之前存活」是这批自己的假绿，记下来：一个用来证明「探针没在空转」的前置断言，
自己先空转了。

**仍然没看到过的**
- 真窗口把宽度拖到 770 边上看一眼（无头量的是 `screen_rect`，winit 那条约束在 macOS 上到底生效没有，
  只能目视——每批欠的那次目视还欠着）。
- `MIN_H = 324` 这一档以下带被夹扁时，文件表只剩 40pt 地板、行被切半（1050×300 量到 0 处重叠，但切半的
  好不好看没看过）。
- SourceTree 的最小窗口尺寸、以及它窄到那个尺寸时**顶栏怎么处理**（缩图标？藏标签？），strings/nib 都给不出。

app 87 例、核心 82 例全绿，`scripts/gates.sh` 四条门禁全绿。

---
## 标题栏合并：那一行 52pt 就是 macOS 标题栏（任务 #34，2026-10-21 傍晚）

**起因是用户一句纠正**：「去掉顶栏表述有误，是去掉**标题栏**（macOS 跑马灯），明白了吗」。
我上一轮把这条读成"去掉窗口内那条工具栏"，于是判它「偏离 SourceTree」。读反的后果是
差点去做一件把 11 枚按钮挪进角落的事。重读参考图 + 读 nib 之后结论**倒过来**：
合并标题栏是**向 SourceTree 靠拢**，我们之前是**多画了一条**。

**决定与依据**

1. **那一行就是标题栏**（不是"标题栏 + 工具栏"两条带）。证据两条：
   - `GitRepoWindow.nib` 里那 11 枚按钮挂在窗口的 `NSToolbar` 上（`NSToolbarItem*` 一整套键都在），
     也就是操作系统的统一标题栏区域；
   - 参考图逐行扫底色：`244,249,246` 从 y=0 到 y=51，y=52 一条 `202,206,203` 分隔线，y=53 起才是
     筛选条带（`240,244,241`）。红绿灯（x 20..72、y 20..32）和仓库名（左沿 **91.5**）和按钮
     （右边距 **21**）**全在这 52pt 里**。
   我们当时是：系统标题栏（28~32pt）+ 自己一条 55pt 带，仓库名排在 x=9 —— 名字正被红绿灯压住。
2. **带高从 55 改成 52**。55 是上一轮"看着像"记的，把分隔线和它下面两三个像素一起算了进去。
   这次是逐 pt 扫底色扫出来的，两张参考图（1050 宽和 1120 宽）给的是同一个 52。
3. **左边让 92pt、右边让 21pt**。egui 0.37 的 `Margin` 四个边是 **i8 整点**，量来的 91.5 只能取 92；
   所以判据钉的是**量来的 91.5**（`TITLE_LEFT`，只给测试看），产品里用 `TITLE_INSET = 92`。
   这俩分开是故意的：判据如果写成 `== TITLE_INSET`，那是**常量自比**，改常量它跟着改、永远绿。
4. **代价：窗口地板从 770 抬到 870**，这条是量出来的、不是妥协出来的。抬完 #30 那条扫宽判据当场红：
   `没有仓库 (Git)` 压 `⊕`、`main ↑2 ↓0` 压 `↓`。重扫 600→1050（步长 10）：860 还剩 1 处撞，
   **870 起 0 处**，两个高度（321 / 545）同一条线。
   SourceTree 不会撞，因为 `NSToolbar` 装不下会把条目收进 » 溢出菜单、标题用「…」截断。
   我们没那套，所以先抬地板，**截断让位排进 #35**（那批本来就要重排这一行的左组）。
   少掉的 100pt 可缩范围是这批**唯一一处明确的退步**，记在这。

**改了什么**：`TOOLBAR_H` 55→52；新增 `TITLE_INSET`/`TOOLBAR_RIGHT`（产品）与
`TRAFFIC_RIGHT`/`TITLE_LEFT`（判据用，`#[cfg(test)]`）；工具栏那条 `Frame` 的内边距从
`symmetric(9,5)` 换成 `Margin{left:92,right:21,top:5,bottom:5}`；`viewport()` 加三条
`.with_titlebar_shown(false)`、`.with_fullsize_content_view(true)`、`.with_title_shown(false)`
（eframe 把它们映射给 winit 的 macOS 扩展：`with_titlebar_transparent` / `with_fullsize_content_view`
/ `with_title_hidden`，见 egui-winit 0.37 的 `macos` 分支）。`MIN_W` 770→870。

**判据 + 反向变异**（app 89 例，核心 82 例不变）

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | `TITLE_INSET` 92→9（名字挪回红绿灯底下） | `nothing_is_drawn_under_the_traffic_lights` 红 | 红：`红绿灯底下有文字：["没有仓库 (Git)"]` |
| M2 | `with_titlebar_shown(false)`→`true` | `the_toolbar_band_is_the_window_titlebar` 红 | 红：`标题栏还单独占一条` |
| M3 | `TOOLBAR_H` 52→55 | 带高判据红 | **第一次跑是绿的** —— 见下 |
| M4 | `MIN_W` 870→770 | #30 的扫宽判据红 | 红：`"没有仓库 (Git)" 压住 "⊕"（13.0×12.5pt）` |
| M5 | `with_title_shown(false)`→`true` | 配置判据红 | 红：`系统标题还在画` |

**M3 是这批抓到的一条假绿**：`nothing_is_drawn_under_the_traffic_lights` 里所有断言都拿
`TOOLBAR_H` 当尺子（`r.min.y < TOOLBAR_H`），尺子自己变长它一点感觉没有 —— 52 这个数改回错的 55
全绿。补一句 `assert_eq!(TOOLBAR_H, 52.0, "参考图标题栏那一条实测 52pt")` 之后 M3 才红。
这正好是台账里第 10 类坑的**新变体**：以前防的是"判据用错口径"，这次是"**判据的尺子就是被测对象**"。

**仍然没看到过的**

- ~~**合并之后那一行的按钮还能不能点，没验到**~~ → **2026-10-21 用户目视确认：能点**。
  这条当时机器上验不了（`osascript … click at {1178,121}` 返回 `missing value` 且没弹卡，
  System Events 的坐标点击对 egui 窗口本来就不一定生效，机器上也没有 `cliclick`），
  是**人眼在真窗口里点了一下**定的。`fullSizeContentView` 没有把该区域的鼠标事件吞给窗口拖动。
- ~~**窗口还能不能拖**，同理没验~~ → **能拖**（同一句确认）。
  所以 `with_movable_by_background` 继续保持**不开**：它不是拖动生效的前提，
  而它的文档自己写着会影响滑块这类可拖控件，我们差异区有横向滚动、文件表有整行点击。
  顶栏那 52pt 现在同时是「可点的工具栏」和「可拖的标题栏」，靠的是 AppKit 默认裁决：
  控件收事件、空白处拖窗。**这条默认裁决本身没有判据**（无头帧里根本没有窗口管理器）。
- 870 这个地板**没在真窗口里试到**（拖到最小看有没有截断/撞车）。
- 目视这次只做了"看一眼顶栏"：确认了红绿灯 + 「comic (Git) main」+ 11 枚按钮同在一行、
  名字不再被压。**顺带看到两处别的**：① 未暂存列表最后一行被提交框那格**切了半行**；
  ② 我们自加的「全部暂存 / 全部取消暂存 / 放弃整组」三枚还在（#32 已登记要换成行末「⋯」）。
  半行那件事 #30 记过"地板以下才有"，但这是在默认 1050×545 上看到的 —— 说明那条结论也不完整，
  归到 #35 一起查。

---
## 差异区去噪 + 文件表左移（任务 #39，2026-10-21 晚）

**起因**：用户三句。「顶栏按钮可以点击，窗口可以拖动」（把 #34 那两条未知关掉）+
「变动文件列表，内容往左移动一点」+「文件改动详情，去掉 commit、Author、Date、改动描述、diff 信息」。

**决定与依据**

1. **那四样是 git 塞的，不是我们排的**。选一条提交之后差异区走的是
   `git show --no-color -M <oid>`，它的输出**开头就是一段日志头**：
   `commit <oid>` / `Author: …` / `Date: …` / 空行 / 缩进的提交说明。
   所以修在核心层：`commit_patch` 加 `--format=`，本机实测第一行立刻变成 `diff --git`。
   不在帧里拿正则删——那等于让每条通路（工作区 / 提交 / 单文件 / 整份补丁）各自再判一遍。
2. **「diff 信息」= `diff --git` / `index …` / `--- a/…` / `+++ b/…` 那几行文件头，也不画**。
   这条**是靠拢 SourceTree**，证据在参考图上：文件名那条灰带 (236,236,236) 止于 **y=368pt**，
   y=368..396 直接就是 hunk 那条带 (240,244,241)，**中间一行都没有**
   （逐行数背景众数 + 前景像素数，脚本 `/tmp/mf_rows.py`、`/tmp/mf_cols.py`）。
   我们之前把这几行灰字画在 hunk 带上面，是自己加的。
3. **文件表"太靠右"不是画错了，是要比 SourceTree 更靠左**。先把参考图重量一遍：
   分类行方框 x **149**、文件行方框 x **165**、状态图标 x **188**、文件名 x **208.5**
   （面板左缘 141 起算 = 8 / 24 / 47 / 67.5），而我们**画出来的正是 166 / 188 / 209** ——
   旧判据 `the_file_row_columns_are_at_the_measured_offsets` 一直是绿的。
   所以这一条按契约第②条记：**自加的密度调整，明确偏离 SourceTree**，不许写成"量来的"。
   偏移取 **8pt**，三档一起挪、列间距 22/43 一个不动；分类行那档（+8）留在参考图原位，
   否则它离面板左缘只剩 0pt。

**改了什么**：`gitcraft-core`：`commit_patch` 的 argv 多一条 `--format=`。
`gitcraft-app`：`layout()` 遇到 `META` 里的前缀**直接跳过**（原来是"画成灰字"），
`Kind::Meta` 整个删掉（留着就是死枚举，零警告构建立刻不绿），`META` 那份名单留着当"筛谁"的理由；
`CHK_ROW_X` 25→17、`ICON_X` 47→39、`NAME_X` 68→60。

**判据 + 反向变异**（核心 83 例、app 90 例，`scripts/gates.sh` 四条全绿）

新增三条：核心 `commit_patch_carries_no_log_header`（真临时仓库、真 git）、
app `the_diff_pane_starts_at_the_first_hunk`、app `the_file_row_columns_are_the_chosen_offsets`；
`the_file_row_columns_are_at_the_measured_offsets` 改名
`the_file_row_columns_are_8pt_left_of_the_reference`（断的还是**画出来的位置**，字面量写死）。

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | 核心去掉 `--format=` | 核心那条红 | 红：`补丁第一行是 "commit 813964…"` |
| M2 | `layout` 里删掉 `if META… { continue }` | 差异区第一行红 | 红：`差异区第一行是 "0"` —— `--- a/…` 被当成一行删除，行号算出 0 |
| M3 | `META` 名单清空（`[&str; 0]`） | 同上红 | 红。**这条故意不吃那份名单**：它断的是落点，名单只是筛的理由 |
| M4 | `CHK_ROW_X` 17→25（退回参考图） | 两条文件表判据红 | 红 ×2 |
| M5 | `ICON_X` 39→47 | 两条红 | 红 ×2 |
| M6 | `NAME_X` 60→68 | 两条红 | 红 ×2 |
| M7 | `CHK_HEAD_X` 8→17（分类行挤进文件行那档） | 三条红 | 红 ×3（多出 `the_section_box_sits_in_its_own_column`） |

M2 顺带抓到一条**探针口径**：`visible_texts` 里 hunk 那一行的两格**行号是空串**，
和 hunk 文字同一条 y，不排掉空文本就会顶到"第一行"上（第一次跑就报 `差异区第一行是 ""`）。
排空的理由写在判据注释里：会漏进来的文件头每一行都带字，排掉空的放水放不进去。

**仍然没看到过的**

- **这批欠一次真窗口目视**：18:20 屏幕锁在登录界面，`screencapture` 拿回来的是锁屏图，
  System Events 数到 0 个窗口 —— 机器上做不了。新构建已经起来了（`target/debug/gitcraft-app`），
  解锁就能看。要看三件事：① 文件表左移 8pt 之后够不够、有没有和分类行黏上（两档只差 9pt）；
  ② 差异区第一行是不是直接就是 hunk 那条带；③ 选一条提交，顶上再不该出现 commit/Author/Date。
- **8pt 这个数是拍的**。用户只说"往左移动一点"，没说多少；参考图实测 24/47/67.5，
  我们取 17/39/60。再多要多少、要不要连分类行一起挪，都没依据。
- **二进制文件现在显示「没有差异」**。它的补丁只有 `Binary files … differ` 一行，
  而这行在 `META` 里 —— 筛掉之后整个列表空了，走到 `lines.is_empty()` 那句。
  以前它会画成一行灰字。这条是这批**唯一一处已知的小退步**。
- **多文件那格（「全部变更」）现在看不出文件边界**：`diff --git` 是唯一的分隔，被筛掉了。
  单文件那格不受影响（也是用户点的那条）。要不要换成我们自己画一条文件名分隔，
  是 SourceTree 没有的东西 —— 想做必须先按契约第②条登记。
- 分类行 +8 与文件行 +17 之间**只剩 9pt**，视觉上分不分得开只有眼睛说了算，而这批眼睛用不了。
