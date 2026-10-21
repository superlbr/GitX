# GitX

一个 Git 桌面客户端，目标是让 **SourceTree 的用户原样迁过来，同时比它更快**。
界面布局与交互语义照着 SourceTree 4.2.19 的实测截图做（`docs/reference/`），差别只有软件名；
「更好」只押在两处：**性能**（大仓库里每帧做的活只跟窗口大小有关，不跟仓库大小有关）和
**功能**（一律走 git CLI，中文/emoji 提交信息、文件溯源、补丁字节上限这些细节是按实际事故写出来的）。

GitX 与 Atlassian 无关，不含任何 SourceTree 的素材、商标或代码。

> **2026-10-21：合并为单 crate**——原 `gitcraft-core/`、`gitcraft-app/` 两个 crate 合成根目录一个包
> （包名 `gitx`），同日项目由 GitCraft 更名 **GitX**；分层边界改由 lib / bin 两个编译单元保住。
>
> **2026-10-20：删掉了 React + Tauri 那一层**（当时的 `src/`、`src-tauri/`、`package.json`、Vite 配置）。
> 它比 egui 壳功能多得多，但它每一帧都要跨一次 IPC 把 git 输出序列化成 JSON，
> 而「每帧的活只跟窗口大小有关」这条目标正是被那一跳堵死的。
> 完整代码留在最后一个提交 `9a3077c` 里，想捡哪块功能从那儿取。
> **下面的「功能」清单里凡是只在已删除那层做过的，都归进了「壳里还缺的」——别按旧清单找活。**

仓库是一个**单 crate 双 target** 的 Rust 包，核心层与壳的边界由 lib / bin 两个编译单元保住：

| | 是什么 | 状态 |
| --- | --- | --- |
| `src/lib.rs`（lib target `gitx`） | 纯 Rust 的 git 与本地状态层，**零 UI 依赖**（86 例单测） | 稳，功能都在这层，改这里最划算 |
| `src/ui/`（bin target `gitx`） | egui/eframe 的原生壳，**直连核心、没有 IPC 那一跳**（103 例无头判据） | 进行中：外壳几何、提交图、暂存/提交已通，其余见「壳里还缺的」 |

> **许可证**：GitX 按 **MIT 或 Apache-2.0 双许可**发布，由你二选一（[`LICENSE-MIT`](LICENSE-MIT)、
> [`LICENSE-APACHE`](LICENSE-APACHE)、[`NOTICE`](NOTICE)）。选 Apache 那一半是为了它的**明示专利授权**，
> 保留 MIT 那一半是因为纯 Apache-2.0 的代码进不了 GPLv2 的项目。
> 名字与图标是**商标**，不在授权范围内；第三方素材（含 `docs/reference/` 里那四张 SourceTree 截图）
> 各自保留自己的条款，逐条登记在 [`ATTRIBUTION.md`](ATTRIBUTION.md)。
> 想参与的话先来读 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 架构

```
gitx/
├── Cargo.toml              # 单包双 target：[lib] 核心 + [[bin]] 壳（边界见上面那张表）
├── src/
│   ├── lib.rs              # 核心层入口：mod 声明 + 统一 re-export（GitxError / models / rusqlite）
│   ├── git.rs              # git CLI 封装与解析（status/log/diff/branch/tag/stash/remote/文件溯源/
│   │                       #   拖拽找仓库根/扫描目录/git 与 git-lfs 版本/LFS 文件/未推送集合/
│   │                       #   加速节点改写/中文路径与 GBK 容错/explain_git_error 30 类中文说明）
│   ├── provider/           # 代码托管平台插件（mod.rs 注册表 + github.rs / gitee.rs / codeup.rs）
│   ├── preview.rs          # github1s 极速预览：把 remote 换成 GitHub 自己的 URL 语法
│   ├── ssh.rs              # ~/.ssh 清点、ssh_config 解析、公钥读取守卫、`ssh -T` 试连措辞翻译
│   ├── update.rs           # 应用更新检查：Release 解析 / 版本比较 / 按本机挑安装包
│   ├── db.rs               # SQLite 持久化（仓库书签、设置、平台令牌）
│   ├── models.rs           # 共享数据模型
│   ├── error.rs
│   ├── main.rs             # 壳的入口：viewport 配置 + eframe::run_native，以及 103 例无头判据的挂载点
│   ├── geometry.rs         # 无头门禁本体（bin 的 cfg(test) 模块，模块名 geometry_tests 不变）
│   └── ui/                 # 界面模块（拆分自旧 main.rs，见 PROCESS.md「拆分 main.rs」）
│       ├── app.rs          #   壳本体：面板布局、动作分发（stage/commit/fetch…全走 gitx::git）
│       ├── theme.rs        #   几何常量组：工具栏 / 侧栏 / 五列左缘 / 行距，每个数都从参考图量的
│       ├── fonts.rs        #   字体装配：窗口与无头测试走同一条 `install(ctx)` 路径
│       ├── state.rs        #   状态与帧间消息（worker → mpsc → 下一帧 poll）
│       ├── data.rs         #   数据装配（snapshot_of / patch_of）与样式装配（style）
│       ├── widgets.rs      #   绘制件
│       ├── graph.rs        #   提交图车道（layout_graph 在取数据那一步跑，不在帧里）
│       └── diffmodel.rs    #   差异渲染模型
├── assets/fonts/           # CJK 字体（8.3MB 那份没进仓库，只留 OFL.txt，见「开发运行」）
├── docs/reference/         # SourceTree 4.2.19 实测截图——界面改动的唯一对照来源，别凭印象调像素
├── scripts/gates.sh        # 四条门禁一次跑完（改动阶段只跑受影响的那一条，见 CONTRIBUTING.md）
├── .github/                # CI（workflows/gates.yml）+ issue / PR 模板（PR 必须填「判据」和「反向变异」）
├── LICENSE-MIT / LICENSE-APACHE / NOTICE   # 双许可 + 商标条款；NOTICE 是再分发时必须保留的那份
├── ATTRIBUTION.md          # 第三方素材逐条登记（字体、截图、依赖），也是素材审计那条门禁读的文件
├── PROCESS.md              # 台账：每批改了什么、判据是什么、反向变异红了哪几条、仍然没看到过的
└── gen-icon.mjs            # 生成应用图标（零依赖：SDF 渲染「提交图交叉成 X」的 Logo）
```

设计要点：
- **git 操作用 git CLI 封装**（而非 libgit2），避免额外 C 依赖，行为稳定、覆盖全面。
- **分层可测**：核心层（lib target）不认识任何 UI——它看不见 `crate::ui::*`，`cargo test --lib` 直接跑；
  壳也认识得很少——它的 103 例判据是**无头**的（不需要屏幕，自己 `git init` 临时仓库，不碰你手上的仓库）。
- 本地状态（仓库书签、设置）存于 SQLite，路径 `~/Library/Application Support/com.gitx.app/gitx.db`。
- **界面态不再有任何 WebView 存储**：已删除那层的 localStorage 键（提交草稿、自定义键位、
  免确认白名单、分栏比例）随着那层一起没了，egui 壳目前只有 SQLite 一条持久化通路。
- **应用图标也是仓库里的代码**：`node gen-icon.mjs` 直接写 PNG（不装任何渲染依赖），
  图形是两条分支线交叉成 **X**、五个提交节点（中心那个是 merge），底板是品牌紫的对角渐变
  圆角方块，输出在 `assets/icon.png`；改图标就是改这份文件里的几何与配色。

## 代码地图（改动该去哪里）

| 想改的东西 | 去哪里 |
|---|---|
| git 行为 / 输出解析 / 错误措辞 | `src/git.rs`（纯函数，加一条 `cargo test` 用例即可验证） |
| **给壳加一条新的后端能力** | ① `src/git.rs`（或 `ssh.rs`/`update.rs`/`provider/`）写函数 + 单测 → ② `src/ui/` 里调用它。**只有两步**，没有 IPC 注册、没有 api.ts 键名对齐 |
| 界面几何（工具栏高、侧栏宽、五列左缘、行距） | `src/ui/theme.rs` 的常量组，每个数都是从 `docs/reference/` 那张截图量出来的；改完跑 `cargo test`，`every_log_column_starts_at_its_own_header` 会告诉你四列还贴不贴得住表头 |
| 提交图（车道、parent 线、ref 徽标） | `src/ui/graph.rs` 的 `layout_graph`；`COL_GRAPH` 是它唯一的宽度预算，超过 4 条车道目前**没有判据** |
| 差异渲染 | `src/ui/diffmodel.rs` 的差异模型与 `src/ui/app.rs` 的差异绘制段（`DIFF_ROW_H` = 15pt、行号栏右对齐、正文齐左） |
| 暂存 / 取消暂存 / 提交 | `src/ui/app.rs` 的 `Action` 分发（stage/commit 全走 `gitx::git`）；端到端那几条判据真的点按钮、真的读 `git log` |
| 仓库书签、上次同步时间、设置 | `src/db.rs`（SQLite，含老库补列的 `migrate`） |
| 代码托管平台适配（新增一个平台） | `src/provider/<新平台>.rs` 实现 `Host`（只声明能力、拼 `ApiRequest`、解析 JSON），再在 `provider/mod.rs` 的 `HOSTS` 注册一行；插件**不发网络请求**，所以能离线单测 |
| 更新检查（版本比较 / 挑安装包） | `src/update.rs`（`is_newer`/`pick_asset` 都有单测）；HTTP 那一跳随 Tauri 一起删了，壳里还没有入口 |
| github1s 极速预览地址 | `src/preview.rs` 的 `github1s_url`（只有 github.com 支持，别的 host 明说）；壳里还没有按钮 |
| SSH 密钥 / ssh_config / 试连 | `src/ssh.rs`（私钥按内容分类、权限检查只在 `#[cfg(unix)]`、`public_key_in` 只允许 `~/.ssh` 下的 `.pub`）；壳里还没有设置页 |
| 拖进来一个路径该开哪个仓库 | `src/git.rs` 的 `find_repo_root`（向上找含 `.git` 的目录，`.git` 是文件也认）；壳里的「添加仓库」走的就是它，**拖放事件本身还没接** |
| 扫描文件夹批量收仓库 | `src/git.rs` 的 `scan_repos(root, max_depth, limit)`（BFS，深度 4 / 上限 100，跳过隐藏目录、`node_modules`、符号链接）；壳里还没接 |
| 字体（中文/全角标点在壳里长什么样） | `src/ui/fonts.rs` 的 `install` + `GITX_FONT_DIR`。**字体是测试数据**：无头判据里 `.notdef` 的字宽正好是字号的一半，宽度断言会静默走偏，见「开发运行」 |

## 功能

### 核心层已经有、也有判据的（这层是资产，改它最安全）

- **一律走 git CLI**：status / log / diff / 区间 diff / branch / tag / stash / remote /
  stage / commit / push / pull / fetch / clone，加文件溯源（`file_log` 用 `--follow` 跟随改名、
  `rev_file_text`、`restore_file`）
- **中文用得起来的四处底层修正**：① 所有调用统一带 `-c core.quotepath=false`，
  中文文件名、diff 标题、补丁头部都是原文（不改用户仓库配置）；② git 输出按 UTF-8 宽松解码，
  GBK 老仓库打得开，坏字节变成一个替换符而不是整条命令失败；③ 常见 git 失败给出**中文说明 +
  原文可查**（`explain_git_error`，30 类：无上游分支、非快进被拒、克隆超时、仓库不存在、
  本地改动会被覆盖、冲突未解决……认不出的原文照出，不编造原因）；④ 提交信息保留多行原文
- **大仓库不卡的底层做法**：`--max-count` 分页读日志、`rev-list HEAD --not --remotes` 一次算出
  未推送集合、`ls-files` 补全有数量上限、旧版本全文与未跟踪文件补丁有 2 MiB 字节上限
- **未跟踪文件的补丁是合成的**（`new file` 形状），超大文件只报大小、二进制只报字节数
- **路径筛选筛的是片段而不是前缀**：裸 pathspec 是前缀匹配，monorepo 里输 `admin` 等于没筛，
  所以传三条 pathspec 的并集（裸路径 + glob 文件名 + glob 目录名）
- **远程跟踪引用**（`origin/main`）单独一路读（`remote_refs`，格式串必须用 `%(refname:lstrip=2)`，
  用 `:short` 会把符号引用打印成 `origin`，过滤失效）
- **上游**：`parse_status` 读 `# branch.upstream`（没有上游时是 `None`，界面据此不说「已同步」）；
  `push --set-upstream` 只给一半参数当场拒绝（git 自己也会拒）
- **拉取可以走加速节点**：一次性 `-c url.<节点>.insteadOf=<远程>`，用完即弃，`origin` 永远是真的那一个；
  **推送刻意不走节点**（节点是只读镜像，推上去等于「界面说成功、同事收不到」）；
  模板只接受 https、不含空格/引号/等号，否则拒
- **Git LFS 感知**：`git lfs version` / `lfs ls-files` 按各平台真实输出解析
  （`2.50.1.windows.1`、`2.47.0.rc0`、`3.0.0-pre` 都取到正确版本号）
- **代码托管插件**（GitHub / Gitee / 阿里云 Codeup）：按 remote 地址识别平台、能力位声明式、
  响应 JSON 解析成统一模型，核心层不碰网络所以全部可离线单测
- **更新检查**：Release 解析、按数字段比较版本（`1.10.0 > 1.9.0`、缺段补零、prerelease 不新于正式版，
  **认不出的 tag 永远不算升级**）、按本机 OS/架构挑安装包并跳过 `.sig`
- **SSH 诊断**：清点 `~/.ssh`，私钥按**文件内容**分类而不是后缀，标出「权限过宽」与「无公钥」；
  解析 `ssh_config` 的 Host 块；「复制公钥」只允许 `~/.ssh` 里的 `.pub`，**私钥不会离开磁盘**；
  `ssh -T` 的各家措辞翻译成「认证通过 / 密钥被拒 / 主机未知 / 连不上」。
  刻意不使用 `StrictHostKeyChecking=accept-new`——那会静默改写 known_hosts、卸掉一个安全检查
- **github1s 极速预览地址**（`/tree`、`/blob`、无 ref 退回仓库根；非 github 直接给拒绝原因）
- SQLite 持久化：仓库书签、设置、平台令牌（只写不读回界面）、fetch 成功后的时间戳，含老库补列

### 壳里已经有判据的

- **从参考截图量出来的外壳几何**：工具栏 55pt、侧栏 141pt、暂存区 245pt、文件列 435pt、
  日志行 21pt、差异行 15pt，以及 sha/主题/作者/日期四列的绝对左缘
- **五列与差异正文齐左**：每格左缘贴住自己表头的左缘，行号栏右对齐——这五个数是用
  一次性工具 pngprobe.py（stdlib 解 PNG；跑完即删，见 git 历史）在 4.2.19 截图上逐像素量的
  （图表 242 / 提交 284.5 / 描述 361.5 / 作者 825 / 日期 1017），不是估的
- **提交图车道**：parent 线一定画在子节点下方，`--graph` 的拓扑解析有独立判据
- **长列表虚拟化**：一屏之外的行不产生任何绘制指令（这是「每帧只跟窗口大小有关」的第一块证据）
- **真的点击与键入**：造临时 git 仓库，在壳里打提交信息、按下提交按钮（或 ⌘/Ctrl+Enter），
  再断言 `git log` 真的多了一条；暂存、取消暂存同一条路
- **添加仓库**：空书签列表也有一条入口（工具栏「添加仓库」→ 输路径 → 「打开」）；
  不是仓库的路径被拒且不动书签列表；接受的路径真的写进 SQLite
- **文件溯源浮层**：文件行右键「查看文件历史」→ `git log --follow` 列表 + 该提交对这个文件的补丁；
  另有「逐行归属」（真 `git blame`）与「全文」（该版本原文件）；「恢复到这一版」必须过确认卡
- **冲突解决与继续/中止**：停在合并/遴选/变基里的仓库，文件表上方出一条横幅（未解决的合并 /
  变基 / 遴选 + 冲突数），冲突文件单独一组、不再混进已暂存文件；右键菜单可用我的/他们的版本解决、
  标记已解决/未解决；继续与终止都过确认卡。判据真的造一个冲突仓库、真的解决、真的继续出一条合并提交

### 壳里还缺的（这就是活干到哪了，也是好活的清单）

已删除那层做过、egui 壳**还没接**的，按「补一条的独立程度」排：

- **设置卡**：工具栏 11 枚里只剩 index 10「设置」没接（点了是显式提示）
- **重命名分支 / `.gitignore` 编辑器 / rebase**：核心层函数齐全或已有 API，界面层**零调用**
- **多选（⌘/Shift 范围）**、**amend HEAD**、**按行暂存**（`line.hunk` 已给每行留了值）
- **键盘操作体系**：已删除那层有一整套可重绑键位 + chord（`g h`、`⌘↵`、`j/k`）和命令面板，
  壳里目前只有 ⌘/Ctrl+Enter 提交
- **拖放开仓库**、**扫描文件夹**、**原生目录选择器**（要引 `rfd`）
- **差异视图的补丁闸门**：核心层有 2 MiB 上限，壳里现在是一次画完整个补丁
- **深色模式**、**语法高亮**、**状态栏**、**窗口标题跟仓库走**
- **push / pull 端到端判据**：fetch 有真仓库判据，push/pull 仍缺真远端样本
- **紧凑视图两态**、「所有分支」语义、部分文案与 SourceTree strings 对不上（任务 #31/#32）
- **文件行末「⋯」弹层**、**提交图超过 4 条车道**、**`--max-count` 截断**目前没有断言

## 开发运行

前置：**Rust 1.96+**、git 2.x、Xcode Command Line Tools（macOS）。Node 只在重新生成图标时用到。

```bash
cargo run --bin gitx              # 起真窗口
cargo test --lib                  # 核心层单测
cargo test --bin gitx             # 103 例无头判据（不需要屏幕）
./scripts/gates.sh                # 四条门禁一次跑完（改动阶段只跑受影响的那一条）
node gen-icon.mjs                 # 重新生成 assets/icon.png
```

改核心层（`src/` 下）的函数签名后请跑 `cargo check --tests`（默认 `cargo check` **不**编译测试代码）。

那一份 8.3MB 的全量 CJK 字体**没进仓库**（`.gitignore` 挡掉了 `assets/fonts/*.otf`，
仓库里只留 `OFL.txt`）。新克隆要么把一份 Noto Sans SC（SIL OFL）放成
`assets/fonts/NotoSansSC-Regular.otf`，要么用 `GITX_FONT_DIR` 指到已有的那份，
否则只能够到退路 `~/Library/Fonts/msyh.ttf`；三者都没有就退回系统字体链，
而 SourceTree 中文提交信息里最常见的「：」（U+FF1A）在那条链上是**豆腐块**
（实测过程见 `PROCESS.md` 的「Spike 结果」）。拉丁一侧不依赖环境：Inter 三档
字重（各 ~400KB，SIL OFL）直接进仓库、嵌进二进制，正文 400 与中文同字重，
标题/虚拟行走命名字族拿真 SemiBold（`src/ui/fonts.rs`）。

**字体也是测试数据**：egui 的默认字体集没有 CJK，缺字体时汉字落到 `.notdef`，
而它的字宽正好是字号的一半（14pt 的中文字只有 7pt）。壳里几条关于**宽度与左缘**的无头判据量的就是字宽，
所以 CI 上跑出的绿灯，测的是产品里从来不会出现的那款字体——workflow 里为此专门打一条警告。
装字体的代价是每帧约 8.6ms（无头测试每帧新建 `Context`，真窗口只解析一次）。

## 测试

- 核心层 86 例（`cargo test --lib`）：在临时 fixture 仓库上验证
  status / stage / commit / diff / range / branch / tag / stash 全流程、
  `untracked_patch` 合成（文本 / 二进制）、`commits` 路径·作者筛选（路径是三条 pathspec 的并集）、
  `repo_files`（`git ls-files`，有数量上限）、SQLite 持久化、
  `file_log` 跟随改名并标注每次改动、删除记录、`rev_file_text` + `restore_file`、
  真冲突仓库上的 ours/theirs、手工 `mark_resolved` / `mark_unresolved`、`continue_op` 落合并提交、
  `abort_op` 回到合并前的工作区、
  `commit_message` 保留多行原文、`pushed_refs` 只在真的 push 过之后才列出远端分支、
  `unpushed_commits`（无远程回空、push 后为空、本地提交都在等待）、
  `remote_refs`（没配远程回空、只 `remote add` 不 fetch 仍回空、fetch 后恰好一条且指向 `head()`、
  符号与普通两种形状的 `origin/HEAD` 都被滤掉）、
  `parse_status` 读 `# branch.upstream`（没有上游时 `upstream` 必为 `None`）、
  `push` 的设置上游参数（只给一半当场拒绝且**不会留下半个上游配置**）、
  `find_repo_root` 从拖进来的文件向上走到仓库根、`scan_repos` 5 例（深度上限、跳过 `node_modules`
  与隐藏目录、仓库不再往里找、符号链接、数量上限）、
  `git --version` / `git lfs version` / `git lfs ls-files` 按各平台真实输出解析、
  提交图 `--graph` 拓扑（`topo_switch_only_adds_the_order_flag`、`every_parent_is_listed_below_its_child`）、
  provider 层 12 例（scp / https / 带凭据 / 嵌套分组的 remote 解析、按域名识别平台且未知域名不硬塞给
  GitHub、GitHub 请求头与企业域名 `/api/v3`、空令牌直接拒、Gitee 令牌百分号编码进查询串、
  fixture JSON 解析出嵌套字段、平台把错误对象回成中文错误、Codeup 认得但能力为空）、
  update 层 7 例、preview 层 5 例、ssh 层 10 例（含 GitHub 非零退出仍判认证通过、
  主机未知与连不上不混为一谈、没有 `~/.ssh` 也是答案不是错误）、
  中文与编码 2 例、`explain_git_error` 30 类与「认不出就原样返回」、
  节点改写 2 例 + **离线端到端**（无节点 fetch 失败、带节点成功、`refs/remotes/origin/main` 落下、
  `origin` 的 URL 一字未改）、老数据库补 `last_fetched_at` 列、`release_api_url` 的镜像主机 /
  `{url}` 模板 / 非 https 拒绝 / 含空格拒绝。
- 壳 103 例（egui 无头，`cargo test --bin gitx`，**不需要屏幕**）：
  各面板的几何带、行距（日志 21pt / 差异 15pt 逐行断言）、长列表虚拟化、提交图车道、
  日志五列与差异正文的**齐左**（每格左缘贴住自己表头左缘，数字取自参考截图实测），
  以及**真的点击与键入**——临时 git 仓库、打提交信息、按提交按钮（或 ⌘/Ctrl+Enter）、
  再断言 `git log` 真的多了一条；还有空书签列表的添加入口、非法路径被拒、
  合法路径真的落库。
- **四条门禁串在一个脚本里**：`./scripts/gates.sh`（核心测试、egui 无头判据、egui 零警告构建、
  许可证与素材审计）。最后那一条只读文本文件，实现在 `scripts/license-audit.sh` 里只有一份，
  CI 调的是同一个脚本。CI 每次 push 跑同一份，见 `.github/workflows/gates.yml`。

已删除那层曾有的 226 例前端测试（vitest）随源码一起没了，其中**结论仍然成立、值得在 egui 里重做**的：
大补丁必须在 `diff --git` 边界折叠（按字节直接砍会留下半个 hunk，画得出来但行号是假的）、
窄列的路径省略必须换头部而不是尾部（CSS 从尾部省略会把 `finance` 和 `user` 切成同一个 `admin/pack…`）、
emoji 不能进 UI（跨系统渲染不一致）、提交行高度只能有一个来源（改一处会让图和文字错位）、
**没有上游时绝不显示 `↑0 ↓0`**、push 的实参里不得出现加速节点串。
只有两条还在核心层成立：「内容通路 2 MiB 上限」有测试，「push 拿不到加速节点」是签名级的
（`git::push` 的参数里根本没有 `node` 这一位，见 `src/git.rs:1464`）。
其余四条**如今没有判据守着**，它们只是下面「壳里还缺的」里的文档。

## 贡献

上手次序：[CONTRIBUTING.md](CONTRIBUTING.md) 讲清两层的职责、三条硬规矩（修复必须配一条会红的判据、
每条判据都要跑一次反向变异、有一类判据必须诚实记下「删掉也没有测试变红」的**契约锁**）和窄门禁表；
「好的第一 PR」列表在那里，`.github/ISSUE_TEMPLATE/` 下有问题与 PR 模板。
流程与已知边界写在 [PROCESS.md](PROCESS.md)——那份文档记录的是「哪里还没验证过」，
比任何 README 都更适合用来挑活。

## 后续路线

按「离能用还差多少」排：

- **把壳补到能日常用**：分支 / 贮藏 / 远程面板 → fetch / pull / push（含领先落后数与「未设置上游」
  的措辞）→ 键盘操作体系 → 右键菜单与多选 → 文件溯源弹窗 → 补丁闸门
- **拖放开仓库**（`find_repo_root` 已就位，缺 eframe 事件）与**扫描文件夹**、**克隆**
- 更新检查的 HTTP 那一跳随 Tauri 一起删了；要恢复就用 `src/update.rs` 的纯函数
  加一层薄客户端，签名发布（`tauri-plugin-updater` 那条路已随 Tauri 作废）
- Provider 插件往「常用操作」长：CI 状态 → 触发 CI → 创建 PR（能力位已预留，界面按 `capabilities`
  渲染，所以加一个能力不会牵动别处）；Codeup 接阿里云 AK/SK 签名；再加 GitLab / 自建 Gitea
- 令牌从 `settings` 表迁到系统钥匙串
- 改写更早提交的信息（reword，需在交互式变基里做，当前只支持 HEAD）
- git flow：`init / feature / release / hotfix` 全套（要先定分支模型与合并策略）
- Git LFS 的实际推拉（`git lfs install/pull/push`），现在只有检测函数
- 平台标记换成各家真实 logo + 提交列表作者头像（先选一套可商用授权的素材）
- 国际化（中/英）、提交信息模板、`.gitignore` 模板库
- 多平台打包（Windows / Linux）；目前只在 macOS 上跑过
- 完整进度与 TODO 见 `PROCESS.md`
