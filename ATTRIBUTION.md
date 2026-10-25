# 第三方素材与许可证

本仓库的代码默认 **MIT OR Apache-2.0**（二选一，见 [`LICENSE-MIT`](LICENSE-MIT)、
[`LICENSE-APACHE`](LICENSE-APACHE)、[`NOTICE`](NOTICE)）。下面这些东西**不在**这个许可里，
各自按自己的条款走。这张表同时是「素材许可证审计」那条门禁的检查对象：
新引进一个字体、图标、图片或依赖，就在这里补一行，说明作者、来源与许可证。

## 不是我们的、也不能替我们授权出去的

| 素材 | 作者 / 归属 | 许可证 | 在仓库里的位置 |
| --- | --- | --- | --- |
| 界面截图（仅本机测量对照） | Atlassian | **专有**，不受本仓库许可证约束 | **不入库**：`.gitignore` 挡住 `docs/reference/` |
| 微软雅黑（`msyh.ttf`）——只在运行时作为退路去读 | Microsoft | **专有**，从不打包、从不入库 | 本机 `~/Library/Fonts/`，不在仓库里 |

那些截图**不随这个仓库分发**：`docs/reference/` 已在 `.gitignore` 里，公开仓库里没有任何 Atlassian
素材。开发时若想对照，请自己准备一份截图放进 `docs/reference/`（同样不入库）——`src/ui/theme.rs`
里的几何常量最初就是在这种对照图上量的。分叉或再分发本仓库不涉及这些截图。
「GitX」这个名字和 `gen-icon.mjs` 生成的那个图标是我们的，但它们是**商标**，同样不在
MIT/Apache 授权范围内（条款在 `NOTICE` 里）：分叉出去请改名、改图标。

## 是开源的、但保留自己条款的

| 素材 | 作者 | 许可证 | 说明 |
| --- | --- | --- | --- |
| `NotoSansSC-Regular.otf`（8.3 MB 全量 CJK） | Google / Adobe（Noto 项目） | SIL Open Font License 1.1 | **故意不入库**（根 `.gitignore` 挡住），授权声明在 [`assets/fonts/OFL.txt`](assets/fonts/OFL.txt)。新克隆按 README 取一份，或用 `GITX_FONT_DIR` 指过去 |
| `Inter-Regular/Medium/SemiBold.ttf`（各 ~400 KB） | Rasmus Andersson（Inter 项目） | SIL Open Font License 1.1 | **直接入库并嵌进二进制**（`src/ui/fonts.rs`），拉丁正文与真字重的来源，授权声明在 [`assets/fonts/OFL-Inter.txt`](assets/fonts/OFL-Inter.txt) |
| `eframe` / `egui` / `epaint` 0.37 | Emil Ernerfeldh 与贡献者 | MIT OR Apache-2.0 | 界面层本体，彩色 emoji 走 `color_fonts` feature |
| `serde` / `serde_json` / `thiserror` / `chrono` / `tempfile` | 各自作者 | MIT OR Apache-2.0 | 序列化、错误类型、时间戳、测试临时目录 |
| `rusqlite` 0.31 | Ryan Olson | MIT | 本地状态存储 |
| `rfd` 0.17.2（原生存盘/选目录面板） | 8pt Labs 与贡献者 | **MIT**（单许可，不是双许可） | 只在 `src/ui/` 里用，核心层不许碰；它的面板是**嵌套事件循环**，只能在 `eframe::App::ui` 里、egui 那一帧之外跑（见 `state::Dialog`） |
| SQLite（`rusqlite` 的 `bundled` feature 编进来的 C 源码） | SQLite 团队 | **公有领域** | 不需要署名，也不设限制 |

字体这条曾经踩过一个坑，值得留在这儿：egui 的默认字体集里没有 CJK，缺字体时汉字落到
`.notdef`，而它的字宽正好是字号的一半——于是所有按宽度/左缘下的无头判据量的是一个
**产品里永远不会出现的字体**。所以「装没装这份 OFL 字体」不是环境问题，是测试数据问题，
`CONTRIBUTING.md` 和 CI 里那两条警告说的都是这件事。

## 我们自己的东西

- `src/`、`scripts/`、`.github/`：全部为本仓库贡献者原创，
  按 MIT OR Apache-2.0 授权。
- `gen-icon.mjs` **这个程序**是代码（同许可）；它**画出来的那个图标**是商标，见上面。
  图标里没有第三方素材：不加载任何渲染依赖，图形是 SDF 自己算的。
- GitX 与上表所列第三方素材的厂商无关，不含其代码、素材或商标。界面布局的实现均为原创，
  本机对照图只用于测量尺寸，不随仓库分发。
