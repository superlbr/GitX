---
name: 出问题
about: 界面做错、卡住、崩溃、和参考图不一致
labels: bug
---

<!--
先跑一遍 `./scripts/gates.sh`：三条门禁里有红的，那就是已知问题，别重复报，直接评论在那条上。
-->

**发生了什么**

**我期望的是**

**复现步骤**（越具体越好，特别是这三样）

- 仓库：私有可以只贴 `git log --graph --max-count=20 --pretty=%h%d%s` 的输出
- 提交数 / 工作区文件数 / 最大那份补丁的行数（虚拟化和补丁上限都吃这三个数）
- 界面：只有 egui 版（`cd gitcraft-app && cargo run`）。2026-10-20 起 React + Tauri 那版已删除，
  报「那版有、这版没有」请先看 README 的「壳里还缺的」

**环境**

- 系统版本：
- `git --version`：
- 大概什么时候的代码（commit 或日期）：

**截图**

贴参考图（`docs/reference/`）同一位置的对照更管用。
