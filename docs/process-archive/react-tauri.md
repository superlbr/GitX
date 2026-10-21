# 台账归档：React + Tauri 那一层（2026-10-08 ~ 10-20）

从 `PROCESS.md` 精简时原样搬来，一行没改（原文 = 提交 `57faab6` 之前的 PROCESS.md 第 20–2043 行）。
那一层已在 2026-10-20 删除，最后一份代码在提交 `9a3077c`；这份归档是「哪条判据为什么存在、哪个数字从哪里量出来」的记录。
捡功能回 egui 时从那儿取源码，也照抄它的判据。参考对象 t8y2/dbx、`highlight.js` / vitest / localStorage 都是当时的栈。

---

## 已完成（Progress）

### 布局与结构
- [x] 顶栏（TopBar）：仓库信息 + 同步控件 + ⚙️ 设置（全局，弹窗）（主题切换已于 2026-10-15 移除，亮色唯一）
- [x] 左侧仓库侧栏（Sidebar）→ 已于 2026-10-15 删除，入口全部并入 RepoTabs（tab 条 + 「+」菜单 + tab 右键）
- [x] 删除当前仓库后右侧状态全部清空（2026-10-08）
- [x] **SourceTree 风格布局（2026-10-10）**：
  - 移除过宽的首页顶部 tab 栏，改为左侧**分组导航**（工作区：文件状态/贮藏；仓库：历史/分支/远程），默认视图即「文件状态」分组+列表
  - 分支视图加入 **tabbar：本地分支 / 远端分支 / 标签**，对应 SourceTree 的分支面板
  - 底部状态栏：仓库名 / 当前分支 / 文件变更数 / 提交数 / 最近操作
  - 标签页恢复：持久化当前仓库与激活 tab（localStorage），重启后自动还原

### 差异视图（DiffView）
- [x] 解析 unified diff，按文件分组渲染
- [x] 按扩展名推断语言，`highlight.js` 语法高亮（上下文/独立增减行）
- [x] 相邻 `-`/`+` 行配对，单词级（word-level）diff 高亮（github-dark 主题）
- [x] 多文件 diff 虚线分隔，空 diff 提示

### GitHub 加速克隆
- [x] 粘贴地址自动探测连通性，自动选最优节点
- [x] 加速节点仅用于初始 fetch，克隆后 origin 改回真实 GitHub 地址
- [x] 节点延迟记忆：检测后持久化最低延迟节点，下次默认优先；显示「上次使用 · Xms」
- [x] 设置内可配置节点模板（`{url}` 占位）

### 参照 dbx 的交互优化（2026-10-08）
- [x] **危险操作二次确认**：通用 `ConfirmDialog` + store `requestConfirm()`
  - 移除仓库、删除分支、丢弃贮藏、硬重置、丢弃改动、交互式变基 均接入确认
- [x] **右键上下文菜单** `ContextMenu`（自动限制在视口内、Esc/外部点击关闭）
  - 仓库：在文件管理器打开 / 复制路径 / 移除
  - 提交：复制哈希 / 复制补丁 / Cherry-pick / 软·混合·硬重置 / 新建分支·标签 / 交互式变基
  - 文件：在文件管理器打开 / 复制路径 / 注解(blame) / 文件历史 / 加入 .gitignore / 丢弃改动
  - 分支：切换 / 合并 / 变基 / 重命名 / 删除（确认）
  - 贮藏：应用 / 弹出 / 丢弃（确认）
- [x] **连接颜色标识**：每个仓库按路径哈希生成稳定色点（原侧栏行，现为 tab 条上的 tab）
- [x] **快捷键**：Ctrl/Cmd+Enter 提交；Esc 关闭克隆/设置弹窗
- [x] 后端新增 `reveal_path` 命令（macOS `open -R` / Windows Explorer / Linux xdg-open）

### 键盘优先操作体系（2026-10-10）
- [x] **快捷键引擎** `src/shortcuts.ts`：命令注册表 + 可覆盖键位（localStorage `gitcraft_keymap_v1`）
  - token 规范化：`mod` 合并 ⌘/Ctrl，字母才带 `shift`，标点按实际字符（`?`）
  - 两段式 chord（`g h`）800ms 超时；断开的 chord 会把当前键当作新序列起始重新解析
  - `scope: "list"` 的命令交给列表组件处理，`input: true` 才在输入框内生效
  - 纯函数 `tokenOf / matchChord / resolveKey / displaySeq` 均有单测
- [x] **全局命令**（默认键位）：
  - 导航：`g w` 工作区 / `g h` 历史 / `g b` 分支 / `g s` 贮藏 / `g r` 远程 / `⌘,` 设置 / `?` 快捷键面板 / `/` 聚焦筛选 / `⌘R` 刷新 / `c` 聚焦提交框
  - 同步：`f` Fetch / `⇧D` Pull / `⇧U` Push（顶栏按钮 title 显示当前键位）
  - 工作区：`a` 全部暂存 / `⇧A` 全部取消暂存 / `d` 丢弃所选 / `h` 追溯文件改动历史 / `⌘↵` 提交
    （`h` 详见下方「文件溯源」小节）
  - 提交列表：`m` 改写最近一次提交的信息（详见下方「改写提交信息」小节）
- [x] **快捷键面板** `ShortcutsHelp`（`?` 打开，按组分栏、显示自定义后的键位）
- [x] **设置内可配置**：`Settings → 快捷键` 点选命令后按键重绑，实时冲突提示（`commandUsing`），支持「恢复默认键位」
- [x] **提交列表键盘导航**：`j/k`、`↑/↓`、`Home/End` 逐条选中，自动 `scrollIntoView`；Esc 取消选中
- [x] **文件多选批量操作**：点击选中、⌘/Ctrl  toggles、Shift 范围选（按列独立）
  - 选中后出现批量条：暂存所选 / 取消暂存 / 丢弃所选 / 清除
  - 已暂存列新增「全部取消」；`refreshStatus` 会剔除已不存在的选中的路径
- [x] **提交草稿进 store**：`commitDraft`（msg/desc/amend/sign/noVerify）按仓库持久化到
  `gitcraft_draft_<path>`，切仓库自动恢复，提交后清空 —— 因此全局 `⌘↵` 可跨组件提交
- [x] **危险操作「不再提示」**：`requestConfirm({key})` + `ConfirmDialog` 勾选「本仓库不再提示」
  - 白名单按仓库存 localStorage `gitcraft_confirm_skips`，命中后直接放行
  - 已接入：discard / deleteBranch / dropStash / resetHard / rebase / restoreFile / amendPushed（移除仓库仍始终确认）
  - 设置内可查看各仓库免确认项并单仓库重置 / 清空全部
- [x] **前端单测起步**（`npm test`，vitest + node 环境 + localStorage shim）
  - `src/shortcuts.test.ts`：token 规范化、chord 前缀/精确/断链、用户覆盖、list scope 隔离、键位展示
  - `src/store.test.ts`：免确认白名单放行/持久化/取消不放行/无 key 必确认/按仓库重置、草稿与选择状态
  - 当时 21 例全绿（本轮增至 25，见下）

### UX 重构：左右分栏 + 行首复选框（2026-10-10）
> 动因（用户反馈）：「很不方便，不够直观，体验很差」。定位到两处硬伤：
> ① 文件行内 5 个常驻文字按钮把路径挤到换行，行高忽高忽低；
> ② diff 被压在提交框下方，经常看不见 —— 点文件像「没反应」。

- [x] **工作区 / 历史页统一为左右分栏**（`.wc-split` / `.hist-split`，左 44% 右剩余）
  > 2026-10-14 修订：工作区仍是这条；**历史页**改成「上半区（提交列表 | 改动文件）+ 下半区整宽差异」，
  > 见「历史页改版 + 仓库标签页」。
  - 左 = 变更列表 + 提交框（工作区）/ 提交列表（历史），右 = **diff 常驻栏** `DiffPane`
  - `DiffPane` 带标题条 `patchLabel`（如「未暂存 · README.md」「f7db6a6 · feature: 主题常量」），
    右侧「复制补丁 / ×（Esc）」；无内容时显示引导性空状态而非空白
  - 删除旧的 `DiffPanel` 与死代码 `diff: DiffEntry[]` / `showWorkingDiff`
- [x] **行首复选框 = 暂存/取消暂存**（用户选定方案）：勾选即 `git add`，取消即 unstage；
  暂存后自动跟随文件跳到「已暂存」列，右栏 diff 不闪空
- [x] **文件行等高 26px + 操作收敛**：路径单行省略（`fname` + 淡色 `fdir`），
  `↺ 丢弃` / `⋯ 更多` 仅 hover 出现（未跟踪行不显示 `↺`），次要操作全部进右键菜单
- [x] **未跟踪文件不再「点了没反应」**：`gitcraft-core` 新增 `untracked_patch()`，
  把新文件内容合成 `new file mode` 补丁（>2MB 只报大小、二进制只报字节数），
  经 `untracked_patch` Tauri 命令 + `api.untrackedPatch` 接入 `showPatch`
- [x] **提交框降噪**：amend / 签名 / 跳过钩子 / AI 改写 收进「更多选项」折叠，
  展开状态存 `gitcraft_commit_more`；主按钮带分支与暂存数「提交到 main (4)」
- [x] **添加仓库即可用**：`addRepo` 末尾自动 `selectRepo`；侧栏头部改为
  「打开仓库… / 克隆 / 路径」；无仓库时主区渲染 `start-card` 引导卡（`pickRepoFolder` 走原生目录选择）
- [x] **历史页即时筛选**：作者/路径输入即筛（300ms 防抖），去掉「筛选」按钮，
  仅在筛选条件非空时出现「清除筛选」；AI 解释/风险审查移入右列 diff 之上
- [x] **`showPatch` 补单测**（+4 例，共 25 例）：未跟踪文件回退合成补丁、已跟踪文件不回退、
  scope → 标题映射（`index`/`worktree`/全部）、`clearPatch` 收起右栏
- [x] **修复 `cargo test` 编译断裂**：`commit()` 加 `sign/no_verify`、`commits()` 加 `author/path`
  后，`gitcraft-core` 旧用例仍按 4 参/2 参调用（此前只跑 `cargo check`，它不编译 `#[cfg(test)]` 代码）
  - 现已改为 `commit(..., false, false)` / `commits(..., None, None)`，并新增
    `untracked_patch`（文本合成 / 二进制只报字节数）与 `commits` 路径·作者筛选用例
  - `cd gitcraft-core && cargo test` → 12 passed
  - 教训：改函数签名后须跑 `cargo check --tests` 或直接 `cargo test`

### 验证（2026-10-10 UX 重构）
- `npm run build`（tsc + vite，384KB JS / 126KB gzip）通过、`npm test` 25/25 通过、
  `cd src-tauri && cargo check` 通过
- `cd gitcraft-core && cargo test` → 12 passed（修复前这条命令因用例签名过期根本编译不过）
- 在 Vite 页面以真实 DOM 事件 + api 打桩验证：分栏网格、`.frow` 高 26px、`.row-acts` 默认
  opacity 0、复选框暂存/取消（计数 3/1 → 2/2）、点行后标题「未暂存 · README.md」、
  未跟踪 `.env.local` 合成补丁、⋯ 菜单 6 项、`a` 全暂存、`c` 聚焦提交框、`⌘↵` 提交并清空草稿、
  `g h` 切历史、点提交/`j`/`k` 更新 diff 标题、筛选防抖
- **未做**：原生 Tauri 窗口内的视觉回归（本环境内嵌浏览器 viewport 为 0×0、无法截图），
  需人工 `npm run tauri dev` 复看间距/配色/滚动条；dev 启动白屏约 7s 待查（疑似 chunk 解析，非本轮改动）

### 文件溯源：FileHistory 分栏弹窗（2026-10-10）

目标：选中任意文件即可追溯它的完整改动史，并在同一处完成「看改动 → 看旧版本 → 回滚」，
不必在历史页里手工拼 pathspec。

- [x] **后端三命令**（`gitcraft-core/src/git.rs`，全部走 git CLI，无新依赖）
  - `file_log(repo, path, max)`：`git log --follow --max-count=N -M --name-status -- <path>`，
    `%x1f` 分隔 pretty 格式解析提交头，再读 `A/M/D/R100\told\tnew` 行得到该次提交**对这个文件做了什么**
  - 新增模型 `FileLogEntry { commit, change: ChangeStatus, old_path }`（`models.rs`）——
    `change` + `old_path` 让列表读起来是「这个文件的故事」而非一串提交
  - `--follow` 让改名前的历史也能追到；改名项显示「原名 xxx」
  - `rev_file_text(repo, oid, path)`：`git show <oid>:<path>`，>2MB 只报大小不返内容，
    失败时回一段中文提示而不是抛错（弹窗不该因一个二进制文件整体打不开）
  - `restore_file(repo, oid, path)`：`git checkout <oid> -- <path>`
  - 封装链路四步：`git.rs` → `src-tauri/src/commands.rs` → `lib.rs` 的 `invoke_handler!` → `src/api.ts`
- [x] **store 动作**：`fileLog / commitFiles / commitFilePatch / revFileText / restoreFileToRev`，
  以及全局入口状态 `fileLogPath`（任何页面都能拉起弹窗）与 `patchFile`（diff 栏当前文件，供快捷键兜底）
  - 旧的组件内 `fileHistory` 动作已删除，统一走 store，方便全局 `h` 键跨组件调用
  - `restoreFileToRev` 返回 `Promise<boolean>`：早先包在 `wrap()` 里只返回 void，
    导致用户**取消确认后窗口也跟着关掉**；现在取消则保持打开，只有真回滚成功才关
  - 回滚接入危险确认 `key: "restoreFile"`，可进免确认白名单
- [x] **UI**（`src/components/FileHistory.tsx` 重写为分栏弹窗）
  - 左 = 版本列表（状态徽标 新增/修改/删除/改名 + subject + 作者·时间），右 = 详情
  - 右栏两种视图切换：「此文件的改动」（该提交对此文件的单文件 diff）/「该版本内容」（全文）
  - 操作区：在历史页打开 / 复制补丁·内容 / 回滚文件到此版本；顶部筛选框按 subject 过滤
  - 弹窗内独占键盘：`j/k`、`↑/↓`、`Home/End`、`Enter` 切换视图、`c` 复制、`Esc` 关闭
- [x] **四处入口**
  - 工作区文件行**双击**；右键菜单「追溯此文件的改动历史」
  - 差异栏（`DiffPane`）标题条「文件历史」按钮 —— 只要右栏正显示某文件就能追它
  - **历史页提交详情右栏新增「本次提交改动的文件」列表**（`.commit-files`）：状态徽标 +
    `+增 −删` 行数 + hover 出现「历史」按钮，点行看单文件 diff，点「历史」进入溯源弹窗
  - 全局快捷键 `h`
- [x] **快捷键 `h`**（命令 `wc.fileLog`，「工作区」组，可在设置内重绑）
  - `traceSelectedFile()`：优先取多选的第一个文件（`selFiles[0]`），没有选则退回右栏正在看的
    文件（`patchFile`）——「我点了它就该追它」，不必先框选
  - 只在无仓库 / 弹窗已开时不响应；弹窗打开期间全局派发被键盘门（modal gate）拦住，
    所以 `h` 在弹窗里是普通按键而非重新打开
  - 与 chord `g h`（打开历史页）不冲突：单键 `h` 只在没有前缀 `g` 时命中，已加单测固定该行为
- [x] **样式**（`src/styles.css`）：`.fh-modal`（1120px / 82vh 分栏）、`.fh-badge` 系列配色、
  `.seg`/`.seg-on` 视图切换、`.rev-lines` 全文行（`list-style:none`，避免与 `::before` 计数器重复编号）、
  `.commit-files`/`.cf-counts`、`.icon.wide`（`.icon` 固定 22px 会挤掉「历史」两个字）

### 验证（2026-10-10 文件溯源）

- `cd gitcraft-core && cargo test` → **15 passed**（原 12）。本轮新增 3 例真实 git 用例（临时 fixture 仓库）：
  `file_log_follows_renames_and_labels_changes`（add → 改名 → edit，断言序列为
  `["edit new","rename it","add old"]` 且 `old_path == Some("old.txt")`）、
  `file_log_marks_deletion`、`rev_file_text_and_restore_file`
- `npm test` → **34/34**（原 25：store +8、shortcuts +1）。`store.test.ts` 新增「文件溯源」describe 7 例
  （`fileLog` 入参、默认 max=500、接口失败回落空数组、`setFileLogPath` 全局入口、
  确认→`restoreFile`+刷新状态+`lastAction`、取消→零调用、`revFileText` 失败→空串 + error），
  「右栏 diff」补 1 例（单文件补丁记下 `patchFile`）；`shortcuts.test.ts` 新增 `h` 与 `g h` 共存 1 例
- `npm run build`（tsc + vite，390.68KB JS / 127.97KB gzip）通过、`cd src-tauri && cargo check --tests` 通过
- Vite 页面真实 DOM 事件 + api 打桩验证：历史页文件列表（2 行，`M | theme.ts | src | +12 | −3 | 历史`）、
  点行右栏标题 `aaaa111 · src/components/Button.tsx`、「文件历史」按钮 → 弹窗 3 个版本
  （徽标 修改/改名/新增 + 「原名」提示）、`j/k/Home/End` 导航、切换「该版本内容」渲染 `.rev-lines`、
  `h` 直接为选中文件打开弹窗、弹窗内再按 `h` 无副作用、回滚取消→保持打开 /
  确认→关闭且 `lastAction = "已把 src/a.ts 恢复到 aaaa111"`、`Esc` 关闭
- **未做**：原生 Tauri 窗口内真机回归（内嵌浏览器 viewport 0×0 且无 `__TAURI_INTERNALS__`，
  真实 `invoke` 走不通），需人工 `npm run tauri dev` 复看
- 踩坑记录：HMR 后应用加载的是 `/src/store.ts?t=<ts>`，直接 `import('/src/store.ts')` 会拿到
  **第二个 store 实例**（测试桩打不上）；须从 `performance.getEntriesByType('resource')` 取带时间戳的
  实时 URL 再 import

### 改写提交信息：简单 amend（2026-10-10）

需求边界（与用户确认过）：只支持**改写最近一次提交（HEAD）的信息**，
改更早的提交、改文件内容、改作者都要走交互式变基，本轮**明确不做**。
已推送到远端的提交：提示风险但允许改写。

- [x] **后端两个只读命令**（`gitcraft-core/src/git.rs`）
  - `commit_message(repo, oid)`：`git log -1 --format=%B`，取回摘要+正文全文供预填，
    只裁掉尾部换行（不能 `trim_end()`，否则吃掉正文结尾的空行）
  - `pushed_refs(repo, oid)`：`git for-each-ref --format=%(refname:short) --contains <oid> refs/remotes`，
    过滤掉 `*/HEAD`；非空即「已公开」，用来把改写历史的风险说清楚
  - amend 本身复用现有 `commit(repo, msg, amend=true, …)`，未新增写操作；
    另外把早已存在但没暴露的 `head()` 注册成了 Tauri 命令 —— 判断「这条是不是 HEAD」需要它
- [x] **store**：`headId`（随 `loadCommits` 一起取，`catch → null`，空仓库自然隐藏入口）、
  `amendOid`/`setAmendOid`（全局入口，与 `fileLogPath` 同一套路）、`amendPrefill`、`amendHeadMessage`
  - **两道守卫**（都是真实坑）：
    1. `oid !== headId` → 拒绝并提示「更早的提交请用交互式变基」；
    2. 暂存区非空 → 拒绝。`git commit --amend` 会把**已暂存内容一并并入**这次提交，
       只想改信息的人会被静默篡改历史内容，所以直接挡掉并给「全部取消暂存」的出口
  - 已推送时走 `requestConfirm({ key: "amendPushed", danger: true })`，可勾选「本仓库不再提示」
  - `amendHeadMessage(oid, message, pushed)` 把 `pushed` 作为入参而不是在函数内再查一次：
    弹窗预填时已经查过，再 `await` 一次会让确认弹窗**不在同一轮微任务里出现**（测试断言 `confirmDialog` 直接拿到 null），
    顺带省掉一次后端往返
- [x] **弹窗** `src/components/AmendMessageModal.tsx`（复用 `.modal-overlay`/`.modal-head`/`.modal-actions`
  与提交框的 `.cm-summary`/`.cm-desc`）
  - 摘要 + 正文两个字段：原信息按 `subject\n\nbody` 拆开回填，保存时若有正文再拼回 `\n\n`
  - 摘要实时字数，>50 变警示色（git 惯例）
  - 两条提醒：暂存区非空（含「全部取消暂存」按钮，点完按钮自动解禁）、已推送（列出具体远端分支）
  - 已推送时保存按钮转 `danger` 配色；`⌘/Ctrl+Enter` 保存、`Esc`/点遮罩关闭；
    取消确认后**窗口保持打开**（沿用文件溯源那次的约定）
- [x] **四处入口**：历史页提交**右键菜单**「改写这次提交的信息…」（仅 HEAD 出现）、
  提交详情操作条「✎ 改写提交信息」按钮（仅 HEAD 出现）、快捷键 `m`（`commits.amend`，
  任意页面拉起并顺手切到历史页）、快捷键面板/设置内可重绑（自动收录进 `COMMANDS`）
  - 键盘门 `src/shortcuts.ts` 加入 `amendOid`：弹窗打开期间全局命令不响应，
    所以 `m` 不会重复开、`⌘↵` 也不会串到工作区的 `wc.commit` 上

### 验证（2026-10-10 改写提交信息）

- `cd gitcraft-core && cargo test` → **16 passed**（原 15）。新增
  `commit_message_keeps_body_and_pushed_refs_only_list_published`：
  多行信息原样取回；未 push 时 `pushed_refs` 为空，
  临时 bare 仓库 `push` 后返回 `["origin/<branch>"]`，之后再补一条本地提交又回到空
- `npm test` → **44/44**（原 34）。`store.test.ts` 新增「改写最近一次提交的信息」9 例
  （预填含远端分支、`pushedRefs` 失败仍预填、非 HEAD 拒绝、暂存非空拒绝、空信息拒绝、
  未推送不弹窗、已推送取消不动历史、已推送确认后写 `amendPushed` 白名单、git 报错不写 `lastAction`）；
  `shortcuts.test.ts` 新增 1 例（`m` 绑到 `commits.amend`，`⇧M` 留空）
- `npm run build`（tsc + vite，395.42KB JS / 129.81KB gzip）、`cd src-tauri && cargo check --tests` 通过
- Vite 页面真实 DOM 事件 + api 打桩验证：`m` 打开弹窗并回填「旧的摘要 / 原来的正文说明」、
  改摘要+正文后 `⌘↵` → `commit(repo, "修正后的摘要\n\n补一段说明", true, [], false, false)`、
  列表刷新、`lastAction = 已改写 aaaa111 的提交信息`、自动关窗；
  已推送分支：红色提醒列出 `origin/main`、按钮转 danger、确认弹窗 `key=amendPushed`、
  取消→**弹窗仍在且零 commit**、确认+勾选不再提示→写进 `gitcraft_confirm_skips`、第二次直接放行不再弹窗；
  暂存区非空：提醒 + 保存禁用 + store 侧同样拒绝 → 点「全部取消暂存」触发 `unstage` → 保存解禁且窗口不关；
  右键菜单 HEAD 有「改写这次提交的信息…」而其他提交没有，右栏「✎ 改写提交信息」按钮同理；`Esc` 关窗
- **未做**：原生 Tauri 窗口真机回归（同上轮，`invoke` 在浏览器不可用）

### 快捷键总开关（2026-10-10）

需求：「加个开关，可以关闭所有的快捷键」。动机是单键命令（`a`/`d`/`f`/`h`/`m`/`c`/`?`）
在输入法、重命名输入框、外部工具抢焦点时容易误触，需要一条"整个键盘层都不拦键"的退路。

- store：`shortcutsOn`（默认开）+ `setShortcutsOn`，落在 localStorage `gitcraft_shortcuts_on`，
  只有 `"0"` 视为关闭 —— 缺键与老状态一律保持开启，不会因为读不到就悄悄失灵。
- 两个派发口都受控，不留死角：
  ① `useGlobalShortcuts` 的 window keydown 在 `shortcutsBlocked()` 时直接 return，
  命令表一个都不派发（`matchChord`/`resolveKey` 保持纯函数，不受开关影响，便于单测）；
  ② 提交列表的 `j/k/Home/End/↑↓` 走 `isEventFor`，同样在那里判定 ——
  否则全局关了、列表还能用键盘跑，是个半开状态。
- **不受开关影响的三样**（有意保留，否则会变成"关了就找不到回来路径"）：
  `Esc` 关弹窗（App.tsx 自己的监听，安全阀）、输入框内正常打字、
  设置里"按下新键"的重绑录制。
- 回来路径：顶栏 `⌨` 关闭时加 `.kbd-off`（半透明 + 删除线）并改 title，
  点它开快捷键面板 → 面板右上「重新开启」；「设置 → 快捷键」顶部也有 开启 / 关闭全部快捷键
  两个按钮与当前状态文字。

### 验证（2026-10-10 快捷键总开关）

- `npm test` → **60/60**（原 55）。`shortcuts.test.ts` 新增「快捷键总开关」3 例
  （没写入开关时一切照旧、关掉后 list 作用域三个命令都不命中、关掉再开立即恢复）；
  `store.test.ts` 新增 2 例（默认开启、关闭写 `"0"` / 开启写 `"1"`）
- `npm run build`（tsc + vite，401.40KB JS / 132.03KB gzip）通过
- Vite 页面真实 keydown 派发 + api 打桩验证：开启时按 `a` → `stage` 调用 1 次；
  关闭后连按 `a`/`f`/`?` → `stage` 仍为 1 次、`showShortcuts` 保持 false、`activeTab` 不动，
  `gitcraft_shortcuts_on = "0"`；关着时快捷键面板显示「重新开启」，点一下 →
  `shortcutsOn = true`、localStorage 写回 `"1"`；关闭态顶栏 `⌨` 类名为 `ghost kbd-off`，
  设置页「快捷键」小节显示「快捷键已全部关闭」与说明文案；
  关闭态下按 `Esc` 仍能关掉设置弹窗（`showSettings` true → false）
- **未做**：原生窗口真机回归（`invoke` 在浏览器不可用，本功能纯前端，风险低）

### Provider 插件机制（2026-10-10）

需求：「考虑引入插件机制，默认 github 插件，绑定 token 之后，可以做一些 ci 等等常用操作，
类似的还有 codeup、gitee」。范围先钉死（已与用户确认）：

- **编译期内置插件**：`Host` trait + `HOSTS` 静态注册表，不做 dylib/wasm 动态加载；
  加一个平台 = 新增一个文件 + 注册表里加一项。
- **令牌沿用现有 SQLite `settings` 表**（key = `provider_token_<id>`），与既有
  `ai_api_key` 同一套约定，不引入新的存储或系统钥匙串。
- **首发能力 = PR 列表 + 详情（只读）**。CI 状态、触发 CI、创建 PR 都不在这一版。

分层职责——`gitcraft-core` 一行网络代码都没有：

- `gitcraft-core/src/provider/mod.rs`：`Host` trait、`Capability`、`ApiRequest`
  （方法/URL/头/体的纯数据）、`RepoRef`、`HOSTS` 注册表、`parse_remote` / `detect` /
  `repo_ref_for`。插件只负责「该发什么请求」和「把响应 JSON 解析成 `PullRequest`」，
  所以全部逻辑能用离线 fixture 测。
- `github.rs`：github.com 走 `api.github.com`，企业域名走 `<host>/api/v3`；
  `Bearer` + `accept: application/vnd.github+json` + `user-agent`；字段路径支持点号
  （`user.login`、`head.label`），空令牌直接回中文原因而不是发请求。
- `gitee.rs`：v5 `repos/{owner}/{repo}/pulls`，令牌走查询参数（自带百分号编码）；
  列表键兼容 `pull_requests` / `data` 与裸数组，分支取 `head.ref`。
- `codeup.rs`：域名识别但 `capabilities()` 返回空，PR 相关方法回
  「暂不支持：需要阿里云 AK/SK 签名与 organizationId」。**能力为空 = 界面上不出现死按钮**，
  而不是点了报错。
- `parse_remote` 同时吃 `git@host:owner/repo.git`（scp 形态先把 `:` 换成 `/`，否则 host
  会被读成 `git@host`）、`https://host/owner/repo.git`、带凭据的
  `https://oauth2:tok@host/owner/repo`，嵌套分组路径原样保留。

host 层（`src-tauri/src/commands.rs`）新增 4 个命令：

- `provider_info(repo?)` → `ProviderStatus { providers[], detected? }`，`detected` 带
  `slug`（`host/owner/repo`）；`repo` 传 null 时只列平台，供「先绑令牌再开仓库」。
- `provider_set_token(provider, token)`：写 `settings.provider_token_<id>`，空串即解绑。
- `provider_list_prs(repo)`：core 产出 `ApiRequest` → `reqwest`（15s 超时）执行 →
  core 解析；非 2xx 把状态码 + 前 300 字原样回传，不吞平台的报错文案。
  DB 锁在任何 `await` 之前释放。
- `open_url(url)`：只放行 `http(s)://`，交给 `open` / `explorer` / `xdg-open`。

**令牌是只写的**：没有任何命令把 token 读回 webview，设置页只显示「已绑定 / 未绑定」，
换令牌只能覆盖或清除。

前端：

- store：`providerStatus`、`pullRequests`；`loadProviders()`（无仓库也可列平台）、
  `setProviderToken()`（trim 后写入 → 立刻重新读取状态，让徽标翻转）、
  `loadPullRequests()`（**先看能力、再看有没有令牌**，缺一就清空列表且不发请求，
  「没绑令牌」是界面能解释的状态而不是错误）、`openPullRequest()`；
  `selectRepo` 并发 `loadProviders()`，切仓库/删仓库时清掉 provider 状态。
- 设置页新增「代码托管」区块：每个平台一行，徽标显示「本仓库 / 已绑定令牌 / 能力」，
  `token_hint` 当 placeholder，输入框回车即绑定，已绑定的可清除；能力为空的平台只显示原因、
  不显示输入框。
- 远程页在 remote 列表下方新增 provider 面板：显示识别结果（平台 + `owner/repo`）、
  「刷新 Pull Request」按钮、PR 列表（编号 / 标题 / 草稿徽标 / `head → base`），
  点行展开详情与「在浏览器打开」；未绑定时提示去「设置 → 代码托管」，不报错。
- **快捷键**：本轮没有新增键位。入口沿用 `g r`（打开「远程」）与 `⌘,`（打开设置），
  弹窗关闭沿用 `Esc` 阶梯。

### 验证（2026-10-10 Provider 插件）

- `cd gitcraft-core && cargo test` → **28 passed**（原 16）。新增 12 例：
  `provider::tests` 4 例（scp/https/带凭据/嵌套分组解析、非 owner/repo 地址拒绝、
  按域名识别平台且未知域名不强行归给 GitHub、每个平台读自己的 owner/repo）、
  `github::tests` 4 例（请求 URL 与 Bearer 头、空令牌拒绝、企业域名走 `/api/v3`、
  fixture 解析 + `{"message":"Bad credentials"}` 回错误）、
  `gitee::tests` 3 例（令牌百分号编码进查询串、`pull_requests` 包裹结构 + `head.ref`、
  只认 gitee.com）、`codeup::tests` 1 例（识别但能力为空，请求回「尚未接入」）
- 过程中由测试抓到一个真 bug：`field(item, &["user", "login"])` 被当成「备选键名」实现，
  嵌套字段永远取回空串——已改为点号路径（`"user.login"`），github/gitee 两个 fixture 例
  就是它的回归测试
- `npm test` → **55/55**（原 44）。`store.test.ts` 新增「代码托管 Provider 插件」11 例
  （按仓库识别平台、无仓库也能列平台、有令牌且有能力才发请求、无令牌不发请求且清空列表、
  能力为空不发请求、未识别平台清空列表、状态缺失时先补一次 `loadProviders`、
  绑定后重新读取且 API 面没有读令牌的口子、留空即解绑、交给系统浏览器打开、
  请求失败只报错不动已有列表）；`shortcuts.test.ts` 仍 15 例（本轮无新键位）
- `npm run build`（tsc + vite，400.37KB JS / 131.44KB gzip）、
  `cd src-tauri && cargo check --tests` 通过
- Vite 页面真实 DOM 事件 + api 打桩验证：远程页显示「GitHub · github.com/acme/app」与刷新按钮 →
  点刷新出 `#12 加个按钮 | 草稿 | acme:feat → acme:main` → 点行出详情（open 徽标、
  作者/分支/更新时间、「在浏览器打开」）；设置页三行平台，GitHub 带「本仓库 + 已绑定令牌 +
  清除」，Codeup 只显示「暂不支持」且无输入框；Gitee 行输入 `  abc123  ` 回车 →
  `provider_set_token("gitee","abc123")`、`lastAction = 已绑定 gitee 的访问令牌`、
  重新读取后该行翻成「本仓库 + 已绑定令牌」；未绑定令牌时提示文案出现且
  `provider_list_prs` **调用次数为 0**、列表被清空；remote 不受支持时显示「不属于 GitCraft
  支持的托管平台」
- **未做**：原生 Tauri 窗口真机回归；真实 GitHub/Gitee 令牌联调（需要用户自己的 token）
- **明确不做**（本期）：CI 状态 / 触发 CI / 创建 PR / Codeup AK/SK 签名 / 动态加载插件 /
  系统钥匙串存储

### 更新检查（2026-10-10）

需求：「参考 github 项目 dbx，引入更新机制」。用户随后明确：「继续，考虑直接 github release」
——所以这一版**不做应用内安装、不引签名密钥**：GitCraft 去自己发布仓库的 GitHub Releases
读最新版，把匹配本机的安装包链接交给系统浏览器。dbx 的
`tauri-plugin-updater` 方案（minisign 公钥 + `latest.json` + `createUpdaterArtifacts`）
记录在 TODO，等真正开始发版再接，避免私钥进仓库或打断现有构建。

- 发布仓库写在 `settings`？不——沿用前端既有偏好约定存 **localStorage**
  （`gitcraft_update_repo` / `gitcraft_update_ignored` / `gitcraft_update_auto`），Rust 侧保持无状态，
  `check_update(repo)` 只吃参数。
- 分层与 provider 一致：`gitcraft-core/src/update.rs` 纯函数（URL 拼装、Release JSON 解析、
  版本比较、挑选安装包），`src-tauri` 唯一一个 `check_update` 命令碰网络
  （reqwest 15s 超时，非 2xx 时保留 GitHub 的 `message` 正文供用户行动）。
- 版本比较是**按数字段**而不是字符串：`1.10.0 > 1.9.0`、缺段补零（`0.1 == 0.1.0`）、
  prerelease 不新于它自己的 release；**解析不了的 tag 永远不算升级**，脏数据不能把用户
  推向旧版本。
- `pick_asset` 按当前 OS/arch 挑安装包：先按「dmg / setup.exe / AppImage…」的偏好分层，
  再按架构词（arm64 vs x64/amd64）匹配并**排除对方架构**与 `.sig`/`.sha256` 附件，
  windows 的 offline 大包降权。挑不出来就只给「在浏览器打开 Release」。
- 前端：store `updateRepo/updateCheck/updateIgnored/updateAuto` + `checkForUpdate(silent)`
  （启动静默、手动失败才 toast）、`setUpdateIgnored`（忽略此版本 / 恢复提醒）、
  `openDownload`（默认打开发布页，传 url 直达安装包）；`updateNotice()` 是唯一判定
  「要不要顶栏提示」的出口。
- UI：顶栏出现绿色「新版本 vX.Y.Z」圆片（点了开设置）；设置页新增「更新」区：
  owner/repo 输入 + 「检查更新」 + 「启动时自动检查」勾选，结果里显示
  当前→最新、发布日期、发布说明（可滚动）、下载按钮（带文件名与大小）、
  「在浏览器打开 Release」「忽略此版本」。
- **快捷键**：本轮无新增键位；设置页入口沿用 `⌘,`，Esc 关闭弹窗不变。

### 验证（2026-10-10 更新检查）

- `cd gitcraft-core && cargo test` → **35 passed**（原 28）。`update::tests` 新增 7 例：
  slug 只认 `owner/repo`、API URL 拼装、Release 解析 + `{"message":"Not Found"}` 回错 +
  无 tag 回错、按平台挑安装包（mac arm64/x64 dmg、windows setup.exe 且 offline 降权、
  linux AppImage、未知 OS 回空）、跳过 `.sig` 且不选错架构、tag 数字化解析、
  数字段版本比较
- 解析器对着 **dbx 真实 release JSON**（`api.github.com/repos/t8y2/dbx/releases/latest`，
  tag `v0.6.38`，33 个附件）核对过字段名：`tag_name/body/html_url/published_at` 与
  `assets[].{name,browser_download_url,size}` 全部对得上，测试 fixture 就用它的命名风格
- `npm test` → **66/66**（原 60）。`store.test.ts` 新增「更新检查（直连 GitHub Releases）」
  6 例：未配置仓库时静默检查零请求且显式检查给提示、检查成功落状态且无新版本不出提示、
  忽略/恢复提醒往返且同版本重查仍安静、换仓库清旧结果、`openDownload` 直达附件与
  缺省开发布页、启动失败安静 / 手动失败报错
- `npm run build` → **404.85 kB JS / 133.23 kB gzip**；`cd src-tauri && cargo check --offline`
  通过（本轮未新增任何 Rust 依赖——`tauri-plugin-updater`/`process` 加过又撤了，
  Cargo.lock 已恢复）
- Vite 页面真实 DOM 验证：设置弹窗出现「更新」区（输入框 placeholder「发布仓库 owner/repo」、
  自动检查默认勾选）；空仓库点「检查更新」弹「还没有配置发布仓库…」且不发请求；
  填 slug 后 localStorage 落盘（浏览器里 `invoke` 不可用，成功路径只能到报错 toast，
  成功链路由 store 单测覆盖）
- **未做**：原生窗口里真实检查（需要先有真实的发布仓库）；应用内下载进度与安装

---

### 环境一体化：拖拽 / 预览 / 扫描 / SSH / git 引导 / LFS / 分组（2026-10-11）

需求原文是八个点：「一是配置 ssh，能不能自动识别本机 .ssh，二是拖动 git 文件夹能不能自动识别，
三是列表有没有分组，四是能不能再列表上加标记，github 图标，五是能不能借用一些能力，比如
github1s 极速预览，六是扫描文件夹，七是国内用户（windows）装 git 麻烦，八是 git flow、git lfs」。
按「先解决找不到仓库、再解决装不上 git、最后补能力」的顺序落，git flow 本轮**只记 TODO**
（它要动分支模型与合并策略，不是一个面板能收口的功能）。

**① 拖拽进窗口自动识别（`gitcraft-core::git::find_repo_root` + `repo_root_of`）**
- 用 `getCurrentWebview().onDragDropEvent()`（`@tauri-apps/api/webview`）拿 `enter/over/drop/leave`
  与 `paths`；纯浏览器 `npm run dev` 下没有这个 API，取不到就 try/catch 静默跳过。
- 拖进来的经常不是仓库根而是仓库里某个文件：`find_repo_root` 从该路径向上找第一个含 `.git`
  的目录（`.git` 既可能是目录也可能是 worktree/submodule 的**文件**，两种都认）。
- 拖入期间整窗盖一层 `.drag-overlay`「松开以打开该仓库」，`pointer-events: none` 保证不误拦；
  drop 后走既有 `addRepo` + `selectRepo`，与「添加仓库」按钮同一条链路。

**② github1s 极速预览（`gitcraft-core/src/preview.rs`）**
- 不引任何依赖：github1s 的地址就是 GitHub 自己的 URL 语法，`/tree/<ref>` 看目录、
  `/blob/<ref>/<path>` 看文件，拼字符串即可，`parse_remote` 复用 provider 的远程解析。
- **只支持 github.com**，其他 host 明确回「极速预览目前只支持 github.com，当前远程在 xx 上」，
  不假装能打开；没有 ref 时退回仓库根（`/tree/`），因为 `/blob/<path>` 缺 ref 就不是合法地址。
- 两个入口：「远程」页每个 remote 的行内按钮、「差异」面板头部（带当前 `patchFile` 的路径）。
  打开仍走既有 `open_url`，不新开 Tauri 窗口。

**③ 扫描文件夹（`gitcraft-core::git::scan_repos`）**
- BFS + `VecDeque`，遇到含 `.git` 的目录就收下并**不再深入**（仓库里的仓库不算第二个仓库）；
  跳过隐藏目录、`node_modules`、符号链接。
- 上限写在 Rust 命令里而不是 UI：深度 4、最多 100 个，让用户敢直接选 `~/Developer` 这种大目录。
- `scanFolder` 在 store 里逐个 `addRepo`，**每个路径单独 try/catch**：一个坏仓库不能带走整批；
  `lastAction` 说清「新增 N 个，跳过 M 个」。

**④ SSH 诊断（`gitcraft-core/src/ssh.rs`，设置页新增「SSH」区）**
- 自动识别 `~/.ssh`：私钥靠**文件内容开头**分类（`-----BEGIN OPENSSH PRIVATE KEY` 等），
  不看文件名后缀——`id_rsa.pub` 被用户改名这事很常见。
- 权限只在 `#[cfg(unix)]` 下检查（Windows 无 POSIX mode），mode 带 group/other 位时标
  「权限过宽」并给 `chmod 600`；没有配对公钥的私钥标「无公钥」。
- 「复制公钥」走 `public_key_in(dir, path)` 守卫：**只允许 `~/.ssh` 下的 `.pub`**，
  路径必须直接父目录等于 `~/.ssh`，私钥永远不会离开磁盘。`ssh_config` 用大小写不敏感、
  空格或 `=` 分隔、`#` 注释的宽松语法解析出 Host / HostName / User / IdentityFile。
- 「试连」是唯一碰网络的按钮：`ssh -o BatchMode=yes -o ConnectTimeout=8 -T user@host`。
  GitHub 认证成功时**退出码非 0** 且正文含 "successfully authenticated"，按 provider 各自措辞
  翻译成「认证通过 / 密钥被拒 / 主机未知 / 连不上」，并把服务器原文放进 `<details>` 供自查。
- 刻意**不加** `StrictHostKeyChecking=accept-new`：那会静默改写 known_hosts 并卸掉一个安全检查。

**⑤ Windows 装 git 引导（`git_env` + `GitSetup` 横幅）**
- 启动时读一次 `git --version`，`installed=false` 时顶栏下方常驻横幅，而不是让用户在各处撞红字。
- 按 `std::env::consts::OS` 分文案：Windows 给 git-scm 官方下载 + **npmmirror 二进制目录**
  （国内直连 github release 慢是事实，这里就是给它的替代），macOS 给 xcode-select / brew。
- 命令与 git-lfs 共用一个版本号解析：`numeric_prefix` 取点分的数字段，`2.50.1.windows.1 → 2.50.1`、
  `2.47.0.rc0 → 2.47.0`、`3.0.0-pre → 3.0.0`（在 `-`/`+` 处截断，否则 prerelease 会被读成 `3.0`）。
- 「重新检测」按钮重跑 `checkGitEnv`，装完不用重启应用。

**⑥ Git LFS 检测与工作区徽标（`lfs_info` + `lfs_status`）**
- `git lfs version` 决定「本机装没装」，`git lfs ls-files` 决定「这个仓库哪些文件走 LFS」；
  两者都**解析真实输出**（`git: 'lfs' is not a git command` 视为未装，而不是报错）。
- `lfs_info` 设计成永不失败并返回空报告，所以能在每次切仓库时随 `Promise.all` 一起读，
  不需要 loading 态；本机没装时 `files` 为空是诚实结果，绝不假装仓库干净。
- UI：工作区文件行出现琥珀色 `LFS` 徽标（tooltip 带上 git-lfs 版本），
  当「有 LFS 文件但本机没装」时列表上方出现 `.lfs-hint`——说清现在看到的差异只是**指针文本**，
  并给出 macOS `brew install git-lfs` / Windows `git lfs install` 两条命令。

**⑦ 列表分组 + 标记**
- 工作区：已暂存/未暂存两段各自**按顶层目录折叠分组**（一层为限，够收拾 200 个文件又不会把
  两个不相干的子树混进一组）。表头显示 `目录/ 数量`，点一下折叠，行内的目录后缀仍保留；
  表头带「暂存目录 / 全部取消」，一次收下一个目录。
  折叠态只活在本次会话（`folds` state）——折叠是一种「此刻怎么看」，不是偏好设置。
  **Shift 范围选择只走可见行**，折叠掉的不会被误选。
- 历史：提交列表插入**日期表头**（今天 / 昨天 / 3 天前 / 2026-10-08，纯相对只在三天内诚实）。
  分组与图形是这件事的全部难点：提交图靠 `row * rowH` 定位，插行会把 DAG 拽歪，所以
  `src/historyGroup.ts`（纯函数、可单测）先算出视觉槽位，`History.tsx` 再按槽位算每条边的 y，
  日期头只占 22px；提交行的日期从整串 ISO 缩成 `HH:MM`，完整时间留在 tooltip。
- 未推送徽标：`unpushed_commits(repo)` 一次 `git rev-list HEAD --not --remotes` 拿到全部待推 id
  （列表 200 行不可能逐行问），行上出现蓝色「未推送」，工具栏补「N 条 · M 条未推送」。
  **没有远程的仓库返回空**——否则整条历史都被标成未推送，那是噪音不是信息。
  commit / push 之后立刻重读，标记跟着变。
- 平台标记：`ProviderMark` 用**字母圆角片**（GitHub 深灰 G / Gitee 红 G / Codeup 橙 C）而不是
  各家的 logo 素材，避免把商标文件打进仓库；出现在设置页 provider 列表和远程页 provider 区。
  真实图标与作者头像一起记进 TODO（要选一套可授权的素材）。

**快捷键**：本轮**没有新增键位**。上面九项都是「按钮 / 菜单 / 自动发生」的交互——拖拽、扫描、
试连、极速预览、LFS 提示、目录折叠、日期头都跟着鼠标走；`unpushed` 与 LFS 状态在切仓库、
commit、push 时自动重读，不需要用户去按什么。既有键位不变：`⌘,` 打开设置即进入新的 SSH 区，
`⌘R`（刷新当前仓库）的清单里补上了 `loadLfs` 与 `loadUnpushed` —— 在终端里 commit 或
`git lfs pull` 之后，按一次刷新就能把 LFS 徽标和「未推送」带回真实状态；
它不会去重跑 SSH 试连这种碰网络的动作。

### 验证（2026-10-11 环境一体化）

- `cd gitcraft-core && cargo test` → **61 passed**（上一轮 35）。本轮新增：
  `ssh::tests` 10 例（密钥内容分类、权限过宽、Host 块宽松语法、`~/.ssh` 外与私钥拒绝复制、
  GitHub 非零退出仍判成功、各家措辞归一、把「主机未知/连不上」和密钥问题分开、
  没有 `.ssh` 目录也是答案不是错误、试连拒绝带空格的主机）、
  `preview::tests` 5 例（`/tree`、`/blob`、远程解析失败回中文错、非 github host 明确拒绝、
  无 ref 退回仓库根）、`git::tests` 新增拖拽向上找根 1 例、扫描 5 例（深度上限、跳过
  node_modules 与隐藏目录、仓库不再深入、符号链接、上限生效）、git/LFS 版本解析 4 例、
  `unpushed_commits_are_only_local_ones` 1 例（无远程回空、push 后为空、本地两提交都在等待）
- `npm test` → **98 passed**（上一轮 66）：`store.test.ts` 75（极速预览 4、扫描 6、SSH 6、
  git 缺失 2、Git LFS 5、未推送标记 4）、`historyGroup.test.ts` 5（日期取整、相对标签边界、
  每天只插一个头、提交顺序不变以便图形按 row 找回、空历史不产生头）、`shortcuts.test.ts` 18
- `npm run build` → **437.66 kB JS / 142.59 kB gzip**（CSS 30.28 kB / 6.42 kB）；
  `cd src-tauri && cargo build --offline` 通过，**本轮未新增任何依赖**
- Tauri 命令层：本轮新增并注册 8 个 —— `git_env`、`scan_repos`、`lfs_status`、`unpushed`、
  `github1s_url`、`ssh_inventory`、`ssh_public_key`、`ssh_probe`；拖拽用的 `repo_root_of`
  随 ① 一起落地并已在 `invoke_handler!` 里注册
- 已知验证边界：内嵌浏览器里没有 `__TAURI_INTERNALS__`，`invoke` 必然失败，所以拖拽、
  试连、扫描这类真实链路靠 core 单测 + store 单测覆盖，界面只验到 DOM 与状态；
  原生窗口内的拖拽/扫描/试连仍需手工回归（见 TODO）
- **未做**：git flow（整个流程模型，单列 TODO）；提交列表作者头像与各家真实 logo（素材授权）；
  LFS 的实际推拉（只做检测与提示，不动 `git lfs pull/push`）

---


### 中文正确性包（2026-10-12）

产品视角下最刺眼的问题不是少功能，而是**中文用户天天踩的四个隐性错误**。这一批全部是
「修正确性」，没有新按钮，也不改键盘：

1. **中文文件名不再变成乱码**（`gitcraft-core/src/git.rs`）。git 自己的默认值是
   `core.quotepath=true`，它会把 `中文文件.txt` 输出成 `"\344\270\255\246\207…"`。
   以前左侧文件列表拿到的是这串八进制转义，勾选后传给 `git add` 的路径也是这串——
   SourceTree 中文路径失效就是这么来的。现在所有 git 调用统一带
   `-c core.quotepath=false`（`CONFIG_ARGS` 注入 `run_git` 与 `run_git_env` 两个咽喉点，
   不是逐个命令补，所以未来新增的调用自动受益）。
2. **GBK 老仓库不再打不开**。`run_git` 原来是 `String::from_utf8(stdout)?`：一个源码
   用 GBK 存的国内老仓库，`git diff` 的字节流不是合法 UTF-8，于是**整个 diff 直接报错**，
   用户看到的是「打不开」而不是「乱码」。现在改成 `decode()`（`from_utf8_lossy`），
   坏字节变 U+FFFD，diff 结构（哪行加哪行删）完好，用户仍能读。宁可乱码，不可空白。
3. **git 报错说中文**（`explain_git_error`）。30 条按子串匹配，命中就在原文上面加一句
   人话（「远程有你本地没有的提交，请先 Fetch 并 Pull / 变基之后再推送，强推会丢掉对方的提交」），
   **原文始终保留在 `—— git 原文 ——` 下面**：中文解决当下，原文方便搜索。未命中的报错
   一字不改地透传——不了解的失败硬编一个解释比不解释更糟。覆盖面按国人踩坑频率排：
   no upstream / non-fast-forward / publickey / timed out / could not resolve host /
   repository not found / RPC failed / early eof / 证书校验 / dubious ownership /
   锁文件 / 冲突 / 未配置身份。克隆路径（`clone_repo`）以前绕过一切，现在同样进这层，
   因为国内用户第一次打开应用做的事就是克隆，而克隆失败的恰恰是网络。
4. **输入法合成态不触发快捷键**（`src/shortcuts.ts` + `App.tsx` + `AmendMessageModal.tsx`）。
   以前用拼音写提交信息时，选候选词的 `Esc` 会关掉弹窗（打字内容全丢）、`⌘↵`
   可能提前提交。现在键盘事件先问 `imeOwns(e)`（`isComposing` 或 WebKit 的遗留信号
   `keyCode===229`），合成期间整个键盘交还给输入法。为了让这条顺序可测，把
   `onKeyDown` 的判定抽成纯函数 `decideKeydown(e, pending)`：返回 `null`＝这键不是我们的，
   `"escape"`＝清组合前缀，否则给出命中的命令；hook 只剩副作用。

**四个判别门都做了反向变异**（临时删掉修复，确认测试按预测转红，再原样还原）：
去掉 `CONFIG_ARGS` → `chinese_paths_are_not_octal_escaped` 红，报错信息里打印出
`"\344\270\255…"` 原文；把 `decode` 换成严格 UTF-8 → `legacy_gbk_bytes_degrade_instead_of_failing` 红；
去掉 `imeOwns` → IME 两条断言各自转红（拆成两个 `it`，否则第一条失败会掩盖第二条）。

### 验证（2026-10-12 中文正确性包）

- `cd gitcraft-core && cargo test` → **64 passed**（上一轮 61）。新增 3 例，全部是真仓库 fixture：
  `chinese_paths_are_not_octal_escaped`（暂存态与工作区态都要给出原文名，且暂存 diff 的
  文件头也是原文名）、`legacy_gbk_bytes_degrade_instead_of_failing`（写入真 GBK 字节，
  断言 diff 不再 Err 且坏字节变 U+FFFD，同时历史列表仍能读该提交的 subject）、
  `git_failures_are_explained_in_chinese_and_keep_the_original`（中文提示 + 原文保留 +
  未识别错误透传 + 克隆超时/仓库不存在的真实措辞 + **端到端**：真的 `checkout nope`
  一次，断言错误里带中文）
- `npm test` → **102 passed**（上一轮 98）：`shortcuts.test.ts` 21（新增 IME 4 例）、
  `store.test.ts` 75、`historyGroup.test.ts` 5
- `npm run build` → **437.87 kB JS / 142.69 kB gzip**（CSS 30.28 kB / 6.42 kB）；
  `cd src-tauri && cargo build --offline` 通过，`cargo fmt --check` 两处 crate 均干净，
  **本轮未新增任何依赖、未新增 Tauri 命令**
- 实测边界（不是「应该没问题」）：macOS/APFS **不允许**创建非法 UTF-8 的文件名
  （`mkdir/open` 直接 `EILSEQ`），所以 GBK 场景无法用文件名复现；改用真正的通路——
  文件**内容**是 GBK 字节（`git show HEAD:gbk.txt` 输出非法 UTF-8，已用
  `python3 -c "…decode('utf-8')"` 确认抛 `invalid continuation byte`）。
  顺带确认：GBK 的**提交信息**不会触发这个问题，git 自己会按
  `i18n.logOutputEncoding` 重编码成合法（但难看）的 UTF-8——所以修的是内容通路，
  不是信息通路，二者不能混为一谈。

### 国内网络闭环（2026-10-12）

用户提的「弥补 SourceTree 对国人不友好」里，最难补的不是翻译，是网络：
拉取走不通、更新检查走不通、克隆选了个假节点照样失败。这一批把四件事做实。

- [x] **fetch / pull 也能走加速节点**（2026-10-12）
  - `git.rs`：`node_rewrite_args(node_tpl, remote_url)` 把节点模板展开成
    `-c url.<展开后的地址>.insteadOf=<远程原始地址>`，**只作用于这一条命令**；
    `fetch(repo, remote, node)` / `pull(repo, remote, branch, node)` 各加一个可选节点参数，
    `remote_url()` 先 `git remote get-url` 拿到真实远程再改写。
  - 为什么不用 `git config`：写进配置就等于把用户的 origin 换成了镜像，
    下次推送会推到镜像上去。一次性 `-c` 覆盖用完即弃，`origin` 始终是真地址。
  - **push 永不走节点**：加速节点是只读镜像，推上去的提交不会出现在真正的远程，
    用户看到的是「推送成功但同事没收到」——这是丢工作，不是慢一点。
    `store.push` 的参数表里根本没有节点，测试用遍历实参的方式把它钉住。
  - SSH 远程（`git@github.com:a/b.git`）在**发起任何网络请求之前**就被拒绝并说明原因
    （节点改写不了 SSH，请配代理或在「设置 › SSH」试连），而不是默默什么都不做。
  - UI：「设置 › GitHub 网络」新增「拉取加速节点」输入框，写 `sync_node` 设置项，
    空=直连；`App` 启动时 `loadSyncNode()`，否则每次冷启动都静默直连。
- [x] **节点清单去陈化 + 探活结论**（2026-10-12）
  - 默认节点去掉 `ghproxy.com`（本机 DNS+HTTP 实测 `000`，早已停服），换成
    `gh-proxy.com` / `ghproxy.net` / `ghfast.top`；文案里的示例同步改掉。
  - 探活从「GET 原始地址」改成「GET **这条操作真正会用到的 URL**」，
    且只有 2xx/3xx 算可达——节点返回 404 是「在线但不承载这个仓库」，
    旧代码用 `res.is_ok()` 把它也叫「可达」，用户选完就在克隆阶段翻车。
  - `src/connectivity.ts`（纯函数，`connectivity.test.ts` 6 例）把候选列表压成一句结论：
    直连可用 / 直连不通已走最快节点 / 全都不通 / **这是 SSH 地址，探测说了不算**。
    最后一条是这批里唯一的"防误报"：HTTP 探测对 SSH 天然无意义，
    没有这个分支，SSH 克隆永远显示「所有节点都不通」。
- [x] **更新检查基址可配**（2026-10-12）
  - `update::release_api_url(slug, base)`：空 base = 官方；
    带 `{url}` 的按节点模板展开；否则只替换主机、重建 `/repos/<slug>/releases/latest` 路径；
    非 http(s) 或含空格直接报错，而不是拼出一个 `curl` 都拒绝的 URL。
  - `check_update` 在锁里读 `update_api_base` 设置（webview 不需要知道镜像的存在）；
    发送失败时错误里直接给下一步——「到设置 › 更新填一个镜像地址或加速节点模板」。
  - UI：「设置 › 更新」加镜像输入框 + 「保存并重查」。
- [x] **上次同步 x 分钟前**（2026-10-12）
  - `db.rs`：`repos.last_fetched_at`（新建表带列 + `PRAGMA table_info` 守卫给老库
    `ALTER TABLE`），`mark_fetched(conn, path)` 只在 fetch **成功之后**打时间戳，
    失败的拉取不能把仓库伪装成「刚刚同步」。未知路径是 no-op 而不是报错。
  - `fetch` 命令里调用 `mark_fetched`，store 成功后重读 `repos`，
    所以时间戳不是靠某个调用方记得刷新。
  - `src/freshness.ts`（`syncLabel(iso, now)` → `{ short, text, stale }`，9 例）：
    刚刚 / N 分钟前 / N 小时前 / N 天前 / 超过一个月 / 从未同步，
    满 `STALE_AFTER_MS`（3 天）转 `stale`，侧栏琥珀色。
    存在的理由是一句话：「领先 2 / 落后 5」只有配一个新鲜度才有意义，
    否则用户会拿三天前的落后数去点推送。
  - 侧栏行用 `short`（「5 分钟前」），完整句子进 tooltip——八行都写「x 分钟前同步」是噪音。
  - 未来时间戳按「刚刚」处理（`Math.max(0, now - then)`），时钟偏移不会显示负数。

### 验证（2026-10-12 国内网络闭环）

- `gitcraft-core` **67 passed**（`cargo test --offline`，+3）：
  `node_templates_become_one_shot_rewrites` 覆盖模板展开、空模板报错、非 https 拒绝、
  含空格/引号/等号的节点被拒（这些字符会破坏 `-c` 的 key=value 结构）；
  `a_node_routes_only_the_network_half_of_a_fetch` 是**离线端到端**门——
  本地搭一个 `file://` 前缀的假节点目录树，无节点 fetch 必须失败、带节点必须成功、
  `refs/remotes/origin/main` 必须真的落下、`origin` 的 URL 必须**一个字都没变**。
  两个坑在这里被实测出来：subsection 必须整体加引号形式写会保留引号字面量；
  git 在**第一个** `://` 处切分协议，所以 `file://…/{url}` 展开成
  `file:///tmp/node/https://…` 才可用，裸 `/tmp/node/https://…` 报
  `protocol '/tmp/node/https' is not supported`。
  `an_old_database_gains_the_freshness_column` 手工建 4 列老表跑 `migrate`，
  断言 `None` → `mark_fetched` → `Some`，且 `/nowhere` 不报错。
- 前端 **122 passed**（5 文件，`npx vitest run`，+20）：store 5 例钉节点透传
  （fetch/pull 带模板、留空传 `null`、push 实参里不得出现节点串、fetch 后必须重读列表、
  `setSyncNode` 去首尾空格）、connectivity 6 例、freshness 9 例。
  `tsc --noEmit` 干净，`npm run build` 440.82 kB / 144.02 kB gzip，CSS 30.42 / 6.45 kB。
- 反向变异 3 道门，各自单独转红后从 `/tmp` 备份还原（`diff -q` 确认字节一致）：
  1. `store.fetch` 改成永远传 `null` → 「配了节点时 fetch/pull 带上模板」红；
  2. `store.push` 的 `force` 实参换成 `get().syncNode || force` → 「推送是写操作，
     节点绝不能出现在参数里」红（这条是这批唯一真正防丢提交的门）；
  3. `connectivity` 的 SSH 分支短路成 `if (false && …)` → 「SSH 地址不谎报全都不通」红；
     `freshness` 的 `age >= STALE_AFTER_MS` 改成严格 `>` → 边界断言红。
- 未引入新依赖（Rust 侧复用 `reqwest`/`chrono`/`rusqlite`，前端只加两个纯函数模块）；
  未新增 Tauri 命令（`fetch`/`pull`/`check_update` 是改签名或改读取来源）。



### 顶栏重构：合并进原生标题栏（2026-10-12）

用户两条硬约束合在一起就是这一批：**没有项目标题栏** 与 **pull/push 太傻**。
顶栏原本是一排裸按钮（Fetch / Pull / Push / ⌨ / ⚙️ /  / ☰），
把 git 的实现细节当成产品功能摊在桌面上，而窗口标题永远是「GitCraft」，
六个仓库的窗口在 Mission Control 里长得一模一样。

- [x] **一行标题栏**（2026-10-12）
  - `tauri.conf.json`：窗口加 `titleBarStyle: "Overlay"` + `hiddenTitle: true`。
    查过 `tauri-utils` 的字段文档：这个键**只在 macOS 生效**，Windows/Linux 仍是原生标题栏，
    所以「合并成一行」在 macOS 成立、在 Windows 退化为「原生标题栏显示仓库名」，不会失去标题栏。
  - `capabilities/default.json` 补 `core:window:allow-set-title` 与 `core:window:allow-start-dragging`。
    不是想当然加的：从 `gen/schemas/acl-manifests.json` 里把 `core:window` 的
    `default_permission` 28 项打出来看过，**只有 `allow-title`（读），没有 set-title / start-dragging**，
    所以这两个必须显式授予；双击最大化用的 `allow-internal-toggle-maximize` 已在默认集里，不加。
  - `data-tauri-drag-region` 挂在 `<header>`、仓库名、以及中间的弹性占位条上（子元素不继承拖动区）；
    macOS 下整行左内边距 78px 让出交通灯，靠 `gitEnv.os === "macos"` 判断——
    启动本来就读一次 `git --version`，不必为此引入 `plugin-os`。
  - `.brand`「GitCraft」文字去掉：应用名在系统菜单栏里本来就有，这一行该说的是**当前仓库**。
- [x] **一个「同步 ↑N ↓N」分裂控件取代三个按钮**（2026-10-12）
  - 主按钮只做 **Fetch**：它只动远程跟踪引用，不改工作区、不产生提交，是唯一一个
    可以不加确认、放在最顺手位置的动词。tooltip 明说「不改动本地」。
  - `▾` 菜单里才是有副作用的动作：`拉取 Pull · 下来 5 个提交`、`推送 Push · 发出 2 个提交`、
    需要时出现 `首次推送：设置上游并推送到 origin/xxx`、以及带具体分支名与后果描述的强制推送确认。
  - 纯函数 `src/syncControl.ts`（`syncModel`）负责读数与菜单文案，界面只负责画它。
  - 绑定不变：`f` / `⇧D` / `⇧U` / `⌘R` 照旧，`shortcuts.ts` 一行没改。
- [x] **↑N ↓N 之前没有基线**（这一批真正的收获）（2026-10-12）
  - `status --porcelain=v2 --branch` 的 `# branch.ab +A -B` 早就在解析，
    但**没有解析 `# branch.upstream`**，于是「分支没关联远程」和「跟远程一模一样」
    在界面上都是 `↑0 ↓0`。用户拿一个从没比较过的 0 去判断该不该推送，
    比不显示数字更糟。
  - `RepoStatus` 加 `upstream: Option<String>`（`(gone)` 视为没有），
    `syncModel` 在没有上游时显示「未设置上游」（无远程则显示「无远程」），
    并把「首次推送：设置上游」送进菜单。
  - 分支 chip 的 tooltip 同样区分两者。
- [x] **兑现一条空头承诺**（2026-10-12）
  - `git.rs:59` 的中文解释一直在说「首次推送请用「推送」里的设置上游」，
    而界面里**根本没有这个能力**。core `push()` 加 `set_upstream`，
    命令 / `api.ts` / `store.push`（可选第 4 参，老调用点不用改）逐层透传。
  - 前置检查（不给分支名就拒绝）**不是新行为**：实测 `git push --set-upstream origin`
    本来就被 git 自己拒绝（`fatal: The current branch main has no upstream branch.`）。
    检查只是把契约说死、少起一个进程、不依赖那句英文的措辞。
    这条在反向变异里表现为「删掉守卫测试仍然绿」——**它不是判别门，记在这里以免被当成一道门**。
- [x] **窗口标题 = 仓库名 — 分支**（2026-10-12）
  - `src/windowTitle.ts` 纯函数（4 例）+ `getCurrentWindow().setTitle()`，
    放在 TopBar 的 effect 里（它同时持有 repo 与 status）。浏览器里跑 `npm run dev` 时
    没有窗口可命名，`try/catch` + `.catch` 只留 console.warn，不给用户报错。
- [x] **emoji → 16px 线性 SVG**（2026-10-12）
  - `src/components/icons.tsx`：`IconSync` / `IconCaret` / `IconBranch` / `IconMoon` /
    `IconSun` / `IconMenu`，`stroke="currentColor"` 跟随主题，尺寸恒定。
  - 顶栏从 7 个控件减到 2 个图标按钮（主题、菜单）：快捷键面板入口收进菜单，
    `.kbd-off` 从「半透明 + 删除线」（对 SVG 无意义）改成琥珀色 + 菜单项文案
    「快捷键…（已关闭，点此重开）」；`Settings.tsx` 与 README 里那句
    「重开入口是顶栏 ⌨」同步改掉，否则文档会指向一个已经不存在的按钮。

### 验证（2026-10-12 标题栏重构）

- `gitcraft-core` **69 passed**（+2）：
  `counts_are_only_claimed_when_a_branch_actually_tracks_something` 是端到端门——
  新分支 `status` 必须 `upstream: None` 且两个计数是「假的 0」；
  `push -u` 之后必须读到 `origin/<branch>` 且 `0/0` 变成**真的**同步；再提交一条必须 `(1, 0, true)`。
  `a_first_push_can_record_its_upstream` 覆盖 `-u` 的透传与被拒时不留半个上游。
- 前端 **134 passed**（7 文件，+12）：`syncControl` 6 例（无上游绝不显示 `↑0 ↓0`、
  无远程是另一句话、有上游的 0/0 才算同步、菜单带上提交数、状态未读出不给数字、
  主按钮只承诺 Fetch）、`windowTitle` 4 例、store 2 例（`setUpstream` 透传且默认 false、
  设置上游后重读分支）、以及原有 push 断言补上第 5 参。
  `tsc --noEmit` 干净；`npm run build` 444.45 kB / 145.66 kB gzip，CSS 31.53 / 6.70 kB；
  `src-tauri` 编译通过（能力文件里的标识符是构建期校验的，能编过说明两个权限名真实存在）。
- 反向变异：删掉 `parse_status` 的 `# branch.upstream` 分支 → 核心门转红；
  删掉 `syncModel` 的无上游分支 → 前端两例转红；两处均从备份还原并 `diff -q` 确认。
  `push` 的守卫**故意不算门**（见上），git 自己兜底。
- 界面证据分两级，不含糊：浏览器（`localhost:1420`）截图确认了合并后的这一行**在 DOM 里成立**——
  仓库名 / 分支 chip / 同步分裂控件 / 右侧两个线性图标排布正常、无仓库时 chip 不显示。
  **原生部分没看到过**：交通灯是否压住内容、`setTitle` 是否真的改了窗口标题，
  只有真机窗口能证明（本机 `osascript` 被拒辅助访问，拿不到窗口标题；
  截屏会拍到整个桌面，没有代跑）。`npm run tauri dev` 已开着，等一次肉眼确认。
- 文档：README 的功能、代码地图（标题栏一行 / 新鲜度 / 网络结论 / 设置上游 / 图标）、
  架构树与测试数字（前端 122 → **134**、核心 67 → **69**）均已同步，数字来自上面两次测试的真实输出。

### WebView 性能三刀（2026-10-14 立项）

起因是一次「要不要把 UI 换成 Zed 的 GPUI」的评估。结论是**不换**：那要重写
`src/**` 的 8289 行 TS/TSX 与 `styles.css` 的 2229 行，而 GPUI 唯一真正解决的
GitCraft 痛点是「大补丁跨 IPC 序列化」和「滚动帧率」——这两条在 WebView 里也有便宜的解法，
于是把它们拆成本清单。**GPUI 这条路不是被感觉否掉的，是否掉在「中文输入法在 GPUI 里是自建通路，
而输入法正是 GitCraft 第一产品约束」这一条上**；若将来有实测证据推翻，再重估（发布版 crate 仍落后
Zed 主干、不保证向前兼容，也是理由之一）。

三条都带判据，不允许只写「优化一下」。

- [x] **① highlight.js 从 `lib/common` 收成实际用到的语言集**（2026-10-14）
  - 事实（本机实测，非推测）：`DiffView.tsx:3` 静态引入 `highlight.js/lib/common`，
    实际注册 **36** 种语言；而 `EXT_LANG`（`:8`）只映射到 **24** 个 language id
    （34 条扩展名映射去重后），映射表之外的文件一律走 `escapeHtml` 根本不上色 ——
    多出来的 12 种是**可证明的死重**。
  - **顺手挖出一个真 Bug**：`EXT_LANG` 里有 `dart`，但 `lib/common` **没注册 dart**，
    于是 `.dart` 文件每次都走进 `:55` 的 `catch` 静默退回纯文本 ——
    界面承诺了颜色、实际永远没有，而且没有任何地方会发现。
    「映射表里的语言必须都已注册」这条不变式正是为了抓住它而存在。
  - 做法：新增 `src/languages.ts` —— `highlight.js/lib/core` + 显式注册 24 种，
    `EXT_LANG` / `langOfPath` / `escapeHtml` / `highlightText` 从 `DiffView.tsx` 搬进来
    （高亮策略与「高亮不了怎么办」从此在同一处）；`DiffView.tsx` 只剩 1 处调用点与 1 行 import。
    **dart 的选择是「补注册」而不是「从映射表删掉」**：映射表就是产品承诺，`.dart` 本来就该有颜色，
    测试断言的是「承诺与兑现一致」，不是「承诺可以变小」。
  - 判据与反向变异（两条都真跑过，源文件从 `/tmp` 备份还原并 `diff -q` 确认字节一致）：
    ① 从注册表删掉 `dart` → **3 例转红**（不变式、集合相等、`.dart 真的能上色`）；
    ② 多注册一个映射表用不到的 `haskell` → **恰好 1 例转红**（`也不注册映射表根本用不到的语言`），
    其余 6 例仍绿 —— 两条门各自只管各的，没有互相顶包。
  - 实测收益（同一台机器、同一份 `npm run build`，改前基线是 2026-10-12 记的同一个数）：
    JS **444.45 → 425.29 kB**（raw），gzip **145.66 → 135.18 kB**（**−10.48 kB，约 −7.2%**）；
    构建模块数 124 → 111；CSS 不变（31.53 / 6.70 kB）。
  - 测试：前端 **141 passed**（8 文件，+7 例来自 `src/languages.test.ts`）。
  - 浏览器侧证据：`npm run dev` + `localhost:1420` 里直接 `import('/src/languages.ts')` 执行 ——
    `listLanguages().length === 24`、`dart` 输出 `<span class="hljs-class"><span class="hljs-keyword">class</span>…`、
    `rust` 仍正常、未映射语言退回转义文本；控制台除 vite 连接与 React DevTools 提示外无错误。
    **没看到过的**：真应用右栏里一条 `.dart` diff 的实际渲染（浏览器里没有 Tauri invoke，点不出补丁）。
  - 对 ② 的影响：现在首屏仍**静态**引这 24 种（② 才把它们挪出首屏 chunk），
    所以 ② 的判据要改成「动态 import `src/languages.ts` 的语言表」，别重复注册。
- [x] **② 语法高亮延后加载（这才是「首屏 chunk 拆分」的正确形状）**（2026-10-14）
  - 做法：把「承诺」（扩展名 → 语言，纯数据）与「兑现」（highlight.js 本体）分成两个模块 ——
    `src/languages.ts` 只剩 `EXT_LANG` / `langOfPath` / `escapeHtml` / `plainCode`，**一行 highlight.js 都不引**；
    注册与 `highlightText` 搬进 `src/highlight.ts`；`DiffView.tsx` 用 `useHighlighter()`
    在挂载时 `import("../highlight")`，拿到之前整栏走 `plainCode`（转义纯文本，绝不空白）。
    结果缓存在模块作用域变量里，重开右栏不会再取一次。
  - 判据（两条都真跑过）：**产物级** —— 构建后 `dist/assets/index-*.js` 里 `grep -c hljs` 得 **0**，
    hljs 全在独立的 `dist/assets/highlight-*.js` 里；**源码级** —— `languages.test.ts`
    用 `?raw` 读 `DiffView.tsx` 与 `languages.ts` 的源文本，断言只有动态 import、映射层不引 highlight.js。
  - 反向变异（改回静态 import 一处，两个证据同时转红，之后从 `/tmp` 备份还原并 `diff -q` 确认字节一致）：
    源码级那条测试转红；**产物同时退化回 426.71 kB / gzip 135.84 kB 且没有 lazy chunk** ——
    说明源码级断言不是假门，它跟用户真正下载的东西是同一件事。
  - 实测收益：首屏 JS **425.29 → 280.77 kB**（gzip **135.18 → 90.96 kB**，**−44.22 kB，约 −33%**）；
    另有 `highlight-*.js` 145.61 kB / gzip 44.16 kB，只在第一次要上色时加载。
    相对 ① 之前的原始基线（444.45 / 145.66）合计 **−164.70 kB raw、−54.70 kB gzip（−37.5%）**。
    CSS 未变（31.53 / 6.70 kB）—— `github-dark.css` 仍随主包，因为它只有几 kB 且没颜色时也无害。
  - **门自己立过一次功**：拆分时把 `highlightText` 的空行分支写成 `if (!lang)`，
    丢掉「空行返回一个空格以保住行高」——`npm test` 立刻转红（144 例里唯一一条），
    修回 `if (!text || !lang) return plainCode(text)`。这条断言不是摆设。
  - 未引入任何新依赖：测试里读源文件用的是 vite 原生 `?raw`，类型靠
    `/// <reference types="vite/client" />`（vite 本来就是 devDependency）。
    中途试过 `node:fs`，`tsc` 因为项目没装 `@types/node` 直接报错，所以换掉了 ——
    记在这里以免下次又想去加依赖。（例外：读 CSS 资产时 `?raw` 在 vitest 下是空串，只能读盘，
    写法是模块名拼成 `"node:" + "fs"` 绕过 TS2307，见「视觉打磨第二批」那条坑。）
  - 前端 **144 passed**（8 文件）。
  - **没看到过的**：真应用里「先纯文本、chunk 到了再变彩色」这一帧的实际过渡
    （浏览器里没有 Tauri invoke，右栏点不出补丁）。已验证的是产物切分、回退路径的单测、
    以及 `import("/src/highlight.ts")` 在 vite dev 下能正常取到并注册 24 种语言。
    → 2026-10-14 更新：「点不出补丁」这一句作废，浏览器里的 invoke 桩能点了（见「验证方式」）；
      但「先纯文本再变彩色」那一帧本身仍没肉眼看到（桩里只轮询到彩色 span 出现为止）；
      ② 的 chunk 判据本来就由 `languages.test.ts`「高亮器留在首屏之外」守着，此前漏记了这一句。
  - 诚实边界：dev 冷启动白屏 ~7s 主要是 Vite 预打包 + 全量依赖，**和生产首屏是两件事**，
    两个数字要分开量，不能拿一个承诺另一个。本轮只证明了后者。
- [x] **③ 大补丁：先加闸门，再谈流式**（2026-10-14 做完，且**计划里那一刀砍错了地方**）
  - 缺陷形状（`git.rs` 实测）：**只有展示「内容」的两条通路有 2 MiB 上限** ——
    `rev_file_text`（`:624`，守卫 `:628`，「该版本约 N KB，未展示内容」）与
    `untracked_patch`（`:882`，守卫 `:886`）。而右栏常驻 diff 每次都调的三条补丁通路
    `working_patch`（`:867`）/ `range_patch`（`:916`）/ `commit_patch`（`:931`）**一个字都没拦**，
    整个 stdout 变成一个 JS 字符串；前端再一次性 `parsePatch` + 逐行 `hljs.highlight`，没有虚拟化。
    一个改了 lockfile 或 vendored 目录的提交就能把主线程钉住。
  - **原设计的前提是错的**：做法 b 写的是「`parsePatch` 与高亮按 hunk 分片」，隐含假设
    「解析 + 逐行 highlight 是成本」。量下来不是 —— 5 MB / 139 551 行的补丁，
    `parsePatch` 加每行 `hljs.highlight` 一共 **52 ms**。真正冻住界面的是**一行一个 DOM 节点的挂载**：

    | 补丁 | 行数 | React 渲染 | 布局 |
    | --- | --- | --- | --- |
    | 308 KB（本仓库真实大提交） | 283 | 45 ms | 125 ms |
    | 1 MB（合成 lockfile） | 28 875 | 921 ms | 1043 ms |
    | 2 MB | 56 544 | 2023 ms | 2382 ms |

    耗时随行数线性涨，与字节数无关 → 判据从「按 hunk 分片解析」改成
    「按**行**限量挂载 + 字节上限兜底」。这张表同时写进 `src/patchGate.ts` 的头注释，
    因为下一个读它的人一定会想去调 `ROW_BATCH`。
    计时是 `performance.now()` 卡在 `flushSync` 前后取的；**rAF 轮询那套作废** ——
    这个 webview 是后台态（`visibilityState=hidden`），rAF 一次都不触发，
    `setInterval(16)` 也只有约 1 次/秒（见「验证方式」那条坑）。
  - 做法 a（已做）：`src/patchGate.ts` 的 `PATCH_FOLD_BYTES = 2 MiB`，与 gitcraft-core 那两条内容通路
    **同一个数**（界面只能有一个「太大」的定义，`patchGate.test.ts` 里就钉这个一致性）。超限时
    `DiffView` 连 `parsePatch` 都不调（`fold.folded && !forced ? "" : …`），出一条 `.diff-fold`：
    「补丁 4.2 MB，超过 2.0 MB 上限，已折叠。一行一个 DOM 节点，全部挂载会让界面停三四秒。」
    两个明确出口：「仍然渲染」（`setForced(true)`，把卡顿换回完整补丁）与「复制整份补丁」
    （剪贴板不可用落 `setError`，不静默）。差异面板头部本来就有一个「复制补丁」，
    这张卡里重复一次是有意的 —— `FileHistory` 那一站没有面板头，而折叠卡是两处共用的。
  - 折叠点必须落在**文件边界**。按字节数直接砍（`end = cut`）会留下半个 hunk：界面照样画得出来，
    但行号是假的。`foldPatch` 因此退到上一个 `diff --git`；单个文件就超限时退到行边界
    （宁可少显示，不要整个不显示）。快路径 `text.length * 3 <= limit` 直接返回，
    连 `split` 都不做 —— 它在行为上不可观测（错的后果是慢不是错），
    所以**没有**伪装成断言，只记在这里和测试注释里当作实现事实。
  - 做法 b（已做，形态改了）：`ROW_BATCH = 400` 行一批（≈2.5 屏 @21px 行高），底部 `.diff-more`
    显示「已渲染 x / y 行」+「继续 N 行」。两个触发口：IntersectionObserver（`rootMargin: 800px`；
    这个哨兵必须留在正常流里，所以它不是 sticky）和那个按钮。
  - 判据（`src/patchGate.test.ts`，7 例）：`patchBytes` 与 `TextEncoder` 逐一对齐（1/3/4 字节三种都要真的出现，
    否则整条断言只是「ASCII 恰好相等」）、没超限的补丁原样返回、超限的断在文件边界
    （判别性的三条：断点之后紧跟 `diff --git`、`head` 结尾是某段最后一行、`parsePatch(head)` 每段行数一致）、
    单文件超限退到行边界、`nextRowCount` 只往前走且不越过总行数、上限 == 2 MiB 且
    `formatBytes(2 MiB) == "2.0 MB"`、DiffView 源码真的过闸门（读 `?raw`，折叠时不解析）。
  - **又抓出一条假门**：「超限…文件边界」那条最初用 `limit = 2000`，而 fixture 每段约 1936 字节 ——
    上限**恰好**落在段首，`end = cut` 与 `end = lastFileStart` 结果相同，把 `foldPatch` 改坏测试仍然全绿。
    改成 `limit = 3000`（真的落在某段中间）再补上面那三条断言，同一个变异才恰好转红 1 例。
    「断言写在了缺陷复现不了的层」这件事在这个项目里已经是反复出现的那一类，不是偶然。
  - 反向变异（本轮全部实测，各恰好红 1 例，还原后 162 复绿）：
    `const end = cut` → 「文件边界断开」；
    `const text = patch`（绕过折叠）→ 「DiffView 真的经过闸门」；
    代理对按 3 字节计（`n += 4; i++` → `n += 3`）→ 「patchBytes 数的是 UTF-8 字节」；
    `nextRowCount` 去掉 `Math.min` → 「行批次永远不会越过总行数」。
  - 数字：前端 **155 → 162 passed**（12 文件，新增 `patchGate.test.ts` 7 例）；`cargo test` **70 不变**
    （这一刀全在前端）；`npm run build` JS **282.40 → 284.53 kB**（gzip **91.43 → 92.38**，
    +2.13 raw / +0.95 gzip ≈ 闸门模块 + 两张新卡片），CSS **33.57 → 34.02 kB**（gzip **7.07 → 7.14**）；
    `highlight-*.js` 仍 **145.61 / 44.16** —— 闸门没把高亮拽回首屏。
  - **这一刀没碰的部分（诚实边界）**：后端仍然把整条 `git show` 的 stdout 变成一个 JS 字符串，
    那份内存与 IPC 序列化**一点没省**。这一轮解决的是「挂 DOM 停三四秒」，
    不是「大补丁根本不该进浏览器」。前者是用户能感知的卡死，后者目前只有量出来的 52 ms 支撑，不构成问题。
  - **没看到过的**：`.diff-fold` / `.diff-more` 的实际观感（截图通道这轮拿到的是后台态，右栏没进画面）；
    IntersectionObserver 那条自动增长的路**一次都没被触发过**，只手动点过「继续 N 行」；
    「真实 308 KB 提交」的 45/125 ms 是浏览器桩里的数，不是 Tauri 窗口里的肉眼停卡顿。
  - 明确不做（更新版）：`tauri::ipc::Channel` / `asset:` 把补丁以 `Uint8Array` 分块送进前端
    —— 仍不做，且现在有了理由：量出来序列化不是瓶颈（5 MB 全程 52 ms），等真有人抱怨 20 MB 提交的
    **传输**耗时再谈。虚拟化列表**不用问了**：`ROW_BATCH` 就是它的无依赖版本（一次只挂 400 行）；
    真要全虚拟化（1 万行也只挂可见那 30 行），得先量出批次挂载本身不够。

### 框架评估第二条：egui / eframe（2026-10-16）

用户提出的参照是 **ArtCraft 套件**（PhotoCraft 对标 Photoshop、PdfCraft 对标 Acrobat 等）。
这条参照有力，而且**打掉了本文件里我上一轮论证的一条论据**，所以按事实记下来。

**参照项目核实到的事实**（GitHub `storytold/*` + 报道，非推测）：
- 前 Square 工程师 Brandon Thomas 带四人团队、以 Claude Opus 5.5 辅助编码，纯 Rust 从零净室重写；
  套件已从 7 款扩到 12 款（Photo/Vector/Film/Light/Effect/Design/Pdf/Word/Sound/Deck/CAD/Grid）。
- **PhotoCraft 的界面就是 egui**（README 自述 "a thin egui UI on top"），底下是自研 wgpu 合成器
  + 写时复制瓦片；53 种工具、16 种调整图层、图层样式、文字排版、压感笔刷；
  为中日韩字形专门做了 `craft-fonts`；MIT/Apache-2.0；macOS/Windows/Linux/FreeBSD/WebAssembly 同源。
- PhotoCraft 9-30 发布、10-11 约 43k star；**PdfCraft 与 PhotoCraft 同标「早期 Alpha」**，
  且官方把「既有文字编辑」「**中文等非拉丁文字 OCR**」「Office 格式转换」列在缺口里；
  试用者对 WordCraft 的评价是「有些按钮按下去还没反应，界面出现一大片空白」。

**被打掉的那条论据**：我原来把「右键菜单 / 模态 / tab / 分栏 / 自动补全都得用 Rust 重搭」算作迁移成本。
PhotoCraft 证明 egui 扛得住比 GitCraft 大一个数量级的 widget 复杂度，这些在 egui 里都是常规件。
**这条不再作为否决理由。**

**否决仍然落在文本上，但要说是两条不同性质**：
1. **显示别人的文本 vs 显示自己的文本。** ArtCraft 需要字形的是有限集（界面串 + 用户敲的排版文字），
   所以 `craft-fonts` 这种「按需造字体子系统」是合算的解。GitCraft 的界面 90% 是**别人仓库里的任意内容**：
   中文提交信息 + emoji、日文分支名、GBK 老仓库解出的 U+FFFD、任意 Unicode 的文件名与文件内容。
   这里没有可打包的有限集，而 WebView 从系统字体栈白拿。**注意**：这条不是「egui 做不到」，
   是「必须先退休的最大风险」——连全力赶工的 ArtCraft 也把非拉丁文字列在缺口里。
2. **它是围着像素的控制面板，GitCraft 本身就是文本。** diff 要的是 28 875 行等宽文本的跨行选择与复制、
   语法高亮 + 单词级着色、行号槽，外加四个多行编辑面（提交信息 / amend / .gitignore / 冲突三栏）。

**成本要说全**：`gitcraft-core`（git / provider / ssh / preview / update / db，72 例 cargo 测试）
**一行都不用改**，要重写的只是 `src/**` 的 9388 行 TS/TSX + 2365 行 CSS，以及那批读源文件钉视觉不变量的
前端判别门。真正说不清的是**迭代速度**：10-14 一天四批视觉返工全靠 CSS + 真窗口截图 + DOM 几何；
PhotoCraft 的布局有权威答案（照抄 Photoshop），一次写完就对了，而 GitCraft 的界面还在探索期。

**结论：不换。** 且判据要写成可推翻的形式——
> 一个单日 spike 只需回答两件事：① 用真实含 emoji 的中文提交信息把 `git log` 原样显示出来不出现豆腐块；
> ② macOS 拼音往多行框里写一段提交信息（合成态可见、候选框位置正确、不吞键）。
> **任一不过结案；两条都过则重开迁移评估**，因为那时最大的风险已经退休。

顺带收下 PhotoCraft 真正该学的**另一半**（它与「用不用 Rust」无关）：
> 文档模型与命令引擎同 UI 分离，**所有 UI 操作映射到 500+ 条标准化命令，CLI / JSON 通道 / MCP 与 GUI 共用一套指令集**。

GitCraft 已有前半（`gitcraft-core` 零 UI），**没有后半**：`src/shortcuts.ts` 的 `COMMANDS` 只覆盖有键位的
十几个动作，右键菜单里几十个操作没进命令表。补成真正的命令层，直接买到三样：
命令面板（⌘P 执行任何操作）、**可被 Agent/MCP 驱动**（SourceTree 做不到，这是「比它更好」最省力的一条）、
以及界面层永久可替换。已排进 TODO。

### Spike 结果：egui 0.37 的两条判据实测（2026-10-17）

工具：`uiprobe/`（在仓库里，证据可复查；不是产品代码）。`eframe 0.37 + epaint 0.37（开 color_fonts）`，
一个窗口同时跑两条判据，截图驱动。`--bare` = 一个字体都不注册，只测默认行为。

**① 中文 + emoji 渲染：条件通过。**
- **不注册任何字体**，stock eframe 就把本仓库真实 `git log` 的中文、`✓ ♠`、彩色 🎉 全渲染出来了，无豆腐块。
  原因：eframe 默认 feature 含 `system_fonts`（`eframe-0.37.0/Cargo.toml` 的 `default`），
  epaint 0.37 有动态字体提供者（`epaint/src/text/font_provider.rs`：「Discovered fonts are found on demand
  … e.g. among the system fonts」）。**这条直接推翻了 10-16 记下的「WebView 从系统字体栈白拿、egui 拿不到」——
  egui 现在也白拿了。**
- 但**同一个窗口里 U+FF1A 全角冒号「：」在 Proportional 家族是豆腐块**，Monospace 家族正常。
  也就是说自动兜底**按字符零敲碎打、覆盖不全**，任意 Unicode 的风险没有退休，只是变小了。
  产品化必须自带一套全覆盖 CJK 字体（思源黑体 / Noto Sans SC，OFL 可打包），不能只靠 provider。
- 系统 `.ttc` 集合能按 face 注册且粗细正确：Hiragino Sans GB 4 face、STHeiti 2、Songti 8 全部可用。
- **踩到两个硬坑（都值一条工程约束）**：
  1. 字体解析失败 / face index 越界 → `panic!` 落在不可 unwind 的闭包里 → **进程直接 abort**，
     不是回退（`epaint/src/text/family.rs:51`、`font_definitions.rs:249`）。产品里字体注册必须
     「链里只放注册成功的面」，缺文件降级而不是崩。
  2. `PingFang.ttc` 在 macOS 27 **不存在于 `/System/Library/Fonts/`**，家族仍注册，真实文件在
     `/System/Library/PrivateFrameworks/FontServices.framework/Resources/Reserved/PingFangUI.ttc`
     （268 face）。按这个路径注册**能加载、认 CJK 码位、但画出来是空白**——比豆腐块更糟，
     因为它不触发回退。结论：**别按路径吃 PingFang。**

**② macOS 拼音：脚本部分通过，真人那一半仍然欠着，且我这边测不了。**
- egui 0.37 的 IME 是一等公民：`ImeEvent::Preedit{text, active_range_chars}` / `Commit`、
  专门的合成态视觉（`Visuals.ime_composition`，macOS 默认不走 legacy 选区样式）、
  由 TextEdit 光标矩形驱动 `window.set_ime_cursor_area(...)`（候选框位置的接口在）。
  winit macOS 的 `setMarkedText:selectedRange:replacementRange:` 会把 marked text 转成
  `Ime::Preedit` 往上抛（`winit-0.30.13/src/platform_impl/macos/view.rs:334`）。
- 脚本注入 `Preedit("nihao")→Preedit("你好")→Preedit("你好，世界")→Commit` 的实测结果：合成态**可见**
  （下划线跟着合成段）、逐次替换不叠加、Commit 后**不重复插入**、缓冲最终正确。
- `egui-winit-0.37.0/src/lib.rs:726` 起，作者手写了 macOS 15 双拼 / Wayland-fcitx5 / Windows 拼音
  三种环境下逐事件的实测表 —— 这条路径是**有人在真机上维护**的，不是纸面支持。
- **没看到过的（必须说全）**：真人切拼音往框里打字。macOS 辅助功能权限未开
  （`System Events UI elements enabled = false`），我无法驱动按键，所以**候选框实际位置、
  是否吞键、退格/回车语义**这三件只有你本人能判。spike 判据②的原始表述**尚未结案**。

**对迁移决定的影响（不改结论，但换理由）**：
10-16 的否决理由里，「egui 拿不到系统字体栈」这条**作废**；仍然成立的是「任意 Unicode 覆盖不全」
（全角冒号就是现证）和「它是围着像素的控制面板、GitCraft 本身就是文本」。
所以：**不换，但换不起的理由少了一条**；真要把②结掉，需要你花 30 秒——
`uiprobe/target/debug/uiprobe --bare` 开窗后切拼音往框里打一句提交信息。
无论②结论如何，**命令层都是无悔投资**（见上），它已经落地：`src/commands.ts` ~75 条命令、
`runCommand(id, target)` 单一入口、破坏性确认收进命令层。

### 方向变更：复刻 SourceTree 界面并超越（2026-10-16）

用户定调：「我想复刻 sourcetree 界面，比它更好」。这是**方向性变更**，与 10-15「双左栏合一」有直接冲突
（SourceTree 恰恰有一条左侧栏），所以动手前必须先把两件事钉死：**复刻到哪个层次**（布局照抄 / 交互语义照抄）、
**「更好」具体指哪几件事**。且**不能凭印象复刻**。

关于「先拿官方文档 + 实际截图」这句：文档这条**已经作废**，不必再找——本机就装着权威事实源
`/Applications/SourceTree.app`（macOS **4.2.19**），它自己的 `.nib` 界面定义和 `zh_CN.lproj/Localizable.strings`
（888 条键值）比任何博客的布局描述都硬。截图这条**仍然欠着**，见下面对照表末尾的「没看到过的」。
对照表本身见下一节。

### SourceTree 界面对照表（2026-10-16，证据 = 本机 4.2.19 应用包）

> 逐块从 `/Applications/SourceTree.app/Contents/Resources/{English,zh_CN}.lproj` 里读出来，
> 每条都能在 nib 字符串或 `Localizable.strings` 的某个键上指认。引号里的中文是它自己的本地化文案。

**① 工具栏**（`GitRepoWindow.nib`，条目标识 `Repo - *`）
Refresh / Fetch / Pull / Push / Commit / Branch / Merge / Tag / Stash / Rebase / Terminal / Show in Finder /
Remote / Reset / Git Flow，另有 Create Pull Request；每项带说明文案（`Pull commits from a remote`）。
GitCraft：`src/components/TopBar.tsx` 只有 Fetch 一颗 + Pull/Push 折叠菜单 + 汉堡菜单。
**缺一级入口：Commit、Branch、Merge、Tag、Stash、Terminal、Git Flow。**

**② 左侧栏**（类名 `STSidebarOutlineView` / `STSidebarCell` / `SidebarHeaderViewCell`）
是**可展开的大纲树**，不是平铺列表：顶层=书签（仓库），展开后固定子项「WORKSPACE（工作区）、历史、分支、标签、
远端、已贮藏、子仓库、子树/子模块」，顶部有搜索框（`AccessibilitySidebarSearch`）。侧栏右键**动词跟着对象走**：
`检出 %@`、`拉取 %@（已跟踪）`、`推送到 %@（已跟踪）`、`拿來 %@`（Fetch）、`升级到 %@`、`合并 %@ 至 %@`、
`从 %@ 拉取...`、`删除 %@`、`添加子模块...`、`重命名...`、`删除远程分支`、`跟踪远程分支`。
GitCraft：10-15 删掉了侧栏，换成 Nav 图标条（文件状态/贮藏/历史/分支/远程，5 个 tab 两组）+ RepoTabs 顶部仓库页签。
**这一条就是和 10-15 决定的正面冲突点，见文末问题①。**

**③ 工作区页**（`FileStatusView2.nib`）
文件列表有**六种模板**（nib 里 `View - * Template` 一字不差）：`Tree view` / `Flat list (single column)` /
`Flat list (multiple columns)` / `No staging` / `Split view staging` / `Fluid staging`。
前三种是「文件怎么摆」，后三种是「暂存语义怎么做」——**两个正交维度，用户可选**，不是我们以为的一种布局。
列：Filename + Path + `Pending files, sorted by path`（可按路径排序）。
右键全量：Stage file / Unstage file / Unstage from index / Discard file / Add file / Remove / Ignore file /
Ignore... / Move... / Revert... / Revert to Revision... / Commit Selected... / Create Patch... / Apply Patch... /
External Diff / Diff vs parent / Diff vs merged / Log Selected... / Annotate Selected...（=blame）/
Open in Terminal / Show In Finder / Quick Look / Copy... / Copy Path To Clipboard / Mark Resolved /
Mark Unresolved / Resolve Conflicts / Resolve Using 'Mine' / Resolve Using 'Theirs' / Restart Merge /
Launch External Merge Tool / Track file type in Git LFS / Stop Tracking / Custom Actions / Collapse All /
Expand All / Show only / More / Sort by / Filter。差异区另有 Ignore whitespace / Show whitespace /
Lines of context。
GitCraft 已覆盖 stage/unstage/discard/ignore/文件历史/blame；**缺 Create-Apply Patch、Move、
Resolve Using Mine/Theirs、LFS track、Custom Actions、显示模式与排序切换**。

**④ 历史页**（`LogView.nib`）
`STLogTableView` + 单元格类 `STGraphCell` / `STLogDescriptionCell` → 图形列与文字列是两个 cell，
和 GitCraft「SVG 图 + HTML 行」的分栏思路同源。列：Revision / Author / Date / Graph。
行右键：Copy SHA-1 to Clipboard / Copy Path To Clipboard / Log Selected... / Diff vs parent / Push revision... /
Reset branch to here... / Revert/Reset To Commit... / Branch... / Tag... / Update To... /
Abort·Continue cherry-pick / Jump to revision（`该修订版本不存在…`）。
视图开关（中文文案现成）：`紧凑视图`、`仅首个父级` / `所有父级` / `所有分支`、`隐藏远程分支`、
`隐藏分支标签` / `显示标记标签`。
GitCraft **缺图遍历开关（仅首个父级 / 所有父级）和「隐藏远程分支」这类显示过滤**。

**⑤ 搜索**（`SearchView.nib`，带 `MGScopeBar`）
搜索是**带作用域条的独立面板**，维度：Commit Message / Commit SHA / File Changes / Author / Branch / Filename /
Path / Date / From: / Item，结果表另有 Custom Actions / External Diff / Quick Look / Annotate Selected。
GitCraft 只有一个提交信息 substring 过滤（`/` 聚焦）。**这是「它还有一块我们完全没有」的实测证据。**

**⑥ 提交面板**（`CommitPaneView.nib`）
`STCommitPaneViewController` 里挂的是 **WKWebView**（连 `WKUserContentController`/`WKConfiguration` 都在），
即 SourceTree 自己的提交信息编辑区就是 WebKit 富文本，不是 NSTextView。
对我们的意义（诚实）：「React 做 Git GUI 不如 native」这条对 SourceTree 不成立。记下来，
但**不推翻** 10-16 已定的「egui 之争收敛成一条可跑的 spike」。

**⑦ 命令日志**（`TaskWindow` / `CommandOutputView` / `ErrorOutputWindow` / `TaskMonitor` / `NetworkMonitor`）
`CommandHistoryTableTitle`「命令历史」+ 一整套 Task* 窗口 → 每条 git 命令的 argv / stdout / 耗时都可回查。
GitCraft 只有 `lastAction` 一条 toast 加一个错误串。与「命令层」TODO 是同一件事的两面（可驱动 / 可观测），
**优先级应一起提，不做两次**。

**⑧ 它有一整块、我们还没碰的**
git flow 全套（FlowInit/Start/Action/FinishFeature/FinishRelease/PreviewView）、submodule+subtree
（AddSubmodule / Edit subtree details / Pull-Push subtree / SubmoduleCheck）、`VisualMergeWindow`（内置三向合并）、
LFS（GitLfsInit/GitLfsTrack）、CreatePullRequest / CreateRemoteRepo / HostingServices / Pipelines*（Bitbucket CI）、
SSHView + SSHConfig（内置生成密钥）、WelcomeWizard、RepoSettings / Preferences、IgnoreSheet、ArchiveSheet（导出 zip）、
RevertSheet / RevertFilesToRevision、MultipleFileLogWindow、BlameWindow、NetworkMonitor。
其中 git flow / submodule / LFS track / Create-Apply Patch / Custom Actions 之前只在 TODO 里挂过名，现在有 UI 事实可参照。

**没看到过的（边界）**：这张表**全部来自字符串与类名，没有一张真实截图**。所以以下仍是未知——各块实际宽高与
分栏方向（侧栏多宽、`Split view staging` 是上下还是左右、差异区占比）、配色与图标、hover/选中态、
以及 `Fluid staging` 到底怎么个「流动」法（只有名字没有交互）。这几项**别拿当已知施工**；
下一轮要么把 SourceTree 打开截图，要么由用户口述。

**两个必须用户拍板、我不擅自动手的问题**：
① 侧栏回来吗（直接推翻 10-15）？② 复刻层次=布局照抄 / 交互语义照抄 / 只求「它有的功能我们都有」，
以及「更好」具体指哪几件（候选：命令层可被 Agent 驱动、搜索作用域面板、显示模式与暂存语义可选、内置 git flow）。

### 复刻拍板（2026-10-20）：层次=界面完全一样（除软件名），「更好」=性能与功能

用户一句话把两个挂着的问题都结了：「**复刻要求界面完全一样（除了软件名），更好是指要超越 sourcetree，性能/功能**」。

- **侧栏回来。** 20-15 那次的「双左栏合一（删侧栏、入口并入 RepoTabs）」**就此作废**：SourceTree 有一条
  贯穿全高的左侧大纲栏，界面要完全一样，`Nav` 图标条 + `RepoTabs` 顶部页签必须让位给它。
- **复刻层次 = 布局照抄 + 交互语义照抄**，不是「功能对齐就行」。
- **「更好」收成两条可量的轴**：① 性能（大仓库、28 875 行级别的 diff 不卡）；② 功能（它有的我们都有，
  再加上它做不到的：命令层可被 Agent/MCP 驱动、命令日志回查、作用域搜索）。
  美观、主题、动效**不在**比较轴上——界面既然要完全一样，这三样天经地义不参与。

### 复刻规格：SourceTree 主窗口实测几何（2026-10-20，证据 = 真截图）

上一张表全部来自字符串与类名，「没看到过的」那一段这次补上了：把本机 4.2.19 打开、
用 `screencapture -l<windowID>` 抓到窗口，**两张参考图已入库**
（`docs/reference/sourcetree-4.2.19-repo-window.png` = 主窗口 1050×545 pt @2x，
`docs/reference/sourcetree-4.2.19-bookmarks.png` = 书签窗口 480×659 pt）。
下面每条都能在图上指认，几何是从 PNG 里量出来的（图像坐标 2x，括号是 pt）。

**几何**
- 侧栏右边界 x=282 → **141 pt**，从工具栏下方一直贯到底。
- 工具栏带 y≈0..110 → **55 pt**（含红绿灯与「gitx (Git)」标题）。
- 内容筛选条 y≈110..150（所有分支 / 显示远程分支 / 层级顺序 / 紧凑视图，右侧「跳转到:」）。
- 日志列：图表 x≈282..380、提交 ≈380..470、描述占中间整段、作者起于 ≈1494（747 pt）、日期起于 ≈1870（935 pt）。
- 日志带 / 底部带分界 y≈600 → **300 pt，即窗口高的 55%**。
- 底部「文件列表 | 差异」竖分界 x≈1152 → **576 pt**（文件列表 435 pt、差异 474 pt）。

**结构事实（比几何更要命，从 nib 字符串读不出来）**
1. **没有「工作区 / 历史」两个 tab**：就一页。上面日志，下面文件+差异；
   选虚拟行「Uncommitted changes」= 工作区，选某个提交 = 那个提交的改动。
   GitCraft 现在把它拆成两页，这是本次复刻最大的一处结构性差异。
2. 虚拟行「Uncommitted changes」常驻日志第一行，提交列与作者列都是 `*`，日期是本地化的「今天 12:08」，
   选中态整行蓝底白字。
3. 侧栏是**大纲**：`WORKSPACE`（灰字标题 + 显示器图标）下挂 文件状态 / 历史 / 搜索；
   再往下 分支 / 标签 / 远端 / 已贮藏 / 子模块 / 子树 各自成组，当前分支带蓝点 + 圆角灰底；
   **最底一条「筛选器」输入框**（不是顶部搜索）。
4. 工具栏是**图标在上、中文在下**的竖按钮，右对齐 11 个：
   提交 / 拉取 / 推送 / 抓取 / 分支 / 合并 / 贮藏 / 显示远程服务器 / 在 Finder 中显示 / 终端 / 设置。
   左侧只有「仓库名 (Git)」标题，没有面包屑、没有页签。
5. 文件列表分组标题自带**复选框**（已暂存文件=勾、未暂存文件=不勾，一键切整组），
   行 = 复选框 + 类型图标 + 路径 + 行尾「…」菜单；列表上方一条
   「待定的文件, 已依照路径排序」下拉 + 视图模式图标 + 「⋯」。
6. 差异区顶栏 = 文件类型图标 + 文件名 + 「…」；hunk 头是「块 1： 行 8-14」，
   右侧两枚按钮 **暂存区块 / 放弃区块**（块级暂存），行号两列（旧/新），增删整行淡红/淡绿底。
7. 书签窗口另成一窗：本地 / 远端 / 新建… 三个分段 + 「过滤仓库」搜索框，
   正文是**按文件夹分组**的树，行 = 头像 + 仓库名 + 右侧分支芯片；远程托管的仓库换服务图标并带未读角标。

**仍然没看到过的（这一轮的边界）**：`Fluid staging` 的实际交互、`Split view staging` 是上下还是左右、
各右键菜单的项序与图标、深色模式、搜索面板的作用域条长什么样、四个下拉各自的选项。
这些要么下一轮点进去再截（需要键盘/鼠标驱动，本机辅助功能未开），要么按 nib 推断并在施工时标注为推断。

### 项目更名：GitX → gitCraft（2026-10-16）

用户一句话定名：「项目更名，改为 gitCraft」。落法（三个问题问过，用户没选，按推荐默认执行并在此留痕）：
**显示层 `GitCraft`、标识符一律小写 `gitcraft`**；**不写任何迁移垫片**（与「能直接改就直接改、不留向后兼容」
的既有规矩一致）；**工作区根目录仍是 `~/Developer/superlbr/gitx` 不动**——本会话 cwd、Qoder 项目键、
项目记忆路径全部绑在这个路径上，改目录会当场断工具链，要改必须在会话外手动来。

改名过程中量到两件**以前没写下来过的存储事实**（都是这次去 `~/Library` 里翻出来才确认的）：

- **SQLite 库按 bundle identifier 分家**：[src-tauri/src/lib.rs:19](file:///Users/baorong/Developer/superlbr/gitx/src-tauri/src/lib.rs#L19)
  的 `db_path()` 拼的是 `~/Library/Application Support/com.gitx.app/gitx.db`，identifier 一改就是换库。
  实测：新库 `com.gitcraft.app/gitcraft.db` 在 12:15 被新建，旧库**原地留着没动**（里面那条 comic 书签是唯一的用户数据）。
- **WKWebView 的 localStorage 不按 identifier 分，按可执行文件名分**：落在
  `~/Library/WebKit/<可执行名>/WebsiteData/Default/<originHash>/LocalStorage/localstorage.sqlite3`，
  目录名从 `gitx` 变成 `gitcraft` 就是一次换存储。旧那份里只剩 3 个键
  （`gitx_active_tab` / `gitx_current_repo` / `gitx_shortcuts_on`）——**注意这 3 个键本身就是历史遗留**：
  代码里写过的 13 个键只留下 3 个，说明这个存储以前就被清空过（换 origin / 换机时），
  所以「localStorage 会跟着改名丢」不是推测，是已经发生过的事。
- 净效果：这次改名把界面状态清零了。新库 12:15:13 多出一条 comic 书签、tab 变成 history，
  这两件事**只可能是人在窗口里点的**（启动路径不会自动 `addRepo`：`loadRepos` 只有在
  `saved` 存在**且**在 `repos` 里命中时才 `selectRepo`，见 [src/store.ts:544-555](file:///Users/baorong/Developer/superlbr/gitx/src/store.ts#L544-L555)）。
  推断，未直接看到。
- **改名后真实窗口跑通了**（截图 `/tmp/gitcraft-window.png`）：标题栏 `comic ⌄ main`、窗口标题走
  `windowTitle()` 的「仓库名 — 分支」，历史页 / 提交图 / 文件列 / 差异区全部正常渲染。
- 待定：`com.gitcraft.app` 这个 identifier 是临时起的，真要发布得跟域名/证书对齐；
  应用图标里那枚「提交图交叉成 X」与新名字无关，暂时不动。

### 视觉打磨第一批：从一张真实截图出发（2026-10-14）
> 起因是用户一句「这个ui我不满意」。这句话本身没法施工，所以先去做了一件以前没做成的事：
> **真的把窗口截下来看**。屏幕录制权限这次通了，于是拿到的是 comic 仓库历史页的实际渲染，
> 而不是凭代码想象。下面 10 条全部是那张截图里能指出来的，且每条都在源码里对到了位置。

- [x] **① 历史页底部一条死黑带**（`styles.css:1774` `.hist-right`）
  - 事实：`.hist-split` 是 `flex-direction: column`，子项 `.hist-commits` 写死 `height:42vh`，
    而 `.hist-right` **一个 flex 属性都没有** → 它按内容取高。里面的 `.diff-pane { flex:1 }`
    只能分配父级**实际拥有**的高度，分配不到父级没有的那部分。结果差异面板停在半屏，
    底下约 25% 窗口高度是纯背景。
  - 修法：`.hist-right { flex: 1 1 auto; min-height: 0 }`。
- [x] **② 底部状态栏在 CSS 里根本没有规则**
  - 事实：`App.tsx` 渲染 `<footer className="statusbar">` 与 `.sb-item`，
    而 `grep -n "statusbar" src/styles.css` **0 命中** —— 这一行从来没被写过样式。
    截图里它读作「comic⎇ main6 文件变更200 提交就绪」，四组数字之间没有间隙。
  - 修法：补 `.statusbar`（24px 一行、`gap:14px`、`bg-2` + 上边框、11.5px muted、
    `font-variant-numeric: tabular-nums` 让读数不跳字）与 `.sb-item` / `.sb-spacer` /
    `.sb-item.last`（最近一次动作过长时省略号，不撑破窗口）。
- [x] **③ `.icon` 被两套东西共用，所有 SVG 图标被撑大**（`styles.css:78` vs `:992`）
  - 事实：78 行的 `.icon` 是给 `icons.tsx` 的 `<svg>` 的（`display:block; flex:none`），
    992 行又出现一个 `.icon`，本意是给行内**按钮**（`button.icon`，全项目只有 3 处用到），
    但它同样命中所有 svg —— 于是标着 `width="16"` 的线性图标实际渲染成 **22px**，
    还带一圈 `padding:0` + 透明边框；`.icon:hover` 更会让指针正好压在图标上时给图标自己画个底框。
  - 修法：992 那一组改成 `button.icon` / `button.icon.wide` / `button.icon:hover`，
    78 行的 svg 侧显式钉 `width/height:16px`。顶栏那一批图标尺寸这才真正是设计时的 16px。
- [x] **④ 空状态文案竖排成「一 行 一 个 字」**（`.repo-list li` 的 grid）
  - 事实：`.repo-list li` 是 `grid-template-columns: 10px 1fr auto`，而空状态
    `<li class="empty">` 沿用同一个规则 → 整句文案落进**第一列那 10px**里，只能一个字一行。
    这条在浏览器里就能看到，不需要 Tauri。
  - 修法：`.repo-list li.empty { display: block }`（空状态是一行说明，不是一行仓库）。
- [x] **⑤ 删除按钮压在「从未同步」上**（`.repo-del { position:absolute; top:6px; right:8px }`）
  - 截图里 × 直接盖住 `.repo-sync` 徽标的右半边 —— 绝对定位不知道网格里已经有人占了那块。
  - 修法：改成第三列第二行的普通网格单元，且 `opacity:0`、`:hover` 时才出现（这一行本来就有
    右键菜单里的「移除」，常驻一个 × 只是噪声）。
- [x] **⑥ 仓库路径折行把行高翻倍**（`.repo-path { word-break: break-all }`）
  - `/Users/baorong/Developer/comic` 被硬折成两行。修法：`nowrap + ellipsis`，
    完整路径移到 `li` 的 `title`（悬浮本来就该给全路径）。
- [x] **⑦ 侧栏四个按钮的栅格少算了一个**（`.sidebar-acts { grid-template-columns: 1fr auto auto }`）
  - 四件事塞进三列 → 「路径」孤零零掉到第二行，右半空着。修法：主按钮 `grid-column: 1/-1`
    占整行，其余三个 `repeat(3, 1fr)` 等宽。
- [x] **⑧ 导航激活态是一整块实心紫**（`.nav-item.active { background: var(--accent) }`）
  - 它是全屏唯一的高饱和色块，比提交图形本身还抢眼，而且和「这一行是当前视图」这件事不成比例。
  - 修法：浅底 `var(--bg-3)` + `box-shadow: inset 2px 0 0 var(--accent)` —— 与
    `.frow.viewed` 早就在用的那套左侧强调条同一种语言。
- [x] **⑨ 一屏两套图标语言：全站去 emoji（12 处）**
  - 事实（node 按 `\p{Extended_Pictographic}` 扫 `src/**/*.tsx`，排除测试）：顶栏 2026-10-12
    已经换成线性 SVG，但**左侧导航五个视图还是彩色 emoji**（📝🕘🌿🌐），另有
    `History.tsx:599,603,606` 的 ✎✨⚠️、`Branches.tsx:71,79` 与 `WorkingCopy.tsx:471,521` 的 ✨、
    `Settings.tsx:640,641` 的 ✅❌ —— 共 12 处。
  - 修法：`icons.tsx` 补 `IconFile / IconBox / IconClock / IconGlobe`（分支已有），
    `Nav.tsx` 的 `icon` 字段从字符串换成组件；AI 按钮去掉 ✨（`.ai-btn` 本来就是渐变底，
    符号是重复信息），`✨ AI` 顺手改成有信息量的「AI 起草」；连通性结果去掉 ✅❌
    （`.conn-ok` / `.conn-bad` 已经用颜色说话）；状态栏的 `⎇` 换成 `IconBranch`。
  - **判别门**：新增 `src/noEmoji.test.ts` —— 用 `import.meta.glob("./**/*.tsx", {query:"?raw"})`
    扫**全部** tsx，以后新加的组件自动在门内。第一条用例断言 glob 真扫到 >15 个文件，
    否则「扫不到任何东西」会伪装成「一切干净」。★ ⎇ × ▾ ↑ ↓ · 这类排版符号经 node 逐个确认
    **不在** Extended_Pictographic 里，所以仓库置顶星标不会被误伤。
  - 反向变异：把 `Nav.tsx` 贮藏图标改回 `"📦"` → **恰好 1 例转红**（另 2 例仍绿，
    说明门只咬它该咬的那件事）；从 `/tmp/Nav.tsx.bak` 还原并 `diff -q` 证明字节一致。
- [x] **⑩ 空状态承诺了一个界面上不存在的按钮**
  - 原文案「暂无仓库，点击 + 添加」，但全项目没有任何 `+` 按钮（真正的入口叫「打开仓库…」）。
    改成「暂无仓库，用上面的「打开仓库…」加入」。
- **一个查了才没做错的判断**：截图里历史页是**上下**布局，我一度以为它偏离了 2026-10-10
  「工作区/历史页 = 左右分栏」那条硬约束，准备"修回去"。翻 `PROCESS.md:1036` 才发现
  上下布局正是用户当天提的诉求（「历史改为上下布局且 diff 带行号」）。**所以没动。**
  记在这里，免得下一轮凭印象再把它翻回去一次。
  （**同日更新**：用户当天又提「改动文件详情显示在提交列表右侧」，历史页现在是「上半区左右并列 +
  下半区差异」，见「历史页改版 + 仓库标签页」。上面那条"别凭印象翻回去"的告诫仍然有效——
  要改布局得等用户说，不是我觉得。）
- **验证与数字**
  - `npx tsc --noEmit` 通过；`npx vitest run` 144 → **147 例全绿**（新增 3 例都在 `noEmoji`）。
  - `npm run build`：JS `280.77 → 281.41 kB`（gzip `90.96 → 91.12`，+0.16 kB = 4 个图标 +
    状态栏规则），CSS `32.54 kB / gzip 6.86`；高亮懒加载 chunk 仍是独立的 `145.61 kB`，
    ② 切出来的那刀没被这批改动破坏。
  - 肉眼：真实窗口截图复核，①②④⑤⑥⑦⑧⑨ 全部确认生效（历史页差异面板撑到底、
    状态栏四组读数分开、路径单行带省略号、× 只在悬浮时出现、导航换成线性图标 + 左侧细条）。
  - **没看到过的**：亮色主题下这一批对比度；工作区页（要有改动才有右栏）；
    小窗口下 `42vh 提交列表 + 文件条 + 差异` 三段会不会把差异挤到不可用；
    emoji 在 Windows/Segoe 下原本长什么样（本机只有 macOS，那条缺陷是从图标语言不一致推出的，
    不是两平台对照看到的）。

### 视觉打磨第二批：SourceTree 密度 + 去卡片嵌套（2026-10-14）

用户给的方向是「更紧凑（SourceTree 式）」，并要求先找 UI 设计 skill 再动手。依据不是凭感觉：
`~/.codebuddy/skills/Impeccable（前端设计工具集）`（其自述「基于 Anthropic 的 frontend-design」）
的 `references/spatial-design.md`「Cards are not required … **Never nest cards inside cards**」和
`references/cognitive-load.md`「视觉噪声底」。顺带记一笔：`npx skills add 'anthropics/skills@frontend-design'`
因 GitHub `RPC failed; curl 16 HTTP2 framing layer` 没装成（大陆网络），本机已有等价 skill，
**没有**为了它去动全局 git config。

- [x] **① 提交行从两行压成一行：`GRAPH.rowH` 46 → 34**
  - 事实：`src/graph.ts:70` 这一个常量同时驱动三处 —— SVG 节点 y（`History.tsx:116,123` 的
    `centers[]` / `graphH`）、行内 `<li style={{ height }}>`（`History.tsx:494`）、`nodeY()`
    （`graph.ts:81`）。**密度只有一个旋钮**，所以 JSX 一行都不用改。
  - 做法：`.commit-list li` 从 `flex-direction: column; justify-content: center` 换成单行
    `align-items: center; gap: 10px`；`.commit-subject` 加 `flex: 1; min-width: 0`（省略号仍归
    `.subj-text`），`.commit-meta` 去掉 `margin-top`、加 `flex: none`。
  - 收益：同一块 42vh 区域可见行 7 → 9（`/tmp/gitcraft_b2_a.png` 数到 9 行；315pt 列表高 ÷ 34 ≈ 9.3，
    而 ÷ 46 ≈ 6.8），且 subject 不再和第二行抢宽度。
- [x] **② `.ref-head` 从实心亮绿块换成描边片**
  - 事实：改前是 `color: #fff; background: #2ea043` —— 一个不跟主题走的硬编码实心块，是整列里
    最亮的东西（正是认知负载说的「噪声底」），而 `.ref-branch` / `.ref-tag` 早就是 15% 底色 +
    40% 描边。三种 ref 两套语言。
  - 做法：HEAD 并入同一族（`var(--green)` 上色 + `rgba(63,185,80,.15)` 底 + `.45` 描边），
    靠色相区分而不是靠亮度抢戏。
- [x] **③ 卡片套卡片：`.hist-commits` 里面又画了一层框**
  - 事实：`.hist-commits` 有 border+radius+bg，内部 `.hist-split .commit-graph-wrap` **又**有
    border+radius+bg，而它 `height: 100%` 正好填满父级 → 两层 1px 框贴成一条 2px 双线、圆角套圆角。
  - 做法：内层只留 `flex` / `height`，外层是唯一那张卡。
- [x] **④ 右栏三张框合成一张卡**
  - 事实：`.hist-right` 原本只是个 `gap: 8px` 的裸列，里面 `.commit-files`（自带 border+radius+bg）
    和 `.diff-pane`（同）各自成框，`.explain-bar` 无框夹在中间 —— 一屏四五种边界语言。
  - 做法：`.hist-right` 变成唯一的卡（border + radius + `var(--bg)` 底 + `overflow: hidden`），
    `.commit-files` 去掉自己的框、`.hist-right .diff-pane` 去框（基线规则留给工作区页的
    `.wc-right .blame` 用），分区之间统一由 `.hist-right > * + * { border-top }` 给一条线 ——
    这样空分区时不会叠出双线。`.cf-head` 补 `background: var(--bg-2)`，与 `.diff-head` 同一种页眉条。
  - `.explain-text` 同理：原本是卡内又一个 border+radius 的框，换成 `bg-2` 底 + 2px 左侧竖线；
    `.risk-text` 的 `border-color` 覆盖仍然生效（同特异度、写在后面）。
- [x] **⑤ 卡片正文底色从 `--bg-2` 改回 `--bg`：悬浮反馈原本是看不见的**
  - 事实：`.commit-list li:hover` 和 `.active` 都用 `var(--bg-2)` 上色，而 `.hist-commits` 的底色
    也是 `var(--bg-2)` → **悬浮和选中在列表里没有任何背景变化**（只剩那条 3px 左条）。这条是读 CSS
    才发现的，截图上没悬浮所以看不出来。
  - 做法：两张卡正文统一 `var(--bg)`，页眉条留 `var(--bg-2)`。亮色主题下 `--bg #f5f6fa` /
    `--bg-2 #fff` 同样成立（悬浮变亮而不是变暗）。
- [x] **⑥ `.day-head` 的四个 `!important` 换成一条复合选择器**
  - 事实：它写的是 `display:flex !important; flex-direction:row !important; padding !important;
    border-left:none !important; cursor !important` —— 那是行内还是 column 时的补丁，为的是盖过
    `.commit-list li`（0,1,1）。单行化之后基线规则本来就对了，只剩 border-left / radius / cursor 三件需要压。
  - 做法：合并为 `.commit-list li.day-head`（0,2,1），删掉 `.day-head:hover`（与基线 hover 同色，
    一直是条空规则）。
- [x] **⑦ 顺手修掉一个我自己造出来的缺陷：未选中时两条页眉叠在一起**
  - 事实：`.commit-files` 的标题在没选中时写「未选择提交」，下面 `.diff-pane` 页眉写「差异」——
    去框成一张卡之后，两条 bg-2 页眉紧贴，读起来像重复标题（`/tmp/gitcraft_b2_a.png` 里看得见）。
  - 做法：整段改成 `{selected && ...}` 才渲染（`History.tsx:540`），顺带删掉「未选择提交」这句 ——
    差异区空状态已经说「点击左侧提交查看差异」，别在同一屏重复用户已经能看到的东西。
    改后截图 `/tmp/gitcraft_b2_b.png`：右栏只剩「差异」一条页眉。
- **判别门**：新增 `src/historyDensity.test.ts` 三条，钉的都是**改回去没有任何功能症状**的那三件事：
  1) `.commit-list li` 规则里不许再出现 `height:`（行高必须只有 `GRAPH.rowH` 一个来源，否则 SVG 节点
     和文字错位，而且**只有滚动时才看得见**）；2) `.hist-split .commit-graph-wrap` 不许有 `border` /
  `background`；3) `.commit-files` 不许有 `border` / `border-radius`，且分隔线必须由
  `.hist-right > * + *` 统一提供。
  - 一个坑记下来：`import css from "./styles.css?raw"` 在 vitest 下**拿到空串**（样式被当资产桩掉，
    `import.meta.glob` 加 `query:"?raw"` 也一样）。测试改成读盘
    `readFileSync(new URL("./styles.css", import.meta.url))`，模块名写成 `"node:" + "fs"`，
    因为本仓库没有 `@types/node`，字面量 `"node:fs"` 会报 TS2307。
  - 反向变异：三处同时破坏（`.commit-list li` 加 `height: 40px`、graph-wrap 加 `border: 1px`、
    `.commit-files` 加 `border-radius: 8px`）→ **恰好各自转红 1 例**，报错分别指向注入的那一行；
    从 `/tmp/hd_styles.orig.css` 还原并 `diff -q` 证明字节一致，复跑 150 全绿。
- **验证与数字**
  - `npx tsc --noEmit` 通过；`npx vitest run` 147 → **150 例全绿**（新增 3 例都在 `historyDensity`）。
  - `npm run build`：JS `281.41 → 281.40 kB`（gzip `91.12 → 91.11`），CSS `32.54 kB` / gzip
    `6.86 → 6.88`；高亮懒加载 chunk 仍是独立的 `145.61 kB`，② 那刀没被破坏。压缩产物里
    `.hist-right>*+*` 与 `.ref-head{…#3fb95026…}` 都在，`.commit-list li` 里没有 height。
  - 肉眼（真实窗口 `screencapture`，仓库 comic / 200 提交）：单行行高、绿色描边 HEAD 片、
    右栏一张卡、未选中时只剩一条「差异」页眉 —— 全部确认。
  - **当时没看到过的**：**选中某个提交之后**那张卡的样子（文件分区 + AI 条 + 差异三分区叠在一起）。
    原生窗口不给自动化发键（`System Events` 报「osascript 不允许发送按键 / 不允许辅助访问」），
    内嵌浏览器又没有 `__TAURI_INTERNALS__`（点不出补丁），这一格只能等用户手点或开辅助访问权限。
    仍未看：亮色主题、工作区页、小窗口下三段挤压。
    → 同一天补上了：浏览器里的 invoke 桩（见「验证方式」）让这一格可见，三分区齐全；也正是它
      抓出了「点提交 → 整屏空白」的真 bug，见下一节。
- **量到了但这一刀没做**：`.commit-graph-wrap` 的左 gutter 按**整张列表**最宽泳道预留
  （`paddingLeft: graphW + 10`，`History.tsx:443`），本屏只有一条直线却占了约 100px —— 行高压到 34
  之后这块空白更显眼。要修得先按可见区算宽度（= 虚拟化，要引依赖，属于要先问用户的那类）。
  同一张卡里的 `.ai-btn` 紫蓝渐变（`linear-gradient(135deg,#6e8efb,#a777e3)`）是 skill 明确点名的
  AI 配色，留给下一刀。

### 缺陷：历史页点提交 → 整屏空白（2026-10-14，用户报「没有报错，导致屏幕没显示」）

- **现象**：进「历史」页随便点一个提交，右栏什么都不出现，界面也没有任何报错 toast；用户只能
  描述成「好像有报错」。实际 React 已经把整棵树卸载了（`#root` 空），**因为项目里没有错误边界**，
  渲染期抛错不会显示，只会白屏。
- **真因**（`gitcraft-core` / 命令 / store 全都没问题，是 WebView 性能②「高亮懒加载」埋下的）：
  `DiffView.tsx` 把高亮函数存进 React state ——
  ```ts
  const [hl, setHl] = useState<HighlightFn | null>(loadedHighlight); // 裸函数 = 惰性初始化器
  setHl(m.highlightText);                                           // 裸函数 = updater，React 会调用它
  ```
  `useState` 的初值和 setter 只要收到**函数**就当它是（惰性）初始化器 / updater，于是
  `highlightText(null)` 被调用；而 `highlightText` 的空值分支返回 `plainCode(null) === " "`
  —— 一个**真值字符串**，不抛错。state 于是变成 `" "`，`hl ? hl(text, lang) : plain(text)`
  走进真值分支，两帧之后才炸出 `TypeError: hl is not a function`
  （`DiffView.tsx:156`，`renderRow`）。类型检查也拦不住：`useState<HighlightFn | null>` 泛型
  写对了，是 React 的调用约定在运行时改了值。
- **修法**：两处都包成 lambda —— `useState<HighlightFn | null>(() => loadedHighlight)` 与
  `setHl(() => m.highlightText)`，并在 `useHighlighter` 里写下为什么。
- **怎么复现出来的**（这一格值得记，它补上了长期缺的验证手段）：macOS 这边 `System Events`
  读不到窗口位置/尺寸，也发不了键（「osascript 不允许辅助访问 -1719」「不允许发送按键 1002」），
  真实窗口驱动不了。于是第一次做出**浏览器里的 Tauri invoke 桩**：`npm run tauri dev` 起着，
  内嵌浏览器开 `http://localhost:1420`，注入 `window.__TAURI_INTERNALS__.invoke = async (cmd) => data[cmd] ?? null`
  + `transformCallback`，桩表按 `src/api.ts` 的命令名逐条给真数据，`window.prompt` 改成返回仓库路径，
  然后点侧栏「手动输入」→ 点 `.commit-list li[data-oid]` → 读 `.hist-right` 和 `window.__errs`。
  脚本存在 `/tmp/gitcraft_invoke_mock.js`（**临时件，没进仓库**）。第一次跑撞的是我自己桩错的名字
  （`commit_files` 而非 `commit_diff`）→ `files` 为 `null` → 又一个白屏，正好把「没有错误边界」
  这件事演示了两遍。
- **判别门**：`src/highlightLazy.test.ts`，3 例
  - `useState<HighlightFn \| null>\(\(\) => loadedHighlight\)` 必须在源码里（改回裸传即红）。
  - `setHl\(\(\) =>` 必须在源码里（改回 `setHl(m.highlightText)` 即红）。
  - 行为证据：`highlightText(null as unknown as string, null)` 返回 `" "`。这一例钉的是「毒值为什么
    一路无拦截」：它不抛错、还是真值，所以 `hl ?` 分支照走。（② 自己的 chunk 判据早有闸门，
    在 `languages.test.ts` 的「高亮器留在首屏之外」，这里不重复，也就没再为它做一次变异。）
  - 反向变异：两例改回旧写法（裸传 `loadedHighlight` + `setHl(m.highlightText)`）→ **恰好同时转红这 2 例**，
    另两例仍绿；从 `/tmp/DiffView.orig.tsx` 还原并 `diff -q` 证明字节一致，复跑 153 全绿。
- **验证与数字**
  - 浏览器桩（同一份 vite dev 服务，改动即时生效）：冷路径（首点，动态 import 落地）与热路径
    （切走再切回、从模块级缓存重建）各跑一次，`.hist-right` 都在、`commit-files / explain-bar / diff-pane`
    三分区齐全、`.diff-pane` 里 8 个 `hljs-*` span、`window.__errs` 为空。
  - `npx tsc --noEmit` 通过；`npx vitest run` 150 → **153 例全绿**（新增 3 例都在 `highlightLazy`）；
    `npm run build` JS `281.40 → 281.41 kB`（gzip `91.11 → 91.12`），CSS `32.54 kB` / gzip 6.88，
    高亮 chunk 仍是独立的 `145.61 kB`（② 未破）。
  - **真实窗口里仍未肉眼复核**：同一台机器发不了键，但 dev 服务是同一个，HMR 已把修复推进正在运行的
    应用；等用户手点一次「历史 → 任意提交」确认。
- **量到了但这一刀没做**：**全站没有错误边界**，任何渲染期抛错都是「整屏空白 + 零提示」，
  这次和 mock 那次都撞上了。补一个 `ErrorBoundary`（显示错误原文 + 「重新加载」，约 25 行）能把
  「用户只会说『没显示』」这类缺陷从不可诊断变成可诊断。已记入下方清单，等用户点头再动。
- 顺带记一笔：`.commit-list` 的行点击是 toggle —— 再点已选中的那一行会 `clearSelection()`，
  右栏随之清空。行为本身是对的（SourceTree 也如此），但第一次自动化复现时被它骗了一次。

### 历史页改版 + 仓库标签页（2026-10-14，用户四条：「ui下，改为2列，2列观感很差，另外能不能改成tab样式，这样方便切换项目查看；缺少remote/main展示；点击提交，可以把改动文件详情显示在提交列表右侧」）

- **范围怎么定的**：这四条我发过一轮澄清选择题，**用户把面板关掉了没答**，所以按字面理解落地：
  ①「2 列」= 提交行被拉成左右两栏（主题靠左、作者/日期/哈希被顶到最右，中间一条死通道）；
  ②「tab」= 顶栏下面一条仓库标签，用来切项目；③ = 远程跟踪引用要在图里看得见；
  ④ = 文件区从差异卡里搬出来，放提交列表右边。范围含糊时先钉死再动手这条教训（见记忆
  「简单版本优先」）这次仍然成立：**下一条消息用户直接给了四条具体诉求，比任何追问都准。**
- **① 提交行回到一行读完**：`.commit-subject` 从 `flex: 1` 改为 `flex: 0 1 auto; min-width: 0`。
  `flex: 1` 会 grow，元数据就被推到最右形成视觉两栏；`0 1 auto` 让元数据紧跟主题，长主题自己省略号。
- **② 仓库标签页**：新组件 `src/components/RepoTabs.tsx`（顶栏下方，`repos.length < 2` 直接不渲染，
  单仓库用户不该多一条空带）。激活页 `margin-bottom: -1px` 压住 `.repo-tabs` 的下边框，读起来
  就是"接在下面那块"。`flex: none` + 容器 `overflow-x: auto`：仓库名再长也不挤压标签，装不下就横向滚。
- **③ remote/main 走完整四步链**：`gitcraft-core::models::RemoteRef` → `git::remote_refs()` →
  `commands.rs::remote_refs` → `lib.rs` 注册 → `api.remoteRefs` → `store.remoteRefs` →
  `History.tsx` 泳道徽标（`.ref-remote`，虚线蓝框，跟实心分支徽标、标签徽标区分开）+ 状态栏
  `→ origin/main`（无上游时写「→ 无上游」，因为那正是 push 会翻车的地方）。
  刷新搭在 `loadBranches` 里（`Promise.all([branches, remote_refs])`）：**12 个既有调用点自动一起刷新**，
  不必逐个改。
  - **`%(refname:short)` 是个坑，而且不是我推的**：符号引用 `refs/remotes/origin/HEAD` 用
    `%(refname:short)` 打印出来是 **`origin`**（最后一段被吃掉），于是 `ends_with("/HEAD")` 的过滤
    永不命中 —— 每个仓库的主线提交上都会多挂一条只写着远程名的幽灵徽标。改用 `%(refname:lstrip=2)`。
    证据有两层：新加的 cargo 测试**第一次跑就红**在 `refs.len() == 1`，打印 `[origin, origin/main]`；
    而真实窗口截图里那条 `origin` 徽标就明晃晃挂着（跑的还是改动前的 Rust 二进制，见下面"没做"）。
- **④ 文件列搬到右侧，顺带暴露一条横向滚动条**：搬过去之后第一张真窗口截图（comic 仓库，18 个文件的
  提交）里，文件列底部长出一条横向滚动条 —— 全站共用的 `.fname` 写了 `flex-shrink: 0`（工作区页够宽，
  该让目录先缩），但这列最多 460px 还要给悬浮「历史」按钮留 38px，行宽被顶破。
  修法：`.commit-files .fname { flex-shrink: 1; min-width: 0 }`，只收窄这一列，工作区页行为不变。
  改完再截一张：滚动条消失，长文件名走省略号，同屏多看到一行。
- **判别门**
  - `gitcraft-core`：`remote_refs_are_the_fetched_remote_pointers` —— 无远程 → 空；只 `remote add` 不
    fetch → 仍空（跟踪引用是 fetch 出来的，不是配出来的）；fetch 后恰好 1 条且 `target == head()`；
    符号 `origin/HEAD` 与 `update-ref` 造出的普通 `origin/HEAD` 都要被滤掉。
  - `src/historyDensity.test.ts` 153→155 里新增/改写的 4 条：`.commit-subject` 必须 `flex: 0 1 auto`；
    `.hist-top` 不带框而 `.commit-files` 必须带框（并列的卡各一层框，容器不带 = 不是卡片套卡片）；
    JSX 里 `.hist-top → .hist-commits → .commit-files` 的顺序 + `.hist-right` 里不许再有文件区；
    `.commit-files .fname` 必须可收缩而全局 `.fname` 保持 `flex-shrink: 0`。
  - **反向变异**（四条各一次，都"恰好转红预测的那一条"）：`.commit-subject` 改回 `flex: 1` → 红「不被
    拉成左右两栏」；给 `.hist-top` 加 `border` → 红「并排不带框」；删 `.commit-files` 的 `border` →
    红同一例的另一半；删 `.commit-files .fname` 整条规则 → 红新加那例；Rust 侧格式串改回
    `%(refname:short)` → 红 `refs.len() == 1`。全部从 `/tmp/*.bak` 还原后复跑全绿。
- **验证与数字**
  - `npx tsc --noEmit` 通过；`npx vitest run` 153 → **155 例全绿**（11 文件）；
    `cd gitcraft-core && cargo test` 69 → **70 例全绿**。
  - `npm run build`：JS `282.40 kB / gzip 91.43`，CSS `33.57 kB / gzip 7.07`（+0.2 kB ≈ tab 条 +
    远程徽标 + 收缩规则），高亮 chunk 仍是独立的 `145.61 kB`（② 未破）。
  - 真实窗口（Swift `CGWindowListCopyWindowInfo` 拿 window id → `screencapture -x -o -l<id>`）：
    文件列在提交列表右侧、提交行一行读完、状态栏 `comic main → origin/main 10 文件变更 200 提交`、
    横向滚动条消失 —— 四张截图前后对比确认。
  - 浏览器 invoke 桩（上一格建的那套）：`list_repos` 给两个仓库 → tab 条渲染且激活态正确；点第二个
    tab → 激活迁移、状态栏换仓库名、`window.__errs` 空；`remote_refs` 给两条 → 泳道上两条虚线蓝徽标。
- **没看到过的**：原生窗口里的 tab 条（用户当前只加了 1 个仓库，`repos.length < 2` 直接不渲染，
  只有浏览器桩见过它）；亮色主题下 tab 条与 `.ref-remote` 的蓝；`@media (max-width: 900px)` 把上半区
  堆叠起来的效果（规则写了，没截过窄窗）。
- **量到了但这一刀没做**
  - **幽灵 `origin` 徽标在正在运行的窗口里还没消失**：`tauri dev` 没监视 `gitcraft-core`（日志里我改完
    Rust 之后再无 `Compiling`），得重启 dev 进程才吃到新二进制。那是用户开着的窗口，动它之前先问。
  - 上半区两栏宽度是写死的 `flex-basis 60% / 40%`（文件列另加 `max-width: 460px`），没有可拖的分隔条，
    也不记忆用户偏好。
  - tab 条没有「关闭 / 从列表移除」，仓库多了以后跟侧栏「仓库」列表是两套入口 —— 收敛成一套之前，
    它只解决"快速切换"这一件事。

### 视觉打磨第四批：改动文件列「看不清楚」（2026-10-14，用户「界面上，文件改动详情，看得不是很清楚，感觉不清晰」）

取证还是那台真窗口：`CGWindowListCopyWindowInfo` 找到 GitCraft（`id=9853`，comic 仓库，1200×767）→
`screencapture -x -o -l9853` → 用 `/tmp/crop`（临时 Swift 件，最近邻取样放大 2×）把右上角那一列裁出来看。
截图里那两行是：

```
M  components.d.ts  admin/pack…      +1
M  CbTable.vue      admin/packa…     +1 −1
```

- 缺陷不是一条，是四条，而且**都不是字号问题**：
  ① 目录被 CSS 的 `text-overflow` 从**尾部**吃掉 —— 留下的正好是不区分 `finance` / `user` 的前缀，
     两行截到不同长度（`admin/pack…` vs `admin/packa…`），完全读不出位置；
  ② `+1` / `+1 −1` 各自按需渲染（`additions > 0 &&`），于是两行的数字起点不同，**成不了列**；
  ③ `M` 徽标是 `#d29922` 描在 20% 同色底上，亮色主题下约 2.5:1，一列 `M` 糊成一片；
  ④ 目录 `.fdir` 是 `flex: 1` + `text-align: right`，被推到行中间偏右，跟文件名脱开，
     整行读起来不像一条路径。
- **第一版修复当场翻车，而且是截图抓住的**：把省略改到 JS 里、但保留了 24 字符的目录尾部，
  并给 `.fdir` 写 `flex: 0 1 auto` —— 结果 flex 把收缩分摊到了文件名上，
  截图里变成 `compo…` / `CbTa…`（文件名比目录更重要，却被挤掉了）。
  教训：窄列里"少切一点目录"是不够的，得**只留最后一段**，并且目录这一格**完全不参与收缩**（`flex: none`）。
- 最终形态：`src/pathLabel.ts`（`baseOf` / `dirOf` / `shortDir` / `tailDir`，工作区页与文件列共用一份，
  不再各写一份然后漂移）+ `.commit-files .fdir { flex: none; text-align: left }`
  + 两个数字槽恒定渲染（0 用 `.zero` 变暗）+ `min-width: 74px` + `tabular-nums`
  + `.st-*` 文字色改成 `color-mix(in srgb, <语义色> 55%, var(--text))`（亮色主题变深、暗色变亮，一条规则两头管）
  并给徽标加 1px 同色描边。
- **几何是用浏览器桩量出来的，不是目测**（真窗口点不出选中态，见下）。把 `.commit-files` 钉到真实的
  442px 宽，读每行子元素的 `getBoundingClientRect()` / `scrollWidth > clientWidth`：

  | 行（comic 的真实长路径） | 旧：文件名宽 / 被截 | 旧：数字 x | 新：文件名 / 目录 | 新：数字 x |
  | --- | --- | --- | --- | --- |
  | `admin/packages/finance/components.d.ts` | 101px / 否 | `[783]` | 完整 / `finance` | `[765, 783]` |
  | `admin/packages/finance/src/components/CbTable.vue` | **61px / 是** | `[765, 783]` | 完整 / `components` | `[765, 783]` |
  | `packages/…/veryLongComponentDirectory/WidgetPanel.tsx` | **55px / 是** | `[776]` | 完整 / `…ComponentDirectory` | `[758, 783]` |
  | `README.md`（根目录，无段） | 76px / 否 | `[783]` | 完整 / 不渲染目录 | `[765, 783]` |

  `−` 槽右端四行恒定在 783；`+` 槽右对齐（`+1` 与 `+42` 个位对齐），列感来自这里而不是左对齐。
  卡片 `scrollWidth === clientWidth === 442`，没有长出横向滚动条。
- **判别门**：`src/pathLabel.test.ts` 4 例（切分、工作区页保留尾部、**窄列结果里不得再有 `/`**、
  宽度可预算 ≤19 字符）+ `src/historyDensity.test.ts` 第 6 例（JSX 用 `tailDir(dirOf(...))`、
  `.commit-files .fdir` 必须 `flex: none` 且左对齐、`.cf-counts` 有 `min-width` 与 `tabular-nums`、
  JSX 里不得再出现 `d.additions > 0 &&`、`.st-modified` 用 `color-mix(`、`.st` 有边框）。
- **反向变异 5 条**：`tailDir` → 原始 `dirOf`（红 1）、`.fdir` 的 `flex: none` → `flex: 1 1 auto`（红 1）、
  数字槽改回按需渲染（红 1）、`tailDir` 退化成 `shortDir`（**红 2**：斜杠那条 + 宽度那条）、
  徽标色退回 `var(--yellow)`（红 1）；每条还原后 10 例复绿。
  **顺带记一个坑**：第 3 条第一次是用 `perl -0pi -e` 打的，正则没匹配上、文件根本没改，
  于是"测试全绿"看起来像门是假的。改用手上的 Edit 真打上之后才如期转红 ——
  **一次没有生效的变异和一门假门长得一模一样**，以后每条变异都要先 `grep` 确认改动真的落进了文件。
- 数字：`npx tsc --noEmit` 通过；前端 **162 → 167 passed**（13 文件，新增 `pathLabel.test.ts` 4 例、
  `historyDensity.test.ts` 5 → 6 例）；`npm run build` CSS **34.02 → 34.50 kB**（gzip **7.14 → 7.22**），
  JS **284.53 → 284.56 kB**（gzip 92.38 不变），高亮 chunk 仍 145.61 / 44.16。
- **没看到过的**：**最终形态在真窗口里没截到** —— 截图时用户已经取消了选中（`.commit-files` 只在有选中时渲染），
  原生窗口又点不动，所以最后一版只由浏览器桩的几何数字 + 中间那版（数字成列、徽标描边已生效）的截图支撑。
  暗色主题的徽标对比度、以及 `.commit-files` 在 `<900px` 堆叠后的样子都没看过。
- **顺手挖出来、还没动的东西**：`window.prompt` 有 4 处入口（历史页右键「新建分支 / 新标签」、
  分支页「重命名」、侧栏「路径」）。macOS 的 WKWebView 默认不弹 JS prompt（Tauri 未实现 `prompt`），
  若属实，这四处**在真应用里点了就是没反应** —— 与本项目已经自建 `ConfirmDialog` 的缘由同源。
  这条要修是一个约 60 行的输入弹窗（`requestInput` + `InputDialog`），已记进待办，等用户点头。

### 按路径筛选「输入了没反应」（2026-10-14，用户「按路径筛选有bug，输入路径没反应，另外可以考虑根据仓库文件补全」）

- **先排掉"链路断了"这个最像的猜测**：浏览器桩里把 `invoke` 换成会记参数的桩，在路径框敲 `docs`
  → 300ms 后确实发出 `commits {repo, max:200, author:null, path:"docs"}`，列表也跟着变成 1 行。
  前端、store、api、命令签名四层都是通的 —— 也就是说 bug 不在"没生效"，在"生效了但看不出来"。
- **真因是 git pathspec 的语义，量在用户自己的 comic 仓库上**（`git log --max-count=200 --pretty=%H -- <q>`）：

  | 输入 | 旧（单条裸 pathspec） | 说明 |
  | --- | --- | --- |
  | （空） | 200 条 | — |
  | `admin` | **200 条** | 顶层目录，几乎每个提交都碰过 → 列表一条没少，就是"没反应" |
  | `main.ts` / `main.go` | **0 条** | 只记得文件名时整列清空，看着像坏了 |
  | `admin/packages/author/src/main.ts` | 9 条 | 只有从根开始的整条精确路径才有用 |
  | `docs/re`、`admin/pack` | 0 条 | 半个目录名也不认 |

  裸 pathspec 是**从仓库根起的前缀匹配**，而用户敲的永远是"我记得的那一段"。
- **改法：一次传三条 pathspec，git 天然求并集**（`git.rs` `commits()`，注释里写清了哪条管哪类输入）：
  ① 原样 `q` —— 精确路径 / 目录 / **绝对路径与 `./` 开头**；
  ② `:(glob,icase)**/*q*` —— 片段落在**文件名**里（`main.ts` → 11 条）；
  ③ `:(glob,icase)**/*q*/**` —— 片段落在**目录名**里（`packa` → 200、`finance` → 200）。
  实测改后：`main.ts` 11、`finance` 200、`docker-compose` 4、`docs/re` 177、`zzzqqq` 0，
  `[a`、`a[b`、`src/*`、空格、`-h` 都不报错（`-` 开头的串走 `--` 之后，不会被当选项，参数注入面已堵）。
- **①能不能删？测了不能。** comic 里 9 个输入两条 glob 与三条结果**完全一致**，但
  `/Users/.../admin`（绝对路径）和 `./admin` 两条 glob 都给 0、只有裸 pathspec 给 200 ——
  而「右键 → 复制路径」这个入口存在的意义就是把绝对路径带进输入框。于是给它写了独立断言（M4 变异就是打它）。
- **补全（用户提的第二半）**：
  - 后端 `repo_files(repo, max)` = `git ls-files`，`commands.rs` + `lib.rs` + `api.repoFiles`；
    store 里 `repoFiles` / `repoFilesFor` **只在路径框聚焦时读一次**并按仓库缓存（`selectRepo` 清掉），
    切仓库、进历史页都不拉 —— comic 3607 条，大仓按 30000 条封顶，这是"一兆 JSON"级别的成本，不该白付。
  - `src/pathComplete.ts`：片段命中的是目录名就**截到那一段目录**（`pack` → `admin/packages/`，带尾斜杠、
    用 `.dir` 弱化色标出这是目录），命中文件名就给整条文件路径；命中段首字母减分、层级越深加分，
    **浅的排前面**（`server/main.go` 赢 `admin/packages/finance/src/main.ts`），最多 12 条。
  - 显示用 `elideHead(p)`（`…` + 尾部 29 字符）+ `title` 存完整路径 —— 就是第四批那条教训：
    **省略必须在 JS 里做、且只能吃头部**，CSS 的 `text-overflow` 吃的是尾部，而尾部才是区分度。
    所以 `.path-suggest li` 里明确禁止出现 `text-overflow`（有门）。
  - 键鼠：`↑/↓` 循环选中、`Enter`/`Tab` 采纳（没建议时 `Enter` 仍按输入内容筛）、`Esc` 收起但保留已输入、
    `onMouseDown` 上 `preventDefault()` —— 否则点建议时 blur 先关下拉，click 永远落不到那行。
- **"没反应"还有另一半是反馈缺失**：筛 `admin` 得 200 条是正确结果，但界面和没筛时一模一样。
  计数行现在写成 `200 条 · 改动过「admin」`，空列表不再统一写「无提交记录」，
  而是 `没有提交改动过「zzz」——片段可以更短，或用下拉里的完整路径`（作者筛选同理）。
  placeholder 也从 `(如 src/app.ts)` 改成讲清新语义的 `（文件名或目录名片段，如 main.ts）`。
- **判别门**：`cd gitcraft-core && cargo test` **70 → 72**（`commits_path_filter_matches_substrings_not_just_prefixes`
  六条断言各自钉死一类输入，`repo_files_lists_tracked_paths_up_to_the_cap` 钉住上限）；
  前端 **167 → 182 passed**（14 文件，新增 `src/pathComplete.test.ts` 15 例：6 例排序/目录截法/中文/上限，
  3 例 `elideHead`，6 例接线门 —— 聚焦才读、只有一个调用点、`title` 与 `elideHead` 同在、
  `mousedown` 防抢焦点、`Enter` 双分支、计数与空态文案、`.path-filter` relative + `.path-suggest` absolute）。
- **反向变异 9 + 5 条，每条都先 `cmp` 确认改动真的落进了文件**：
  前端 —— 删 `onMouseDown`（红 1）、多插一次 `loadRepoFiles()`（红 1，验"只读一次"）、
  `{elideHead(p)}` → `{p}`（红 1）、`改动过「` → `匹配「`（红 1）、空态文案回退（红 1）、
  `.path-filter` 删掉 `position: relative`（红 1）、`elideHead` 不省略（红 2）、
  目录命中不再截成目录（红 1）、排序不看打分（红 2）；
  Rust —— 去掉两条 glob（红）、去掉 `**/*q*/**`（红）、去掉 `icase`（红）、去掉裸 pathspec（红）、
  `repo_files` 不遵守 max（红）。全部还原后复绿。
- **浏览器桩新踩的坑（补进「验证方式」）**：后台页面里 `element.focus()` **不派发 focus 事件**，
  所以 `onFocus` 里的懒加载在桩里看起来像没生效；得手动 `dispatchEvent(new FocusEvent('focusin', {bubbles:true}))`。
  这条不是应用 bug —— 真窗口有焦点，聚焦事件正常发。
- 数字：`npx tsc --noEmit` 通过；`cd src-tauri && cargo check` 通过；`npm run build`
  JS **284.56 → 286.34 kB**（gzip **92.38 → 93.17**），CSS **34.50 → 35.06 kB**（gzip **7.22 → 7.33**），
  高亮 chunk 仍 145.61 / 44.16。
- **真窗口里验证过的部分：无。** 全部结论来自浏览器桩（`http://localhost:1420` + 记参数的 invoke 桩）：
  聚焦 → `repo_files` 恰好 1 次；敲 `main` → 下拉两条并按浅-深排；`↓↓` 循环到第二条、`Enter` →
  `commits path="admin/packages/finance/src/main.ts"`、1 行、计数行写明筛的是什么；
  鼠标 `mousedown`+`click` 选中 `docs/` → 生效；`Esc` → 下拉收起、输入保留；`zzzqqq` → 空态文案；
  「清除筛选」→ 回到 4 行且**没有**重新拉 `repo_files`。原生窗口既点不动也打不了字，
  而且 `tauri dev` 不监视 `gitcraft-core`，**用户开着的窗口还在吃旧的 Rust 二进制**，新语义要等重启。
- **顺手挖出来、还没动的东西**：`--fg` 在 `styles.css` 里被用了 4 处（1489 / 2221 / 2372 / 2408），
  但主题变量表里**根本没有定义**（只有 `--text`）。`color: var(--fg)` 退化成继承、看不出来；
  而 2221 行 `background: color-mix(in srgb, var(--fg) 4%, var(--bg))` 会在计算值阶段整条失效 ——
  **那一条背景从来没画出来**。修法是 4 处换成 `--text`，但它与这次的任务无关，先记账不动。

### push 报 "missing required key setUpstream"（2026-10-14，用户截图报错）

**根因**：Tauri v2 把 Rust 命令的 `snake_case` 参数映射成 JS 侧的**驼峰**键。
`commands.rs` 的 `push(…, set_upstream)` 要求前端传 `setUpstream`，而 `api.ts` 传的是
`set_upstream` —— 命令眼里这个键根本不存在，必填的 `bool` 当场报
`invalid args … missing required key setUpstream`。之前没炸是因为测试全在 store 层 mock `api`，
invoke 实参的键名从没被任何用例看过。

**顺手排雷**：全项目扫了一遍 `commands.rs` 的多单词参数，同样写错的一共四处，
全在 `api.ts`——除 `set_upstream` 外还有 `create_branch` 的 `start_point`、
`rename_branch` 的 `old_name/new_name`、`stash_push` 的 `include_untracked`。
后三类里 `Option<String>` 参数最阴险：键错了 serde 把缺失当 `None`，**不报错但语义全变**
（带起点建分支会悄悄变成从 HEAD 建）；`rename_branch` 两个必填 `String` 则是一改名就炸。
其余多单词参数（`clone_url`/`remote_url`/`repo_url` 等）前端已经用驼峰，没问题。

**修法**：四处全部改成驼峰直传（`{ repo, name, startPoint, checkout }` 这种形状），
不改 Rust 侧——TS 侧配合 Tauri 的默认约定是正路，给每个命令加 `rename_all = "snake_case"` 反而埋新坑。

**判别门**：`src/api.test.ts` 读 `api.ts` 原文（`?raw`），正则抽出全部
`invoke("cmd", { … })` 实参对象（82 处，先断言 >40 防 glob 失效伪装干净），
拒绝任何 `[a-z]+_[a-z_]+\s*:` 蛇形键名。反向变异：把 `stash_push` 改回
`include_untracked`，恰好红这 1 例，还原复绿。

### 应用图标：提交图交叉成 X（2026-10-14，用户「设计一个Logo，并更新当前项目」）

原来的 `icon.png` 是一张品牌紫纯色方块加暗角，纯占位。这次把它变成真正的 Logo，
**仍然零依赖**——`gen-icon.mjs` 不引入任何渲染库，直接在像素缓冲上用 SDF 光栅化：

- **图形**：两条分支线交叉成 **X**（既是 git 的提交图泳道，也是 GitCraft 名字里的 X），
  四个端点 + 中心一个更大的 merge 节点，共五个提交点；底板是 22.5% 圆角的方块，
  品牌紫做对角渐变（左上 `#9680ff` → 右下 `#583cd8`）加一点左上高光。
- **几何全部走中心坐标系**（±256），SDF（圆角盒 / 线段 / 圆）取 `min` 做并集，
  抗锯齿来自 `cover(d, 1.5)` 的 1.5px 渐变带。
- **踩了一脚**：第一版端点写成像素坐标（0..512）而 x/y 是中心坐标，整幅图形偏移 256px——
  渲染出来只剩中心一个点和右下角一截出界的线。定位方式是把 PNG 解码回来逐像素采样，
  对着 SDF 手算值比对才确认是坐标系混用；教训是**这类「画错位置」先解码验证再怀疑算法**。
- **小尺寸复核**：`sips` 缩到 64px 与 32px 目检，X 与节点仍清晰可辨。
- **未做**：macOS 打包目前只用这一张 512px `icon.png`（`tauri.conf.json` 也只引了它），
  正式的 `.icns` / 各尺寸 `icon@2x`、`Square*.png` 还没铺；真窗口的 Dock 图标要**重启应用**才会换
  （vite 热更新只管前端）。

### 界面评估后的 P0 三刀（2026-10-15，用户「整个界面设计和布局，感觉不专业，不好用」）

先用 `screencapture -l<windowId>` 截了真窗口（历史页）逐条对证据，结论收进对话；先动「不好用」的部分：

1. **没选提交时不再浪费半屏**：原来下半区恒定是「点击左侧提交查看差异」的空面板（1200pt 窗口
   里约 45vh 纯空白）。现在没选中 → `.hist-split.no-diff` 直接隐藏差异区、提交列表占满整页，
   底部只压一条 28px 的提示（含 `j/k` 键盘提示）；选中 → 恢复上下分栏。
2. **42vh 死高度改成可拖分隔**：`.hist-top` 高度来自 `--hist-top-h`（History.tsx 内联写入，
   42vh 只是没拖过时的默认）。拖手是 7px 的 `.split-handle`，比例钳在 0.25–0.85，
   松手时落 `localStorage["gitcraft_hist_split"]`（键名小写，见命名约定）。
   窄窗（<900px）媒体查询里隐藏拖手并退回纵向堆叠。
3. **提交行不再常驻 sha**：作者/时间/8 位哈希恒定占右侧 ~230px 是主题中段截断的直接原因之一。
   行上只留「作者 · 时间」，哈希收进行悬浮提示（`title` = 主题+作者+日期+短哈希），
   正规入口仍是右键「复制提交哈希」。

**判别门**（`historyDensity.test.ts` 新增 2 条，各做过一次反向变异、恰好红 1 例后还原）：
`no-diff` 类 + `onMouseDown={startSplitDrag}` + 比例钳制 + localStorage 键 + CSS
`var(--hist-top-h)`；以及 `className="oid"` 不得回到行内 + `title` 模板串。

**截图复核（选中态）**：上半区 60%、行内无 sha、diff 正常。**仍未解决的「主题截断」**
其实是 P1 信息架构问题的直接后果：1200pt 窗口被侧栏 240pt + 导航 168pt 吃掉 408pt，
提交列只剩 ~460pt，主题只能分到 ~350pt——省多少元数据都省不回来，除非合并左栏（下一批）。

### 默认亮色 + 双左栏合一（2026-10-15，用户「继续，默认亮色，不要暗色，暗色基本看不清」）

两件事一起落地：主题砍到只剩亮色；P0 评估里收益最大的「侧栏 240pt + 导航 168pt = 吃掉 408pt」合掉一半。

1. **亮色是唯一主题**。查下来发现 theme 本来就只是内存态——每次启动默认 `"dark"`，从没持久化过，
   也就是说用户看到的「默认暗色」其实从来没被记住过。与其补持久化，不如按用户结论直接砍掉：
   `store.ts` 删 `Theme` 类型与 `setTheme`，TopBar 的月亮/太阳按钮和设置页「外观」区块删除，
   `App.tsx` 只剩一行 `document.body.className = "light"`（注释里留了决策依据）。
   注意：styles.css 里暗色 `:root` 调色板成了死代码，下次动 CSS 时顺手删。
2. **RepoTabs 吸收侧栏**。`Sidebar.tsx` 整个删掉；tab 条从「≥2 个仓库才出现」改为 ≥1 即渲染，
   末尾常驻一个「+」按钮，点开 ContextMenu 收纳原侧栏四个入口（打开本地仓库 / 从 GitHub 克隆 /
   扫描文件夹 / 手动输入路径）；每个 tab 右键出原侧栏菜单（置顶、在文件管理器中打开、复制路径、
   移除，移除走 `requestConfirm`）。置顶仓库排前面、tab 悬浮显示完整路径，仓库色点跟着 tab 走。
   随侧栏消失的还有仓库搜索框和行内「上次同步」徽标——后者时间戳仍在写库，可改挂 tab tooltip。
3. **净收益**：内容起点从 408px 提前到 168px（单导航栏），P0 里救不回来的「主题截断」随之消失
   ——1200pt 窗口提交列从 ~460pt 回到 ~700pt。

**判别门**（新文件 `src/appShell.test.ts`，2 条；各做过一次反向变异、恰好红 1 例后还原）：
App.tsx 不得出现 `Sidebar`、必须挂 `<RepoTabs />` 和 `<Nav />`、必须写 `document.body.className = "light"`；
store.ts 不得再出现 `setTheme|type Theme`；RepoTabs 必须有 `repos.length === 0` 早退、「+」菜单四项
（含「手动输入路径…」「从 GitHub 克隆…」）、tab 的 `onContextMenu=` 和「移除」。

**真窗口复核**（`/tmp/gitcraft_ui_p1.png`）：亮色可读、左侧只剩 168px 导航栏、tab 条有「comic + +」、
空选中态提交列表占满整页且主题不再截断。
`tsc --noEmit` 干净；vitest **16 文件 / 187 例全绿**；`npm run build` 284.56 kB（gzip 92.44）。

### 操作增强（参照 dbx）

- [x] 拖拽文件到窗口进行添加/打开（`onDragDropEvent` + `find_repo_root` 向上找仓库根）（2026-10-11）
- [x] 历史页「按路径筛选」改成**片段匹配**（裸 pathspec ∪ 文件名 glob ∪ 目录名 glob，忽略大小写），
      并按 `git ls-files` 给路径框做补全（聚焦才读、按仓库缓存、浅的在前、目录命中补到目录）
      （2026-10-14，见上「按路径筛选『输入了没反应』」）
- [ ] 快捷键冲突可视化与「一键导入/导出键位方案」
- [ ] 提交列表内联搜索（`/` 后直接过滤 subject，而非仅作者/路径）
- [ ] `g a` 之类 chord 的状态提示（顶栏显示「等待第二个键…」）
- [ ] 复选框支持 Shift 范围勾选（目前只能逐行点，批量仍需先选行再用批量条）
- [ ] 文件溯源弹窗内接 blame（每个版本「谁改的哪几行」）与「对比两个版本」选择
- [ ] 已删除文件的溯源入口（`file_log` 已能返回删除记录，但工作区无行可点，需从历史页进）
- [ ] 改历史的安全网：`m` 改写信息前先把原信息写入 reflog 提示 / 提供一次「撤销改写」
- [ ] 改写提交的作者与日期（仍是 amend 范畴，比改信息更少见，暂不做）

### 布局 / 视觉现代化
- [x] 视觉打磨第一批：一张真实截图里指得出的 10 个缺陷（死黑带 / 无样式的状态栏 / 被撑大的图标 /
      竖排的空状态 / 重叠的删除按钮 / 折行的路径 / 掉队的第四个按钮 / 实心紫激活态 / 12 处 emoji /
      不存在的「+」按钮），见上面「视觉打磨第一批：从一张真实截图出发（2026-10-14）」
- [x] 视觉打磨第二批（方向由用户定：更紧凑 / SourceTree 式）：提交行两行→单行（`GRAPH.rowH` 46→34，
      可见行 7→9）、HEAD 标签去实心绿、历史页两处卡片套卡片合成两张卡、卡片正文底色改回 `--bg`
      让悬浮重新看得见、`.day-head` 的四个 `!important` 收成一条复合选择器，
      见上面「视觉打磨第二批：SourceTree 密度 + 去卡片嵌套（2026-10-14）」
- [x] 视觉打磨第三批（用户四条诉求）：提交行不再被拉成左右两栏、仓库切换改成顶栏下的 tab 条、
      远程跟踪引用（`origin/main`）上图 + 状态栏显示上游、改动文件列搬到提交列表右侧，
      见上面「历史页改版 + 仓库标签页（2026-10-14）」
- [x] 视觉打磨第四批（用户「文件改动详情看不清楚」）：目录只留末段、两个数字槽恒定渲染成列、
      状态徽标对比度、`.commit-files .fdir` 不再吃掉文件名，
      见上面「视觉打磨第四批：改动文件列「看不清楚」（2026-10-14…）」
- [ ] 输入型入口不再用 `window.prompt`（历史页新建分支/新标签、分支页重命名、RepoTabs「手动输入路径」共 4 处）：
      macOS 的 WKWebView 默认不弹 JS prompt，若属实这四处点了没反应；要自建 `requestInput` + `InputDialog`，
      **等用户点头再动**（约 60 行）
- [ ] 第二批剩下的：历史页上半区两栏的可拖分隔已做（见 P0 三刀）；剩设计令牌与卡片化面板
- [ ] 引入统一设计令牌与卡片化面板（更接近 dbx 的 Tailwind/shadcn 观感）
- [x] 提交详情右栏改为常驻分栏（diff + AI 解释/风险审查 同列）（2026-10-10；
      2026-10-14 起「改动文件」不再在这栏，改与提交列表并列）
- [x] 无仓库 / 无改动空状态引导（`start-card` + `DiffPane` 占位）（2026-10-10）
- [ ] 空状态插画（当前为纯文案引导）
- [ ] 暗/亮主题细化（语义化变量、对比度校对 WCAG AA）
- [ ] 清掉幽灵变量 `--fg`（2026-10-14 发现，尚未修）：`src/styles.css` 用了它 4 处，但 `:root` 与
      `body.light` 都没定义过。`color: var(--fg)` 只是退化成 inherit，看不出问题；
      而 `background: color-mix(in srgb, var(--fg) 4%, var(--bg))` 整条声明失效，那块背景
      **从来没画出来过** —— 主题细化时一并换成 `--text`
- [x] 顶栏仓库切换：做成 tab 条 `RepoTabs`（≥2 仓库才出现，横向可滚）而不是下拉
      （2026-10-14）；「支持搜索 / 最近」仍未做，且它与侧栏仓库列表是两套入口，待收敛
- [ ] 首屏 chunk 拆分 → 已展开成带判据的三条，见上面「WebView 性能三刀（2026-10-14 立项）」

### 功能补齐
- [x] 标签（tag）的创建 / 删除 UI **其实早就接通了**（2026-10-14 复核：这一条是文档腐烂）——
      创建在历史页右键「新标签」（`History.tsx` 的 `actNewTag` → `store.createTag`），
      删除在分支页标签区右键（带 `deleteTag` 免确认白名单）。
      真正还缺的是：注解标签的**正文**没有入口（`createTag` 的 `message` 永远传 `null`，
      所以界面只能造轻量标签），以及标签没有自己的列表视图
- [ ] 远程管理：增删改远程、prune
- [ ] `.gitignore` 模板与快捷忽略常见项
- [ ] 提交信息模板 / 常用片段（dbx 的「查询模板」对应物）
- [ ] 国际化（中/英切换）

### 环境一体化后续（2026-10-11 本轮明确不做）
- [ ] git flow：`git flow init / feature / release / hotfix` 全套（要先定分支模型与合并策略，
      不是加一个面板就能收口；本轮只把它排进来）
- [ ] Git LFS 的实际推拉（`git lfs pull/push/install`），本轮只做检测与提示
- [ ] 扫描文件夹时排除「同一个仓库被软链接进来两次」的重复计数
- [ ] 提交列表作者头像 + 各家**真实 logo**（要先选一套可商用授权的素材，现在用的是字母片）
- [ ] 原生窗口里手工回归：拖拽、扫描、SSH 试连、git 缺失横幅（内嵌浏览器无 `__TAURI_INTERNALS__`，
      这些链路在自动化里只能验到 core/store 层）

### AI 能力
- [ ] 提交信息生成结果可编辑预览后再提交
- [ ] 风险审查输出结构化（文件级高亮定位到 diff 行）
- [ ] 设置内区分「全局 AI 配置」与「当前仓库覆盖」

### 代码托管集成（2026-10-10 起步）
- [ ] CI 状态（GitHub check-runs / Gitee 流水线）——`Capability::CiStatus` 已留位，
      补一个 provider 方法 + 远程页一个状态列即可
- [ ] 创建 PR（`Capability::CreatePullRequest` 已留位，需要标题/正文表单与 POST 请求体）
- [ ] PR 详情再深一层：正文、评论、改动文件列表（当前只有编号/标题/分支/作者/时间）
- [ ] Codeup 真正接入：阿里云 AK/SK 签名 + organizationId，之后补 `capabilities()`
- [ ] 令牌改存系统钥匙串（现在沿用 `settings` 表明文，与既有 `ai_api_key` 一致）
- [ ] 新平台插件：GitLab / 自建 Gitea（一个文件 + `HOSTS` 注册表一行）
- [ ] 同一平台多账号 / 按仓库覆盖全局令牌

### 工程
- [ ] `cargo test` 继续覆盖：merge / rebase / 冲突解决 / remote / blame / rev_parse（签名与解析）
- [ ] 发布配置（tauri bundle / 签名）；发版后可选接入应用内更新
      （`tauri-plugin-updater` + minisign 公钥 + `createUpdaterArtifacts`，参照 dbx 的
      `latest.json` 方案；届时设置页「更新」从「浏览器下载」升级为「下载并安装」）
- [ ] gitcraft 自己的发布仓库就位后，把默认 `gitcraft_update_repo` 预填进设置页
- [ ] store 关键动作补测：删除仓库清空右侧状态、clone 参数与 origin 回写
- [ ] 组件层测试（jsdom / happy-dom）覆盖 chord 派发与多选交互
- [ ] **全站错误边界**（2026-10-14 立项）：现在任何渲染期抛错都是「整屏空白 + 零提示」，
      用户只能描述成「好像有报错但没显示」。补一个 `ErrorBoundary`（把错误原文显示出来 + 一次重试）
      约 25 行，能把这类缺陷从不可诊断变成可诊断 —— 等用户点头再动。

---

## 验证方式
```bash
# 前端类型检查 + 构建
npm run build

# 后端编译检查（注意：不含测试代码，改函数签名后请用 cargo check --tests 或直接 cargo test）
cd src-tauri && cargo check

# 核心逻辑单测（git 全流程 + SQLite 持久化）
cd gitcraft-core && cargo test

# 前端单测（快捷键解析 / store 确认流 / showPatch）
npm test

# 启动开发
npm run tauri dev
```

### 浏览器里跑真实 UI（没有 Tauri invoke 也能点出补丁）

原生窗口在这台机器上驱动不了（`System Events` 拒绝读窗口尺寸与发键），`screencapture` 只能看不能点。
可行的替代：`npm run tauri dev` 起来后，用内嵌浏览器开 `http://localhost:1420`，注入一层 invoke 桩 ——

```js
const I = (window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {});
I.transformCallback = I.transformCallback || (() => 1);
I.unregisterCallback = I.unregisterCallback || (() => {});
I.invoke = async (cmd) => data[cmd] ?? null;   // data 的键 = src/api.ts 里的命令名，逐条对齐
window.prompt = () => "/tmp/mockrepo";         // 侧栏「路径」的降级入口
```

桩表的键必须和 `src/api.ts` 的命令名逐条一致（`commit_diff` 而不是 `commit_files`、`stash_list` 而不是
`stashes`）：**没桩到的命令返回 `null`，读取它的组件当场抛错，而全站没有错误边界 → 整屏空白**，
看起来就像被测代码有 bug。错误捕获挂 `window.addEventListener("error", …)` 收进 `window.__errs`，
断言只读 DOM（`.hist-right` / `.commit-list li[data-oid]` / `.diff-pane [class*=hljs]`）。
2026-10-14 就是靠这层桩抓到了「点提交 → 白屏」的真 bug（见「缺陷：历史页点提交 → 整屏空白」）。

**这层桩有三个先天限制，别拿它当能测形状或测时间的东西**（③ 与「按路径筛选」里各踩实一条）：
- 那个页是**后台态**（`document.visibilityState === "hidden"`）：`requestAnimationFrame` 一次都不触发，
  `setInterval(16)` 实际约 1 次/秒。所以「等下一帧再读」的自动化会永远等不到，读到的时间也全不可信。
  量渲染耗时只能把 `performance.now()` 死死卡在 `flushSync` 前后（直接 `import` vite 预打包好的
  `/node_modules/.vite/deps/react-dom.js?v=…` 取 `flushSync`，注意它是 CJS interop，要 `.default`；
  `react-dom_client.js` 那个依赖里只有 `createRoot`/`hydrateRoot`）。
- 视口只有 **439×490**：列宽、换行、是否溢出这类几何结论在这个宽度下都是假的
  （给真实设计写 `1160px` 时，它把三列挤成互相矛盾的宽度）。几何一律看真窗口截图，不看桩。
- 后台态里 `element.focus()` **不派发 focus / focusin**，于是 `onFocus` 里的懒加载看起来"没生效"
  （`document.activeElement` 却确实变了，很容易误判成应用 bug）。要驱动聚焦得手动
  `dispatchEvent(new FocusEvent('focusin', { bubbles: true }))` —— React 的 `onFocus` 挂的是 focusin。
  同理键盘输入要用原生 setter 改 `value` 再派 `input` 事件，`Enter` 之类派 `keydown`（`bubbles: true`）
  才能被 React 的根监听接住。

## 变更文件速览（本次键盘优先体系）
- 新增：`src/shortcuts.ts`（命令注册表 + chord 解析 + 可覆盖键位）、
  `src/components/ShortcutsHelp.tsx`、`src/test/setup.ts`、
  `src/shortcuts.test.ts`、`src/store.test.ts`
- 修改：`src/store.ts`（`commitDraft`/文件多选/免确认白名单/`showShortcuts`）、
  `src/App.tsx`（全局派发 + Esc 层级）、
  `src/components/{History,WorkingCopy,Settings,TopBar,ConfirmDialog}.tsx`、
  `src/styles.css`（快捷键面板/键位编辑/批量条/选中行）、
  `vite.config.ts`（vitest node 环境 + localStorage shim）
- 未改动 Rust 后端（`cargo check` 通过）
- 已在 Vite 页面（与 WebView 加载同一份前端）用真实 DOM 事件验证，注入模拟 store 状态驱动：
  chord `g h`/`g w` 切标签、`j/k/End/Home` 选中提交、`?` 面板开关、`⌘,` 设置、
  `/` 聚焦筛选、输入框内不劫持按键、设置内重绑（旧键位失效、`gitcraft_keymap_v1` 写入）、
  多选跨列不串选 + 批量条按钮随分组变化、`d` → 勾选不再提示 → 白名单生效跳过弹窗
- 待补：原生窗口内回归（Tauri `invoke` 在浏览器不可用，真实 git 操作未走通）、拖拽进窗口

## 变更文件速览（本次 UX 重构：左右分栏 + 行首复选框）
- 后端：`gitcraft-core/src/git.rs`（新增 `untracked_patch`）、
  `src-tauri/src/commands.rs` + `src-tauri/src/lib.rs`（命令与注册）、`src/api.ts`（`untrackedPatch`）
- 状态：`src/store.ts`（`showPatch` 重写 + `patchLabel`、`clearPatch`、`pickRepoFolder`、
  `addRepo` 自动选中、`selectCommit` 改走 `commitPatch`、移除死代码 `diff`/`showWorkingDiff`）
- 组件：`src/components/WorkingCopy.tsx`（分栏 + 复选框行 + 提交框折叠）、
  `src/components/DiffView.tsx`（`DiffPanel` → `DiffPane`）、`src/components/History.tsx`（分栏 + 即时筛选）、
  `src/components/Sidebar.tsx`（头部操作区 + 移除手动浏览弹窗）、`src/App.tsx`（`start-card` 空状态）
- 样式：`src/styles.css`（`.wc-split`/`.hist-split`/`.frow` 等高/`.row-acts` hover/`.diff-pane`/`.start-card` 等）
- 后端：`gitcraft-core/src/git.rs`（`untracked_patch`）、`src-tauri/src/commands.rs`、
  `src-tauri/src/lib.rs`（命令注册）、`src/api.ts`（`untrackedPatch`）—— `cargo check` 通过
- 测试：`src/store.test.ts`（api 改为 `vi.hoisted` 打桩，新增 `showPatch` 4 例）
- 已删除临时验证产物：`public/gitx-mock.json`、`/tmp/gitx-*`、测试仓库书签行

## 变更文件速览（本次文件溯源：FileHistory 分栏弹窗）
- 后端：`gitcraft-core/src/git.rs`（`file_log` / `rev_file_text` / `restore_file`）、
  `gitcraft-core/src/models.rs`（`FileLogEntry`）、
  `src-tauri/src/commands.rs` + `src-tauri/src/lib.rs`（三条命令封装与注册）
- 前端：`src/api.ts`（`fileLog`/`revFileText`/`restoreFile`）、`src/types.ts`（`FileLogEntry`）、
  `src/store.ts`（`fileLogPath`/`patchFile` 状态 + 五个动作，移除旧 `fileHistory`）、
  `src/components/FileHistory.tsx`（重写为分栏弹窗）、
  `src/components/History.tsx`（提交详情文件列表 `.commit-files`）、
  `src/components/WorkingCopy.tsx`（双击/右键入口，改走 `setFileLogPath`）、
  `src/components/DiffView.tsx`（`DiffPane` 标题「文件历史」按钮）、
  `src/App.tsx`（弹窗挂载 + Esc 层级）、`src/shortcuts.ts`（命令 `wc.fileLog` = `h`、键盘门加入 `fileLogPath`）、
  `src/styles.css`（`.fh-*`、`.commit-files`/`.cf-*`、`.icon.wide`）
- 测试：`gitcraft-core/src/git.rs`（真实 git 用例 12 → 15）、`src/store.test.ts`（+6 例）、
  `src/shortcuts.test.ts`（+1 例），前端合计 34 例
- 待补：原生窗口真机回归（`invoke` 在浏览器不可用）、弹窗内 blame / 两版本对比

## 变更文件速览（本次改写提交信息）
- 后端：`gitcraft-core/src/git.rs`（`commit_message` / `pushed_refs`）、
  `src-tauri/src/commands.rs`（`head` / `commit_message` / `pushed_refs` 三个薄封装）、
  `src-tauri/src/lib.rs`（注册）
- 前端：`src/api.ts`（`head`/`commitMessage`/`pushedRefs`）、
  `src/store.ts`（`headId`/`amendOid`/`amendPrefill`/`amendHeadMessage`，`loadCommits` 顺带取 HEAD）、
  `src/components/AmendMessageModal.tsx`（新增）、
  `src/components/History.tsx`（HEAD 才出现的右键项 + 「✎ 改写提交信息」按钮）、
  `src/App.tsx`（弹窗挂载 + Esc 层级）、`src/shortcuts.ts`（`commits.amend` = `m`、键盘门加 `amendOid`）、
  `src/styles.css`（`.amend-modal`/`.amend-body`/`.amend-label`/`.amend-count`/`.amend-warn`/`.amend-hint`）
- 测试：`gitcraft-core/src/git.rs`（+1 例真实 git 用例，15 → 16）、
  `src/store.test.ts`（+9 例，mockApi 补 `head`/`commits`/`commitMessage`/`pushedRefs`/`commit`）、
  `src/shortcuts.test.ts`（+1 例）
- 未做：原生窗口真机回归；改更早提交（走交互式变基）

## 变更文件速览（本次 Provider 插件机制）
- 后端核心：`gitcraft-core/src/provider/mod.rs`（`Host` trait、`Capability`、`ApiRequest`、
  `RepoRef`、`HOSTS` 注册表、`parse_remote`/`detect`/`repo_ref_for`、点号路径的 `field` 助手）、
  `gitcraft-core/src/provider/github.rs`、`gitee.rs`、`codeup.rs`（各带 fixture 单测）、
  `gitcraft-core/src/models.rs`（`PullRequest`）、`gitcraft-core/src/lib.rs`（`pub mod provider;`）
- host 层：`src-tauri/src/commands.rs`（`provider_info` / `provider_set_token` /
  `provider_list_prs` / 新增 `open_url`，`remote_urls` 把 origin 排到最前）、
  `src-tauri/src/lib.rs`（四条命令注册）
- 前端：`src/types.ts`（`Capability`/`ProviderInfo`/`Detected`/`ProviderStatus`/`PullRequest`）、
  `src/api.ts`（`providerInfo`/`providerSetToken`/`providerListPrs`/`openUrl`）、
  `src/store.ts`（`providerStatus`/`pullRequests` + 四个动作，`selectRepo` 并发 `loadProviders`，
  删仓库/切仓库清 provider 状态）、
  `src/components/Settings.tsx`（`Providers` 区块 = 「代码托管」）、
  `src/components/Remotes.tsx`（remote 列表 + provider PR 面板）、
  `src/styles.css`（`.badge`/`.provider-row`/`.provider-head`/`.provider-token`/`.pr-panel`/`.pr-detail`）
- 测试：`gitcraft-core` 16 → 28 例（provider 层 12 例），`src/store.test.ts` +11 例（55 例总）
- 修出的 bug：`field()` 把 `["user","login"]` 当备选键名而非路径，嵌套字段恒为空 →
  改为点号路径，两个 fixture 解析例守住
- 未做：真机回归、真实令牌联调、CI 状态 / 创建 PR / Codeup 签名

## 变更文件速览（本次快捷键总开关）
- 状态：`src/store.ts`（`shortcutsOn` + `SHORTCUTS_KEY`/`readShortcutsOn` + `setShortcutsOn`）
- 派发：`src/shortcuts.ts`（`shortcutsBlocked()`；`isEventFor` 与 `useGlobalShortcuts` 两处闸门）
- 界面：`src/components/Settings.tsx`（「快捷键」小节顶部的开启/全部关闭 + 状态说明）、
  `src/components/ShortcutsHelp.tsx`（「重新开启」按钮 + 关闭态提示条）、
  `src/components/TopBar.tsx`（关闭时 `⌨` 变 `.kbd-off`）、`src/styles.css`（`.kbd-off`/`.sc-off-note`）
- 测试：`src/shortcuts.test.ts`（store 打桩改为可变 holder，+3 例）、`src/store.test.ts`（+2 例），前端 55 → 60 例
- 未做：按页面/按作用域分别开关（现在是一刀切）、原生窗口真机回归

## 变更文件速览（本次更新检查：直连 GitHub Releases）
- 核心：`gitcraft-core/src/update.rs`（新增模块：`check_slug`/`release_api_url`/`parse_release`/
  `pick_asset`/`parse_version`/`is_newer`）、`gitcraft-core/src/lib.rs`（注册模块）、
  `gitcraft-core/src/models.rs`（`Release`/`Asset`）
- 后端：`src-tauri/src/commands.rs`（`UpdateCheck` + `check_update`，15s 超时，
  非 2xx 保留 GitHub `message`）、`src-tauri/src/lib.rs`（注册命令）
- 前端：`src/types.ts`（`ReleaseAsset`/`Release`/`UpdateCheck`）、
  `src/api.ts`（`checkUpdate`）、
  `src/store.ts`（`updateRepo/updateCheck/updateChecking/updateIgnored/updateAuto` +
  `setUpdateRepo`/`setUpdateAuto`/`checkForUpdate`/`setUpdateIgnored`/`openDownload` +
  `updateNotice()`）、
  `src/App.tsx`（启动静默检查一次）、
  `src/components/Settings.tsx`（`Updates` 区块 = 「更新」）、
  `src/components/TopBar.tsx`（`.update-chip` 新版本提示）、
  `src/styles.css`（`.update-chip`/`.update-repo`/`.update-auto`/`.update-result`/`.update-notes`）
- 测试：`gitcraft-core` 28 → 35 例（update 层 7 例），`src/store.test.ts` +6 例（前端 66 例总）
- 明确不做：应用内安装、签名密钥、updater 插件依赖（用户拍板「直接 github release」；
  TODO 里留了升级路径）

## 变更文件速览（SourceTree 风格布局，2026-10-10）
> 诉求：首页 tab 太宽，参照 SourceTree——默认只显示分组与列表，用 tabbar 切换本地/远端。

- 新增：`src/components/Nav.tsx`（左侧分组导航：工作区[文件状态/贮藏]、仓库[历史/分支/远程]，
  替代原本横跨顶部的 `tabs` 栏；无仓库时禁用）
- 修改：`src/App.tsx`（移除顶部 `tabs` 渲染，插入 `<Nav />`，内容区仍按 `activeTab` 渲染；
  默认 `activeTab` 即「文件状态」分组+列表视图）、
  `src/components/Branches.tsx`（顶部加 tabbar：本地分支 / 远端分支 / 标签；
  远端分支只读+「检出」，标签可删除）、
  `src/styles.css`（`.nav`/`.nav-group`/`.nav-group-title`/`.nav-item`(active 反白)/`.nav-icon`、
  `.branch-tabs` 下划线式 tabbar）
- 验证：`npm run build` + `npx tsc --noEmit` 通过

## 变更文件速览（2026-10-10 布局与菜单微调）
> 诉求：文件状态未暂存放下面；历史改为上下布局且 diff 带行号；右键 / 顶部菜单需优化。

- **文件状态分组顺序**：`WorkingCopy.tsx` 改为「已暂存（上）→ 未暂存（下）」（`fileSection` 调用顺序对调）
- **历史上下布局**：`History.tsx` 提交列表包入 `.hist-commits`（顶部，固定 42vh 滚动），详情（改动文件+AI+diff）移到下方；
  `styles.css` 的 `.hist-split` 由左右 grid 改为 `flex-direction: column` + `.hist-commits` 固定高度
  > **2026-10-14 再修订**：用户「点击提交，可以把改动文件详情显示在提交列表右侧」。现在上半区
  > `.hist-top` 是「提交列表 | 改动文件」左右并列，下半区整宽给差异。**「上下」保留的是差异与列表的分层，
  > 不再是文件区的位置** —— 别再按今天之前的记录把文件列搬回 `.hist-right`。
- **diff 行号**：`DiffView.tsx` 的 `parsePatch` 跟踪 old/new 行号（hunk `@@` 解析起点），`renderRow` 渲染
  `.dl-no-old`/`.dl-no-new` 两列行号槽；`styles.css` 新增 `.dl-no` 样式
- **顶部菜单**：`TopBar.tsx` 新增 ☰ 分组下拉（仓库：打开/复制路径/刷新；视图：主题/快捷键/设置），
  复用 `ContextMenu` 组件保持一致；原有 Fetch/Pull/Push/⚙️/主题按钮保留
- **右键菜单补全**：`History.tsx` 的「改动文件」列表项加右键（打开/复制路径/追溯历史/加入 .gitignore）
- 验证：`npm run build` + `npx tsc --noEmit` 通过


## 变更文件速览（2026-10-11 环境一体化：拖拽 / 预览 / 扫描 / SSH / git 引导 / LFS / 分组）
> 诉求：八个点——SSH 自动识别本机 `~/.ssh`、拖拽识别、列表分组、列表加标记（github 图标）、
> github1s 极速预览、扫描文件夹、Windows 装 git 引导、git flow 与 git lfs。

- **core 新增模块**：`gitcraft-core/src/preview.rs`（github1s 地址拼装）、`gitcraft-core/src/ssh.rs`
  （`~/.ssh` 清点、ssh_config 解析、公钥读取守卫、`ssh -T` 试连措辞翻译）；`lib.rs` 补 `pub mod`
- **core 扩展**：`git.rs` 加 `find_repo_root` / `scan_repos` / `git_version` / `lfs_version` /
  `lfs_info` / `unpushed_commits` 与共用的 `numeric_prefix`；`models.rs` 加 `LfsInfo`
- **命令层**：`src-tauri/src/commands.rs` + `lib.rs` 新增并注册 `repo_root_of`、`scan_repos`、
  `git_env`、`github1s_url`、`ssh_inventory`、`ssh_public_key`、`ssh_probe`、`lfs_status`、`unpushed`
- **前端接线**：`types.ts`（`GitEnv`/`LfsInfo`/`Ssh*`/`AuthReport`）、`api.ts`、`store.ts`
  （`gitEnv`/`ssh`/`lfs`/`unpushed` 四组状态与 `checkGitEnv`/`loadSsh`/`copyPublicKey`/
  `probeSsh`/`openPreview`/`scanFolder`/`loadLfs`/`loadUnpushed` 八个动作）
- **界面**：`App.tsx`（拖拽遮罩 + git 缺失横幅 `GitSetup` + 首屏「扫描」入口）、
  `Sidebar.tsx`（扫描按钮）、`Remotes.tsx` / `DiffView.tsx`（极速预览按钮）、
  `Settings.tsx`（新增 SSH 区）、`WorkingCopy.tsx`（目录折叠分组 + LFS 徽标与提示）、
  `History.tsx`（日期分组 + 未推送徽标 + 图形 y 坐标改走槽位表）、
  新增 `components/ProviderMark.tsx`、`historyGroup.ts`（纯函数分组）
- **样式**：`styles.css` 追加 `.drag-overlay`、`.git-setup`、`.ssh-*`、`.badge.lfs`、`.lfs-hint`、
  `.dir-head`、`.day-head`、`.badge.unpushed`、`.pmark`
- 顺带：`cargo fmt` 把上一轮 provider/update 的几处换行补齐（**纯格式化，无逻辑改动**）
- 验证：`cargo test` 61 / `npm test` 98 / `npm run build` 437.66 kB / `cargo build --offline` 通过

## 变更文件速览（本次：输入弹窗替身 —— window.prompt 在真实窗口里从来不通）
> 起因是对照 SourceTree 时顺手查了 wry：`gitcraft` 的四条 `window.prompt` 是不是真能用。
- **事实（读盘核实，非记忆）**：`~/.cargo/registry/.../wry-0.57.0/src/wkwebview/class/wry_web_view_ui_delegate.rs`
  的 `unsafe impl WKUIDelegate` 只实现了 `runOpenPanel`（文件上传）与媒体授权，
  **没有** `runJavaScriptTextInputPanelWithPrompt`（也没有 alert/confirm 两个）。
  → 打包版里 `window.prompt()` 恒返回 `null`：「新建分支」「新建标签」「重命名分支」「手动输入路径」
  四个菜单项**点了没反应**，而且没有任何报错。浏览器桩（Chrome 有原生 prompt）里全部正常，
  所以这个缺陷从来没被我们的验证通道看见过。全仓库确认只有这 4 处，且没有用到 `alert`/`confirm`。
- 状态：`src/store.ts`（`inputDialog` + `requestInput`/`resolveInput`，模块级 `inputResolver`
  与 confirm 同一形状；`resolveInput` 里做边界归一：纯空白与取消都算 `null`，默认值补空串防受控退化）
- 界面：新增 `src/components/InputDialog.tsx`（`autoFocus` + `onFocus select`；Enter 提交、
  `imeOwns` 挡住拼音选词那个 Enter；空串时「确定」禁用且 Enter 无效）；
  `src/App.tsx`（挂 `<InputDialog />`，Esc 阶梯新增一档 `return s.resolveInput(null)`，排在 confirm 之后）；
  `src/shortcuts.ts`（弹层闸门加 `s.inputDialog`）；`src/styles.css`（`.input-modal`，
  其中 `flex: none` 是必要的：`.filter-input` 原为横向工具条写的 `flex: 1`，而 `.modal` 是纵向容器，
  主轴因此变成高度）
- 改调用点：`src/components/History.tsx`（`actNewBranch`/`actNewTag` 先 `closeMenu()` 再 `await requestInput`，
  顺带把 `menu.oid` 先取到局部变量，避免 await 之后读已经卸载的菜单）、
  `src/components/Branches.tsx`（`rename` 改 async，改名前后同名则不发命令）、`src/components/RepoTabs.tsx`
- 顺带（CSS 变量）：`src/styles.css` 4 处 `var(--fg)` → `var(--text)`（`--fg` 从来没定义过，
  那 4 条声明一直是无效的）；删掉 `.modal` 里失效的 `background: var(--panel)`（后被 542 行同名规则覆盖，
  属于死声明而非渲染 bug）。新增 `src/cssVars.test.ts`：扫全表，任何**无兜底**的 `var(--x)` 必须在样式表里
  有定义，并额外钉死 `--fg`/`--panel` 两个名字不许回来。
- 判据：`npx tsc --noEmit` 干净；`npm test` 187 → **194 例 / 17 文件全绿**；`npm run build` 通过（index.js 285.86 kB）。
- 反向变异（三处，一次跑完，失败集合与预测完全一致后整文件还原 `cmp` 逐字节相同）：
  ① RepoTabs 塞回 `window.prompt(` → 只有 `appShell` 那条红；
  ② 从 shortcuts 闸门删 `s.inputDialog ||` → 只有「弹层占用键盘」红；
  ③ 把 `value: value ?? ""` 改回 `value` → 只有「取消给 null，纯空白也按取消处理」红。
- 浏览器桩里**看到过的**：弹窗渲染出标题「重命名分支「main」为」、输入框初值 `main`、`document.activeElement`
  确实是这个 input（autoFocus 生效）；把值改成 `  feature/登录  ` 后按 Enter，promise 拿到 `feature/登录`
  （中文往返 + 首尾空白都按约定处理），弹窗卸载、`inputDialog` 归 null；空值时「确定」禁用且 Enter 不关闭；
  往 `window` 派 Esc 后 promise 拿到 `null`。**桩本身的窗口标题/侧栏渲染没变过，也不在本批范围内。**
- **没看到过的**：真实 Tauri 窗口的截图（本批没重开窗口）。已知 `window.innerWidth` 在桩里是 **0**，
  所以 `.input-modal` 量出来 46×42 px（`92vw` 塌成 0 + padding），**这条几何结论无效**，
  卡片宽度、输入框是否真占满整行、按钮排布都得等真窗口截图。另外「输入中文时用拼音候选框的 Enter 不误提交」
  只做了源码级约束（`imeOwns`），没在窗口里真敲过一次拼音。

## 变更文件速览（本次：渲染兜底 ErrorBoundary，崩屏不再等于白屏）
- 新增 `src/components/ErrorBoundary.tsx`（class 组件，React 18 的 render 抛错只有 class 生命周期能接）：
  `getDerivedStateFromError` 接住 → 画标题 + **完整 stack** + 两个出口（重试渲染 = `setState({error:null})`、
  重新加载 = `location.reload()`）。stack 上屏是因为这个 WebView 里用户够不到控制台，页面是唯一一份错误副本。
- 挂载：`src/main.tsx` 在 `StrictMode` 外面包 `<ErrorBoundary><App /></ErrorBoundary>`。
- 样式：`src/styles.css` 追加 `.crash`/`.crash-title`/`.crash-detail`/`.crash-acts`（全部走主题变量，
  `--red` 标题、`--bg-3` 底、等宽 + `max-height: 40vh` 可滚）。
- 判据：`npx tsc --noEmit` 干净；`npm test` 194 → **195 例全绿**；`npm run build` 通过（index.js 286.66 kB）。
- 浏览器桩里**看到过的**（这才是本批的主要证据，光有源码断言不够）：把 `throw new Error("mutant: 崩屏验证")`
  临时插进 `App()` 第一行，页面渲染出的是崩溃卡片而不是白屏——标题「界面渲染出错，已停在这里」、
  `.crash-detail` 里是 `Error: mutant: 崩屏验证 at App (http://localhost:1420/src/App.tsx:…)`、
  两个按钮都在；点「重试渲染」后再次抛错，卡片**原地保留**（没有白屏、也没有死循环）。
- 反向变异：把 `main.tsx` 的包裹拆成裸 `<App />` → 只有新增那条「根上有 ErrorBoundary」红，1 失败 / 194 通过。
- **过程失误（记下来，因为它差点毁掉自己的改动）**：还原变异时图省事跑了 `git checkout -- src/main.tsx`，
  而这个文件本轮刚从 HEAD 之外的状态被我改过——checkout 直接把它退回 HEAD，本批的挂载代码整个消失。
  重写之后才恢复。**约定：还原变异一律用先前 `cp` 到 /tmp 的快照 + `cmp` 逐字节校验；`git checkout` 只在
  确认过该文件相对 HEAD 无自有改动时才可用。**
- **没看到过的**：真实 Tauri 窗口里的崩屏外观（桩里 `innerWidth` 是 0，卡片宽度/`max-height:40vh` 的实际
  观感无效）；异步抛错（`.then` 里的 reject、`setTimeout` 回调里的 throw）**不归 error boundary 管**，
  本批没有处理这类路径——它们仍会落到 `src/store.ts` 的 `wrap()`（写进 `error` toast）或静默；
  `componentDidCatch` 上报（没有上报通道，故意不加）。

## 变更文件速览（本次：项目更名 GitX → gitCraft，390 处 / 42 文件）
- 标识符：`src-tauri/tauri.conf.json`（productName `GitCraft`、identifier `com.gitcraft.app`、窗口 title）、
  `src-tauri/capabilities/default.json` 与 `gen/schemas/capabilities.json`（描述文案）、
  `src-tauri/Cargo.toml`（`name`/`default-run` = `gitcraft`，lib 名 `gitcraft_lib`）、
  `src-tauri/src/main.rs`（`gitcraft_lib::run()`）、`package.json` / `package-lock.json`（`gitcraft`）、
  `index.html`（`<title>`）、`src/windowTitle.ts`（无仓库时的兜底标题）
- crate：`gitx-core/` → **`gitcraft-core/`**（`git mv`，git 里记成 rename），
  `Cargo.toml` 包名 `gitcraft-core`、`GitxError` → `GitcraftError`、全仓库 `gitx_core::` → `gitcraft_core::`、
  provider 的 `user-agent: gitx` → `gitcraft`、rebase 临时文件 `gitx-rebase-*.todo` → `gitcraft-*`、
  更新检查的资源名样例 `gitx_*.dmg` → `gitcraft_*.dmg`（**发布产物真改名时这条匹配规则要再对一遍**）
- 前端：13 个 `gitx_*` localStorage 键 + 动态键 `gitx_draft_<repo>` → `gitcraft_*`（[src/store.ts](file:///Users/baorong/Developer/superlbr/gitx/src/store.ts)、
  `src/shortcuts.ts`、`src/patchGate.ts`、`History.tsx` 及对应断言）、`GitxState` → `GitcraftState`、
  CSS keyframes `gitx-spin` → `gitcraft-spin`、`.gitignore` 注释、`gen-icon.mjs`
- 文档：README 全量改名（含代码地图里的 `gitcraft/`、`gitcraft-core/` 路径）；
  PROCESS.md 也一并改名以保持正文里的代码引用可对上，**只有 1698 行那条历史产物名保留 `gitx-*`**
  （那是一次性临时文件的真实旧路径，改了反而对不上历史）
- 判据（整批一次跑齐，不逐文件跑）：`cargo test`（gitcraft-core）**72 例全绿**、
  `cargo check --tests`（src-tauri）通过、`npx tsc --noEmit` 干净、`npm test` **195 例全绿**、
  `npm run build` 通过（index.js 286.79 kB）、`npm run tauri dev` 真实起窗口并截图
- **没看到过的**：`tauri build` 打出的 `.app`/`.dmg` 包（改名后没打包验证过，`productName` 参与产物命名）；
  Windows/Linux 上 identifier 与存储目录的行为（本机只有 macOS）；
  「用户手上如果有别的机器/别的 clone，改名后第一次启动会看到空书签」这个后果**没有**在真机之外验证。

