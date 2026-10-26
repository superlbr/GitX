# GitX

[![门禁](https://github.com/superlbr/GitX/actions/workflows/gates.yml/badge.svg)](https://github.com/superlbr/GitX/actions/workflows/gates.yml)
[![发布](https://github.com/superlbr/GitX/actions/workflows/release.yml/badge.svg)](https://github.com/superlbr/GitX/actions/workflows/release.yml)

当前版本 **0.0.1**（版本号只在 `Cargo.toml` 写一次：窗口标题、状态栏右下角、release 标签都读它）。
体验版安装包在 [Releases](https://github.com/superlbr/GitX/releases)：macOS 两个架构各一个 zip，
解压后把 `GitX.app` 拖进「应用程序」。包没有签名与公证，第一次打开要右键 → 打开。

用 Rust 写的原生 Git 桌面客户端：日志、提交图、暂存与差异、分支、标签、贮藏、远端操作都在一个窗口里。
界面用 egui/eframe 直接绘制，git 操作一律走官方 CLI。

设计上只押两件事：

- **快**：每帧要做的活只与窗口大小有关，与仓库大小无关（日志与差异按行虚拟化，提交图在取数据时排一次）。
- **稳**：git 行为与错误信息来自 git 本体；中文路径、GBK 老仓库、多行提交信息、超大补丁这些边角按实际事故处理。

## 功能

**仓库操作**

- status / log / diff / 区间 diff / stage / unstage / commit / amend / branch / rename / tag / stash /
  remote / fetch / pull / push / clone
- 文件溯源：`git log --follow` 历史、`git blame` 逐行归属、查看某版本全文、恢复到旧版本
- 冲突处理：合并 / 遴选 / 变基进行中时提示冲突，支持用「我的 / 他们的」解决、标记已解决 / 未解决、
  继续或终止
- 中文与编码：统一关闭 `core.quotepath`，输出按 UTF-8 宽松解码，常见 git 失败给出中文说明并保留原文
- 大仓库：日志分页读取、未推送集合一次算出、旧版本全文与未跟踪文件补丁有 2 MiB 上限
- 路径筛选按片段匹配（裸路径 + 文件名 glob + 目录 glob 三条 pathspec 的并集）
- 远程：上游跟踪、`--set-upstream`、拉取可走加速节点（推送刻意不走只读镜像）
- 代码托管平台插件：GitHub / Gitee / 阿里云 Codeup（按 remote 识别，能力按 JSON 声明）
- 更新检查、SSH 诊断、GitHub 快速预览地址；本地书签、设置与令牌存于 SQLite

**界面**

- 三栏布局：侧栏（书签、分支、标签、贮藏、远端）+ 提交日志（图表 / 提交号 / 描述 / 作者 / 日期）+
  暂存区与差异
- 提交图车道、按需懒加载、长列表虚拟化
- 暂存 / 取消暂存（含按块）、提交、修正上一条、放弃改动、忽略文件
- 提交 / 推送 / 拉取走模态卡；拉取卡可按远端刷新分支列表
- 多文件差异按 `diff --git` 文件头折叠；右上角搜索只留命中行并带上文件头 / hunk 上下文
- 分支 / 标签 / 贮藏 / 远端操作走右键菜单与模态卡
- 底部状态栏（分支、领先落后数、进行中的操作、仓库路径）与轻量 toast
- 中文界面，内置 Inter 字体，可选 CJK 字体

## 架构

单 crate 双 target：核心层编译为 lib，界面层编译为 bin，边界由编译单元保证。

```
gitx/
├── Cargo.toml              # [lib] 核心 + [[bin]] 壳
├── src/
│   ├── lib.rs              # 核心层入口：mod 声明 + 统一 re-export
│   ├── git.rs              # git CLI 封装与解析
│   ├── provider/           # 代码托管平台插件（github / gitee / codeup）
│   ├── preview.rs          # GitHub 快速预览地址
│   ├── ssh.rs              # ~/.ssh 清点、ssh_config 解析、试连措辞
│   ├── update.rs           # 更新检查：Release 解析 / 版本比较 / 挑安装包
│   ├── db.rs               # SQLite 持久化
│   ├── models.rs
│   ├── error.rs
│   ├── main.rs             # 界面入口：viewport + eframe::run_native
│   ├── geometry.rs         # 无头界面判据（bin 的测试模块）
│   └── ui/                 # 界面模块
│       ├── app.rs          #   面板布局、动作分发
│       ├── theme.rs        #   几何常量
│       ├── fonts.rs        #   字体装配
│       ├── state.rs        #   状态与帧间消息
│       ├── data.rs         #   快照与样式装配
│       ├── widgets.rs      #   绘制件
│       ├── graph.rs        #   提交图车道
│       ├── toast.rs        #   操作反馈
│       └── diffmodel.rs    #   差异渲染模型
├── assets/fonts/           # Inter（入库）+ CJK 字体声明（字体本体不入库）
├── assets/icon.png         # 应用图标（1024，由 gen-icon.mjs 生成；窗口图标与 .icns 共用这一张）
├── scripts/gates.sh        # 四项门禁
├── scripts/bundle-macos.sh # 打包 GitX.app（图标 + Info.plist + ad-hoc 签名）与 zip
├── .github/                # CI（门禁 / 发布）、issue 与 PR 模板
├── docs/
│   ├── PROCESS.md          # 开发台账：每批的决定、判据、反向变异与已知边界
│   └── process-archive/    # 早期历史记录
├── LICENSE-MIT / LICENSE-APACHE / NOTICE
├── ATTRIBUTION.md          # 第三方素材逐条登记
└── gen-icon.mjs            # 生成应用图标（零依赖）
```

核心层看不见界面（`crate::ui::*` 不在它的编译单元里），界面层通过 `gitx::` 调用核心。
本地状态（书签、设置）在 SQLite，路径 `~/Library/Application Support/com.gitx.app/gitx.db`。

## 开发运行

前置：Rust 1.96+、git 2.x、Xcode Command Line Tools（macOS）。

```bash
cargo run --bin gitx              # 启动
cargo test --lib                  # 核心层单测
cargo test --bin gitx             # 无头界面判据（不需要屏幕）
./scripts/gates.sh                # 四项门禁
node gen-icon.mjs                 # 重新生成 assets/icon.png
./scripts/bundle-macos.sh         # 打包 dist/GitX.app 与本机架构的 zip
```

改了核心层函数签名后请跑 `cargo check --tests`（默认 `cargo check` 不编译测试代码）。

### 发版

版本号只有一处来源：`Cargo.toml` 的 `version`。改完版本号，打一个同名标签推上去即可：

```bash
# Cargo.toml: version = "0.0.2"
git commit -am "bump version to 0.0.2"
git tag v0.0.2 && git push origin main --tags
```

`release.yml` 会先核对标签与 `Cargo.toml`（不一致直接停），再在 macOS runner 上
按 arm64 / x86_64 各打一个 zip，挂到 GitHub Release 上。

### 中文字体

CJK 字体（Noto Sans SC，约 8.3 MB）不入库，`.gitignore` 挡住 `assets/fonts/*.otf`，仓库只保留
`OFL.txt`。新克隆任选一种方式提供字体：

1. 把 `NotoSansSC-Regular.otf` 放到 `assets/fonts/`；
2. 用 `GITX_FONT_DIR` 指向已有字体目录；
3. 退路 `~/Library/Fonts/msyh.ttf`。

三者都没有时退回系统字体链，中文与全角标点会落到 `.notdef`（宽度为字号的一半）——这也是测试问题：
宽度类判据量的会是产品里不会出现的字体。拉丁一侧不依赖环境，Inter 三档字重入库并嵌入二进制。

## 测试

- **核心层**：在临时 fixture 仓库上跑真实 git，覆盖 status / stage / commit / diff / range / branch /
  tag / stash / remote / 文件溯源 / 冲突与继续 / 编码与错误措辞 / 平台插件 / 更新检查 / SSH 诊断等。
- **界面层**：egui 无头运行，不需要屏幕。判据覆盖几何、行距、虚拟化、提交图，以及真的点击与键入——
  造临时仓库、打提交信息、按提交按钮，再回仓库断言 `git log`。
- **四项门禁**：`cargo test --lib`、`cargo test --bin gitx`、零警告 `cargo build`、
  `./scripts/license-audit.sh`，由 `./scripts/gates.sh` 一次跑完；CI 每次 push / PR 都会跑，
  素材审计在 ubuntu、三条 Rust 门禁在 macos-14，两条 job 并行。

## 贡献

先读 [CONTRIBUTING.md](CONTRIBUTING.md)：两层职责、三条硬规矩（修复必须配一条会红的判据、每条判据都要
做一次反向变异、没有判据守着的契约要如实登记），以及门禁与“好的第一 PR”列表。
开发台账和已知边界在 [docs/PROCESS.md](docs/PROCESS.md)。

## 许可证

代码按 **MIT 或 Apache-2.0** 二选一发布，见 [LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)
与 [NOTICE](NOTICE)。「GitX」这个名字与图标是商标，不在授权范围内；分叉请改名换图标。
第三方素材与各自条款登记在 [ATTRIBUTION.md](ATTRIBUTION.md)。

## 路线图

- 设置页其余分页、`.gitignore` 编辑器、交互式 rebase
- 多选（⌘/Shift 范围）与按行暂存
- 可重绑的键盘操作与命令面板
- 拖放打开仓库、扫描文件夹、原生目录选择器
- 深色模式、语法高亮、国际化（中 / 英）
- 真网络远端上的推送 / 拉取判据；Git LFS 的推拉
- Windows / Linux 构建与打包（目前只在 macOS 上验证）
