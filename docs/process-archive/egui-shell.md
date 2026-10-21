# 台账归档：egui 壳的批次全文（2026-10-17 ~ 10-21）

从 `PROCESS.md` 精简时原样搬来，一行没改（原文 = 第 2044–3103 行）。
这批批次已经翻篇的部分留在这儿：命令层与命令面板（React 时代最后两批）、egui 壳落地、几何门禁、行距与虚拟化、提交图序列、提交端到端、齐左、添加仓库、删 Tauri、许可证与素材审计、外壳几何重量、侧栏重画、点击修复、日志分页、hunk 级暂存、文件表方框列。
每条判据的反向变异结果、以及每批「仍然没看到过的」边界，都在这份文件里；`PROCESS.md` 只留最近两批全文 + 一行一批的台账。

---

## 变更文件速览（本次：命令层落地 + egui spike 实测）

- 新增 `src/commands.ts`（~520 行，76 条命令）：一张表同时描述键位、面板、外部驱动。
  `runCommand(id, target)` 是唯一入口，未知 id / `when()` 为假 / 列表持有的命令一律返回 `false`，
  不返回假成功。破坏性确认（删分支、删标签、丢贮藏、丢文件、移除仓库、硬重置）全部搬进这一层，
  鼠标菜单和键位不可能再和它意见不一致。
- 重写 `src/shortcuts.ts`（353 → 236 行）：只留键位派发，命令表从 `commands.ts` 导入并 re-export。
  `matchChord` 跳过 `isBindable` 为假的命令（带 target 的键位说不出「哪个分支」，绑了也只会静默什么都不做）。
- `src/store.ts`：`showGitignore` 从组件局部状态提到 store（**顺手修掉一个真 bug**：
  .gitignore 编辑器开着时全局快捷键仍然拦截按键，因为派发器看不见它）；
  新增 `fileLogOid` / `setFileLogOid`，把溯源面板的选区发布出来，`wc.restoreRev` 才叫得出目标。
- `src/components/{Settings,ShortcutsHelp,WorkingCopy,FileHistory}.tsx`：硬编码的分组列表
  `["导航","同步","工作区","提交列表"]` 换成 `CMD_GROUPS`（这一类「表改了文档没改」的陈旧清单一次清掉），
  快捷键帮助只显示真有键位的命令，空分组不再出现。
- 新增 `src/commands.test.ts`（22 例）：结构不变量、`runCommand` 三种拒绝、面板搜索排序、
  以及**覆盖率闸门** —— 界面上每个 `useStore((s) => s.xxx)` 拿到的 store 动作，命令层必须认识，
  或者出现在带理由的白名单里；白名单不许写「同上」、不许躺尸体、匹配前**先剥注释**。
- 新增 `uiprobe/`（独立 crate，非产品代码）：PROCESS.md 那两条 spike 判据的测量工具。
- `package.json`：`@types/node`（覆盖率闸门要读源文件）。

**判据（整批一次跑齐）**：`npx tsc --noEmit` 干净；`npm test` 195 → **217 例 / 18 文件全绿**；
`npm run build` 通过（index.js 299.64 kB，命令层把 76 条表项带进首屏，比上一批 +13 kB）；
`cargo test`（gitcraft-core）**72 例全绿**；`cargo check --tests`（src-tauri）通过。

**反向变异（三条，都真跑过，源文件从 `/tmp` 快照还原并 `cmp` 确认字节一致）**：
1. 删掉 `wc.restoreRev` 那一行、**留下提到 `restoreFileToRev` 的注释** →
   覆盖率闸门报红（`这些动作只有鼠标能触发：restoreFileToRev`）。
   这一条是补出来的：第一版闸门用全文文本匹配，被注释骗过，剥注释后才真的有效。
2. 给某条带 `target` 的命令加默认键位 → 「有默认键位的命令要么全局可绑定要么由列表派发」报红。
3. 白名单里写 `clearSelection: "同上"` → 「每条都必须给出理由」报红，逼出独立理由。

**没看到过的**：
- 命令面板 UI 本身（⌘K 那个界面）——`paletteCommands` / `searchCommands` / `targetsFor` 全部有测试，
  但**没有界面消费它们**，所以「外部驱动能跑通」目前只在单测里成立，没在真窗口里点过。
- `restoreFileToRev` 走命令层之后的真实回滚效果：只测了 `when()` 的开关，没在真窗口里点过取回。
- 覆盖率闸门的动作识别靠 `store.ts` 里 `interface GitcraftState` 的缩进正则；
  接口写法一旦变化（比如换行断句）它会**静默漏识别**，漏掉的是「本该被闸门管到的动作」。

## 变更文件速览（本次：命令面板 UI 落地，⌘K）

上一批欠的那条「没看到过的」——**没有任何界面消费命令表**——这一批补上了。
现在 ⌘K 能执行表里每一条 `when()` 通过的操作，鼠标点不到的动作第一次有了第二条通路。

**改了哪些**：
- `src/palette.ts`（新，纯逻辑）：两阶段的三个决定——选中一条命令是**直接执行**、
  **先问是哪个对象**、还是**明说没对象可操作**。放在这儿是为了能被单测钉住（vitest 是 node 环境，
  组件本身测不了）。
- `src/components/CommandPalette.tsx`（新）：面板本体。只 import `commands` / `palette` / `shortcuts`，
  **不 import 任何视图组件**——「界面层永久可替换」从此不是口号，是这条 import 列表。
- `src/commands.ts`：加 `nav.palette`（`mod+k`，`input: true`：在提交框里按 ⌘K 也要开面板）。
- `src/store.ts`：`showPalette` / `setShowPalette`（面板的开关状态必须能被命令层摸到）。
- `src/shortcuts.ts`：`showPalette` 进「模态占用键盘」名单；**顺手修掉一条过期注释**——
  它说溯源面板的键位「住在 shortcuts.ts」，其实住在 `FileHistory.tsx` 自己的监听器里。
- `src/App.tsx`：Esc 阶梯里加 `if (s.showPalette) return;`（面板的第一个 Esc 是**退回上一步**，
  不是关窗口，所以这一层不能让给阶梯），并挂上 `<CommandPalette />`。
- `src/components/TopBar.tsx`：右上角「命令 ⌘K」按钮。没有可见入口的面板等于没有面板。
- `src/styles.css`：`.palette-*` 一组，复用 `.modal` / `.modal-overlay` 的框，只写内部。
- `src/palette.test.ts`（新，9 例）。

**判据**：⌘K 开面板 → 输入中文筛 → 回车执行 → 面板关闭且动作真的发生；Esc 关面板；
带 `target` 的命令绝不带着 `undefined` 去跑。

**真窗口实测**（browser-use 驱动 `npm run dev` 的 1420 端口，截图 `/tmp/palette_1..5.png`）：
开面板 17 项（浏览器里没仓库，`when: hasRepo` 把大半张表挡在外面，这正是 `when()` 该有的表现）；
输入「贮藏」筛到 1 项且面板**不缩短**（`min-height` 生效）；输入「设置」回车 →
设置面板打开、命令面板自己收起；Esc 关掉设置面板；再按 ⌘K → 面板收起。

**反向变异三条**（都按快照 + `cmp` 还原）：
1. `chooseCommand` 改成「带 target 也直接跑」→ 恰好红两条（先问对象 / 空列表要明说）。
2. `targetItems` 去掉 hint 与大小写归一 → 恰好红「标签、值、提示三处都能命中」。
3. `escapeStep` 永远返回 close → 恰好红「Esc 阶梯」。

**仍然没看到过的**：
- **第二步（选对象）的界面**。浏览器里出不来：wry 的 `invoke` 不存在 → 仓库列表为空 →
  所有带 `target` 的命令被 `when()` 挡在面板外面。它的逻辑有 4 例钉着，渲染跟第一步共用
  `.palette-row`，但面包屑、hint 列、空列表提示这三样**没有在任何一个真窗口里看到过**。
  补法很省：打开一个仓库，⌘K，输「检出」，回车。
- 面板在**含 emoji 的中文提交信息**上的搜索命中（`searchCommands` 只测了静态串）。
- 上一批那条 ⌛ 待办仍然挂着：**判据②（真人拼音）还没人测过**，面板这一层同样是拼音输入，
  所以这一次实测顺手把判据②的现场也准备好了——在面板输入框里打一句中文，
  合成态、候选框位置、吞不吞键，一次看全。

## 变更文件速览（本次：egui 壳落地，外加一道不依赖屏幕的几何门禁）

「推进 egui 替代」和「复刻 SourceTree」合成一件事做：**壳直接在 egui 里长**，React 版退成行为参照。
上一批卡在「判据要截图，而截图拿不到」，这一批把判据换成了不依赖屏幕的形式。

**新增 `gitcraft-app/`**（`main.rs` 1130 行 + `fonts.rs` 150 行 + `Cargo.toml` 18 行）：
eframe 0.37，直连 `gitcraft-core`，**没有 IPC 那一跳**——git 函数直接调。四块带（工具栏 55pt /
全高侧栏 141pt / 日志带 / 底部「文件 435pt | 差异」245pt）按「复刻规格」的实测值摆；
所有 git 动作在工作线程跑，mpsc 回帧，帧只画；日志与差异都走 `show_rows` 虚拟滚动
（「更好 = 性能」那条轴上，28 875 行的 diff 是这一版的靶子）。

**判据换了形式**：`geometry_tests` 无头跑一帧（`Context::run_ui` + `root_ui`，不必伪造
`eframe::Frame`），把这一帧画出的**纯色矩形**全收下来，逐块比对四块带的四条边——
`RawInput.screen_rect` 就是窗口内容区 1050×545（参考图 2100×1090 @2x），
`WIN_W`/`WIN_H` 两个常量把这件事钉在代码里。容差 1.5pt 是给 egui 那条 1px 分隔线的。
字体那条判据同理：`Fonts::has_glyph` 直接问字体链里有没有「：」「修」「交」这几个码位。

**这道门禁当场抓出四个真 bug**（都不是测试自己的问题，是壳的问题）：
1. `style()` 只改 Light 那份样式，而 `Context::default()` 是 Dark；系统一进深色模式，
   整份调色板（含 `panel_fill`、文字色）**静默失效**。→ `ctx.set_theme(Theme::Light)`。
2. 工具栏写成两行（仓库名一行、11 个竖按钮一行），43pt 的按钮把 55pt 的带**撑到 74pt**，
   底色会盖到侧栏上。→ 并成一行（`with_layout` 不换行），`interact_size.y` 定到 22pt = `ROW_H`。
3. 侧栏的 `ScrollArea` 默认吃掉全部可用高度，最底那条「筛选器」被**挤出窗口**
   （底色矩形底边 572 > 545）。→ 先扣掉 `SIDE_FILTER_H` 再开滚动区。
4. 暂存带用默认 `Frame::side_top_panel`，自带 8×2 内边距，把文件表整体**右移 8pt**——
   参考图里文件表是紧贴侧栏的。→ 显式 `Frame::NONE`，内边距交给两个子面板自己给。

**反向变异四条**（都按 `/tmp` 快照 + `cmp` 还原，还原后 5 例全绿）：删 `set_theme` → 红侧栏那条；
按钮高度 +26pt 模拟内容超高 → 红工具栏那条；去掉暂存带显式 frame → 红文件表那条（x0 141→149）；
去掉滚动区 `max_height` → 红侧栏那条（底边 545→572）。每条都只红它该红的那一条。

**字体（任务 #10）落地路径**：`raw.githubusercontent` 在这台机器上只有 ~1 KB/s 且反复截断
（12:58 那份 5 297 911 字节，10 张表越界——正是会让 epaint 在不展开的闭包里 panic、
进程直接 abort 的那种输入），换 **jsDelivr** 一次拿全 8 331 336 字节。
`fonts.rs::complete()` 校 sfnt 表目录，不完整的直接跳过并退回用户机器上的
`~/Library/Fonts/msyh.ttf`（15 MB 单面 TTF；`.ttc` 是字体集合，ab_glyph 解析不了，所以不进候选）。
真进程日志实证：**「gitcraft-app: CJK 字体用了 gitcraft-app/assets/fonts/NotoSansSC-Regular.otf」**。
字体不入库（`.gitignore` 加 `gitcraft-app/assets/fonts/*.otf|*.ttf`），
`OFL.txt` 从一张 404 页面改成「怎么重新取一份」+ 字体自身 `name` 表里读出的 OFL 1.1 声明。

**上一批那桩悬案结了**：「窗口截图 196 440 字节、纯白、探针色一个都没有」**不是渲染失败，是屏幕锁了**。
13:21 的整屏截图就是锁屏页（大时钟 + 日期），`screencapture -l` 在锁屏会话里只能拿到一张空白表面。
`pmset` 那个 13 秒休眠是同一件事的另一半。

**门禁**：`npx tsc --noEmit` 0 错；`npm test` 19 文件 226 例全绿；`gitcraft-core` `cargo test` 72 例全绿；
`gitcraft-app` `cargo test` 5 例全绿、`cargo build` 零警告。

**仍然没看到过的（这一轮的边界）**：
- **这个窗口被人眼看过一次都没有**。四块带的边界与配色现在是断言级证据，
  但「截图叠在参考图旁边、四块边界目视一致」这条判据要等屏幕解锁才能补。
- 无头测试跑的是**空态壳**（`snap: None`）。日志行、文件表、差异区在有真数据时的排版没测过。
- hunk 级「暂存区块 / 放弃区块」两枚按钮**没渲染**：`gitcraft-core` 还没有按 hunk 应用的函数，
  先不假装有。
- 提交图那一列（`COL_GRAPH` 49pt）是空的，`src/graph.ts` 还没搬。
- 侧栏 标签/远端/贮藏/搜索、工具栏 提交/分支/合并/贮藏/远程服务器/终端/设置，
  点了都是**显式的「egui 版还没接」提示**，不是静默失效——但提示本身也还没在真窗口里看到过。
- 深色模式在 egui 版里不存在（整份调色板硬编码亮色 + 强制 Light）。

## 变更文件速览（本次：把「面板里有真数据」变成断言，顺带钉住行距与虚拟化）

上一批的无头门禁跑的是**空态壳**。这一批给它喂假数据：`geometry_tests::filled(commits, &[DiffLine])`
（`gitcraft-app/src/main.rs:1082`，`loaded()` = `filled(3, &layout(PATCH))`），五条新判据随之落地——
`log_rows_step_by_exactly_one_row_height`（1278）、`diff_rows_are_painted_row_by_row`（1306）、
`scrolling_moves_every_log_row_by_exactly_the_scrolled_pixels`（1339）、
`file_list_shows_every_working_change_once`、`a_real_repo_arrives_through_the_worker_without_blocking_a_frame`（1413）。
判据写得很具体：**日志相邻两行的文字落点差必须正好
`ROW_H` 21pt，差异行必须正好 `DIFF_ROW_H` 15pt，滚 42pt 每行也必须精确走 42pt，
一帧里排的行数只跟窗口高度有关**。

**抓出两个真问题**：

1. **行距被 `Ui::put` 往回拽**：实测 17.59375 ≠ 21、14.21875 ≠ 15。根因在 egui 内部——
   `Ui::put`（`egui-0.37.0/src/ui.rs:1691`）= `scope_builder(max_rect, Layout::centered_and_justified)`，
   而 `scope_dyn`（同一文件 2338 行）按**子 ui 的 min_rect**推进父游标。子 ui 是垂直居中的，
   min_rect 只有文字那一条（高 ≈ 字体行高 17.6），比整行 21 矮，于是游标**倒回到行中间**。
   `ScrollArea::show_rows` 恰恰用 `cursor.min.y / row_height` 反解可见区间——游标倒回等于每行都比
   上一行矮一截，几千行下来可见窗口和实际落点彻底对不上。
   → 每行画完补一次 `fix_pitch(ui, rect)`（`main.rs:728`），里面走 0.37 中**公开**的
   `Ui::advance_cursor_after_rect`（ui.rs:1389）把游标按回整行底部。
2. **差异行底色右边差 2pt**（1048 ≠ 1050）：差异面板的 `inner_margin` 写成了 `symmetric(2, 2)`。
   SourceTree 的增删行是整块铺满的 → 改 `symmetric(0, 2)`，只留上下那 2pt。

**无头滚动怎么喂**（`scroll_log`，`main.rs:1159`）——两个坑都是读 egui 源码读出来的，不是试出来的：
* `ScrollArea` 只把滚轮给**指针悬停的那一块**（scroll_area.rs:1324），所以每帧都得带一个落在日志带里的
  `Event::PointerMoved`，光有滚轮事件不滚；
* 一次离散的滚轮事件会被 egui **摊平到后面几帧**（`wheel_state.rs` 顶部注释 + `SCROLL_ACTION_TIMEOUT`
  150ms），所以 `RawInput.time` 必须往前走、还要多跑几帧，一帧只放出一小段。
  第一版只发一个 `TouchPhase::Move`、只跑一帧，测出来「移动了 0」——那不是壳的 bug，是判据没喂对输入。

**#9（真实数据通路）也一并补了证据**：`a_real_repo_arrives_through_the_worker_without_blocking_a_frame`
（`main.rs:1413`）**开的是这个仓库自己**——`open(repo_root)` → 工作线程 `snapshot_of` → mpsc → 后续帧的
`poll`。「异步」不是靠时序赌运气，是**结构**断言：`open()` 返回的那一刻 `busy == true && snap.is_none()`
（同步加载的话这里就有数据），然后最多 600 帧、每帧之间睡 5ms，等到数据为止；实测**等 17 帧**真 snapshot 到了，
中间每一帧都仍然画得出东西（`out.shapes` 非空）。落到的数据也检查了是真 git 输出：分支名非空、提交列表非空。

**反向变异七条**（每条只红它该红的，全部用 `/tmp/gc_green.rs` + `cmp` 还原）：
- 删日志行的 `fix_pitch` → `log_rows_step…`（17.59375）**和** `scrolling_moves…`（-38.59375）两条红，其余 9 条绿。
- 删差异行的 `fix_pitch` → `diff_rows_are_painted_row_by_row` 红（14.21875），其余绿。
- `style()` 里 `item_spacing.y` 0 → 4 → 三条几何判据一起红（25 = 21+4、19 = 15+4、工具栏带被撑出）。
  上一批那句「`item_spacing.y` 钉成 0 才有意义」从注释变成了断言。
- 日志 `show_rows` 换成 `show` + `for r in 0..total` → **只有** `a_frame_paints_the_window_not_the_repo` 红
  （41 行）；差异那边同样换掉 → 红在差异那条断言（402 → 40002）。
- `show_rows(ui, ROW_H, …)` 改成 `ROW_H + 1.0`（分配仍是 21pt，只有可见区间算错）→
  **只有** `scrolling_moves…` 红（-41 ≠ -42），静态那条全绿。这条证明滚动判据不是静态那条的重复。
- `open()` 里把 `spawn_snapshot(...)` 换成同步的 `self.snap = snapshot_of(path).ok()` →
  **只有** `a_real_repo…` 红，报的正是那句「open() 刚返回就有 snapshot 了：那是同步加载」。

**这一批最值钱的一条发现，差点写出一个假门禁**：`a_frame_paints_the_window_not_the_repo` 第一版数的是
`FullOutput` 里的行数，于是**把虚拟化整个关掉它照样绿**——40 条提交全排一遍，落图仍然只有 9 行文字。
egui 在细分阶段按 `clip_rect` 把屏幕外的形状剪掉了，所以「画了几个形状」根本看不出「排了多少行」，
而虚拟化的成本恰恰在排版那一半（每行一次中文串拷贝 + 一次 galley 布局）。
判据必须读**行闭包真的跑了几次** → `ROWS_PAINTED`（`main.rs:51`，thread_local `Cell<usize>`，
`shell()` 每帧开头清零，`main.rs:671`）。产品代码里多这一个计数器就是为了这一条断言，注释写明了为什么。
实测：日志一帧 **11 行**（视口 10 条 + 那条常驻「未提交」行），差异一帧 **18 行**（其中 16 行是差异行）；
提交 40 → 4000、差异 400 → 40 000 行，两个数纹丝不动。

**门禁**：`gitcraft-app` `cargo test` 11 例全绿、`cargo build` 零警告；`gitcraft-core` `cargo test` 全绿；
`src-tauri` `cargo check --tests` 全绿；`npx tsc --noEmit` 0 错；`npm test` 226 例全绿；`npm run build` 通过。

**仍然没看到过的（这一轮的边界）**：
- 上一批那句仍然成立：**这个窗口被人眼看过一次都没有**，「截图叠在参考图旁边」还等屏幕解锁。
- 假数据不等于真仓库：`filled()` 造的 `Snapshot` 只填够排版的那点。真仓库里 200 字节的英文提交信息、
  `feature/2026-10-xxx-release-cut` 那种长分支名，行宽分配没测；`starts_with("主题")` 只保证前缀在，
  省略号出现在第几个字、截断后还能不能被跳转框命中，都没断言。
- `ROWS_PAINTED` 数的是行闭包跑了几次，不是这帧真实花了多少毫秒：hunk 头折叠、二进制文件那种
  「这是二进制文件」占位行还没进判据。
- 滚动那条只覆盖**滚轮 × 日志带**：拖滚动条、点行之后的自动滚动、差异区的横向滚动都还没进判据；
  而且它靠 12 帧把摊平的余量放干，改 `SCROLL_ACTION_TIMEOUT` 之类的外部行为不会让它红。
- 上一批挂着的仍然挂着：hunk 级暂存两枚按钮（`gitcraft-core` 还没有按 hunk 应用的函数）、
  `COL_GRAPH` 空列、侧栏/工具栏未接动作、深色模式。

## 变更文件速览（本次：49pt 那条空列填上了提交图序列）

`COL_GRAPH` 那一列从上一批起就空着，注释里写着「`src/graph.ts` 还没搬」。这一批搬完了，
而且搬过来的**不是那份代码**——`src/graph.ts:30` 的 `layoutGraph` 只把车道往上添，
**从不释放**（`lanes[pl] = p` 之后没有一处把 lane 设回 null），一条支线合回主线，
那条道一辈子占着；越往后翻，历史越宽。Rust 版 `layout_graph`（`gitcraft-app/src/main.rs:1040`）
在每个提交排完之后把「下游不再有人认领」的道收回 `None`，下一行需要道时先复用空位。
这是「比 SourceTree 更好」里**性能**那一格：图的车道数不随仓库合并次数单调增长。

**数据侧先把排序开关拆干净**（`gitcraft-core/src/git.rs`）：`commits`（507）和新加的
`commits_topo`（519）都走同一个 `log_commits`（587），argv 由 `log_args`（531）生成、解析由
`parse_log`（563）负责，两条列表**只差 `--topo-order` 这一个 flag**。配两条核心门禁：
`topo_switch_only_adds_the_order_flag`（1861）断言开关前后剥掉那一个参数的 argv 逐字节相同
（换排序不许顺手把 `--author`/路径过滤也换了），`every_parent_is_listed_below_its_child`（1878）
建一个真的菱形合并（init → main1 → 支线 → 合回 → main2，`merge --no-ff`），断言
`commits` 和 `commits_topo` 都存在一个两父提交，且**每个父的下标都大于其子**——
图序列只扫一遍就能连线，靠的就是这条；哪天真出现父在上、子在下，图会往上弯，那时该红的是这条。

**几何**：`COL_GRAPH` 49 = `GRAPH_PAD` 1 + 4 × `LANE_W` 12（`main.rs:23`、30、31），
`lane_x`（1100）= `min_x + 1 + lane*12 + 6`，日志带从 `SIDEBAR_W` 141 起，所以第 0 道圆心 **x=148.0**。
`graph_cell`（1105）先画线再画点：竖线 `Seg::Stem`/`Through`/`Tail` 用 `line_segment`（宽 `LINE_W` 1.3），
弯线 `Seg::Arm` 用 `CubicBezierShape::from_points_stroke`（控制点上下各偏半行高，`d = (b.y-a.y)*0.55`），
点 `circle_filled`（半径 `DOT_R` 2.6）。车道配色 8 色循环，逐字节抄自 `src/graph.ts:11`。
每行的线段种类按「这行提交是不是某条道的头」决定：`Stem`（自己接着上面的道）→
`Through`（别的道从这一行穿过去）→ `Tail` + `Arm`（本轮新释放的和分叉）——顺序错了会把别人的线接到自己头上。

**七条新判据**（`gitcraft-app` 11 → **18 例**）：
`a_linear_history_keeps_one_dot_per_row_in_the_first_lane`（1793）——三条提交三个点、
车道 `[0,0,0]`、零弯线、相邻点 y 差正好 `ROW_H` 21（残差 <0.01）；
`a_rejoin_frees_the_lane_it_had_borrowed`（1814）——直接读 `layout_graph`，钉住
7 行的车道序列 `[0,0,0,2,1,0,0]`、弯线 `[(0,1),(0,2)]`、顶行 `[Tail(0), Arm{0→1}]`、
第 2 行 `[Stem(0), Through(1), Through(2), Tail(0)]`（两条支线都得从主线这行穿过去），
并回之后 `rows[5] == [Stem(0), Tail(0)]`、`rows[6] == [Stem(0)]`——**上面借用的道一条不许还占着**，
最后断言最多 4 条道（49pt 装得下的上限）；
`the_graph_never_escapes_its_column`（1849）——七个点、两条弯线的**每个端点** x 都落在 (141, 190) 内；
`the_graph_is_laid_out_once_per_snapshot_not_per_frame`（1870）——造快照排 1 次，之后跑 5 帧仍是 1 次，
报错信息把多排的次数直接写出来；`a_filtered_log_leaves_the_graph_column_empty`（1887）——筛「主题2」后
点/线/弯线全 0，但「主题」开头的文字照排（留空的是那一列，不是整块日志）；
`graph_marks_are_windowed_too`（1909）——提交 40 → 4000，一帧的圆点数**不变**；
`the_topology_button_asks_for_the_list_again`（1919）——**真点击**（见下）「层级顺序」，断言
`topo` 翻面且 `busy && snap.is_none()`（就地重排会留下旧行号的图）。

图元怎么数出来的，这批踩了**两个假门禁**，都写进了注释：
1. 第一版 `graph_marks` 按 **x 窗口**筛形状——结果侧栏那根 140.5pt 的分隔线被当成图元算进去，
   而越出列的形状早在筛掉之列，「不越界」那条断言于是恒真。现在按**车道颜色**认领：
   `Circle`/`LineSegment` 的 `Color32` 直接比，`CubicBezierShape` 的 `stroke.color` 是
   `ColorMode`（非 Copy，`matches!` 里得用 `&`），任何颜色不在那 8 色表里的形状都不是图。
2. `GRAPH_LAYOUTS`（`main.rs:69`）第一版是全局 `AtomicUsize`，被并行跑的其它夹具污染，
   「跑完五帧」报出 9 次。改成 **thread_local `Cell<usize>`** 才对：生产路径的正确形状是
   **worker 排 1 次、主线程 0 次**，两边必须分开数——真实仓库那条判据正是这样断言的
   （`snap.graph.len() == snap.commits.len()` 且主线程 `graph_layouts() == 0`）。

点击也进了无头 harness（`click_text`，1749）：先在输出里找到那块文字的 galley rect
（`t.galley.rect.translate(t.pos.to_vec2()).center()`，`Rect + Vec2` 在 0.37 没实现），
再喂三帧 `PointerMoved` → `PointerButton{pressed:true}` → `pressed:false`（0.37 是 `pressed`，
不是 `action`；每帧的 `textures_delta` 要清掉，否则 `Context::run_ui` 的 `#[must_use]` 掉落时 panic）。

**反向变异七条**（还原一律走 `/tmp/gc_graph_green.rs` + `cmp`）：
- 去掉「合回后释放车道」→ **只有** `a_rejoin_frees…` 红，报的正是 `left: [Stem(0), Through(1),
  Through(2), Tail(0)]` vs `right: [Stem(0), Tail(0)]`——就是 `src/graph.ts` 那个行为。
- `layout_graph` 从 `snapshot_of` 挪进每帧的 `log_band` → `the_graph_is_laid_out_once…`（多排 **21** 次）
  和真实仓库那条（**4**）红，其余绿。
- 去掉行尾 `None` 道的裁剪 → **一条都不红**。诚实记下来：那条裁剪只把每行扫车道的长度封住
  （性能），不改变任何一行输出的线段，所以没有语义判据能看见它。
- 图元按「虚拟行号」而不是「`keep` 索引」取（`snap.graph.get(r)`）→ 圆点数 3→**4**，
  落在 `[[148.0,107.5],[148.0,128.5],[148.0,149.5],[148.0,170.5]]`，`graph_marks_are_windowed…`
  那条 7→**8**。多出来的正是那条常驻「未提交」行——它有行没有提交，图不能给它画点。
- `lane_x` 的基准从 `row.min.x` 挪到 `row.min.x + COL_SHA` → **只有** `the_graph_never_escapes…` 红。
- `log_commits` 里 `topo` 参数忽略掉、永远加 `--topo-order` → **只有** `topo_switch_only_adds_the_order_flag` 红，
  核心另外 73 例全绿（说明这个开关确实与过滤、解析正交）。
- 筛过的日志照画图序列（去掉 `unfiltered`）→ **只有** `a_filtered_log…` 红，
  报「筛过的日志里还画了 **1** 个点、**1** 条线、0 条弯线」——那一条线就是接到没显示出来的提交上的竖线。

**门禁**：`gitcraft-app` `cargo test` **18 例全绿**、`cargo build` **零警告**；
`gitcraft-core` `cargo test` **74 例全绿**；`src-tauri` `cargo check --tests` 全绿（无警告）；
`npx tsc --noEmit` 0 错；`npm test` 19 文件 / **226 例**全绿；`npm run build` 通过。整批一次跑完，日志 `/tmp/gates_graph.log`。

**仍然没看到过的（这一轮的边界）**：
- 「这个窗口被人眼看过一次都没有」这句仍然成立，图序列也一样——所有形状都是无头数出来的。
- 按钮那条只断言到**「点了 → `topo` 翻面 → 重新取列表」**，最后那一跳（列表真的换成拓扑序）
  在 app 侧没有断言：`filled()`/`branched()` 是人造的 `Snapshot`，而 `snapshot_of` 需要仓库；
  要钉住它得找一个**日期序和拓扑序确实不同**的真仓库（一批乱序合入的历史），那是核心侧的活。
- 车道超过 4 条时现在是 **assert 报错**（宁可红也不压扁），还没做「列宽自适应/滚动」；
  真实仓库第一屏就撞上限的样本没测过。
- `--max-count=500` 把列表从中间截断，尾部那半条边的形状（父在下标外）没断言；
  `every_parent_is_listed_below_its_child` 只覆盖小菱形，合并 3 个以上父、以及 octopus merge 未测。
- `ROWS_PAINTED`/`GRAPH_LAYOUTS` 数的都是**次数**不是毫秒：图序列在 worker 上跑多久没有门禁，
  4000 条提交的 `layout_graph` 耗时没量过。
- 上一批挂着的仍然挂着：hunk 级暂存两枚按钮、侧栏/工具栏未接动作、深色模式；
  「层级顺序」是筛选条四枚开关里**唯一**接了动作的一枚。

## 变更文件速览（本次：egui 壳里按下提交按钮，真的仓库多了一条提交）

上一批留下的边界是「按钮 → 列表真的换成新内容，app 侧没断言」。这一批直接把它吃掉：
提交那条通路在**无头壳里跑真 git**。

**产品代码**（`gitcraft-app/src/main.rs`）：
`Action` 多一枚 `Commit(String)`（130）；`act`（303）把它接到 `git::commit(&repo, m, false, &[], false, false)`
（318）——**不带路径**，操作暂存区，和参考版 `store.commit`（`src/store.ts:763`）传的空 `paths` 一致；
`file_list`（544）在两块 section 下面加提交框：`TextEdit::singleline(&mut self.commit_msg)`
带 placeholder「提交摘要… (⌘/Ctrl+Enter 提交)」（582）、按钮文案「提交到 {branch} ({staged_count})」、
禁用式 `staged_count > 0 && !msg.trim().is_empty()`（584，抄 `WorkingCopy.tsx:337` 的 `canCommit`）、
点不动时旁边那行小字分两种：「先勾选要提交的文件」/「提交信息不能为空」（后者抄 core `git.rs:1082` 原话）。
清空只跟着 `Msg::Loaded` 走（358），且只有提交那一个动作会 arm（308）。

**测试脚手架**（同文件测试模块）：`scratch_repo(tag)`（2026）用 `git init` 造一次初始提交 +
一个**已暂存**的改动，`open_repo(tag)`（2070）把壳开在这个真仓库上，`pump(app, done)`（2058）一帧帧等到
条件成立（600 帧上限，约 3s，超时带着 `note` 一起 panic）；`frame_events`/`raw_input`（1951/1959）喂真事件，
`locate`/`click_at`（1972/1806）从**这一帧真的画出来的文字**反推矩形中心、按下与抬起分帧喂，
`type_into`（1988）先点出焦点再发 `Event::Text`（macOS 输入法上屏那一刻交给控件的就是这个事件），
`press_mod_enter(app, m)`（2005）发 `ModifiersChanged(m)` → `Key{Enter, pressed}`。

**五条判据**：`a_typed_message_and_a_click_lands_a_real_commit`（2082，打「补一条中文提交」→点「提交到」→
`commits.len()+1`、`commits[0].subject` 就是那句、工作区清空、那格信息清空、`note` 是 None、
`graph.len() == commits.len()`）、`cmd_enter_commits_without_the_mouse`（2111，不碰鼠标）、
`ctrl_enter_commits_on_a_windows_keyboard`（2126）、`the_commit_rule_is_stated_in_one_place`（2140，
禁用式两半各钉一次：干净暂存区点不动、写了字也不能提交，且信息留着；再测暂存了东西但空信息时报那句 core 的原话）、
`a_rejected_commit_keeps_the_message`（2214，`#[cfg(unix)]` 写一条 `exit 1` 的 pre-commit 钩子 → 提交数不变、
字还在 → 紧接着点「全部暂存」字**还得**在 → 拆掉钩子同一句话再点，落地并清空）。

**抓出四个真问题**：

1. **⌘Enter 点了没反应**。临时插了个 probe 模块打印这一帧的输入状态，得到
   `pressed=true cmd=false ctrl=false`——egui 的 `InputState.modifiers` **只由 `Event::ModifiersChanged`
   更新**（`egui-0.37.0/src/data/input/input_state/mod.rs:434`），键盘事件自带的那份 `modifiers` 它不看。
   真窗口里 winit 是先发修饰键再发按键，所以人手按没问题；是无头测试在骗自己。
   → 脚手架补发 `ModifiersChanged`，产品代码同时认 `command || ctrl`。probe 模块跑完从快照还原。
2. **同一个开关读两遍，判据杀不死它**。第一次反向变异只掐按钮那行的 `&& enter`，22 例全绿——
   因为 `can && enter` 在 591 那块和按钮那行**各判了一次**。→ 现在键盘与鼠标各只有一处判断。
3. **`Msg::Failed` 里那句 `clear_msg_on_load = false` 删掉，没有任何判据变红**：`act` 每次动作都重算
   这个标记（308），走 `act` 的路永远看不见它——它只挡「不经 `act` 的那次 `Loaded`」，也就是切仓库时的
   `open`（999）。代码里就地写明它是**契约锁而不是判别门**。真正承重的是 308 那行，于是补了
   `a_rejected_commit_keeps_the_message` 里那句新断言，并把反向变异挪到 308 上（见 M5）。
4. **`|| i.modifiers.ctrl` 那一半没有判据**：脚手架只会发 `COMMAND`，删掉 ctrl 半边 22 例全绿。
   → `press_cmd_enter` 改成 `press_mod_enter(app, m)`，补 `ctrl_enter_commits_on_a_windows_keyboard`；
   再删 ctrl 半边就只剩这一条红了。

**反向变异八条**（改一次跑一次 `cd gitcraft-app && cargo test`，跑完 `cp /tmp/gc_commit_final.rs` + `cmp` 还原）：
- 删掉 591 那块键盘判断 → 只 `cmd_enter…` 红。
- `can` 去掉 `staged_count > 0` → 只 `the_commit_rule…` 红。
- `Msg::Failed` 里加一句 `commit_msg.clear()` → 只 `a_rejected…` 红，报「提交失败把用户写的字弄丢了」。
- 删掉 `Msg::Failed` 里 `clear_msg_on_load = false` → **一条都不红**（见问题 3，已就地记录）。
- `clear_msg_on_load = matches!(action, Action::Commit(_))` 改成 `= true` → 只 `a_rejected…` 红，
  报新加那句「失败的提交把清空标记留给了下一个动作」。
- `git::commit(.., false, ..)` 的 amend 位改成 `true` → **端到端四条全红**（18/23）：假快照骗不了真 git。
- `press_mod_enter` 不发 `ModifiersChanged` → 只 `cmd_enter…` 红。
- 去掉 `|| i.modifiers.ctrl` → 只 `ctrl_enter…` 红。

**顺带**：`raw_input` 里 `focused: true` 仍然不是承重的（改成 false 一条都不红，注释在 1962）；
焦点在 `Context` 的 memory 里，所以点击与敲字必须共用同一个 `Context`。

**开源准备**：新增 `CONTRIBUTING.md`（分层表、三条硬规矩、窄门禁表、测试数据规矩、「好的第一 PR」清单）、
`scripts/gates.sh`（六条门禁 + egui 零警告构建，`chmod +x` 已做）、`.github/workflows/gates.yml`
（`frontend` 跑 ubuntu / `rust` 跑 macos-14）、issue 与 PR 模板（PR 模板要求填「判据 / 反向变异红了什么没红什么 /
仍然没看到过的」）。README 补了 `gitcraft-app` 与 `uiprobe` 两层、无头测试跑法、贡献一段。
**LICENSE 仍然没有**：这一条是法律决定，没替用户选，README 里写明「许可证尚未确定 → 默认保留所有权利」。

**门禁**：`gitcraft-app` `cargo test` **23 例全绿**、`cargo build` **零警告**；`gitcraft-core` **74 例全绿**；
`src-tauri` `cargo check --tests` 全绿；`npx tsc --noEmit` 0 错；`npm test` 19 文件 / **226 例**全绿；
`npm run build` 通过。整批一次跑完（`./scripts/gates.sh`，exit 0），日志 `/tmp/gates_commit.log`。

**仍然没看到过的（这一轮的边界）**：
- 「这个窗口被人眼看过一次都没有」照旧——连提交框都是无头的，placeholder 有没有被截断、
  按钮在窄窗口里会不会挤掉行，都不知道。
- 提交跑的是本地仓库，**没有 remote**：push/pull/fetch 三条动作在 app 侧仍一条判据都没有。
- 只测了「暂存区非空 + 有信息」这一条通路；一次提交多个文件、重名中文路径、`.gitattributes`
  换行转换下的内容都没进这几条判据。
- `--no-verify`、`-S` 签名、amend、详细描述（React 版「更多选项」那几枚开关）egui 版**一概没有**，
  所以也没有判据；`git::commit` 的 `amend` 位现在是硬编码 `false`。
- 钩子只测了 `pre-commit exit 1`；`commit-msg` 拒绝、钩子打印一堆 stderr 的情形没测。
- 提交后 `Msg::CommitFiles`/差异区的联动没断言：新提交那条被选中时右侧显示什么，仍然没验。
- `pump` 的 600 帧上限意味着这三条端到端判据最坏各等 3s；本机 0.7s 跑完全套 23 例，
  CI 的 macos-14 runner 上没跑过（workflow 是这次新写的，第一次 push 才知道对不对）。

## 本次：日志四列与差异正文改成**齐左**，并给「对齐」这件事补上判据（`gitcraft-app`，23 → 25 例）

两张 4.2.19 真窗口截图进了 `docs/reference/`（`-repo-window-2026-10-20.png`、`-bookshelf-2026-10-20.png`）。
上一批的结论是「大体像、细节欠」，细节里最先露馅的是**对齐**：SourceTree 的表头与数据都在同一侧，
差异正文每一行共用一个行首。egui 版当时四列走 `RIGHT_CENTER`、描述与差异走 `Ui::put`。

**先把数字钉住，再改代码。** 机器上没有 PIL/numpy，`uiprobe/pngprobe.py` 用 stdlib 解 PNG
（IHDR/IDAT + 五种 scanline filter），把指定 y 带内的着墨 x 范围按 logical pt 打出来。实测：
图表 242 / 数据 239–246.5，提交 284.5 / 282.5，描述 361.5 / 357，作者 825 / 824，日期 1017 / 1013.5。
**五列全部靠左**，表头与数据的差只是字形侧 bearing（≤4.5pt）。差异正文同理齐左，行号栏才是右对齐。

**`Ui::put` 不是靠左的**：它内部是 `scope_builder(max_rect, Layout::centered_and_justified)`
（egui-0.37.0/src/ui.rs:1691），所以短文字被**居中**、带空格的长文字被**拉宽两端对齐**——
同一列里每行的行首 x 都不一样。加了 `put_left`（`UiBuilder::new().max_rect(rect)`
+ `Layout::left_to_right(Align::Center)`），描述列与差异正文走它；四列数据与表头改成 `Align2::LEFT_*`。
提交号那一格还退回过一次起点：它曾画在 `rect.min.x`，也就是**整列在图列里面、圆点压在提交号上**，
现在是 `rect.min.x + COL_GRAPH`。

**新判据（`geometry_tests`）**：
- `every_log_column_starts_at_its_own_header`：三行故意长短不齐的提交（`ragged()`：`a1`/`b22`/`c333`、
  「短」/「一条长得多的…」/「中等长度的一条」、单字名 / 长邮箱 / 单字母），四列**每格左缘**都必须在
  自己表头左缘 1pt 内；再把绝对位置钉死——`sha == 190`、`subject == sha+COL_SHA+8 == 258`、
  `author == 742`、`date == 932`。一条判据同时管三种错法：右对齐、居中/两端对齐、提交号画进图列。
- `diff_lines_share_one_left_edge`：暂存区那六行文字（hunk 头 + 两上下文 + 一删两加）必须共用一个
  `min.x`，且该 x 在两个行号栏右缘（`SIDEBAR_W + FILES_W + NO_W*2 == 636`）之外。
- 支撑件：`text_rect` / `all_text_rects`——从 `FullOutput.shapes` 里的 `Shape::Text` 反解
  **绝对**矩形（`galley.rect.translate(pos)`）。之前所有判据只看「有没有画」和「行距」，
  **没有任何一条量过对齐**，所以四列全右对齐时 23 例照样全绿。

**反向变异（各跑一次，每条都只有该红的红，24/25 或 25/25 其余全绿）**：
- 提交号 `LEFT_CENTER` → `RIGHT_CENTER` → 只 `every_log_column…` 红。
- 差异正文 `put_left` → `centered_and_justified` → 只 `diff_lines…` 红（「 上下文甲」行首 823 vs 767）。
- 描述列同样换成居中 → 只 `every_log_column…` 红（「短」左缘 484，表头 258）。
- 提交号起点退回 `rect.min.x` → 只 `every_log_column…` 红（绝对位置那条先炸）。
- 表头「提交」`LEFT_TOP` → `RIGHT_TOP` → 只 `every_log_column…` 红（表头左缘 169 vs 数据 190）。
这一批**没有**「删掉一条都不红」的变异。

**顺手发现的真问题：无头帧一直在用产品里永远看不到的字体。** `fonts::install` 原先只挂在
`main()` 的 `Box::new(|cc| …)` 闭包里，测试路径根本不走它——缺 CJK 时 `.notdef` 的 advance 正好是
**0.5 × 字号**（实测「中」@14 = 7.00pt，装进 Noto Sans SC 之后 14.00pt），所以之前按宽度下的判据
量的是一个半宽的字。现在 `style(ctx)` 第一行就 `fonts::install(ctx)`，窗口与无头帧共用同一条装配路径，
全套仍然 25 绿，但从此量的才是真宽度。代价：无头每帧 +8.6ms（5 帧 43.4ms vs 17.6ms），
因为每个 `frame()` 新建一个 `Context` 要重新解析 8.3MB 字体——**这是测试脚手架的开销，
真窗口一个进程只解析一次**。

**仍然没看到过的（这一轮的边界）**：
- 字体**没进仓库**（`.gitignore` 挡掉 `assets/fonts/*.otf`，8.3MB），克隆下来只剩 `OFL.txt`；
  退路是 `GITCRAFT_FONT_DIR` 或 `~/Library/Fonts/msyh.ttf`，两个都没有就退回豆腐块。
  README/CONTRIBUTING 这次各补了一句，但**没有**下载脚本，也没有判据保证新克隆出得来汉字。
- 只断了**左缘**。行号栏的右对齐（`Align2::RIGHT_CENTER`，main.rs:692）、颜色、字号、
  以及 hunk 头右侧那两枚「暂存区块/放弃区块」在参考图里的位置，一条判据都没有。
- 参考图的八个数字是从**@2x 截图**上量的，量的是 SourceTree 当前这个窗口尺寸；
  列宽随窗口变的部分（描述列吸收剩余宽度）只有 `every_log_column…` 里那几个绝对值，窗口一变就得重量。
- 「眼睛看过一次」仍然为零：这一批改的全是坐标，视觉上「齐左好不好看」不在判据里。

## 本次：删 Tauri 之前，先给 egui 壳补上「添加仓库」这条路（`gitcraft-app`，25 → 28 例）

**为什么这一步排在删除前面**：仓库列表以前只有 React 版能录入——`db::add_repo` / `scan_repos` 只挂在
Tauri 命令上。egui 壳读同一份库，但壳里 `repo_picker` 在 `repos.is_empty()` 时直接 `return None`，
于是新克隆（空库）打开应用**没有任何入口**：既切不了仓库也开不了仓库，是一条死路。删掉 Tauri 之前
不补这条路，这一层就是不可用的。

**改了什么**：工具栏同一行加「添加仓库」，点开露出路径输入框 + 「打开」；`Gitcraft::add_repo(path)`
走核心层既有的规矩——`find_repo_root` 往上走到仓库根、`is_repo` 复核一次（`gitcraft-core/src/git.rs:232`
那段注释说的就是这两步），然后 `db::add_repo` + 内存列表一份，最后 `open`。
数据库打不开时仍然开：只在内存里记，本次会话能用。被拒绝的路径**不清空、不收起**，改完能直接再来一次。

**三条判据（`geometry_tests`）**：
- `an_empty_bookmark_list_still_offers_a_way_in`：空 `repos` 的壳 → 真点「添加仓库」→ 真敲一个**子目录**
  路径（`<仓库>/src`）→ 真点「打开」→ 快照回来，`repo` 是仓库根、`repos` 多了一条、输入收起。
- `a_path_that_is_not_a_repo_is_refused_without_touching_the_list`：拒绝要有提示（「不是仓库」），
  `repos`/`repo` 一字不动，且输入**留着**。
- `an_added_repo_is_written_to_the_database`：把 `conn` 换成临时库里的一份，走完 UI 之后
  `db::list_repos` 读回来必须真有那行——只在内存里记一份的话重启就没了。

**反向变异**：
- 去掉 `find_repo_root`、直接用填进来的路径 → 只 `…way_in` 红（「填子目录没走到仓库根」）。
- 去掉那次 `db::add_repo` 调用 → 只 `…written_to_the_database` 红（读回来是空表）。
- 把入口按钮改名 → `…way_in` 与 `…refused…` 两条红，落库那条**不红**：它直接置 `add_open`，
  入口与登记各归一条判据管，这是故意的。
- 拒绝时顺手清空输入并收起 → 只 `…refused…` 红。
- **删掉 `is_repo` 那次复核：28 例一条都不红。**手上没有「`.git` 是残留文件」的 fixture，
  这一步只是照抄核心层定的规矩——它是契约锁，不是判别门，记在这儿。

**顺带撞出来的两个真问题**：
- `scratch_repo(tag)` 的目录名只有 `pid + tag`，两条判据撞同一个 tag 会互相 `remove_dir_all`，
  另一边正跑到一半的 `git init` 报「unable to get current working directory」。
  `an_empty_bookmark_list…` 第一次就是这么红的（它和 `open_repo("enter")` 撞了）。
  规矩写进了 `scratch_repo` 的注释：**tag 每条判据一个**。
- 确认按钮本来叫「添加」，但 `locate()` 用 `starts_with` 认文字，「添加仓库」是它的**前缀**且先画出来，
  于是判据永远点到入口而不是确认按钮。改名「打开」。**这不是界面选择，是无头 harness 的认字方式**——
  往工具栏加文案时要先想清楚有没有人和已有的字构成前缀关系。

**仍然没看到过的（这一轮的边界）**：
- 没有文件夹选择器（eframe 侧要引 `rfd` 之类的依赖），填的是**文本路径**；打错了只能靠那句「不是仓库」，
  路径不存在和「是目录但不是仓库」用的是同一句话。
- `db::add_repo` 的失败被 `let _ =` 吞了：库只读或磁盘满时内存仍然加了一条、界面照常打开，重启就没了。
  **没有任何判据会为此变红。**
- 「添加仓库」这枚按钮的位置是本层自己定的（SourceTree 那个「+」在书签窗里，这一层没有第二个窗口），
  没和 `docs/reference/` 对过量。
- 扫描文件夹（`scan_repos`）仍然没有入口：一次只能填一个路径。

## 本次：删掉 React + Tauri 那一层（`src/`、`src-tauri/`、Vite 配置）

**这一批的判据是什么**：一句话——**三条门禁全绿**（`gitcraft-core` 74 例、`gitcraft-app` 28 例、
egui 构建零警告），外加「`grep -rn "Tauri\|src-tauri\|npm" .github/ docs/` 只许剩一行——
bug_report 里那句『那一版已删除』的说明」。
删除类改动没有新判据可写：它不新增行为，只让行为少一半。**这本身就是这一批的边界**，
下面「仍然没看到过的」全部围绕这一点。

**为什么删**：那一层是功能最全的一层，但它的每一帧都要把 git 输出序列化成 JSON 跨过 IPC 再变成 DOM。
「大仓库里每帧做的活只跟窗口大小有关」这条目标（README 开篇押的『更好』的第一半）
正是被那一跳堵死的：一次 `git log` 回来 20MB 文本，DOM 只画得起一屏。
egui 壳已经量到过另一头：日志一帧排 11 行、提交从 40 条涨到 4000 条，每帧的活纹丝不动。
留着两套，等于每改一处交互语义要写两遍、判据也要写两遍。

**删之前必须先补的**：空书签列表在壳里没有任何入口（`repo_picker` 遇 `repos.is_empty()` 直接
`return None`，`db::add_repo` / `scan_repos` 以前只挂在 Tauri 命令上）。
上一节那三条判据就是为这个补的；没有那一步，删完得到一个打不开仓库的界面。

**改了什么（全部 8 处）**：
- `git rm -r src src-tauri index.html package.json package-lock.json tsconfig.json tsconfig.node.json vite.config.ts`
- `scripts/gates.sh`：六条 → **三条**（前端 tsc/vitest/build 与 `src-tauri cargo check --tests` 没了），
  收尾文案与「慢的那一条」注释改成 eframe 链接。
- `.github/workflows/gates.yml`：去掉整个 `frontend` job 和 src-tauri 步骤；macos-14 那一步加一条
  **字体缺失警告**（见下面边界第 3 条）。
- `.gitignore`：前端/Tauri 那几块换成一段注释说明本机的 leftover（`node_modules`、`dist`、`src-tauri/`
  仍挡着，方便你手上那份没删干净也不会误提交）。
- `gen-icon.mjs`：输出从 `src-tauri/icons/icon.png` 改到 `gitcraft-app/assets/icon.png`。
- `gitcraft-app/Cargo.toml`、`gitcraft-core/src/lib.rs:13` 的注释不再指向 `src-tauri`。
- README / CONTRIBUTING / 两个 issue 模板 / PR 模板：改成两层。
- **本地文件没删**：`src-tauri/`（含 5.5GB `target`）、`node_modules`（103MB）、`dist` 还留在盘上，
  只是不再被 git 跟踪。要清掉请自己 `rm -rf`——这批不替你删盘上的东西。

**核心层一点没动**：`gitcraft-core` 里没有 `tauri` 依赖（这是它从第一天就定下的规矩），
所以 `preview.rs` / `ssh.rs` / `update.rs` / `provider/` 整整齐齐留在那儿，测试照旧全绿。
它们现在是**没有调用者的函数**——不再死，因为是 lib crate 的 `pub` 项，编译器不会报，
`cargo check --tests` 也零警告。这正是「核心层是资产」的具体含义：界面换掉了，74 例判据一行没改。

**仍然没看到过的（这一批的边界）**：
- **226 例前端判据消失了，没有任何东西替代它们。**其中不少结论仍然成立但如今无人守：
  大补丁必须在 `diff --git` 边界折叠（按字节砍会留下半个 hunk，行号是假的）、
  窄列的路径省略必须换头部、UI 里不许出现 emoji、提交行高度只能有一个来源、
  「没有上游时绝不显示 `↑0 ↓0`」、push 的实参里不得出现加速节点串。
  这六条里只有两条还落在核心层：2 MiB 内容上限有测试，`git::push` 的签名里**根本没有** `node`
  这一位（`gitcraft-core/src/git.rs:1464`，所以它是契约锁而不是判别门）。
  其余四条**彻底没了**。这些已经逐条写进 README
  「壳里还缺的」和 CONTRIBUTING「好的第一 PR」，但它们现在是**文档**，不是判据。
- **删除的验证只能靠「剩下东西仍然绿」**，而绿不证明够。egui 壳覆盖的功能远小于被删的那层：
  没有 fetch/pull/push 按钮、没有分支/标签/贮藏/远程/设置面板、没有右键菜单、没有多选、
  没有键盘操作体系、没有文件溯源、没有拖放与扫描入口、没有补丁闸门、没有深色模式。
  **现在这个仓库里没有任何一套界面能替代 2026-10-19 之前的功能**，这是真的进度，不是修辞。
- **CI 上没有那份 8.3MB 的 CJK 字体**，缺它时 `.notdef` 的字宽正好是字号的一半，
  所以 `every_log_column_starts_at_its_own_header`、`diff_lines_share_one_left_edge`
  这几条**按宽度/左缘下的断言在 CI 上量的是产品里永远看不到的字体**。
  workflow 现在只打一条警告，不失败——把字体塞进 CI 要么增加 8.3MB 下载，要么把断言改成
  只比左缘不比宽度，两条路都还没走。本机是装了字体跑的，绿灯是装了字体的那种绿。
- 「本地 leftover 没删」这条是我刻意停手的位置：`rm -rf src-tauri node_modules dist` 不可逆，
  而且 `src-tauri/target` 里是你花过编译时间的东西。要清就自己敲。
- 这一批**没有目视核对过任何东西**。删除不改变界面，但下一批要往壳里加面板时，
  仍然欠一次「真窗口 + 参考图并排放」的比对（任务 #8 至今是 pending，原因写在台账里：屏幕锁着）。

## 本次：许可证定下来（MIT OR Apache-2.0）+ 第四条门禁「素材审计」

**决定与依据**：用户拍的板 —— 双许可、署名 `baorong and the GitCraft contributors`。
对标做法实测自 pdfCraft（`github.com/storytold/pdfcraft`，raw.githubusercontent 反复超时，重试后拿到
`LICENSE-MIT` / `LICENSE-APACHE` / `NOTICE` / `ATTRIBUTION.md` 四个文件的原文）：
它是「at your option」双许可 + 商标单独划出（`docs/brand/LICENSE-brand.txt`，要求分叉移除）+
素材逐条登记，**CI 里也有一条 asset-licence audit**。我们照这个形状做，但把「商标」和「截图」两条
按自己仓库的真实情况重写。

**为什么留两条许可证而不是抄一条 MIT**：Apache-2.0 的专利条款（§3）与终止条款（§9）和 GPLv2 不相容
（FSF 只认它兼容 GPLv3），所以纯 Apache 的代码进不了 GPLv2 项目；补一条 MIT 就把这条路留下来了。
反过来不成立：Apache 的代码不能降级进纯 MIT 项目（NOTICE 与专利授权会被丢掉）。
另外 Rust 生态默认就是 `MIT OR Apache-2.0`，`spdx` 这么写下游一眼看得懂。

**改了什么（8 处）**：新增 `LICENSE-MIT`、`LICENSE-APACHE`（**从 apache.org 逐字节取的原文**，
`cmp` 对过，含附录，一个字都没改）、`NOTICE`、`ATTRIBUTION.md`、`scripts/license-audit.sh`；
两个 `Cargo.toml` 加 `license`；`scripts/gates.sh` 从三条变四条（**审计逻辑只有一份实现**，
gates.sh 和 CI 都调同一个脚本，免得「本地绿 / CI 绿」判的不是同一件事）；
`.github/workflows/gates.yml` 加那一步；README 的「保留所有权利」段落换成双许可说明，
CONTRIBUTING 新增「许可证与素材」一节（提交 PR 即按同一天两条许可证授权，**刻意不写 CLA** ——
Apache-2.0 §5 天然管这件事）。

**四种条款要分清**，这是这一步真正的内容，不是走形式：
① 我们原创的代码；② `docs/reference/` 里那四张 **Atlassian 的截图**（专有、不是我们的，
只作为几何测量来源，再分发必须去掉或换成自己拍的）；③ **OFL 字体**（故意不入库，`OFL.txt` 入库）；
④ **「GitCraft」这个名字和 `gen-icon.mjs` 画出来的图标**——程序是代码（随双许可），
画出来的是**商标**（不在授权范围内，分叉必须改名换图标）。

**判据 = 第四条门禁本身**（`./scripts/license-audit.sh`：8 条 grep 断言 + 2 条存在性断言 +
字体不许入库 / `OFL.txt` 必须在）。反向变异跑了四次，每次只红预测的那一条，快照 + `cmp` 还原：

| 变异 | 预测 | 实际 |
| --- | --- | --- |
| NOTICE 里 `trademarks` 换成 `marks` | 只红「商标划出授权范围」 | 只红那一条 |
| 核心层 `Cargo.toml` 删掉 `license` | 只红「核心层要声明双许可」 | 只红那一条 |
| 把 `LICENSE-MIT` 挪出仓库 | 红「缺少 LICENSE-MIT」 | 红，且其余全绿 |
| NOTICE 里 `Apache License, Version 2.0` **被换行截断** | —— | **意外撞出来的真问题**：`grep -F` 认整行，排版换行会让断言恒红。第一次写完审计就是这样红的，改的是排版而不是判据 |

**仍然没看到过的（这一批的边界）**：
- **审计判的是「字串在不在」，不是「写得对不对」。** 把 `NOTICE` 里的商标条款换成一句意思相反的话，
  只要还留着 `trademarks` 这个词，审计照样绿。法律文本没有机器可判的正确性，这一条只能靠人读。
- 「字体不许入库」那条分支（`git ls-files --error-unmatch`）**只用替身验过**：
  真字体现在确实没入库，所以我只能用 `README.md`（已跟踪）证明这个判定会返回「在库里」，
  没有真的把 8.3MB 字体 `git add` 进去跑一遍。
- `ATTRIBUTION.md` 里第三方依赖的许可证是从**本机 cargo registry 的 `license` 字段**读的
  （egui/eframe/epaint/serde/serde_json/thiserror/chrono/tempfile = MIT OR Apache-2.0，
  rusqlite = MIT，SQLite = 公有领域）。**没有逐份打开上游 LICENSE 原文核对**，
  也没有覆盖传递依赖——真要审计到那一步得引 `cargo-about` / `deno licenses` 之类的工具，
  而它本身又是一个新依赖，先在台账里挂着。
- **截图这条是取舍而不是结论**：把专有软件的截图留在开源仓库里，业界常见做法是「仅作评论/测量参考」，
  但**没有任何判据能保证我们这样写就安全**。更保守的选择是把 `docs/reference/` 移出仓库、
  让贡献者自己截对照图——代价是全层几何常量失去了唯一的来源。用户如果要走保守那条，改的是一个决定，
  不是代码。
- 这一步仍然**没有目视核对**任何东西，也没有改动 `gitcraft-core` / `gitcraft-app` 的任何行为：
  74 例与 28 例判据一条没动，跑出来仍是 74 / 28 全绿。



## 本次：对照参考图把外壳几何全部重量一遍（行距 26 / 表头 24 / 列宽 / 表头竖线）

**决定与依据**：这一批不猜任何一个数。所有几何都从 `docs/reference/sourcetree-4.2.19-repo-window.png`
重量（2100×1090 px = **1050×545 逻辑 pt**，和我们 `WIN_W`/`WIN_H` 一模一样；这张是**聚焦**窗口，
选中蓝 `(41,98,217)`）。量出来的落点：

| 量到的 | 值 | 怎么量的 |
| --- | --- | --- |
| 数据行行距 | **26pt** | 行 y 中心 130→156→182→210 |
| 表头那一条 | **24pt**（比数据行矮 2） | 文字带 y 79..103 |
| 四列边线 | x **198 / 275 / 739 / 929** | 表头带内 1pt 宽、16pt 高、`(229,229,229)` 的竖线 |
| 列内缩 | **5pt** | 标签左缘 202.5 / 280 / 744 / 934，各比边线靠右 5 |
| 列宽 | 图表 **57**、提交 **77**、作者 **190**、日期 **121** | 边线相减 |
| 第 0 道中心 | 列左缘 **+22.75** | 1120 那张量圆点 239..246.5，列从 220 起 |
| 日志字色 | 普通 **(38,38,38)**、虚拟行 **(33,33,33)** | 逐像素取最暗的那档 |

**推翻了一条上一批的判定**：「表头没有竖线」是错的。当时的探针把整条带求平均暗度，
1pt 宽、16pt 高的浅灰线在 24pt 带里只占极小面积，**过不了阈值就等于不存在**。
错在探针，不在图。现在按「单列连续 16pt 都是 (229,229,229)」判，四条竖线全浮出来。
这条纠正写进了 `the_header_has_the_measured_column_rules` 的文档注释里，免得下次又「量」回去。

**改了什么（`gitcraft-app/src/main.rs`，判据从 35 例到 36 例）**：
① 常量块按上表重写；② `log_row` 四列改成「边线 + 内缩 5」而不是各自凑数，描述字色改成三档
（选中白 / 虚拟行 (33,33,33) / 其余 (38,38,38)）；③ `header_row` 整段重写——「图表」在自己列里居中，
其余四列走同一个循环，再补 4 条竖线 + 2 条通宽横线；④ 新增 `SIDE_FILTER_H = 4 + ROW_H`：
全局 `interact_size.y = ROW_H` 会**漏进侧栏底部那个输入框**，把侧栏画到 y=551，
`bands_match_the_measured_reference` 因此红。这一条是真 bug，不是断言太严；
⑤ 探针加 `head_band` / `rows_band` / `col_edges`——以后改行高不会把一堆 y 带常量一起带崩；
⑥ `scroll_log` 从 12 帧改 40 帧：egui 的 `smooth_scroll_delta` 是逐帧衰减的，12 帧只落 49pt 而不是 52，
差的 1pt 是它的尾巴不是我们的错（先量了 dy=25..78 才敢这么判）。

**判据 + 反向变异**（每次只红预测的那一条，`cp`/`cmp` 从 `/tmp/gc_ui_green5.rs` 还原）：

| 变异 | 预测 | 实际 |
| --- | --- | --- |
| M1 `COL_INSET` 5→8 | 只红「每列从自己表头起」 | 只红那一条 |
| M2 `GRAPH_PAD` 16.75→1 | 红圆点/线/环的位置 | **一条都没红**——所有断言比的是 `lane_x(...)` 自己算出来的值，常量改了什么都不判 |
| M2′（补）同上 | 加了「第 0 道中心 = 侧栏右 +22.75」的绝对断言后 | 只红那一条 |
| M3 `ROW_H` 26→21 | 红「每列从自己表头起」 | 只红那一条（行距常量被钉住了） |
| M4 删掉 4 条表头竖线 | 只红新加的那条门禁 | 只红那一条，35 例仍绿 |
| M5 删掉顶部横线 | 同上 | 只红那一条 |

**M2 是这一批最值钱的产出**：它和上一批「常量跟自己比」是同一类漏洞——**断言里有多少是判据，
只有把产品改坏才知道**。补的是绝对坐标，不是再引入一个常量。

**目视核对（补上一直欠的那一次）**：真窗口 1050×577（含 32pt 标题栏）摆在参考图旁边看过，
行距、列边线、表头竖线、第 0 道圆点、ref 药丸、暂存带都对得上。任务 #8 就此关掉。

**仍然没看到过的（这一批的边界）**：
- **表头竖线只有形状坐标的证据，没有像素证据。** 拿探针去实拍图上找「整列都是 (229,229,229)」，
  参考图侧被侧栏底色（也是 229 档）淹掉，我们侧一条都没有——egui 的 1pt 线落在半像素上会被抗锯齿
  摊开。所以「实窗里看得见竖线」这句是**眼睛判的**，不是量的。判据在 `Shape::LineSegment` 那一层。
- 虚拟行**没被选中时**的背景到底是什么，还是没定。只有一张聚焦（选中蓝 41,98,217）和一张失焦
  （220,220,220）的图，**分不清 (220,220,220) 是「失焦选中」还是「常驻灰带」**。现在按灰带实现，
  如果其实是前者，那是一条状态没建模。
- **第 1 道往右的间距没量过**：手上所有 fixture 都是线性或两道的历史，参考图那张也只有第 0 道有东西。
  `LANE_W = 12` 是估的，只有第 0 道的 22.75 是量出来的。
- **窗口宽度 ≠ 1050 时的行为没测过**。四列全是绝对 pt，窄窗口会把日期列挤出去而不是回流；
  SourceTree 会截断/让位。这一条要等做多仓标签页或侧栏可拖时一起处理。
- 26pt 是**参考图那个字号**下的行距。行高常量被钉住了，但用户改显示缩放时，钉住的是绝对值不是比例。
- **仍然没有真正的粗体**：`RichText::strong()` 在 egui 里只改颜色不改字面（没随包带 Bold face），
  所以参考图里虚拟行那种「加粗」我们只是加深了一档。

## 本次：侧栏照参考图重画（图标 + 47pt 分类 + 灰色药丸），顺手抓到一条假判据

**决定与依据**：把 `docs/reference/sourcetree-4.2.19-repo-window.png` 的左栏逐行量了一遍
（探针：对每个逻辑 y，报出 x 0..141 里最暗像素与暗像素的 x 范围）。量到的和上一批**目视核对的结论不一致**，
所以这一批的起点是「眼睛看错了」：

| 量到的 | 值 |
| --- | --- |
| 文字列左缘 | **x=41**，四组标签（WORKSPACE / 分支 / main / 文件状态）**全在同一列** |
| 图标格 | x **18..38**（中心 28），线描色 `(69,81,108)` |
| 分类行距 | **47pt**（202.5→275.5→322.5→369.5→416.5→463.5，五段等距） |
| 「工作区」那一条 | 30pt；子项 26pt（= 日志行距） |
| 选中子项的药丸 | x **10..129.5**、高 26、圆角 ~5、底色 `(204,208,205)` |
| 当前分支的圆点 | 圆心 x **29.5**、直径 7、**蓝** `(36,93,184)` |
| 分类标签色 / 子项文字色 | `(164,167,165)` / `(61,62,61)` |

**推翻的三件事**（都是我们自己加的，参考图上不存在）：
① 折叠三角 `▼`；② 「（3）」计数；③ 两档缩进（标题 5、子项 20）。
`CollapsingHeader` 整个从侧栏撤掉，换成自己排的行——**布局系统给不了「标签左缘 = 面板 x=41」这个约束**，
滚动区、内边距、item 间距每一层都会挪它，所以只能 `allocate_exact_size` + 绝对坐标。

**图标是自己画的**：`docs/reference/` 那四张是 Atlassian 的专有素材，图标不能描。
六枚（显示器 / 分支 / 标签 / 云 / 抽屉 / 蓝点）全是 18..38 格里的线描，1.5pt、`(69,81,108)`。

**判据 + 反向变异**（`cp`/`cmp` 对 `/tmp/gc_ui_green6.rs`，判据从 36 例到 44 例）：

| 变异 | 预测 | 实际 |
| --- | --- | --- |
| M1 `SIDE_TEXT_X` 41→20 | 钉死常量的那条红 | 只红 `the_sidebar_constants_are_...` |
| M2 `SIDE_CAT_H` 47→26 | 同上 | 红**两条**：常量表 + `..._step_by_47pt`（它抄的是实测字面量 47.0/26.0） |
| M3 药丸不画 | 只红药丸那条 | 只红那一条 |
| M4 分类不画图标 | 只红图标那条 | 只红那一条 |
| M5 蓝点改纯黑 | —— | **一条都没红**：判据按 `SIDE_DOT_C` 筛圆，常量一改筛子跟着改。补了颜色钉（五个色一起比字面量）后重跑，只红常量表 |
| M6 `SIDE_PILL_R` 129.5→136 | 只红常量表 | 只红那一条 |
| M7 把「（1）」加回标题 | 只红计数/三角那条 | 只红那一条 |
| M8 提示条改回吃掉整行 | 只红提示条那条 | **第一次一条都没红**——见下 |

**M8 抓到的是一条假判据，不是产品 bug**：我原本判「提示条右缘 ≤ 后面控件的左缘」。
旧写法把整行吃掉时，后面的控件被**推到更右边**，于是「不重叠」照样成立。
补上「提示条自己的宽度 ≤ `NOTE_W`」才判得住（跑出来 323.66 > 240）。
**「A 不压住 B」这种判据必须连 A 的尺寸一起钉**，否则 B 让位就能骗过去——和 M2、M5 是同一类：
**判据里出现常量，就得有人在常量层钉一次**。

**目视核对**：真窗口 1050×577 摆到参考图旁边看过侧栏（图标、药丸、蓝点、47pt 分类、同一列文字全对上），
也正因为这次是把窗口提到最前重拍，才发现**工具栏那条「egui 版还没接」提示压住了书签下拉**——
上一批记的「目视核对通过」里没看到它，因为那条提示要点了侧栏才出现。

**仍然没看到过的（这一批的边界）**：
- **图标画得像不像，没有任何判据。** `icon_ink` 只判「18..38 那格里有墨」，把云换成一个方块它照样绿。
  形状对不对，目前只有我这一双眼睛。
- 侧栏**滚动过**的样子没量过：分类改成 47pt 之后整栏高了 100 多pt，仓库一多就要滚；
  参考图那张只有一条分支，滚动的样子它没给。
- 「工作区」这个词参考图里是**没翻译的大写 WORKSPACE**。我们译了。这是刻意偏离契约还是照抄，
  还没定——照抄等于抄一个本地化 bug。
- 分类**没有子项时**（标签 0、已贮藏 0）参考图和我们的表现一致，但**筛选器筛空了**的时候
  分类标题该不该淡下去，两边都没依据。
- 蓝点/药丸在**失焦窗口**下的样子没判：手上另一张 2026-10-20 的失焦截图里选中是灰的，
  侧栏药丸本来也是灰的，**分不出失焦时它会不会变**。
- `NOTE_W = 240` 是**算出来的预算**（右侧 11 枚按钮 473pt + 左边几个控件），不是量出来的；
  工具栏文案一改它就可能不对。窗口更窄时这条提示会先挤掉谁，没测。

## 本次：日志行去掉 hover 底色 + 修掉「有时候点不中」（2026-10-20）

敏捷模式：这一批只补了两条判据（都是反向变异验过的），其余留给后面统一补。

**去 hover**：`log_row` 里那条 `resp.hovered()` 分支删了。整条带子跟着鼠标闪，
反而让人以为「亮起来那行才是我点到的」，而它和真正命中的行并不总是一致。
`HOVER` 常量留着——工具栏按钮还在用。

**点不中查出来两条，都是真丢点击**（用一次性无头探针扫出来的：沿着一行扫 x、扫 y、
滚完隔 N 帧再点，跑完就删）：

1. **滚轮的下一帧就点，点击整个丢掉。** 探针：`settle=0` 什么都不选，`settle≥1` 才选中。
   落点是 egui `interaction.rs:117-123`——松开那一帧在注册表里找不到按下的 Id，
   `potential_click_id` 直接清空。改法：日志行的 Id 由**这一行代表哪条提交**发
   （`log_band` 里那段 `push_id`），不吃 `show_rows` 的槽位 Id。
   `scroll_area.rs:1113` 的 `skip_ahead_auto_ids` 本该让槽位 Id 跨帧稳定，
   **实测滚动刚开始那一帧仍然对不上**——这条我只复现了现象，没读通它为什么不稳。
2. **按住 0.8 秒以上才松手，不算点击。** egui 默认 `max_click_duration = 0.8`，
   超时把这次按下转成拖拽。`style()` 里放到 3.0。慢一点的点击、按住看一眼再松手，都不该丢。

**探针顺手量到的两条事实**（没变成判据，记下来）：
- 行与行**没有死带**：216..239 全落第 3 条、240..265 第 4 条、266..275 第 5 条，26pt 一格接得上。
- 一行右端 **1036..1050 点不中**——那是竖滚动条占的那 14pt。行宽用的是滚动条**外面**的
  `available_width`，所以选中蓝带铺到窗口边（和参考图一致），代价是最右一小条归滚动条。
  日期列文字在 994 就收笔了，实际点不到那儿，先不动。

**判据**：`a_click_on_the_frame_right_after_the_wheel_still_lands`（四档 settle）、
`a_slow_click_is_still_a_click`。反向变异：删 `push_id` → 第一条红（settle=0 那档）；
`max_click_duration` 改回 0.8 → 第二条红。46 例全绿、零警告。

**目视核对**：真窗口 1050×577 提到最前拍过一张（`/tmp/gc_click1.png`）——侧栏图标、
47pt 分类、灰色药丸、日志四列、蓝点都在位，日志区没有 hover 带。
**但点击这件事真窗口里我没验**：这台机器上没法合成鼠标事件（`cliclick` 没装、
python 没 Quartz），只有无头帧的证据。

**仍然没看到过的**：
- 上面第 1 条的**成因**只到现象层。槽位 Id 为什么在滚动那一帧不稳，我没读出来。
- 慢点击 3.0 秒这个值是我定的，参考 SourceTree 没有依据；按住超过 3 秒再松手仍然不算点击。
- 侧栏、文件行也吃同一条 `max_click_duration`（改在 `style()` 里是全局的），
  但**只给日志行换了稳定 Id**——侧栏和文件行有没有同样的「滚动后第一帧点不中」，没测。

## 本次：日志滚到底接下一页（2026-10-20）

现象是「滚到底没反应」。根因不在渲染，在**取数**：`snapshot_of` 写死 `git log --max-count=500`，
一页取完就再没有第二页——而 `comic` 有 **4400** 条提交，所以列表底下那 3900 条根本不在内存里。
绘制层一直是虚拟化的（`show_rows`），所以这不是性能 bug，是**分页缺失**。

- core：`log_args` 多一个 `skip` 参数，`commits_page(repo, skip, max, topo)` 走 `--skip`。
  第一页仍然由 `commits`/`commits_topo` 取（`skip=0` 时连 `--skip` 都不发）。
- app：`Gitcraft` 多三个字段——`log_more`（还有下一页吗）、`log_more_busy`（在途）、
  `log_seq`（分页序号）。`open()` 一次 `log_seq += 1`，于是切仓库/换排序之后**在途的旧页**
  回来对不上号，整页丢掉；`Msg::Loaded` 按「首屏满不满一页」拧 `log_more`，
  不满就再也不问（小仓库每帧都「到底」，不然会一直白跑 git）。
- 扳机在 `log_band`：`show_rows` 给的 `range.end >= total` 就是视口追上列表尾。
  只能记在局部 `need_more` 上——`snap` 还借用着 `self`，闭包里碰不了 `self.log_*`。
- 接页用 `append_page`：`commits` 尾尾相接，`graph`/`badges`/`dates` **整列重算**。
  不做增量——增量要维护「车道跨页延续」这种状态，而 500 条重排也就几毫秒。
  `taken_at` 沿用旧值，所以已显示行的日期不会因翻页而跳。
- `Snapshot` 因此**多存一份 `remote_refs`**：重算药丸要认得远程指针，否则下一页的
  「超前 N 个版本」会掉。

**判据三条**（都过了反向变异）：
- `the_page_offset_only_adds_skip` + `stage_and_commit_flow` 里加的两行：`--skip` 只多这一个参数、
  第二页和第一页不重叠。变异：`if skip > 0` 改恒假 → 两条一起红。
- `reaching_the_bottom_of_the_list_asks_for_the_next_page`：到底就发、已经到底就不发。
  变异：`range.end >= total` 改恒假 → 红。
- `a_second_page_extends_every_parallel_column`：接页时三列派生数据一起长齐、旧序号的页丢掉。
  变异：删 `snap.dates = …` → 红（3 vs 5）；删序号判断 → 红（6 vs 5）。
- `the_bottom_of_the_list_pulls_the_next_page_from_real_git`：端到端。四条提交的临时仓库，
  把列表腰斩成两条当「第一页只取回一半」，之后由**帧里的扳机**起线程、真跑
  `git log --skip=2`、回复真落回列表，再断编号不重叠。

core 76 例 + app 49 例全绿、零警告、`scripts/gates.sh` 四条门禁全绿。

**目视核对**：重启了进程（旧 44105 → 新 82175），真窗口 1050×577 拍到 `/tmp/gc_page1.png`——
`comic` 的日志、蓝点、`main 超前1个版本`/`upstream/main`/`origin/main` 三枚药丸、选中蓝带、
下面文件表与差异区都在位，翻页改动没碰坏几何。
**但「滚到底接上第二页」这一步真窗口里仍然没验**：这台机器的滚轮事件是能合成的
（`ctypes` 直调 CoreGraphics `CGEventCreateScrollWheelEvent`，脚本 `/tmp/gc_scroll.py`），
指针也 warp 得动；我把滚轮发到日志带时，**前面是用户的 Chrome，正跑着一个 91% 的 yum 安装**，
事件落进了别人的会话。往别人正在用的会话里灌输入是我不该做的，所以立刻停了手，
改成只读核对：`git log --max-count=500` 的末条是 `f8aeaa14`，`--skip=500` 的首条是 `8bee57e2`，
`git log --oneline --skip=499 --max-count=3` 里这两条正好相邻——分页边界在真仓库上接得上。

**仍然没看到过的**：
- 真窗口里滚到底那一瞬间的画面（滚动条短截、新行接上来）没看过，只有无头 + 只读 git 的证据。
- 翻页期间用户**提交/切分支**：`log_seq` 会挡掉旧页，但「新页正好接在一条刚提交完的列表上」
  这种边界没验。
- `git log --skip` 在两次取页之间历史变了（比如后台 fetch 之后），边界会漂一条 ——
  这是所有分页历史共同的取舍，core 的注释里写了，没有额外处理。
- 首屏 500 条这个数仍然没依据。在 `comic`（4400 条）上量过一次：
  一页 500 条 `git log` ≈ 22-30 ms，整份 4400 条 ≈ 70 ms——**进程启动就占掉大半**，
  所以分页省的主要是内存和排版时间，不是 git 时间。500 这个值还是拍的。
- 翻页**没在真窗口里被用户手势触发过**（见上），扳机的「真滚到底」只有 `show_rows`
  的 `range` 语义 + 无头断言撑着。

## 更正：「点不中」真正的根因是 selectable label（2026-10-20，用户指出）

上一批记的两条（槽位 Id 不稳、`max_click_duration = 0.8`）都是真的，但**都不是主因**。
用户一句「移动到文字上，鼠标变成插入状态，怪不得点不中」直接指到病根：

**egui 默认 `Interaction::selectable_labels = true`（style.rs:1599）**。开着文字拖选的
`Label` 会给自己叠一层 `Sense::click_and_drag()`（label.rs:143-170），并把光标设成
`CursorIcon::Text`（label_text_selection.rs:645）。于是：

1. **日志行**：描述列是唯一的真 `Label`（提交号/作者/日期/药丸都是 `Painter::text`，没有这问题）。
   那块矩形压在整行 `Sense::click()` 的**上面**，按下被 `LabelSelectionState` 抢去开始拖选，
   行的 `resp.clicked()` 永远不响 —— **鼠标落在描述文字上就点不中**，落在空白处才中。
   改：`log_row` 里 `Label::…truncate().selectable(false)`。
2. **侧栏行**：`side_row` 同一个形状（整行 `Sense::click()` + 上面盖一块可选文字的 Label），
   同样改 `selectable(false)`。
3. **文件表（比上面两条都严重）**：那两处写的是 `Label::selectable(on)`，`on` 想表达
   「这一行是当前文件」，但 egui 的 `selectable` 只管**文字能不能拖选**：
   `false` 时那块矩形只剩 `Sense::hover()` → `clicked()` 永不响。
   刚打开仓库时**所有**行都没选中，所以**整列文件一个都点不动**。
   改：`selectable(false)` + `sense(Sense::click())`，`section` 的 `picked` 参数随之删掉。
   「哪一行是当前文件」的底色带**没有**跟着回来 —— 那个 `selectable(on)` 本来也没画出任何底色，
   现在仍然只有差异区那格标题写着文件名。留给暂存区那一批（任务 #20）。

**判据**：`a_click_on_a_rows_text_selects_the_row`（点在描述文字上 → 选中那一行；
点在侧栏「历史」文字上 → 跳到第一条提交）、`a_click_on_an_unpicked_file_row_switches_to_it`
（点未选中的文件行 → `self.file` 真的换过去）。反向变异：`selectable(false)` 改回 `true`
→ 第一条红（日志那一格）；侧栏那一格单独改回 `true` → 同一条判据在第二个断言红；
文件行改回 `selectable(on)` → 第二条红。51 例全绿、`scripts/gates.sh` 四条门禁全绿。

**顺手记下的 egui 事实**（下次别再猜）：`Label::selectable` ≠ 「选中样式」，它只控制拖选；
`Label` 也没有 `selected` 构造器（0.37 只有 `selectable`/`sense`），要选中态自己铺底。
`Sense::hover()` 的响应 `clicked()` 一定是 false。

**仍然没看到过的**：差异区的文字**故意**留着可拖选（复制代码要用），所以那里光标仍是插入态 ——
真窗口里没核对过「差异区能选字、日志区不能」。工具栏/底部那几处普通 `ui.label` 也还是插入态，
它们不是点击目标，先不动。

## 顶部栏重排 + 仓库 tab + 提交框/差异区（任务 #22/#23/#24，2026-10-20 深夜）

诉求三条：顶栏有重叠、仓库列表摊成 tab（点一枚开一个仓库）、提交内容与改动详情照 SourceTree 排。

**重叠是真的，而且量得出是哪两对**：无头把每个控件的矩形收下来两两求交，抓到
`书签 ▼` 压在 `⤴` 上、`添加仓库` 压在 `≡` 上。根因不是按钮太宽，是**第一行被塞了四样东西**
（仓库名 + 分支 + 11 枚按钮 + 提示语/添加仓库那格）。`TOOLBAR_H = 55` 的带里，11 枚 43pt 的按钮
单独占掉右侧 ~470pt，剩下的预算给不了左侧两格文本，只能溢出叠上去。改法是把提示语和
「添加仓库」整格搬到第二行（`tab_bar`），第一行**只留参考图有的东西**。判据
`the_toolbar_row_has_no_overlapping_widgets`（任意两个可交互矩形不相交）。

**tab 条这 24pt 没有依据，别假装量过**：SourceTree 4.2.19 把书签放在**另一个窗口**
（书柜），仓库窗口里没有 tab 条这一层，两张参考图都翻不到。所以 `TAB_H = 24.0` 是自己定的，
它只有一条判据撑着功能：`a_repo_tab_opens_that_repo`（点 tab → `self.repo` 真换成那条书签的路径）。

**提交框**：参考图那一格是**两个框**（摘要 + 描述），都在文件表下方、都靠左，落进 git 是一条 `-m`、
两框之间隔一个空行。判据 `the_description_box_joins_the_summary_with_a_blank_line` 走真仓库端到端：
两框敲字 → 点「提交到 …」→ 用 `git::commit_message()`（`%B`）把信息读回来逐字比，
断的是 `一行标题\n\n第二行说清楚为什么`。变异：`\n\n` 改 `\n` → 红。

**差异区那格标题带**（这次唯一真按像素量的一层）：两张参考图都是 **(236,236,236) 高 30pt**
（一张 y338..368、一张 y378..408），色块离带左缘 **8pt**（585-577、665-657），状态色块 **14×14**
（琥珀块两张图都是 14×14）。三个颜色也全是数出来的：改动 (238,197,91) 680 个像素、
重命名 (163,115,122) 242 个、未跟踪 (101,73,129) 只在 2026-10-20 那张里，302 个。

**写错的三个数字**（全是我上一批拍的，这次统一按量到的改）：`BADGE_S` 17→14、
`DIFF_HEAD_H` 28→30、色块内缩 6→8。为防下次再拍，加了一条只钉常数本身的
`the_diff_header_constants_are_the_measured_reference_numbers`，注释里逐条写清「这个数出自哪张图、
哪一段 y」，包括两处**没有**参考图可对的诚实交代：`PICK_BG` (220,220,220) 实际量的是**日志区**
第一条选中行（y109..128），文件表那两张图里根本没有选中行；`COMMIT_H = 96` 是自己给提交框的预算，
参考图那张只露出摘要一行，量不到三行描述该多高。

**顺手抓到两条真 bug**：

1. **hunk 头有两份翻译**。`layout()`（工作线程）已经翻成「块 N： 行 A-B（新 C-D）」，
   帧里 `diff_pane` 还揣着另一份自己的规则（`hunk_label` + 一个每帧归零的计数器），画的是原始 `@@`。
   判据 `a_hunk_row_reads_as_a_block_number_and_line_range` 一挂就露了。删掉帧里那份，
   `layout` 是唯一出口。变异：`layout` 不翻译 → 红（顺带 `diff_lines_share_one_left_edge` 也红）。
2. **提交框整个被顶出窗口**（真窗口目视抓的，不是无头）。文件一多，`file_list` 里那张表顺着
   自己的自然高度往下长，把下面的提交框推到 `WIN_H` 之外 —— egui 按裁剪矩形裁掉越界的 text shape，
   所以「提交到」那行字**从 `out.shapes` 里消失**，这正好让判据可反向验证。改法是 SourceTree 的关系：
   「列表滚，两头钉」——`待定的文件…` 那行和提交框都不跟列表滚，中间用
   `ScrollArea::max_height(可用高 - COMMIT_H)` 滚。变异：`list_h` 多加 400pt → 红，报的就是
   「提交按钮掉到窗口外面（列表把提交框挤下去了）」。

**判据**：本批 5 条（常数带、差异带头带、hunk 文案、当前文件灰带、提交框在带内）+ 端到端那条。
7 次反向变异逐条对红，其中 `rect_of(ui, DIFF_HEAD_H + 12.0)` 只让几何判据红、常数判据照样绿 ——
两层不是冗余，一层钉字面值、一层钉画出来的东西。59 例全绿、`scripts/gates.sh` 四条门禁全绿。

**仍然没看到过的**：

- **本批的真窗口目视欠着**。进程活着、窗口在（7037，946×520，`CGWindowBounds` 报 X247 Y115），
  但 `screencapture` 连拍三张整屏**全是 (0,0,0)**，按 id 抓单窗口报 could not create image ——
  凌晨这台机器的显示器睡了，不是代码的问题。上一批那张能拍的（`/tmp/st_visual.png`）恰恰抓到的是
  提交框被切掉的画面，**修好之后的画面一张都没有**。上面所有几何只有无头 + 像素量的证据。
- tab 条**没有横向溢出处理**：书签一多就往窗口外排，既不会滚也不会收成「+N」。
- 差异区缺参考图右上角那条「搜索」，缺 hunk 上的「暂存区块」/「放弃区块」两枚按钮；
  文件表缺每行前的复选框（#20 剩下的部分）。
- 当前文件那枚灰带没分「窗口有没有焦点」：SourceTree 失焦时是灰 (220,220,220)、有焦点是蓝 (0,100,225)，
  现在一律灰。
- 量到但**没采用**的两个数：底带上沿那条 24pt 的 (240,244,241)、差异正文那一格的 (240,244,241)。
  等目视过再决定，别把没核对的底色铺上去。

## hunk 级暂存/放弃：两枚按钮 + 三个底色改对（任务 #20 的第一半，2026-10-21 凌晨）

**量的**（`docs/reference/sourcetree-4.2.19-repo-window-2026-10-20.png`，先裁出来用眼睛看，再逐像素扫）：

- hunk 头右端两枚按钮：x 977..1032、1041..1096，那条带的右缘 1105 → **55×15、中间 9、离右缘 9**。
  文案是「暂存区块」「放弃区块」，已暂存那一格换成「取消暂存区块」且**没有**放弃那枚。
- hunk 头那句话是「块 1： 行 569-575」—— **只报一个区间**（旧侧），不带「（新 …）」。
- 三条底色是从被删掉的 React 层继承下来的估值，这次照图改对：`ADD_BG` (232,253,242)、
  `DEL_BG` (255,238,234)、`HUNK_BG` (240,244,241)。描边 (228,232,229)、按钮字 (109,110,109)。

**核心层**（`gitcraft-core/src/git.rs`）：`hunk_patch`（从补丁里抠第 N 块，连它自己那格文件的头）、
`run_git_stdin`、`stage_hunk`（`apply --cached`）、`unstage_hunk`（`apply --cached -R`）、
`discard_hunk`（`apply -R`，注释里写明**这条真删用户没提交的行**）。核心 80 例。

**两条偏离，都是故意的**：

1. **hunk 那一条带画成 15pt，参考图是 29pt（双倍行高）**。`ScrollArea::show_rows` 只认统一行距，
   要按行给不同高度得自己写虚拟化。先把 29 记在这儿，等文件表/日志那一层一起做虚拟化时替换。
2. **「放弃区块」用两击内联确认（第一下翻成「确认放弃」），没抄 SourceTree 的模态框**。
   这一步删的是工作区里没提交的行，代价不可逆；而那个模态框我们还没画过、也没目视核对过它的样子。
   判据 `a_discard_needs_two_clicks` 同时钉「第一下把按钮武装起来」和「第一下之后文件一个字节没变」。

**抓到一条假判据（记忆里那类「常量比自己」）**：`the_hunk_row_has_two_measured_buttons_at_the_right`
第一版断的是 `near(r.width(), HUNK_BTN_W)` —— 把 `HUNK_BTN_W` 从 55 改成 60，画的那头和断的那头一起挪，
判据**照样绿**。全部改成字面量（55/15/9/9/(240,244,241)），另开
`the_hunk_button_constants_are_the_measured_reference_numbers` 去钉常量的出处。两层不冗余：
一层钉画出来的东西，一层钉字面值。

**8 次反向变异逐条对红**：

| 改坏什么 | 哪条红 |
| --- | --- |
| 去掉 `line.kind == Kind::Hunk` 守卫 | `the_buttons_follow_which_pane_the_diff_is`（数出 12 枚） |
| `HUNK_BTN_W` 55→60 | 几何判据 + 常数判据 |
| `HUNK_BTN_H` 15→19 | 几何判据（报「找到 []」：19pt 的矩形装不进 15pt 的带） |
| `hunk_ops` 允许 `untracked` | 归属判据（未跟踪那格长出按钮） |
| 第一下就真丢 | 两击判据 |
| 文案永远单区间 | `a_hunk_row_reads_as_a_block_number_and_line_range` |
| 文案永远带「（新 …）」 | `an_equal_length_hunk_reads_as_a_single_range` |
| `stage_hunk` 打整份补丁 / `unstage_hunk` 去掉 `-R` | 各自那条核心判据 |

**顺手记下的一次漏红**：去掉 `Kind::Hunk` 守卫之后，`…_two_measured_buttons_…` **依然绿** ——
它只数「hunk 那条带里的两枚」，看不见块里另外五行也各长了两枚。这一条的唯一目击者是归属判据，
所以整组文案那种数法（`sort()` 后整体比对）不能退回逐词 `contains`。

**egui 事实（0.37，下次别再猜）**：`egui::Button` 的高度由 `spacing.button_padding` 说了算，
10.5 号的字撑出 19pt，钉不到 15pt —— 只能自己画（`layout_no_wrap` + `rect_filled` + `rect_stroke` +
`galley` + `allocate_rect`）。`Ui::fonts(|f| …)` 给的是 `&FontsView`，里面的 `layout*` 要 `&mut self`；
`Painter::layout_no_wrap` 是 `&self`，量字宽走 painter 这条路。`ui.interact(rect, Id::new(..), ..)`
的 `Id::new` 在 0.37 已弃用，零警告门禁会红，改 `ui.allocate_rect(rect, Sense::click())` 走 auto id
（和这一格其他行同一套 id 规则）。

**仍然没看到过的**：这两枚按钮的落点只有无头证据，真窗口目视还欠着（#25，显示器睡了那件事没解决）；
第二下之后没有任何提示，丢了几行用户不知道；差异区右上角那条「搜索」还没有；文件行前的复选框（#20 剩下的）；
按**行**暂存没做 —— `line.hunk` 在块里每一行都留了值，就是给这一步准备的。

app 65 例、核心 80 例全绿，`scripts/gates.sh` 四条门禁全绿。

## 文件表的方框列、状态图标列、分类行的全选框（任务 #20 的第二半，2026-10-21）

**量的**（同一张 `sourcetree-4.2.19-repo-window-2026-10-20.png`，裁出文件面板那一竖条逐像素扫）：

- 面板左缘 **141**（分隔线在 140..140.5，色 (217,220,217)）。以下都从这个左缘起算。
- 分类行：方框 x 149..162.5 → 列偏移 **+8**、**13.5 见方**；标签从 169.5 起 → **离方框 7**。
- 文件行：方框 x 166..179.5 → **+25**；状态图标 x 188..201.5 → **+47**，也是 13.5 见方；
  文件名从 209 起 → **+68**；行末那枚「⋯」x 519..534.5；面板内容右缘 560.5。
- 行距 **27**（三行量到 27/27/27）。**这条没采纳**：`ROW_H = 26` 和日志行共用，
  单改文件表就得先把行高拆成两个常量，等虚拟化那一批一起做。
- 勾上的蓝：中心色 (71,141,247)。**未勾选的方框两张参考图里一枚都没有**（能看见的全是勾上的），
  所以 `CHK_EDGE` (190,190,190) 是**估的**，下面单独说它钉不住。

**画的**（`gitcraft-app/src/main.rs`）：`check_box`（13.5 见方、圆角 3，勾上铺蓝 + 两段白勾线，
没勾铺白 + 1pt 描边）、`file_row` 多收一个 `check: Option<bool>` 并返回 `(点了整行, 点了方框)`、
`section()` 的分类行不再用 `ui.checkbox`——egui 那枚跟着字号长（11.5 号撑出 18pt），
三列对不齐，而且它的 `changed()` 把「整组勾掉」和「用户点了别的」混成一个信号。
文件面板的边距从 `Margin::symmetric(4, 2)` 改成 `(0, 2)`，不然 `rect.min.x` 是 145，
跟量到的 141 差 4，每一列都得各减 4 —— 去掉边距让常量直接对得上参考图。
提交面板那份文件列表传 `check = None`（那边没有方框列）。

**行为契约**：点行 = 选中（差异区切过去），点方框 = 只暂存/取消暂存这一个文件，
分类框 = 整组。端到端两条：`ticking_a_file_box_stages_only_that_file`、
`the_section_box_toggles_the_whole_group`，都在临时 git 仓库上跑真 `git add/reset`。

**这一批最重要的一条：判据必须问「应用状态」，不能问 git 状态。**
两条端到端一开始全超时，写了一个临时调试例打坐标，发现点击**确实**把 b.txt 暂存了
（`staged=Ok(108)`），是**第二下点空了**：`pump` 原来等的是 `git::working_patch(...)` 变了，
而索引在 worker 线程回快照之前就先变，于是等到的那一帧里 `Snapshot` 还是旧的，
第二次点击落在过期那一行上（行已经不属于它了）。改成等 `staged_in_app(app)`（从应用自己的
快照里读），两条立刻绿。删掉调试例。这条适用于**所有**动作后的连续点击：等应用看得见的那份状态。

**又抓到一条假判据，这次是「多余的守卫没法被证伪」**：`file_row` 里原本写
`resp.clicked() && !toggled`，注释说 egui 会把一次点击同时给压着的两层。变异 M4（删掉 `&& !toggled`）
**全绿**——0.37 的命中裁决把点击只给面积更小的那枚，注释写的那个机制不存在。
于是删掉守卫（不是补判据去钉一行死代码），并把注释改成实测结论：
`ticking_a_file_box_stages_only_that_file` 和 `a_click_on_a_rows_text_selects_the_row` 钉的是行为，
两条在有无守卫下都绿，所以它们本来就是这一格真正的合同。

**`rects()`/`rounded()` 看不见空心方框**：两者都按「底色非透明」筛，M8（`CHK_EDGE` 190→120）全绿。
一半是没法子——那个颜色没有参考样本；另一半有法子：描边**有没有画**能问。补了
`stroked()`（`fill` 透明 + `stroke` 非透明 + 圆角非 0）和
`an_unchecked_box_has_a_border_and_a_checked_one_does_not`：三枚白方框必须各有压着的描边，
两枚蓝方框必须没有。M9（删掉 `rect_stroke` 那行）只有这一条红。

**9 次反向变异逐条对红**：

| 改坏什么 | 哪条红 |
| --- | --- |
| M1 `CHK_ROW_X` 25→8（两栏挤成一栏） | 列位置判据（「该是两白一蓝」，全塌到 x 149）+ 分类行判据（数出 5 枚） |
| M2 标签离框 7→2 | 只有 `the_section_box_sits_in_its_own_column` |
| M3 `ICON_X` 47→40 | 列位置判据 + 常数判据 |
| M4 删 `&& !toggled` | **全绿 → 判定为死代码，删掉**（见上） |
| M5 分类框方向反了 | 只有 `the_section_box_toggles_the_whole_group` |
| M6 `CHK_S` 13.5→16 | 常数判据 + 列位置判据 + 分类行判据 |
| M7 `CHK_BLUE` 蓝 247→200 | 常数判据 + 列位置判据（那条按颜色分档） |
| M8 `CHK_EDGE` 190→120 | **全绿**：`rects()` 只看 `fill`，且这色是估的 |
| M9 删掉 `rect_stroke` 那行 | 只有新的描边判据 |

**egui / 脚手架事实（下次别再撞）**：自己 `frame_events` 出来的 `FullOutput` _drop_ 前必须
`out.textures_delta.clear()`，不然 epaint `textures.rs:333` 那句 `debug_assert!` 崩
（「Dropped TexturesDelta with 1 unapplied deltas」）；`click_box_in_row` 里踩过。
`allocate_exact_size` 拿到的 `resp` 和后面 `check_box` 里的 `allocate_rect` 都要 `&mut ui`，
所以 paint 顺序只能「先分配行 → 画带 → 问方框 → 再 `ui.painter()`」，
把 `let p = ui.painter()` 提到前面就 E0500（闭包重复借用）。
高负载下的偶发失败要认出来：`a_discard_needs_two_clicks` 并行跑全量时红过、单跑必过——
600 帧泵不够是因为 CPU 让给了别的测试，worker 线程回快照回得慢，不是代码缺陷。

**仍然没看到过的**：这三列的落点仍然只有无头证据，真窗口目视照旧欠着（#25；
`screencapture` 连拍三张整屏全是 (0,0,0)，显示器睡这件事还没解决）；
行末那枚「⋯」没画（要先有弹层）；状态图标目前是一枚纯色方块，参考图那种「改动 + 未跟踪」
重叠两枚的形状还没做；未勾选方框的边框色是估的。

app 71 例、核心 80 例全绿，`scripts/gates.sh` 四条门禁全绿。

