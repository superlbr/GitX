# GitX 开发进度与 TODO

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

> **2026-10-24 更新：这一节的「复刻」已作废。** 用户拍板换成**现代化清爽风**
> （向 GitHub Desktop / Fork 靠：大留白、柔和灰阶、清晰信息层级），力度是「皮肤 + 结构」，
> 布局一起动。视觉基线从「本机 4.2.19 参考图」改为
> [`docs/DESIGN.md`](file:///Users/baorong/Developer/superlbr/gitx/docs/DESIGN.md)；
> `docs/reference/` 那四张截图降级为历史素材（仍受 Atlassian 条款约束，再分发要去掉），
> 不再作为设计锚点。下面 2026-10-20 / 10-21 的拍板文字原样留着，用来看「当时为什么这么定」。

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
  `ibtool --dump` 这个选项不存在 —— 所以**当年判了「数字拿不到」**。
  2026-10-24 更正：**数值读得出来**，走 `NSNib` 实例化（见下面「逆向本机 SourceTree.app」那节）；
  当年只试了 property list 那条路就下结论，属于「工具不认路，赖目标没给」。
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
> plist，`plutil` 拒、`ibtool --dump` 不存在）。下面这些是当年靠眼睛看出来的，现在有了第二条路
> （**2026-10-24 再更正：数字也读得出来**，把 nib 用 `NSNib` 实例化后问 frame —— 见「逆向本机
> SourceTree.app」那一节；当时是工具不认路，不是 nib 没给）。
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
- keyed archive **不是 property list**，`plutil -convert xml1` 直接拒绝。早先据此写过「nib 只能读到
  outlet 名和类名、读不到数值」——**那条已经作废**：数值实例化之后读得出来。
  工具在 `/tmp/nibwork/main.swift`（Swift + AppKit，一次性，不入库），要点三条：
  `NSNib(nibData:bundle:)` → `instantiate(withOwner: Stub(), topLevelObjects:&top)`，然后递归问对象 ——
  `NSWindow.contentRect(forFrameRect:)` 给内容区尺寸，`NSView.frame` 给每一格，
  `NSPopUpButton.menu?.items` 给菜单文案与顺序，`NSTableView.rowHeight` / `NSTableColumn.(title,width)`
  给表格（推送卡那四列 38/186/260/43 与行高 16 就是这么来的）。
  ① owner 必须传一个**对未定义键安静返回 nil** 的对象（`Stub` + `method_setImplementation` 掉
  `valueForUndefinedKey:`/`setValue:forUndefinedKey:`）：nib 里绑到缺失类的对象否则会抛异常，
  而我们只要布局、不要绑定值。
  ② NSWindow 原点在**左下**，换算成我们的「距内容区顶边」要 `顶 = 内容高 − y − 高`。
  ③ `contentRect` 给的那个数**已经**扣掉了标题栏，别再减 32——设置卡早先按 409 摆就是把 441 又减了一遍。
  没读的一格也记在这儿：`contentMinSize`（窗口最小尺寸）工具没问过，#30 那两个数仍是参考图量的。

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
12. **夹具必须让被测的几档两两有区别**：五档「推送行为」对着两条分支的夹具，上流/简单/当前三档
    算出的勾选完全相同——把 `Simple` 写成 `upstream.is_some()` 改坏后**全绿**。这不是判据写得不对，
    是**样本不够**：加两条分支（上游不同名的、上游同名但非当前的）让五档结果互不相同，变异当场红。
    第 2 条讲的是一个控件吃掉另一个，这一条讲的是**夹具太瘦，几档挤在同一结果上**。
13. **一个值有两条出口，判据得两条都问**：`PushBehavior` 显示走 `label()`→`index()`，落库走 `key()`。
    只读 `app.push_behavior` 与 `Prefs::load` 的话，把 `from_index` 里「简单/当前」两项互换
    （界面念错、存的还对）**不会红**。补一条「那一格要把选中的这一档念回来」。

---

## 还没接上的 / 已知缺口

**任务单上欠的**
- **#26** 文件行末那枚「⋯」弹层（现在用分类行头的「放弃整组」临时顶着，判据钉的是那枚的文字）。
- **「紧凑视图」要按两态接**：strings 里 `LogCompactView`=紧凑视图 / `LogExtendedView`=扩展视图，
  它**不是**「还没接的一枚按钮」，是有的第二层 → 该带箭头（现在 `only_the_three_switches_carry_an_arrow`
  钉的是「不带箭头」，#31 一起翻）。
- **「所有分支」这一档 = 不给 git 传 rev**（走 HEAD 那一条线）——**#31 已闭**：现在是三档
  （`所有分支` 传 `--all` / `当前分支` 不传 / 具体分支传位置参数），核心层用 `git::Scope`
  枚举而不是「rev + 布尔」。**没结案的还剩一条**：SourceTree 开仓默认落在哪一档。
  2026-10-24 拿 `NSNib` 实例化复核过：`FileStatusView2.nib` / `LogView.nib` 里**没有**
  带「所有分支/当前分支」那几项的菜单，那枚按钮的列表是代码按当前仓库现搭的，
  所以 nib 这条路到这儿确实取不到默认值（不是「工具读不出数值」，那一条已经作废，见上面那节）；
  我们按「不动现状」取了 `当前分支`。
- **文案与 strings 对不上的**（#31/#32）：**排序那档、路径排序那行、跳转框 hint 已在 #31 逐字抄过**
  （按日期排序 / 待定的文件, 已依照路径排序 / 转到Commit的SHA-1）。hunk 头与区块文案早就对上了
  （`块 %d ： 行 %d-%d` = strings 的 `SummaryTextHunkSameLines`，`暂存区块 / 取消暂存区块 /
  放弃区块` 见 #22 那批）。**还剩**：领先落后那两枚的**呈现位置**没对过参考图；
  侧栏还有**子模块**、**子树**两组（我们的侧栏没有）；
  「反向排序」那一档（官方 `已依照路径 (反向) 排序`）我们**只做正向**，等真接反向时再抄那条。
  **「侧栏没有折叠三角/计数」这条结论只对着那张参考截图**，不代表 SourceTree 全部——`#32` 会连这条一起复核。
- **还没量到的**：`LogFollowFirst`=仅首个父级 / `LogAllParents`=所有父级 —— 一个我们完全没有的
  图遍历维度；提交图标上的**暂存数角标**；`upstream/HEAD` 那枚药丸。

**功能面**：设置卡只接了「通用 / 关于」两页（其余六页一张空壳比没有更糟，见 `PrefPage`）、
网络远端的 push/pull 判据（本地裸仓库的端到端已有）、命令层 + 命令面板（⌘K 那半在 egui 版还没搬）、
多选、拖拽与「扫描文件夹」入口、原生目录选择器（要引 `rfd`）、深色模式、
按**行**暂存（`line.hunk` 已经给每一行留了值）、
`COL_GRAPH` 超过 4 道目前是 **assert 报错**（没做列宽自适应）、`--depth`/bare/submodule、
克隆进度与 stderr 流式（现在只有结果一条 note）、`.gitignore` 编辑器、`--no-verify` / `-S` 签名、
上面「它有一整块」那一段里的全部。
对照表里这几条现在只是**文档**、没有判据：窄列路径省略换头部、
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
  （`src/ui/fonts.rs:5` 的注释仍指向这一节；`uiprobe/` 已于 2026-10-21 按本台账「跑完就删」
  的惯例删除，结论都在这一节里）
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

### 最近的批次：全文在下面

- **#31** 筛选条照抄官方文案 + 分支范围三档（`--all` / 不传 / 位置参数各占 argv 里同一格）
- **#30** 窗口地板 770×324：**高度不背锅，撞车的是宽度**；顺带把「裁剪后可见」做成 harness 的一等公民
- **#34** 标题栏合并：那一行 52pt **就是** macOS 标题栏（红绿灯 / 仓库名 / 11 枚按钮同排），
  代价是地板从 770 抬到 **870**
- **#39** 差异区去噪（git 的文件头 + `git show` 的日志头一行不画，**靠拢**）+
  文件表三档各左移 8pt（**偏离**，用户拍板的密度）
- **贮藏可点开 + 名字剥包装 + 差异向右看**：侧栏那行只留用户说明，包装前缀剥掉
- **#47** 三张卡（添加仓库 / 提交 / 设置）：`Form` 一个枚举决定卡上长什么；设置卡内容区
  实测 **855×441**（早先那个 409 是把标题栏又减了一遍）
- **#48** 推送卡 + 拉取卡按 nib 逐格摆（`GitPushSheet` 650×271 / `GitPullSheet` 614×299）、
  整份差异按文件折叠；拉取先切再拉、rebase 策略两支都明写
- **#49** 差异区搜索是**过滤**不是高亮：命中行 + 文件头 + hunk 头留下，只在状态变那一下重建
- **#43/#52** 多远端：默认远端那条链**不猜 origin**、五档「推送行为」只改预选不进实参、
  一条真 bug（分岔时没明写 `--no-rebase`，git ≥2.34 直接拒跑）、**两条假绿**
  （夹具让五档两两相同、档位有 `index()`/`key()` 两条路只验到一半）
- **界面重设计** 换锚：不再复刻 SourceTree，改**现代化清爽风**。首页两条顶带并成 46pt、
  侧栏 141→188、行高 26→28、选中改 tint + 左缘主色条、状态色块 12pt 圆角；弹窗去格线、
  三档宽度、确认卡单列。**只写了文档，代码与判据锚点一行没动**（全文见下）

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
最小窗口尺寸**没量到**，所以这两个数是按**我们的
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

## 文件溯源浮层：历史 / 逐行归属 / 全文 / 恢复旧版（任务 #40，2026-10-21 深夜）

**决定与依据**

- 核心层的 `file_log` / `blame` / `rev_file_text` / `restore_file` **早已有单测、界面零调用**。
  这一批只接界面，不改 git 语义。
- SourceTree 4.2.19 的文件历史是**独立窗口**，两张参考截图都没拍到，`.nib` 里也没读到它的控件结构。
  所以这一块按契约第②条登记为**我们的增强**：用同层浮层、页签式布局，不声称「照抄 SourceTree」。
  尺寸 `HIST_W/H`、`HIST_LIST_W` 都是估的，没有量值。
- 三个请求各走一条线程，回复带 `seq` + `path`（全文/补丁再带 `index`）：
  换文件、换仓库、换选中项之后，迟到的旧回复对不上号就丢掉。帧里不跑 git。
- `blame` 走**工作区那份**（`oid = None`）：「现在这些行是谁写的」；历史页签选中项则走那条提交的补丁。
- 「恢复到这一版」是覆盖工作区和索引的破坏性动作，必须走 `land` → 确认卡，不能 `act` 直落。
- 「全文」与「补丁」分开取：看全文的人通常是在找「这行原来长什么样」，和逐行差异不是同一件事。

**改了什么**

- `src/ui/state.rs`：`HistTab` / `HistView` / `FileHistory`（含 `full_for` 防换行错配）、
  四条消息 `FileLog` / `HistoryPatch` / `HistoryText` / `HistoryBlame`、`Action::RestoreFile`。
- `src/ui/data.rs`：`spawn_file_log` / `spawn_history_patch` / `spawn_history_text` / `spawn_history_blame`、
  `change_label`、文件行菜单新增「查看文件历史」、提交改动列表的 `commit_file_menu`。
- `src/ui/widgets.rs`：`history_row` / `blame_row` / `history_diff_row` / `history_text_row` / `history_note`；
  `section` 多一个 `hist` 出参。
- `src/ui/app.rs`：`open_history`、`history_layer`（遮罩 + 页签 + 双栏 + Esc/点外关闭）、poll 四个分支、
  切换仓库时清面板、`Failed` 时熄灭所有「读取中」。
- `src/ui/theme.rs`：`HIST_*` 九个无参考图常量。
- `src/geometry.rs`：6 条新判据 + `history_repo` / `open_history_for_test` / `history_click` 三个助手。
  `history_click` 按**字号 11.5** 认面板行：日志行是 12.5，按文字找会先撞上遮罩后面的日志行。

**判据 + 反向变异表**

| 判据 | 钉的是 |
|---|---|
| `the_file_history_lists_the_commits_that_touched_the_file` | 右键菜单 → `git log --follow` → 面板列表；默认选最新；右侧是那条提交对该文件的补丁 |
| `picking_an_older_entry_swaps_the_patch` | 选中项变化 → 新请求 → 同 `seq`/`path`/`index` 才落地；旧补丁不残留 |
| `the_blame_tab_attributes_each_line_to_a_commit` | 真 `git blame`：四行、第二行内容/作者/提交号；blame 行与作者真的画出来 |
| `the_full_text_view_shows_the_file_at_that_version` | 「全文」拉的是该版本原文件；换选中项后 `full_for` 跟着换 |
| `restoring_an_old_version_asks_before_it_overwrites_the_worktree` | 恢复按钮**只开确认卡**；确认前工作区一个字节不许变；点「确定」后才覆盖；面板不关 |
| `the_file_history_layer_closes_on_escape_and_outside_click` | Esc 关；点卡片外遮罩关（按/松必须同一个 Context） |

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | 选中项变化时删掉 `spawn_history_patch` | `picking…` 红 | 红：600 帧还没等到（`patch_loading` 永远亮着） |
| M2 | 恢复改成 `self.act(...)`，绕过 `land` | `restoring…` 红 | 红：`没问一句就动手了` |
| M3 | 文件列表不调 `open_history` | `the_file_history_lists…` 红 | 红：600 帧还没等到（面板从没打开） |

**仍然没看到过的**

- **这一批同样欠一次真窗口目视**：浮层的宽度、列表 320pt、46pt 行高、双栏分隔位置全是估的；
  没有 SourceTree 的对应截图可量。要看三件事：① 860×500 在 870 宽窗口里会不会缩得太窄；
  ② 历史行两行文字在 46pt 里是否挤；③ 页签右侧三个按钮（改动/全文/恢复到这一版）在窄窗口会不会撞。
- **`blame` 只认工作区**：选中历史里某条提交时，逐行归属仍显示当前文件。SourceTree 是否跟随选中项，
  没有依据；要做成两态得先登记（契约第②条）。
- **历史只取 200 条**（`FILE_LOG_MAX`）。再多没有分页，也没有「加载更多」。
- **恢复的副作用写进了确认文案**：`git checkout <oid> -- <path>` 会连索引一起改，
  所以恢复完这个文件处于**已暂存修改**。这是 git 语义，不是我们的选择，但没有真窗口复看过。
- **选中「删除该文件的提交」时全文是一句错误提示**（`rev_file_text` 读不到），
  不是空白页；这条只在核心层有单测，面板里的呈现没目视过。
- **超长 / 二进制文件**：全文走核心层字节上限，超限时同样是一句错误提示；没有流式或截断提示。

---

## 冲突解决 + 继续/中止（任务 #41，2026-10-21 深夜）

**决定与依据**

- 核心层的 `resolve_ours` / `resolve_theirs` / `conflict_content` / `resolve_manual` /
  `continue_op` / `abort_op` 早已存在、界面零调用；`repo_state` 也在 `status()` 里算好了，
  只是没人读。这一批只接界面，外加两个核心缺口：`mark_resolved` / `mark_unresolved`。
- **修了一个核心 bug**：`repo_state` 原来跑 `git rev-parse --git-dir`（输出是 `.git`），
  再拿这个相对路径去 `exists()`——查的是壳进程 cwd，不是被打开的仓库。所以任何仓库都报
  `repo_state = "none"`，冲突横幅永远画不出来。改成 `--absolute-git-dir`，三条核心判据守住。
- `parse_status` 把 `u` 行标成 `staged: true`（porcelain 里它确实占着索引）。界面不能照搬：
  冲突文件若混进「已暂存文件」，用户会以为已经解决。`file_list` 先按 `Unmerged` 挑出来单独成组，
  **不改核心的状态模型**。
- 文案全部取自本机 SourceTree 4.2.19 `zh_CN.lproj/Localizable.strings`：
  「冲突的文件」「未解决的合并：」「未解决的变基:」「继续」「终止」「放弃变基」
  「确认使用「我的版本」解决冲突」等。遴选按官方菜单叫「遴选」。
- 冲突行**不带方框**（`check: None`）：冲突不是「暂存/未暂存」两态，勾上不等于解决。
  点行看的是工作区那份（带冲突标记）的差异，不是 `--cached`。
- 继续/终止、ours/theirs、标记未解决都会丢东西或落提交，全部进 `needs_confirm`；
  文件表出口从 `self.act(...)` 改成 `land(Intent{..})`，让右键菜单里的「放弃」也走同一条确认路径。
- 横幅只在 `repo_state != "none"` 时占位（30pt）；正常仓库的几何一个像素都不动。

**改了什么**

- `src/git.rs`：`mark_resolved`（`git add -- path`）、`mark_unresolved`（`git checkout -m -- path`）；
  `repo_state` 改绝对 git-dir；三条核心判据 + `merge_conflict()` 夹具（真分支、真冲突）。
- `src/ui/state.rs`：`Action::{ResolveOurs, ResolveTheirs, MarkResolved, MarkUnresolved, ContinueOp, AbortOp}`。
- `src/ui/data.rs`：`conflict_menu`、`is_conflict`。
- `src/ui/widgets.rs`：`conflict_section`（无组框、无整组操作）、`op_banner`（标题 + 冲突数 + 继续/终止）、
  `op_noun/op_title/continue_label/abort_label`、确认文案；`needs_confirm` 新增六条。
- `src/ui/app.rs`：`act()` 映射；`file_list` 冲突分组 + 横幅 + 出口改走 `land`；
  提交描述框的 Inter 行高溢出 1.6pt 记录在 `COMMIT_H` 注释里。
- `src/ui/theme.rs`：`WARN_BG` / `CONFLICT_BANNER_H` 两个无参考图常量。
- `src/geometry.rs`：3 条新判据 + `conflict_repo` 夹具；`band()` 容差 1.5 → 2.0（Inter 行盒）。

**判据 + 反向变异表**

| 判据 | 钉的是 |
|---|---|
| `resolving_with_theirs_stages_that_side_and_continue_finishes_the_merge` | 真冲突 → theirs 改工作区并暂存 → `continue_op` 落合并提交、状态回到 none（核心层） |
| `mark_resolved_stages_the_hand_edited_file_and_mark_unresolved_puts_it_back` | 手工合并后 `add` 离开未合并态；`checkout -m` 把冲突标记放回去（核心层） |
| `abort_restores_the_pre_merge_worktree` | `merge --abort` 回到合并前内容（核心层） |
| `a_half_finished_merge_gets_its_own_banner_and_conflict_group` | 横幅「未解决的合并：冲突：1」；冲突单独成组、**不**混进已暂存 |
| `resolving_with_ours_and_continuing_finishes_the_merge` | 右键 ours → 确认卡 → 冲突组消失、横幅还在 → 继续 → 确认 → 真合并提交 |
| `aborting_a_merge_asks_before_it_rolls_back` | 「终止」先开卡；点「取消」仓库一动不许动；确认后回到 `mine` |

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | `is_conflict` 恒 false | 分组/横幅那两条红 | 红：横幅只剩「未解决的合并：」，没有冲突数，`已暂存文件（1）` 里混着 `a.txt` |
| M2 | `needs_confirm` 删掉 `ResolveOurs` | `resolving…` 红 | 红：`覆盖工作区之前没先问一句` |
| M3 | `needs_confirm` 删掉 `AbortOp` | `aborting…` 红 | 红：终止直接执行，确认卡是 None |
| M4 | `repo_state` 退回 `--git-dir` | 三条核心冲突判据红 | 红：`repo_state` 恒为 `none`，合并提交/中止断言全挂 |

**仍然没看到过的**

- **真窗口目视**：横幅只在冲突时出现，30pt 高、左侧标题右侧两枚小按钮；窄到 870 时
  「未解决的合并：冲突：1」会不会撞按钮，没有目视。`WARN_BG` 的浅红也是估的。
- **rebase / cherry-pick 只验证了 `repo_state` 的分支逻辑**（核心层读 sentinel 文件），
  没有造真 rebase / cherry-pick 冲突仓库跑端到端；界面文案按官方 strings 写，但三种操作的
  continue/abort 语义（尤其 rebase 的 `--continue` 与 `--abort`）没有真仓库判据。
- **手动编辑冲突文件**没有内置编辑器：用户得在外部改完再点「标记为已解决」。
  `conflict_content` / `resolve_manual` 仍是零调用（那需要文本编辑器，不在这一批）。
- ~~**继续合并不会帮用户填提交信息**~~ **已在 CI 报错后修掉（2026-10-22）**：`--continue`
  三兄弟默认开 $EDITOR，CI 的 dumb 终端和从 Finder 启动的 GUI（连 TERM 都没有）都会吃到
  "Terminal is dumb, but EDITOR unset"。`continue_op` 改为 `-c core.editor=true` 跑三条
  `--continue`，直接采纳 git 备在 MERGE_MSG 里的默认信息；`resolving_with_theirs_stages_…`
  那条判据就是回归闸（它在本机过是碰巧有 EDITOR，CI 挂了才暴露）。

---
## 重命名分支（任务 #42，2026-10-22 上午）

**这批也是接线**：核心层 `git::rename_branch` 早就存在（`git branch -m old new`），界面层
零调用。判据同样走端到端：真建一条分支 → 右键菜单 → 填卡 → Enter → 回仓库用 `git branch` 复核。

**决定与依据**

- 卡上**不预填旧名字**：预填后用户会在末尾接着打，得到 `main-new` 这种串。旧名字写在
  说明行（「旧名字 改名为」），输入框空着等新名字；空名字时主按钮不产出动作。
- 主按钮文案「重命名」与卡标题「重命名分支」同前缀，判据里按文字定位会先撞上标题。
  所以判据走 **Enter**——它和点主按钮是同一条 `primary_action` 出口，不另开一条路径。
- 菜单顺序按危险程度排：合并 → 重命名 → 删除。改名不动提交、不动工作区，落在删除之前。

**改了什么**

- `src/git.rs`：核心判据 `renaming_the_current_branch_keeps_its_commits_and_head`
  （旧名字从 refs 消失、新名字是当前分支、HEAD 不变）。
- `src/ui/state.rs`：`Action::RenameBranch { from, to }`、`Form::RenameBranch { from }`、
  `Modal::rename_branch`、hint「新分支名」、primary「重命名」。
- `src/ui/widgets.rs`：`primary_action` 里 `raw` 非空才产出 `RenameBranch`。
- `src/ui/app.rs`：`act()` 映射到 `git::rename_branch`；分支右键菜单加「重命名分支…」。
- `src/geometry.rs`：`renaming_a_branch_lands_a_real_rename`。

**判据 + 反向变异表**

| 判据 | 钉的是 |
|---|---|
| `renaming_the_current_branch_keeps_its_commits_and_head` | 改名后旧 ref 消失、新 ref 是 HEAD、提交号原样（核心层） |
| `renaming_a_branch_lands_a_real_rename` | 右键 → 卡 → 键入 → Enter 之后，仓库里 `git branch` 只剩新名字、旧名字没了 |

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | `act()` 不调 `rename_branch`，直接 `Ok(())` | 判据红 | 红：快照里等不到「新名字」 |
| M2 | `primary_action` 对 `RenameBranch` 返回 `None` | 判据红 | 红：卡不关、仓库不动 |
| M3 | 菜单里删掉「重命名分支…」这一项 | 判据红 | 红：`menu_pick` 在菜单里找不到这一项 |

**顺带收拾的两处判据（状态栏 / toast 接线后的红）**

- 状态栏左边也写着「main ↑2」，按绘制顺序排在侧栏**之前**，`side_text` 不卡 y 会先撞上它；
  上界收到 `WIN_H - STATUS_H`。状态栏原来把「main ↑2」拼成一条 galley，`↑2` 无法单独定位，
  改成逐段画（间隔 4pt），判据才能比「↑2 跟没跟在分支名后面」。
- 表头竖线判据扫全宽横线时会把状态栏顶那条 1pt 分隔线算进来，加 y 带过滤。
  提交图判据的 7 条提交被状态栏吃掉一行，那条判据改用 `WIN_H + STATUS_H` 的窗口跑
  （它判的是图元越不越列，不是一屏塞得下几条）。
- toast 在 `Order::Foreground`：全新 `Context` 的第一遍不画那一层，而且 `Area` 要几帧才把
  位置收敛（第一遍只量尺寸）。两条 toast 判据改成同一个 `Context` 多跑几遍再量；
  卡片底色带 Area 的淡入透明度，所以只认位置/宽度，不认「纯白」。
  `Frame` 的底色+阴影是塞在 `Shape::Vec` 里发的，`rects()/rounded()` 已能拆组（否则 toast 隐形）。

**仍然没看到过的**

- **真窗口目视**：改完名后侧栏点、日志里的分支药丸、状态栏同时换名字，没有在真窗口里看过。
- **远端分支改名**：菜单只出现在本地分支上；`origin/xxx` 这一类没有入口，核心层也没做
  `push origin :old old:new` 的远端联动——本地改完后远端仍是旧名字，这一条没判据。
- **当前分支正在被别的 worktree 占用**时 `git branch -m` 会失败，错误会原样落到 toast，
  但没造那个夹具，文案也没目视。



---
## 底部状态栏与 toast（2026-10-22）

**这批是借 deckcraft 的两件家具**：状态栏（`Panel::bottom` + painter 直绘）和 toast
（`Area` 锚在状态栏上方）。动机：操作反馈原来挤在 tab 条右端的 240pt 定宽小格里，
长提示被截断、还常驻占一行；状态栏则是 SourceTree 有、我们一直空着的窗底。

**决定与依据**

- 状态栏 **26pt 没有参考依据**：四张参考图全把窗口底裁掉了。高度按行节奏取 `ROW_H`，
  内容只挑没有争议的三样（分支名、↑↓ 计数、进行中操作）+ 右侧仓库路径，等拿到一张
  带状态栏的参考图或真窗口对照再修（theme.rs `STATUS_H` 注释已登记）。
- **↑↓ 各自只在有数的时候出现**：`↑0 ↓0` 是噪音，没上游时更是谎言——比「不显示全零」
  更严，连个位数都不许冒出来。进行中操作（合并/遴选/变基）只认 `repo_state` 非 `none`。
- toast 生命周期：8 秒自己消失（`request_repaint_after(500ms)` 唤醒下一帧结算）、
  点 × 立刻关。`note_at`（首次出现时刻）放在 state 里而不是 toast 自己攒：无头判据要能
  在 `RawInput::time` 里直接造「已经过了 8 秒」。
- **面板叠序踩坑**：egui 的规矩是**后声明的避让先声明的**（跟直觉相反）。状态栏要贴窗底，
  就得在 `shell()` 里排在侧栏/暂存带**之前**声明，否则状态栏只画到侧栏左缘为止
  （`bands_match_the_measured_reference` 量出了那条斜茬）。
- 状态栏左边逐段画（分支、↑、↓、操作各一块 galley）：拼成一条字符串的话，
  「↑2 跟没跟在分支名后面」这类判据按前缀定位不到。

**无头判据的三条新账**

- `Order::Foreground` 的 Area：全新 `Context` 第一遍不画、第二遍起才有形状，且 **Area 有
  淡入动画**——第二遍时白底是 `#3C3C3C3C`（255×0.235）、阴影 α28→7，全是
  `gamma_multiply(进度)` 的中间色。判据要多跑几帧等动画收敛，颜色断言也别卡「纯白」。
- `Frame` 带阴影时底色**塞在 `Shape::Vec` 组里发**（egui 0.37 `frame.rs` 的
  `Shape::Vec([shadow, frame])`）：`rects()/rounded()` 不拆组的话整张卡对判据隐形。
  新增 `collect_rects` 递归拆组。
- 状态栏让出的 26pt 波及一批旧判据（带的下缘、侧栏文字 y 带、表头横线搜索区、
  日志虚拟化行数），全部跟着 `WIN_H - STATUS_H` 重排。

**判据**

| 判据 | 钉的是 |
|---|---|
| `the_status_bar_bands_the_bottom_edge` | 窗底 26pt 灰条横贯整窗，暂存带底边上移；`STATUS_H=26` 钉常量 |
| `the_status_bar_names_the_branch_and_counts` | 分支名贴左、↑2 跟在后面、绝不出现 ↑0/↓0 |
| `the_note_shows_as_a_toast_above_the_status_bar` | 卡片悬在状态栏上 8pt、水平居中、不超宽，文字在卡内 |
| `the_toast_closes_on_click_and_fades_after_eight_seconds` | 真 PointerButton 点 × 立刻关；拨钟 8.5s 后自己消失 |

**仍然没看到过的**

- **真窗口目视**：状态栏与 toast 的字色、间距、阴影浓淡都没在真窗口里看过。
- toast 的出入场动画（淡入有了，淡出是瞬间消失）与多行提示的换行观感没有对照。

---

## 修正上一条提交（amend HEAD，2026-10-22）

**这批也是接线**：核心层 `git::commit` 早就有 `amend` 参数，界面层十一处调用全传 `false`。
加的是一枚勾 + 一条出口。

**决定与依据**

- **单独 `Action::Amend(String)`，不给 `Commit` 加布尔字段**：它要过确认卡、落地后还要把勾
  去掉，和普通提交不是同一条路。合在一个变体里的话，`needs_confirm` 与 `clear_msg_on_load`
  都得在运行时读布尔，判据也只能间接钉。
- **勾上时不再要求暂存区非空**：`git commit --amend` 只改信息本来就合法。原来的
  `staged_count > 0` 闸门在 amend 时会把「只改提交信息」这条正当用法一起拦掉。
- **必须过确认卡**：amend 换的是已经落地的 HEAD，推送过的分支这一下会让本地与远端分叉。
  拦在 `needs_confirm`（出口）而不是勾选框上——以后再加入口也不会漏。
- **落地后把勾清掉**：不 `self.amend = false` 的话，下一次普通提交会静默变成 amend，
  那正是最该防的误操作。清空动作挂在 `Msg::Loaded` 的 `clear_msg_on_load` 块里，
  与提交信息一起走（失败时两者都留着）。
- 按钮文案跟着勾换（「提交到 main (2)」↔「修正上一条提交」），否则勾在左边、按钮还说
  「提交」，用户分不清落地的是哪一条。

**改了什么**

- `src/git.rs`：核心判据 `amend_replaces_head_instead_of_adding_a_commit`
  （提交条数仍是 1、信息换成新的、已暂存内容并进同一条）。
- `src/ui/state.rs`：`Action::Amend(String)`、`Gitx.amend: bool`。
- `src/ui/app.rs`：`act()` 映射到 `git::commit(..., amend: true, ...)`；提交框加勾选框与
  两套按钮文案；`Msg::Loaded` 里落地后复位勾。
- `src/ui/widgets.rs`：`needs_confirm` 收 `Amend`；`confirm_of` 给标题「修正上一条提交」
  与「推送过的分支要强推」的说明。
- `src/geometry.rs`：`ticking_amend_rewrites_head_instead_of_adding_a_commit`。

**判据 + 反向变异表**

| 判据 | 钉的是 |
|---|---|
| `amend_replaces_head_instead_of_adding_a_commit` | `--amend` 不多出提交，HEAD 的信息与树都被替换（核心层） |
| `ticking_amend_rewrites_head_instead_of_adding_a_commit` | 勾上 → 按钮换文案 → 确认卡 → 确定之后：条数不变、勾复位、暂存内容并进 HEAD |

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | `needs_confirm` 去掉 `Action::Amend` | 判据红 | 红：`amend 没先问一句` |
| M2 | `act()` 把 `Amend` 当普通提交（`amend=false`） | 判据红 | 红：提交条数 2 ≠ 1 |
| M3 | 删掉落地后的 `self.amend = false` | 判据红 | 红：`amend 落地后勾还留着` |

**无头判据踩到的两处**

- 勾选框刚被点过，焦点在它身上，`⌘Enter` 会被当成它的激活键（把勾又按掉），于是发出的是
  普通 `Commit`、不过确认卡。判据改用点按钮，与回车走同一个 `Action::Amend` 出口。
- 确认卡也在 `Order::Foreground`，`click_text` 用全新 `Context` 第一遍画不出那一层；
  改用既有的 `click_warm(&mut app, "确定")`（同 `Context` 多跑几帧）。

**仍然没看到过的**

- **真窗口目视**：勾选框压在提交框左下角、与按钮同行的间距，没在真窗口里看过。
- **amend 之后的推送**：本地改写了 HEAD、远端仍是旧的，界面没有「需要强推」的提示，
  只靠确认卡那句说明；`git push` 会失败并把错误落到 toast，没造那个夹具。
- **冲突中 amend**：合并/变基进行到一半时勾上会怎样（git 自己会拒），文案没目视，
  也没判据。

---

## 筛选条照抄官方文案 + 分支范围三档（任务 #31，2026-10-23 上午）

**为什么先做它**：#29 把筛选条改成带箭头的弹出按钮时，自己登记了两笔债——① 「所有分支」那一档
**标签和语义正好说反**（不传 rev 走的是 HEAD，那是「当前分支」）；② 排序档、路径排序那行、
跳转框 hint 三个文案是我们自己写的，官方另有其字。这两笔都属于契约第③条（**它有的、我们写错了，
一律按它的文案与结构照抄**），比「补一个它没有的面板」优先。

**决定与依据**

1. **文案只从本机 app 取**：`/Applications/Sourcetree.app/Contents/Resources/zh_CN.lproj/Localizable.strings`
   是 UTF-16LE，先 `iconv -f UTF-16 -t UTF-8` 再搜键名。抄到的四条连键名一起注在常量后面
   （`LogDateOrder` / `LogBranchesCurrent` / `JumpToRevisionLabelGit` / `PendingFiles`+`SortedByPath`），
   下次核对按键搜，不用再猜这字是从哪来的。
2. **核心层用 `git::Scope` 枚举，不用「rev + 布尔」两个字段**：`{rev: Some("side"), all: true}`
   这种组合菜单产不出来（`git log --all side` 问的根本不是用户选的那一档），而把它**从类型上建不出来**
   比拿一条守卫去防便宜。三档各占 argv 里 `--` 之前那**同一格**：`--all` / 什么都不发 / 那条分支名。
3. **默认那一档拆成 `default_scope()` 函数**。原因是台账方法论第 11 条那一招的复用：无头夹具
   （`geometry.rs::shell`）自己填 `Gitx` 的字段，产品构造函数里那一位**在任何一帧里都读不到**，
   改坏它一条判据都不会红——这是方法论第 5 条说的「删掉也不红」，但它不是死代码，是**没处读的默认值**。
   拆出来以后 `the_log_opens_on_the_current_branch` 直接读函数，和核心层
   `commits()` == `commits_on(HEAD)` 配成一对（一条钉壳的默认、一条钉核心的默认）。
4. **「待定的文件」那行去掉自加的「共 N 个」**：官方那一整行就是 `待定的文件, 已依照路径排序`，
   文件数是我们自己塞的。数量在状态栏和分类行头（`已暂存文件（1）`）都还看得到，不算丢信息。
5. **「紧凑视图」这批**仍然**不接**，但证据更新了。之前只记着「没量过它折叠什么」，这次把 app 包搜了一遍：
   `compactViewEnabled` 只在 **`RepoBrowserWindow.nib`**（仓库浏览器那个独立窗口）里命中，
   主二进制里另有 `showCompactView` / `setCompactViewEnabled:`，且带 `TB,N,V_` 前缀
   （那是 `NSToolbar` item 的 user-default 键）——**「工具栏上有这么一枚开关」有证据，
   「它在日志里折叠掉什么」依旧零证据**（`LogView.nib` 里搜不到 compact）。
   所以保持 #29 立的规矩：不给箭头、不接第二层，点了说实话。造一个空菜单假装像下拉，比承认没接更糟。

**改了什么**

- `src/git.rs`：新增 `pub enum Scope { All, Current, Rev(String) }`；`log_args` / `log_commits` /
  `commits_page` 那一位从 `rev: Option<&str>` 换成 `scope: &Scope`；`commits` / `commits_topo`
  传 `Current`，`commits_on` 传 `Rev`。
- `src/ui/state.rs`：`log_rev: Option<String>` → `scope: git::Scope`（一个字段，不是两个）。
- `src/ui/app.rs`：`set_rev` → `set_scope`（先比后写，同档重点不重取）；`ask_for_more` / `act`
  里参数一次抓齐；新增 `default_scope()`；筛选条那枚按钮的菜单项从「所有分支 + 本地分支」
  变成「所有分支 + **当前分支** + 本地分支」；跳转框 hint、文件表那行文案。
- `src/ui/widgets.rs`：`DATE_ORDER` 改字面、新增 `CURRENT_BRANCH` / `PENDING_SORTED_BY_PATH` /
  `JUMP_TO_SHA` 三条官方文案常量与 `scope_label()`（档位→标签**只这一处**，和 #22 那次
  「两份翻译」的教训同一个防线）。
- `src/ui/data.rs`：`snapshot_of` / `spawn_snapshot` 跟着换签名。

**判据 + 反向变异表**（核心 90 例、app 111 例，`scripts/gates.sh` 四条全绿）

新增四条：核心 `the_all_scope_adds_only_dash_all_in_the_rev_slot`（argv 逐字节 + 位置）、
核心 `the_all_scope_reaches_commits_only_a_side_branch_has`（真 git，结尾拿 `rev-list --all --count` 复核条数）、
app `the_log_opens_on_the_current_branch`、app `the_filter_bar_and_summary_carry_the_official_strings`。
改写两条：`the_scope_menu_walks_only_the_picked_branch` → `the_scope_menu_switches_between_all_branches_and_the_current_one`
（三档各走一遍，每档都比**提交主题**再比一次长度）；`only_the_three_switches_carry_an_arrow` 认的默认标签
从「所有分支」换成「当前分支」。

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | `Scope::All` 那一支不 push `--all`（枚举留着、argv 里吃掉） | 核心两条 + app 三档那条红 | 红 ×3，app 那句是 `「所有分支」那一档没有「只在支线」：["只在主线", "起点"]` |
| M2 | `--all` 改到 pathspec **之后** | 核心 argv 那条红 | argv 那条红：`--all 落在 -- 后面就被当成路径了`。**端到端那条反而绿**——夹具没开路径筛选，`--` 压根不存在，git 把末尾的 `--all` 照样当 rev 解析。所以「参数位置」这类契约**只有 argv 层看得见**，这是方法论第 8 条（界面类优先端到端）的反向补充 |
| M3 | `default_scope()` 改成 `All` | 只该红配置那条 | 红：`开仓默认没走当前分支`（其余全绿，正是它该孤立的形态） |
| M4 | `scope_label` 两态标签说反 | 三条标签判据红 | 红 ×3 |
| M5 | `DATE_ORDER` 退回自创的「日期顺序」 | 整串那条红 | 红**只有它**。另外三条引用这个常量的判据（第二层标记、排序菜单、三档范围）**全绿** = 方法论第 1 条「常量自比」的又一实证，也是这批新判据必须写**字面量**的理由 |
| M6 | `JUMP_TO_SHA` 退回「跳转到:」 | 整串那条红 | 红：`没有整串「转到Commit的SHA-1」`，失败信息把整帧文字倒出来，旧文案在哪一格一眼可见 |
| M7 | `PENDING_SORTED_BY_PATH` 换成官方**反向**那条（只多 `(反向)`） | 整串那条红 | 红：`没有整串「待定的文件, 已依照路径排序」`。正向/反向只差四个字，按 `contains` 比放得住 |
| M8 | `set_scope` 删掉那次 `open()`（落了范围不重取） | app 三档那条红 | 红：`换了范围没重取列表：日志会停在旧范围上` |

还原：M1/M2/M4/M5/M6/M7 走 `/tmp/g31_{git,widgets}.rs` 的 `cp` + `cmp`；
**M3/M8 那两条不能 `cp`**——快照是在拆 `default_scope()` 之前拍的，`cp` 会把新函数一起抹掉，
所以改成 Edit 就地翻回来（这也是「快照还原」这个习惯本身要留神的地方：快照越旧，还原越危险）。

**仍然没看到过的**

- **真窗口目视照旧欠着**（#25 起就没还上）：10:05 再试，System Events 数到 **0 个窗口** =
  屏幕还锁在登录界面，`screencapture` 拍不到东西。新构建已经起来（`target/debug/gitx`，进程活着，
  CJK 字体走 `assets/fonts/NotoSansSC-Regular.otf`），解锁就能看。要核三件事：
  ① 按钮上「当前分支 ▼」替掉「所有分支 ▼」之后短了一个字，那排按钮的宽度有没有挪；
  ② 展开那一列「所有分支 / 当前分支 / 本地分支」的**顺序**——官方只有两态，第三条是我们加的，
  把它排在两态之后是拍的，没有依据；③ hint「转到Commit的SHA-1」比「跳转到:」长得多，
  260pt 上限够不够、会不会被截断（这条只有眼睛说了算，无头只数得到字在不在）。
- **SourceTree 开仓默认落在哪一档没依据**：这一档的**默认值**是运行时写的（`NSUserDefaults`），
  nib 里那枚按钮归档的是**上次会话**的选中项，不能当默认值用，所以 `Current` 是按「不动现状」取的，
  不是抄来的。（2026-10-24 更正：这一句原来写的是「值在 keyed archive 里读不出来」——
  读得出来，只是读到的不是「默认」。偏好卡「Push 分支：」那枚归档的选中项是 `匹配`，
  这一条已经用在新代码里，见 `PushBehavior::Matching` 那段注释。）
- **`--all` 和「显示远程分支」那枚互不干涉**：`--all` 会把远程跟踪分支独有的提交也带进列表，
  而药丸仍可被藏掉——藏掉之后列表里出现「看不出来源」的提交。这个组合对不对，没人判过。
- **`--all` 会不会把 `refs/stash` 也算进去**：实测不会（git 的 `--all` 走 `refs/` 默认命名空间），
  但**没有判据**钉这件事，换 git 版本要复看。
- **反向排序没实现**，所以官方那条 `已依照路径 (反向) 排序` 现在画不出来，也不该画。

## 贮藏可点开 + 名字剥包装 + 差异区向右看（2026-10-23 下午）

用户三条：「左侧已贮藏，不能点开看内容」「stash 名字去掉前缀 `stash@{0}: On`」
「右侧，文件改动详情不能往右查看内容（目前只有三个点省略）」。
真窗口目视又抓到第四条：**开仓库那一屏右边写着「没有差异」**，而日志第一行
「Uncommitted changes」明明是选中态——选中行和差异区各说各话。

**决定与依据**

- **名字那层包装在核心层剥**（`git::stash_message`）：`%gs` 给的是 `On <分支>: <说明>` /
  `WIP on <分支>: <短号> <主题>`，两截都是 git 的包装，不是用户写的话。剥完
  `Stash.message` 就只剩用户那句；那句空着（`git stash push -m ""`）时侧栏退回
  `stash <短号>`，否则那一行是空的、看不出是哪条。界面层不再自己截字符串。
- **按 oid 取内容，不按 `stash@{n}`**：`Stash` 多带一个 `oid` 字段。列表随时会因为新存/弹出
  而重排，下标只在这份快照里有效；点行到内容落地中间隔着一次后台取数，用下标会拿到别人的贮藏。
- **贮藏的未跟踪文件在第三父提交里**：`git stash show -p -u <oid> -- <path>` git 自己就拒
  （`Too many revisions specified`，实测），所以单文件补丁拆成
  `git diff --no-color -M <oid>^1 <oid> -- <path>` + `git show --format= --no-color <oid>^3 -- <path>`，
  后者在**没有未跟踪文件**的贮藏上必失败（`bad revision`），失败要吞掉而不是报错——
  这一条有专门的判据（M7）。文件清单仍走 `stash show --raw/--numstat -M -u`。
- **向右看这件事拆成两个互不重叠的职责**（这是这批唯一的设计决定，理由见变异表 M1/M2/M4）：
  ① **底色条**铺多宽 = `diff_cols`（`diffmodel::columns`：ASCII 1 格、制表符 4 格、其余 2 格），
  在 `Msg::Patch` 落地时**整份补丁扫一次**存下来，帧里不重扫；② **文字**只按**看得见**的那格
  （`ui.clip_rect().right()`）申请宽度，模式用 `.extend()` 不用 `.truncate()`——申请窄了也不许
  把尾巴收成三个点，省略号等于谎报「没有了」。
  为什么不让 egui 自己量出来的宽度当唯一来源：`show_rows` 每帧只排**可见行**，
  底色条要铺满的是**全补丁最长那行**，两者不是一回事。
  为什么文字格不按内容宽申请（第一版就是这么写的）：那样 `.truncate()` 照样不截，
  两个机制互相顶包，M1 那类改回去的变异全绿。
- **hunk 那两枚按钮钉视口右缘，不钉内容右缘**：行滚得动之后，钉内容右缘等于把按钮送出屏幕，
  「只动这一块」再也点不到。
- **开仓即取工作区差异**（`open()` 末尾一句 `request`）：SourceTree 开仓选中的就是
  `Uncommitted changes` 并显示它的 diff——规则③（它有、我们没有 → 照它），不算我们的增强。

**改了什么**

- `src/models.rs`：`Stash` 加 `oid`。
- `src/git.rs`：`stash_list` 改 `%H|%gd|%gs|%aI` 四字段 + `stash_message()`；新增
  `stash_diff`、`stash_patch`（上面那套父提交拼法）。
- `src/ui/state.rs`：`Sel::Stash(usize)`（顺带 `Debug`）、`Target::Stash { oid, path }`、
  `Msg::StashFiles`、`Gitx::diff_cols`。
- `src/ui/data.rs`：`patch_of` / `patch_label` / `target_path` 三处 match 各加贮藏那一支
  （标签是 `<短号> · 贮藏的全部改动`）。
- `src/ui/app.rs`：`select()` 认提交**也**认贮藏（oid 分别从日志/侧栏那份列表取，后台并发起
  `stash_diff` 与 `commit_diff`）；`poll()` 的 `Msg::Loaded` 保留还数得上的 `Sel::Stash`、
  `Msg::StashFiles` 照 `CommitFiles` 那一支、`Msg::Patch` 顺手算 `diff_cols`；
  `act()` 里弹出/丢弃之前把选中收回工作区；侧栏「已贮藏」那一栏改用说明当行名并**左键可点开**
  （原来只有右键有操作）；`file_list()` 的表头分「这条贮藏改了 N 个文件」/「这个提交改了 N 个文件」；
  `diff_pane()` 换 `ScrollArea::both()` + `content_w` + `.extend()` + 按钮钉视口；
  `open()` 末尾补一次工作区差异。
- `src/ui/diffmodel.rs`：新增 `columns()`。
- `src/geometry.rs`：夹具 `shell` 补 `diff_cols`；新增 `stash_one` / `stash_two` / `laid_out` /
  `scroll_diff_x` 四个帮手与五条判据。

**判据 + 反向变异表**（核心 93 例、app 118 例，`scripts/gates.sh` 四条全绿）

新增：核心 `a_stash_without_a_message_falls_back_to_gits_own_wip_line`、
`a_stash_keeps_the_untracked_files_it_was_stored_with`、
`a_stash_without_untracked_files_still_has_a_patch`；
app `a_stash_row_is_named_by_its_own_message`、`clicking_a_stash_row_opens_its_changes`、
`dropping_a_stash_does_not_leave_the_next_one_selected`、
`dropping_the_selected_stash_does_not_shift_another_one_under_it`、
`opening_a_dirty_repo_shows_its_working_diff_without_a_click`、
`the_column_ruler_counts_double_wide_glyphs`、`a_long_diff_line_can_be_scrolled_to_its_end`。
改写：`a_stash_round_trips_through_the_sidebar` 改锚在**说明**那一行上（它原来找 `stash@{0}`，
名字一改就找不到——那是判据跟着产品改，不是产品坏了）；
`a_real_repo_arrives_through_the_worker_without_blocking_a_frame` 改成等**第二件事**也落地
（`open()` 现在多派了一份差异，快照落地那一刻 busy 可以是 true）。

| # | 变异 | 期望 | 实际 |
|---|---|---|---|
| M1 | 差异行 `.extend()` → `.truncate()` | 长行那条红 | **第一次跑全绿**。两处假绿：① 判据拿 `galley.text()` 比全文，而 epaint 那个函数返回的是**原始整句**，切掉的尾巴照样在字符串里；② 文字格当时按内容宽申请，压根没截。→ 探针换成 `galley.elided` + 数 `rows[].glyphs`，文字格改按视口宽申请。改完再跑：红 `那一行被切掉了尾巴（36/307 个字落笔）` |
| M2 | `Msg::Patch` 里 `diff_cols` 记 0 | 长行那条红 | 红：`底色条只铺到 1050，那一行的尾巴在 4167.9`——**滚那条反而还绿**，因为 `.extend()` 的 galley 自己会把内容撑开。这就是「两个机制互相顶包」的实证，也是把判据拆成 ①落笔字数 ②底色条 ③滚得动 ④按钮还在视口的原因 |
| M3 | 底色条 + 文字模式**一起**退回改前（`content_w = width` + `.truncate()`） | 长行那条红 | 红，报在第 ③ 条：`横向滚 200pt，那一行从 … 只走到 …` |
| M4 | 按钮锚点 `ui.clip_rect().right()` → `rect.max.x` | 长行那条红 | **第一次跑 118 全绿**（既有那两条按钮判据用的是短补丁，内容根本没超过视口）。补第 ④ 条断言后红：`区块按钮滚完落在 4580.7，看得见的格子是 576..1050` |
| M5 | 侧栏行名退回 `stash@{0}: <说明>` | 名字那条红 | 红 ×4（名字、点开、两条丢弃）——四条都靠行名定位，一起红是对的 |
| M6 | `stash_message()` 原样返回 | 核心两条红 | 红 ×2：`WIP 贮藏还带着 git 的包装："WIP on main: b1a0ff1 init"` |
| M7 | `stash_patch` 里 `^3` 取不到就报错 | 无未跟踪文件那条红 | 红：`fatal: ambiguous argument '<oid>^3'` |
| M8 | `select()` 里贮藏那一支拿掉（一律 `commit_diff`） | 点开那条红 | 红：`600 帧还没等到`（`stash@{0}` 是合并提交，`commit_diff` 给不出那三个文件） |
| M9 | `columns()` 把非 ASCII 记 1 格 | 尺子那条 + 长行那条红 | 红 ×2：尺子那条比的是字面量，长行那条报 `底色条只铺到 2783.5` |
| M10 | `act()` 里「丢完回工作区」那道闸短路 | 两条丢弃判据红 | 一条**绿**：只有一条贮藏时列表清空，`Msg::Loaded` 的越界回退抢先把选中改了，真正的挪位（两条、丢顶上那条）没测到。→ 加 `stash_two` 与新判据，再跑红：`丢完还停在旧下标：头一条 挪到了 stash@{0} 顶上，看着像同一条、其实换了人` |
| M11 | `open()` 末尾那句 `request` 拿掉 | 开仓那条红 | 红 ×2（开仓那条等不到差异；异步那条等不到 `patch_label`）——第二处说明它不是重复判据 |

还原全部走 `/tmp/gitx-batch11/` 的 `cp` + `cmp`。**这批踩了一次自己的坑**：跑完 M4 之后
`cp` 快照，把快照**之后**新写的 `scroll_diff_x` 与第 ④ 条断言一起抹掉了（变异确实红了，改动丢了，
得重写）。规矩补进记忆：每跑绿一次就重刷那批文件的快照。

**仍然没看到过的**

- **真窗口里「点贮藏那一行」那一屏没看过**：这台机器上 `osascript` 的 `click at` 被挡
  （`-25211 osascript 不允许辅助访问`），合成点击做不了；`AXRaise` / 设 frontmost 是通的。
  所以目视只覆盖**开仓那一屏**（16:23 那张）：侧栏两行写的是「第二条」「头一条」，没有
  `stash@{0}: On main:`；差异区 `-第一版` / `+宽宽…` 那两行铺到窗口右缘，长行**没有**三个点，
  是被窗口边裁掉的。点开贮藏的内容、横向滚过之后的样子，都只有无头证据。
- **横向滚动条本身长什么样没拍到**：egui 的滚动条会淡出，截图时已经没了；它出现时多高、
  和底部状态栏会不会叠，没人看过。
- 目视这一屏是**种子出来的**：临时 `HOME` 下光写 `gitx_last_repo` 那条 setting 不够——它要同时
  在 `repos` 表里有行才恢复，否则停在「没有仓库」。配方记这儿：
  `HOME=/tmp/gitx-visual/home` + `sqlite3` 塞 `repos`、`settings` 两行，再
  `HOME=… target/debug/gitx`（窗口在 195,86 起，`screencapture -x -R195,86,1050,545 -o`）。
- **超长行只在「已跟踪文件的未暂存差异」上测过**：某条提交的单文件差异、未跟踪文件
  （`^3` 那支）的差异区向右滚，都没测。
- `columns()` 的制表符记 4 格是**估**的（egui 不认制表位），没拿真带 tab 的补丁比过像素；
  估宽偏宽（条子超出内容右缘）没有判据，也不该有。
- `Action::StashApply`（应用到工作区）之后选中态没动、也没判据：apply 不删条目、下标不变，
  所以没加那道闸。真撞上「apply 完列表重排」要复看。
- **开仓即取差异**这一改的代价没量：仓库有几千处未暂存改动时，补丁在后台线程排、主线程只多收
  一次 `Msg::Patch`，但 `columns()` 那一次全量扫描是**每个补丁一次**，没测过 28 875 行那种
  文件落地时要多久。
- `Sel::Stash` 和 tab 条、#37（首个 tab = 分组项目列表 + 贮藏展开）的关系没动：贮藏展开成
  子树那种画法这批改的是**行名**，不是结构。

## 三张卡：添加仓库 / 提交 / 设置（任务 #47，2026-10-23 傍晚）

用户一句：「添加仓库、提交太简陋了，要弹窗，设置都没有」。三件事一起，因为它们共用同一层。

**决定与依据**

- **一张卡只有一个画法**：`widgets::card_rect` 定框，`app::modal_layer` 定内容，卡上长什么
  由 `state::Form` 那一个枚举决定。再加第四张卡不该再画第三个框——所以三张卡共用
  `MODAL_W / MODAL_TITLE_H / MODAL_ROW_H / MODAL_FOOT_H` 那四个常量，设置卡是唯一按整窗尺寸
  量的例外（见下）。
- **设置卡的 855×441 是从 `.nib` 里量出来的**，不是估的，但**当年那个数是错的**（409）：
  `strings` 扫到的那条 `{{436.5, 574}, {855, 409}}` 是**归档里某个视图**，不是窗口内容框。
  2026-10-24 用 `NSNib` 把这份 nib 实例化重问一遍：`title=通用` 那扇窗
  `frame={{293, 311}, {855, 507}}`、`contentRect` = **855×441**（441−32 恰好是 409，
  说明当年是把标题栏又减了一遍）。`PREF_H` 已改 441，判据
  `the_settings_card_is_the_measured_size` 钉的是那一帧的高，不是这个常量。
  顺带量到另外两扇：`自定义操作` 内容区 855×244、第三扇 453×272（我们没这两张卡，先记出处）。
  规则③——它有、我们没有 → 照它，连尺寸一起照。
- **添加仓库那三档（打开已有 / 克隆到本地 / 新建仓库）是我们的入口，不是复刻**（规则②）：
  SourceTree 的 File ▸ Open 走的是系统文件选择器，这台机器上没法拿它的像素当基准
  （`docs/reference/` 那两张只有主窗口和书签列表）。所以卡上的字是我们自己的话，
  结构照它「一个路径 + 一个主按钮」的最小形。
- **提交卡是第二条入口，内联那块留着**（规则②）：SourceTree 的提交框在主窗口左下角，
  **没有**弹窗提交。用户要弹窗，那就加一张卡，但**不许**把已经照它抄过的那块内联框挪走或
  删掉；两处发的是同一个 `Action::Commit`，卡上的文案（提交摘要 / 扩展描述 / 修正上一条提交 /
  提交之后立刻推送）逐字取自内联那块，也就是取自它的 `.nib`。
- **关卡的决定只有一处**：`act()` / `land()` 的返回值就是「这个动作被接住了没」。卡开着、
  字留着，只在**没接住**的时候发生（填的路径不是仓库、手上根本还没仓库）；打错一个字母不该
  从头再打一遍。确认卡走 `from_confirm` 绕过 `land`，否则它自己会再触发一遍关卡、把自己
  又问一遍（M3 就是这条）。
- **刚 `git init`、一条提交还没有的仓库：日志是一份空的日志，不是错误**（`log_commits` 里
  那一支）。不这么写，新建仓库那一档落地后整屏是红的错误条——用户刚点完「创建」，看到的
  却是失败。反向的一起钉：真错误（路径压根不是仓库）必须还是错误，否则这一支等于把
  `log_commits` 所有 `Err` 咽下去。
- **卡上的开关「改一格算一格」**：偏好卡的三枚开关当场生效、当场 `p.save(conn)`，
  所以那张卡**没有**「取消」——说「取消」会谎报还没算的那部分能回退。

**改了什么**

- `src/ui/state.rs`：`Form` 加 `AddRepo { tab, dest }` / `Commit { .. }` / `Settings { .. }`
  三支与 `AddTab`；`Modal::add_repo()` / `commit(branch, staged)` / `settings(prefs, repos, db)`
  三个构造子；`primary()` / `detail()` / `rows()` 按档给文案。
- `src/ui/widgets.rs`：`card_rect`（设置卡走 `PREF_W`/`PREF_H` + 40pt 屏边距，其余走 `MODAL_W`）、
  `primary_action`（把卡上的字变成一个动作，名字空着返回 `None`）、`hunk_button` 复用成卡上的
  横排按钮。
- `src/ui/app.rs`：`modal_layer` 一张卡画完（标题、档签、字段、开关、底部两枚右对齐）；
  `add_repo()` 拒收不关卡；`apply_prefs()` 回写 + 落库；`open_add_card` 等入口从工具条接上。
- `src/ui/theme.rs`：`PREF_W` / `PREF_H`（上面那两个量出来的数）。
- `src/git.rs`：`log_commits` 认 unborn 分支那一条。
- `src/geometry.rs`：帮手 `card_back` / `settled_frame` / `raw_timed` / `card_now` /
  `locate_card` / `click_card` / `click_card_button` / `type_into_card` / `card_has` /
  `open_add_card` / `temp_db`，与下面那批判据。

**判据 + 反向变异表**（核心 94 例、app 134 例，`scripts/gates.sh` 四条全绿）

新增：核心 `a_repo_without_any_commits_has_an_empty_log`；
app `the_commit_button_in_the_toolbar_opens_a_commit_card`、
`a_commit_typed_in_the_card_lands_in_the_repo`、`an_empty_message_keeps_the_commit_card_open`、
`the_commit_card_amend_goes_through_the_confirm_card`、`ticking_push_after_commit_lands_on_the_remote`、
`the_add_repo_card_tabs_change_what_the_field_asks_for`、
`a_path_that_is_not_a_repo_is_refused_without_touching_the_list`、
`an_added_repo_is_written_to_the_database`、
`the_new_repo_tab_initializes_a_directory_and_opens_it`、`a_clone_needs_both_fields`、
`an_empty_bookmark_list_still_offers_a_way_in`、`the_settings_card_is_the_measured_size`、
`the_settings_card_fits_the_window_floor`、`the_settings_card_lists_the_wired_switches`、
`the_about_page_states_two_facts`、`the_pill_switch_takes_the_pills_off_and_persists`、
`the_topo_switch_resorts_and_persists`、`the_restore_last_switch_is_read_back_at_startup`、
`the_settings_card_closes_without_firing_anything`、`the_modal_card_fades_in_and_settles_at_full_opacity`。

**踩了一次自己的坑（这次是判据，不是产品）**：`the_settings_card_is_the_measured_size` 头一次跑
红在 `left: (426.0, 45.59)`——量到的是卡里那块扩展描述，不是卡底。挖下去是 egui 的 `Area`
**淡入**（`area.rs:152 fade_in: true`，`Style::animation_time = 0.2`）把整层颜色乘了一遍，
而无头帧的 `RawInput::time` 一直是 `None`，墙钟在两帧之间几乎不走，于是**淡入永远停在中间**：
白卡底在测试里是 60/255，标题从白色文字里穿出来。两条路：关掉淡入（改产品，为了测试）或
量**淡完那一格**。选后者——`settled_frame` 让时钟走 0.4s，判据量的是那一帧。并且配一条
双向判据 `the_modal_card_fades_in_and_settles_at_full_opacity`：头一帧不许是全白（淡入没在跑），
淡完那格必须是全白（淡入走不完，比如 Area 的 `Id` 不稳）。只加一头，另一头的坏法就漏了。

| # | 变异（把产品改坏） | 红了什么 |
|---|---|---|
| M1 | `add_repo` 拒绝分支 `return false` → `true` | `a_path_that_is_not_a_repo_is_refused_without_touching_the_list`：「拒了一条就把卡关了，打错的那串路径跟着没了」 |
| M2 | 提交关卡 `!raw.is_empty()` → `!msg.is_empty()` | `an_empty_message_keeps_the_commit_card_open`：「只填了描述也把卡关了」——摘要才是必填那格 |
| M3 | 确认卡也走 `land`（`from_confirm` → `false`） | `the_commit_card_amend_goes_through_the_confirm_card` + 既有两条（`ticking_amend_rewrites_head…`、`aborting_a_merge_asks…`）：卡关掉又立刻重开，死循环 |
| M4 | `PREF_H` 409 → 380 | `the_settings_card_is_the_measured_size` 给 `left: (855.0, 380.0)`——**尺子是那一帧，不是那个常量**，常量自己改红了才算这条判据有效 |
| M5 | 拿掉 `log_commits` 认 unborn 那一支 | 核心 `a_repo_without_any_commits_has_an_empty_log` + `the_new_repo_tab_initializes_a_directory_and_opens_it`（note 里露出 git 128） |
| M6 | `p.save(self.conn.as_ref())` → `p.save(None)` | 三条「落库」判据红；**纯行为那几条仍绿**——开关当场有效和下次启动还有效是两件事，只测前者就是假绿 |
| M7 | Clone 档去掉 `!dest.is_empty()` | `a_clone_needs_both_fields` |
| M8 | `if *push_after` → `if false` | `ticking_push_after_commit_lands_on_the_remote` |
| M9 | 模态 Area 加 `.fade_in(false)` | `the_modal_card_fades_in_and_settles_at_full_opacity`（头一帧就全白） |
| M10 | `Modal::add_repo()` 初始档 Open → Clone | `the_add_repo_card_tabs_change_what_the_field_asks_for` + `an_empty_bookmark_list_still_offers_a_way_in` |
| M11 | 拿掉 `db::add_repo(c, &root, &name)` | `an_added_repo_is_written_to_the_database`——列表当场多了那条，重启就没了 |

还原 11 处变异全走 `/tmp/gitx-mut-47/` 的 `cp` + `cmp`（每跑绿一次重刷快照，上一批那条规矩在这儿
救了一次），`git diff --stat` 对过没有残留。

**仍然没看到过的**

- **真窗口这三张卡都拍到了**（欠的目视还上了），而且是量的不是看的：截图 @2x，
  设置卡白底连读 854×408 pt 落在 (98, 70.5)、添加卡 379×115 @(335.5, 215)、
  提交卡 379×245 @(335.5, 150)。窗口 1050×545，居中算下来分别是 (97.5, 68) / (335, 215) /
  (335, 150)——每一头差的那 1px 是抗锯齿边。**卡溢出窗口、字号、CJK 都在这三张里对过。**
- **但卡是探针叫出来的，不是点出来的**：临时在 `main.rs` 里读 `GITX_SHOT=add|commit|settings`
  启动即挂卡，跑完已 `cp /tmp/gitx-mut-47/main.rs src/main.rs` + `cmp` 还原，`grep GITX_SHOT`
  空。所以**从工具条点 ⊕ / ⊙ 真的把卡叫出来**那一条路径在真窗口里仍然没走过——合成点击被
  `-25211 osascript 不允许辅助访问` 挡着，这条目视这台机器上做不了。
- **卡上的交互一次都没在真窗口做过**：打字、切档、点开关、按 Enter、按 Esc、点遮罩、
  提交卡真提交一条下去。全部只有无头证据。淡入这件事**肉眼没看过**——无头帧里它是停格的，
  真窗口里 0.2s 一闪，没人确认它好看、也没人确认它不闪。
- **克隆 / 新建两档没拍过**（截图停在默认的「打开已有」），「关于」那一档也没拍过（停在「通用」）。
- 提交卡那个摘要+描述框在真窗口里**看着比预期高**：框从标题下面一直留到复选框上面，
  中间一大片空白（卡 245pt，框占了 80pt 而里面只有两行字）。SourceTree 那块内联框的
  摘要/描述比例没量过，所以没动它——**这是一处已知没定的几何**，别当它是对的。
- 探针实例被 `AXRaise` 之后崩过一次：`NSRangeException: Cannot remove an observer
  <_NSTouchBarFinderObservation …> for the key path "nextResponder" from <WinitView …>`。
  没诊断、没判据，也不知道是探针特有（启动即挂卡）还是产品特有——第二个实例同样 raise 过、
  没崩。真窗口用户手点标题栏/切焦点会不会撞上，**没看过**。
- **添加仓库卡没有基准可比**：SourceTree 那一屏是系统文件选择器，我们这三档是自己设计的，
  「像不像它」这个问题在这张卡上没有答案，只有「我们有没有把它做顺手」。
- 提交卡与内联提交框是**两份状态**（卡上那份是 `Form::Commit { desc, amend, push_after }`，
  内联那格是 `State::commit_msg`）：卡上打的字不会出现在内联那格里，反向也是。
  这是设计上的选择还是漏的，没定——判据只测了「卡上打字 → 落到仓库」那一条。

## 推送 / 拉取卡 + 整份差异按文件折叠（任务 #48，2026-10-23 深夜）

用户一句：「继续，完善ui功能」。这一批把工具栏里剩的两枚（拉取、推送）接上真 git，
并把路线图里「大补丁在 `diff --git` 边界折叠」做完。

**决定与依据**

- **两张卡照 `.nib` 量，不套 `MODAL_ROW_H` 那套流水布局**：`GitPushSheet` 内容区 650×271、
  `GitPullSheet` 614×299，每一格坐标都在 `theme.rs` 的 `PUSH_*` / `PULL_*` 里，
  `Modal::rows()` 对这两支返回 0。规则③：它有、我们没有 → 连尺寸一起照。
- **推送按行发 refspec**：卡上「远程分支」那一格说的算，所以走
  `git push <远端> <本地>:<远端名>` 而不是裸分支名（裸名字走 `push.default`，那是另一件事）。
  远端名为空当场拒——`main:` 这种半截 refspec 的报错说的是左边、坏的是右边。多行依次推、
  失败即停、不回滚（和 git 自己推多条一致）。
- **拉取先切再拉**：`git pull <远端> <分支>` 只拉得进当前分支，目标不是当前那条时先
  `checkout`，切失败就不拉——在错的分支上拉一次，冲突落到工作区里比报一句错难收拾。
  `--rebase` 与 `--no-rebase` 都明写，不让策略落回仓库配置。
- **「刷新」是正文右上那枚，不是卡脚主按钮**：nib 里 `PULL_REFRESH` 在 (513, 72) 87×32。
  它跑 `git fetch <远端>`，卡留着不关；新快照落地后「远程分支」那一列自己换。
  判据因此走 `click_card`（整张卡里按文字找），`click_card_button` 只看脚部，找不到它。
- **折叠只在 `diff --git` 边界折**：文件头带本身不折，点一下在折/展之间翻；可见下标
  `diff_rows` 在**点击时**重建（O(n) 只发生在点击那一刻），不是每帧过滤。
  换补丁/换仓库/换选中都清空折叠状态——这一份的第 3 个文件不是上一份的第 3 个。
- **文件头只在整份差异里画**（`path.is_none()`）：单文件差异顶上那条标题已经写着路径，
  再画一遍只是把同一句话说两次。

**改了什么**

- `src/ui/diffmodel.rs`：`Kind::File`、`DiffLine.file/hunk`、`layout(patch, show_files)`、
  `diff_path` / `unquote`（引号路径按最后一个 `" "` 从右切）。
- `src/ui/state.rs`：`diff_rows` / `collapsed`；`PushRef` / `PushRow`、`PushBehavior`
  （五档只决定初勾，不进 git 实参）、`Form::Push` / `Form::Pull`、`Modal::push` /
  `Modal::pull`、`pull_modal()`。
- `src/ui/widgets.rs`：`push_card` / `pull_card` / `cell` / `cell_left` / `cell_popup`、
  `primary_action` 的 Push/Pull 两支、`primary_closes`。
- `src/ui/app.rs`：`request` 按 target 决定画不画文件头；`diff_pane` 文件头带 + 折叠；
  `Action::PushRefs` / `Action::PullFrom`；`fetch_remote` 那条（刷新不关卡）。
- `src/ui/theme.rs`：`PUSH_*` / `PULL_*` 两组实测常量。
- `src/git.rs`：`push_ref`、`push_all_tags`、`remote_branches`、`pull(..., PullOpts)`
  （明写 rebase 策略）。
- `src/geometry.rs`：`pull_cell` / `pull_menu_shows` / `pull_pick` / `push_row_box` /
  `push_remote_cell` / `menu_shows` / `retype_at_card`，与下面那批判据。

**判据 + 反向变异表**（核心 99 例、app 148 例，`scripts/gates.sh` 四条全绿）

新增核心：`a_push_row_lands_under_the_name_on_the_card`、
`a_push_row_with_a_blank_target_never_reaches_git`、
`remote_branches_are_listed_per_remote_without_the_prefix`、
`push_all_tags_sends_the_tags_and_only_the_tags`、`pull_options_change_what_lands_in_the_history`。

新增 app：`a_multi_file_diff_folds_one_file_at_a_time`、
`opening_a_dirty_repo_shows_its_working_diff_without_a_click`、
`the_push_card_is_the_measured_size`、
`the_push_card_shows_every_local_branch_and_where_it_would_land`、
`the_push_behavior_pref_picks_which_rows_start_checked`、
`the_push_card_only_fires_when_something_is_picked`、`the_pull_card_is_the_measured_size`、
`the_pull_card_shows_the_three_rows_and_the_options`、
`the_pull_card_lists_only_the_selected_remote`、
`the_push_preferences_change_what_the_push_card_starts_with`、
`the_push_card_sends_only_the_checked_rows_under_the_names_it_shows`、
`pulling_lands_on_the_local_branch_the_card_names`、
`pulling_a_diverged_remote_merges_or_rebases_as_the_switch_says`、
`the_refresh_button_brings_the_remote_branch_column_up_to_date`。

| # | 变异（把产品改坏） | 红了什么 |
|---|---|---|
| M1 | `request` 里 `show_files` 恒 `false` | 折叠判据 600 帧等不到两条文件头 |
| M2 | 重建可见行时 `filter(... || true)` | 「折叠之后可见行没变少」 |
| M3 | 文件头 `Sense::click()` → `Sense::hover()` | 「点了 a.txt 的文件头但它没被折叠」 |
| M4 | 推送表去掉 `.filter(|r| r.picked)` | 「只推勾上的那几行」——没勾的也进了实参 |
| M5 | 拉取目标分支那一支 `checkout` → `Ok(())` | 「没切到要拉进的那条上」 |
| M6 | 刷新后不跑 `FetchRemote` | 刷新判据超时，`from-afar` 一直不出现 |
| M7 | `PUSH_W` 650 → 640 | 推送卡实测尺寸判据（尺子是那一帧） |
| M8 | `PULL_W` 614 → 600 | 拉取卡实测尺寸判据 |
| M9 | `PushBehavior::Matching` 的 `picks` 由 `true` → `false` | 「推送偏好决定哪些行预先勾上」 |
| M10 | `pull` 里 `opts.rebase && false` | 「分岔时按开关合并或变基」——历史形状不对 |
| M11 | `open()` 里删掉第一格差异请求 | 「开仓就有工作区差异」600 帧等不到 |
| M12 | 拉取卡分支列去掉 `starts_with(prefix)` | 「只列选中远端的分支」——别的远端混进来 |
| M13 | 主按钮的「一条都没勾」闸拿掉 | 「没勾任何行时按不动」 |
| M14 | 推送表只遍历当前分支 | 「列出每一条本地分支」 |
| M15 | 拉取卡第二行标签改成「远程分支：」 | 「三行标签与选项」 |
| M16 | `Form::Push` 的 `tags` 恒 `false` | 「偏好改变推送卡初值」 |

16 处变异全部先红后 `cp` + `cmp` 还原（快照 `/tmp/gitx-mut-48/`），`git status` 对过没有残留。

**仍然没看到过的**

- **真窗口里这两张卡没拍过**：没有 `GITX_SHOT` 探针这张截图，坐标只有 nib 数值与无头判据；
  卡在 1050×545 窗口里居中后底边还剩多少、CJK 字号对不对，肉眼看的是「没截」。
- **推送 / 拉取只连过本地裸仓库**：没有网络远端、没有凭据 / SSH / 加速节点那条路；
  推送失败时前半截已经落地、`note` 怎么措辞，只有核心层判据，没有真远端。
- **折叠只在无头帧里点过**：真窗口里大补丁折开的手感、折叠后横向滚动宽度、
  折到视口外的文件再滚回去位置对不对，都没看过。
- **改名 / 二进制 / 引号路径的文件头**：`diff_path` 只被 `layout` 层的判据读过一眼，
  折叠判据用的是 `a.txt` / `b.txt` 这种最普通的路径；`diff --git a/x b/x` 里带空格的那条没走。
- **推送卡表格**：行高 16pt、表头 28pt 是 nib 的数，但 egui 的行内控件不按 16pt 自动收，
  真窗口里可能挤；无头判据只量了卡框和文字在不在，没量行距。

## 差异区搜索（任务 #49，2026-10-23 深夜）

**决定与依据**

- **搜索是过滤，不是高亮**：补丁动辄上千行，高亮还得一路滚着找；过滤直接把不相关的行拿掉。
- **命中行留下，文件头与 hunk 头也留下**：几段命中混排时没有文件头就不知道属于哪个文件；
  没有 hunk 头就丢了行号范围（`块 2： 行 12-18`）。这是「上下文」不是「碰巧没被筛掉」。
- **只在状态变的那一下重建**：空串走便宜分支（只按折叠筛），有词才扫整份补丁；
  敲字看 `changed()`，帧里不重扫。2.8 万行的补丁每帧过一遍会直接掉帧。
- **换补丁 / 换仓库清空搜索**：上一份补丁的词对着这一份多半一个字都不命中，
  留着就是打开新差异先看到「没有匹配的行」。
- **命中全无时说一句「没有匹配的行」**：整块留白会让人以为差异没取回来。
- **输入框显式 `id_salt("diff-search")`**：灰带里只有这一格，焦点不该跟着筛剩多少行漂。
- 参考图里这条搜索**没有量到宽高**（灰带边界在图上不清楚），所以 `DIFF_SEARCH_W/H`
  是估的——按参考契约，估出来的值要和「没看到过的」一起登记。

**改了什么**

- `src/ui/state.rs`：`diff_search` 字段。
- `src/ui/app.rs`：灰带右端的输入框、`rebuild_diff_rows()`（折叠也改走它）、
  `Msg::Patch` 清空搜索并重建、`open()` 清空、空结果文案。
- `src/ui/theme.rs`：`DIFF_SEARCH_W/H` 两个估值常量。
- `src/geometry.rs`：新判据；`shell` 夹具补 `diff_search`。

**判据 + 反向变异表**（核心 99 例、app 149 例，`scripts/gates.sh` 四条全绿）

- `the_diff_search_keeps_only_matching_lines_with_their_context`：敲「第三行」→
  命中行、a.txt 的文件头、hunk 头留下，b.txt 整份消失；按 End 再三下退格清空，
  两份文件都回来（`locate` 点中的是灰带上先画的那格框，不是差异里同名的行）。
- M1：`rebuild_diff_rows` 里 `let q = String::new()`（搜索失效）→ 「没命中的行还画着」。
  先红后 `cp` + `cmp` 还原（快照 `/tmp/gitx-mut-48/`）。

**仍然没看到过的**

- **搜索框的宽高是估的**：170×20 没在真窗口里拍过，中文提示语会不会被截、
  深色模式下边框看不看得清，都没看。
- **只测过一条子串**：大小写混合（代码里 `to_lowercase`）、多词、正则、命中跳转都没测；
  egui 版也没有「上一个 / 下一个」。
- **真实大补丁的重建开销**：只在无头小补丁上跑过，2.8 万行的补丁敲字时会不会卡，
  没有量过；重建是 O(n)，只在敲字那帧发生。
- **搜索与折叠叠在一起**：判据没覆盖「先折一个文件、再搜另一个文件里的词」的组合。

## 多远端：默认远端 + 推送行为五档，与两条假绿的更正（任务 #43/#49/#50/#51/#52，2026-10-24）

上面 #48 那批把两张卡的**形**照 nib 摆好了；这批补的是**多远端**这条没人走过的路——一本仓库有两个
远端时默认走哪条、五档「推送行为」到底改什么、以及最要紧的：**回头查上面那些绿是不是真的**。
查出来两条假绿，都记在下面。

**决定与依据**

- **五档的文案与顺序逐字取自 nib，今天实测复核过**：`PreferencesWindow.nib` 里那枚弹出按钮打出来是
  `MENU items: 无 | 匹配 | 上流 | 简单 | 当前`，归档的选中项是 `匹配` → SourceTree 的默认那档就是
  `matching`（`PushBehavior::Matching` 的注释就钉这条）。
- **但它只改预选，不进 git 的实参**：卡上的推送一律带明确 refspec，而带 refspec 的 `git push`
  **完全无视** `push.default`。把这一档当成 `-c push.default=…` 传下去等于传了个不起作用的参数，
  还要谎称「选了就有区别」。所以五档唯一的可见后果就是表格里预先勾哪几条。
- **默认远端那条链不猜 `origin`**：`当前分支的上游 > 偏好的默认远端 > 空`。落到「空」时那一格留空、
  主按钮按不动——一本仓库可以先推 fork 再推 upstream，替他选一条就是替他做错的那条。
- **`allow_force` 是闸，不是装饰**：关着的时候「强制推送」那枚勾**摆都不摆**。摆一枚点了没用的勾
  等于没有闸。
- **不勾变基就明写 `--no-rebase`**（这批的**真 bug**，是被判据跑出来的，不是读代码读出来的）：
  git ≥ 2.34 遇到分岔又没给合并策略时直接拒跑（「need to specify how to reconcile divergent branches」），
  此时策略落到了**仓库配置**头上，而卡上那枚勾才是用户此刻的选择。只在传 `--rebase` 的那一支写 flag，
  另一支什么都不传，等于「不勾」这件事没有说出口。

**判据加固：两条假绿被抓出来**

1. **夹具不够 → 「A 不压 B 但 B 让位」**（方法论第 2 条）：`with_remotes()` 原本只有 `main`（上游同名）
   和 `feature/x`（没上游），上流/简单/当前三档算出来的勾选**一模一样**。把 `Simple` 写成
   `upstream.is_some()` 改坏后**全绿**。补两条分支——`tricky`（上游 `origin/other`，不同名）与
   `docs`（上游同名但**不是**当前那条）——五档的勾选结果两两不同，那条变异当场红：
   `left: [true,false,true,true] right: [true,false,false,true]`。
2. **尺子就是被测对象**（方法论第 1 条的变体）：那一档有**两条独立的路**——`label()` 走 `index()`，
   落库走 `key()`。判据原来只读 `app.push_behavior` 和 `Prefs::load`，所以把 `from_index` 的
   `3`/`_` 两项互换（界面显示成「简单」、存的还是「current」）**不会红**。补两条：那一格要把选中的
   那一档**念回来**（`card_has("当前 ▼")`），以及重开的推送卡上勾的必须是 `feature/x` 那一行。
3. 另外两条是顺手补的洞：表格首列原来只比第 0/1 行，把方框绑到 `row.track` 才红出「点一行带走后面几行」
   → 判据补 `rows[2..]` 全勾；「跟踪？」那一列的初值（= 这条还没有上游）此前没人问。
4. **遮罩会让人以为产品坏了**：模态卡的遮罩铺满全屏，点在任何控件**命中矩形之外**都算「点外面」→ 关卡。
   `MenuButton` 的命中矩形只有文字那么宽，按单元格**正中**恰好落在遮罩上，判据于是报「卡没了」。
   `pull_cell` 因此贴左走 28pt，注释写明「x 只敢往左偏」。这不是产品的错，是判据的坐标不老实。

**改了什么**

- `src/git.rs`：`pull` 的另一支补 `--no-rebase`（见上面那条真 bug）。
- `src/ui/state.rs`：`Prefs` 多出默认远端 / 五档 / 两枚勾（一共七条，`KEY_*` 各一格）；
  `PushBehavior` 的 `label`/`index`/`from_index` 与 `key`/`from_key` **两套**反向函数分开写；
  `picks()` 那一段排版在变异还原时撞坏过一行，顺手修回。
- `src/ui/app.rs`：`push_modal` 的远端初值走那条三段链；`Action::PullFrom` 先切再拉；
  「刷新」落 `Action::FetchRemote`（卡不关）。
- `src/ui/widgets.rs`：推送表四列的初值（远端侧预填同名、「跟踪？」= 没有上游）、主按钮的两道闸。
- `src/geometry.rs`：`with_remotes()` 从两条分支扩到四条、`pull_cell`/`pull_pick`/`pull_menu_shows`、
  上面那几条加固断言。

**判据 + 反向变异表**（核心 99 例、app 149 例，`scripts/gates.sh` 四条全绿）

本批的账一次列全。和 #48 那张表有重叠，重叠处如实标出：M2 是那边 M9 的另一半（那边改 `false`、
这边改成 `is_current`），M10=那边 M13、M11=那边 M4、M13=那边 M5、M16=那边 M6、M17=那边 M12、
M20=那边 M15；那边的 M7/M8 改的是两张卡的**宽**，这边 M18/M19 改的是**高**——同一条判据的两个方向。

| # | 变异（把产品改坏） | 红了什么 |
|---|---|---|
| M1 | `picks()` 的 `Nothing => true` | 五档预选那条，`[true,true,…]` 对不上 `[false,false,…]` |
| M2 | `Matching => branch.is_current` | 同一条，Matching 那行 |
| M3 | `Upstream => upstream.is_none()` | 同一条，Upstream 那行 |
| M4 | `Simple => true` | 同一条，Simple 那行 |
| M5 | `Current => true` | 同一条 + 「偏好选了『当前』，重开的卡却勾了别的几条」 |
| M6 | `Simple => upstream.is_some()` | **第一次全绿**；夹具补 `tricky`/`docs` 之后才红（假绿 1） |
| M7 | `from_index` 的 `3`/`_` 互换 | **第一次全绿**；补「当前 ▼」念回来之后才红（假绿 2） |
| M8 | `key()` 的 `Current => "matching"` | 「五条没一起落库」 |
| M9 | `push_modal` 的远端初值硬编码 `origin` | 「默认远端没成为卡上那一格的初值」 |
| M10 | 主按钮的「远端空着 / 一条都没勾」那道闸拿掉 | `the_push_card_only_fires_when_something_is_picked` |
| M11 | `primary_action` 去掉 `.filter(\|r\| r.picked)` | 「只推勾上的那几行」——没勾的也进了实参 |
| M12 | refspec 右侧改用本地名（无视那一格填的） | 端到端那条：改了远端名的分支推成了旧名 |
| M13 | `PullFrom` 跳过 `checkout` | 「没切到要拉进的那条上」 |
| M14 | `pull` 里无视 `opts.rebase` | 分岔那条：勾了变基却走了合并（父提交 2 ≠ 1） |
| M15 | `--no-rebase` 去掉 | 分岔那条的另一半：git 直接拒跑，note 报错 |
| M16 | 「刷新」后不跑 `FetchRemote` | 刷新那条 600 帧等不到 `from-afar` |
| M17 | 远程分支列去掉 `starts_with(prefix)` | 「只列选中远端的分支」 |
| M18 | `PUSH_H` 271 → 270 | `the_push_card_is_the_measured_size` |
| M19 | `PULL_H` 299 → 298 | `the_pull_card_is_the_measured_size` |
| M20 | 删掉「拉取到本地分支：」那行标签 | `the_pull_card_shows_the_three_rows_and_the_options` |
| M21 | 表格首列绑 `row.track` 而不是 `row.picked` | 「点一行却把后面的行一起带走了」+ 端到端推错了分支 |
| M22 | 忽略 `allow_force`（那枚勾恒摆） | 「闸关了，卡上那枚勾还摆着」两条 |

M18–M22 这五条一起改坏、跑一次全量，按红灯归属归因（7 红 = 5 条变异，M21/M22 各多杀一条）；
还原全部走 Edit 就地翻回——本仓库有过 `perl -pi` 误伤同一字符串第二处的记录，这批不再用批量替换。

**同一批把「nib 读不到数值」这条旧结论推翻了**（三处更正已写回文档）：`NSNib` 实例化后 frame、
菜单项、`rowHeight`、列宽全都读得出来。连带查出设置卡那个 409 是把 441 又减了一遍标题栏。
今天顺手把上面 #48 引用的那几个数**重新读了一遍**，全部对上：`GitPushSheet` 内容区 650×271
（窗口 frame 650×303）、`GitPullSheet` 614×299（frame 614×331）、三行标签
「从仓库拉取：/要拉取的远程分支：/拉取到本地分支：」、`刷新` 那枚 `{{513,195},{87,32}}`
（换算成距顶 = (513, 72)，正是代码里 `PULL_REFRESH` 那个矩形）、「选项」框里四行开关文案逐字一致。
另外量到两扇我们还没有的窗：偏好「自定义操作」页内容区 855×244、`GitRepoWindow` 内容区 1050×497
（后者比我们目前用的 1050×545 矮 48pt，是不是把工具栏那 52pt 也算进了内容区，#40 目视时一并核）。

**仍然没看到过的**

- **真窗口目视照旧欠着**：合成点击进真窗口这条路被 `-25211（osascript 不允许辅助访问）` 挡着，
  推送/拉取两张卡**一次都没在真窗口打开过**。卡上四列表格（行高 16pt）在 egui 里会不会挤、
  弹出按钮在 650pt 宽的卡上文字会不会截，只有 nib 的数和无头判据说话。
- **只连过本地裸仓库**：没有网络远端、没有凭据/SSH 那条路；推多条时前半截已落地、后半截失败的
  `note` 怎么措辞，只有核心层判据。
- **五档与 refspec 的组合**：判据钉的是「预选哪几条」，没钉「预选错了会不会推漏」——
  用户手工把某一行勾掉之后卡发出去的实参只由表格决定，这条路径靠的是 M11/M12 那两次红，
  不是靠一条独立判据。
- **上游指向一条已经被 `git remote remove` 掉的远端**：一个分支在 git 里只有**一个**上游
  （`branch.<name>.remote` 就一格），所以那种远端不是「候选之一」，而是**已经不存在的名字**。
  按代码读下来的行为是那一格照样念它、URL 那格随之为空（`remotes` 里找不到），
  但这条组合没有判据，真窗口里长什么样也没看过。

---

## 界面重设计：首页与弹窗风格定稿（2026-10-24）

这批**只写文档，一行代码没动**。用户 2026-10-24 拍板换锚：不再复刻 SourceTree，改成
**现代化清爽风**（向 GitHub Desktop / Fork 靠），力度是「皮肤 + 结构」——布局一起动。
于是 `docs/DESIGN.md` 成为 UI 的唯一事实来源；`geometry.rs` 里 68 处「参考图实测」的锚
以后一律改挂到它的章节号上（**还没改**，见下面「仍然没看到过的」）。

**决定与依据**

- **换锚不是「美化一下」**：参考图常量（`TOOLBAR_H 52`、`SIDEBAR_W 141`、`SIDE_CAT_H 47`……）
  是「照抄」时代的产物。继续拿它们当判据锚，等于把旧皮肤钉死在测试里，新设计永远落不了地。
- **首页两条顶带并成一条 46pt**（`TOOLBAR_H 52 + TAB_H 24` → `HEADER_H 46`）：仓库 tab 从
  独占一行降级成胶囊开关，和仓库名、动作组同排；窗口少一条带，日志区多 30pt。
- **选中态从实心蓝底白字改成浅蓝 tint + 左缘 2pt 主色条**：实心蓝在暗色下对比过强，
  也压掉了图序列的车道色；tint 让「选中」和「ref 药丸」同族，两个列表（日志/侧栏）一致。
- **文件状态色块 14pt 直角 → 12pt 圆角 3**：14pt 是参考图量的，但直角方块在 13px 行高里
  比文字还重；12pt / 圆角 3 与勾选框同族。
- **弹窗去掉两条格线**：现实现标题带下沿 + footer 上沿各一条 `RULE`，卡被切成三块，
  像带格线的表。改成语义间距（16 / 12）分组 + 真阴影，卡才像浮起来的卡。
- **确认卡单独一档**（380 宽、红圆标、红色主按钮）：此前确认卡和表单卡同尺寸同色，
  「不可逆」只能从文案里读出来。
- **toast 只换皮肤**：位置（状态栏上方 8pt）与 8 秒生命周期**不动**——它们有判据钉着，
  换皮不动行为。
- **空态补成规范**：没有仓库时 LOG 区居中欢迎页 + 两枚入口。此前空态没有设计，
  各面板各写一句「还没有仓库」。

**改了什么**

- 新增 `docs/DESIGN.md`：设计原则、首页布局表、Token（亮/暗两套色板、字级、间距、圆角、
  图序列车道色、状态色块）、组件规范（按钮 / 输入 / 开关 / 分段 / 药丸 / 选中态）、
  弹窗体系（遮罩、三档宽度、表单卡、确认卡、右键菜单、toast）、§6「旧常量 → 新 token」迁移对照表。
- 新增 `docs/design-preview.html`：单文件静态预览，首页主窗口 + 空态 + 弹窗四件套 + Token 表，
  右上角切亮/暗；不依赖网络与构建，双击即看。
- 本台账 §契约 补一条 2026-10-24 换锚声明（参考图降级为历史素材，不再作设计锚）；
  台账加一行，本节记全文。

**判据 + 反向变异表**

这批**没有判据、没有变异**——因为代码一个字没改。下一批必须补上的：

- `geometry.rs` 68 处「参考图」「参考图实测」改挂 `DESIGN.md §N`；新 spec 里**没在真窗口量过**
  的值（暗色整套、阴影、字重）必须在注释里明写「取自 DESIGN.md，未实测」，不许冒充量出来的数。
- 按旧值拍阈值的判据要重拍，否则改完照样绿（**假绿**）：
  `bands_match_the_measured_reference`（52+24 → 46）、侧栏 141 → 188、`ROW_H 26` → 28、
  `STATUS_H 26` → 24、`MIN_W 870` → 860。
- 选中态判据若写的是「整行铺 `SELECTED`」，要改判 tint 底 + 左缘 2pt 条，否则新皮肤下
  那条判据永远杀不死实现。
- `COMMIT_H 96` 这个预算是按 Inter 行盒算的；字级从 12.5/11.5 换成 13/12/11 之后要重新核。

**仍然没看到过的**

- **新皮肤一次都没有在真窗口里目视过**：`design-preview.html` 只是 HTML 示意，
  egui 的圆角、阴影、字体渲染和浏览器不是一回事；卡片阴影尤其——egui 没有 CSS 那种阴影，
  只有多层描边近似（现实现那三圈就是这么来的）。
- **暗色根本没实现**：§3.1 给了暗色值，但 `theme.rs` 现在只有一套亮色常量，也没接过
  `Theme::Dark`。暗色那半张表目前纯属设计稿。
- **`MIN_W 860` 是算出来的，不是量出来的**：870 是旧布局无头扫出来的；合并顶带后可用宽度变了，
  这个数要重扫一遍才算数。
- **溢出菜单 `»` 只是提案**：顶栏收窄时动作按钮怎么收、仓库 tab 怎么截断，文档写了设计意图，
  没有任何实现，也没有判据。

