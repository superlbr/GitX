#!/usr/bin/env bash
# 许可证与素材审计。**只有一条实现**：`scripts/gates.sh` 和 CI 都调这个文件，
# 免得「本地绿、CI 绿」判的不是同一件事。
#
# 为什么要有这一步：这个仓库里同时躺着四种条款完全不同的东西——我们原创的代码、
# Atlassian 的截图（专有，不能替它授权）、一份 OFL 字体（要保留其声明）、
# 以及「GitCraft」这个名字本身（商标，不在 MIT/Apache 授权范围内）。
# 漏标一条，就等于替别人做了一个我们没资格做的承诺。
set -uo pipefail
cd "$(dirname "$0")/.."
failed=0

audit() { # audit <判据描述> <文件> <必须出现的字串>
  if ! grep -qF -- "$3" "$2"; then
    printf '\033[31m✗ %s：%s 里找不到「%s」\033[0m\n' "$1" "$2" "$3"
    failed=1
  fi
}

for f in LICENSE-MIT LICENSE-APACHE NOTICE ATTRIBUTION.md; do
  [ -f "$f" ] || { printf '\033[31m✗ 缺少 %s\033[0m\n' "$f"; failed=1; }
done

audit "NOTICE 要同时点名两个许可证" NOTICE "MIT License"
audit "NOTICE 要同时点名两个许可证" NOTICE "Apache License, Version 2.0"
audit "NOTICE 要把商标划出授权范围" NOTICE "trademarks"
audit "要在 ATTRIBUTION 里说清「本机对照图不入库」" ATTRIBUTION.md "docs/reference"
audit "包要声明双许可" Cargo.toml 'license = "MIT OR Apache-2.0"'
audit "新增的原生面板依赖要在 ATTRIBUTION 里点名" ATTRIBUTION.md "rfd\` 0.17.2"

# 每一条直接依赖都要在 ATTRIBUTION 的那张表里有一行。为什么钉这一条而不是靠人记：
# `ureq` 那一批同时拖进一棵 TLS 子树，里面有 ISC、BSD-3-Clause、CDLA-Permissive-2.0 三种
# 条款，全都不是本仓库的 MIT/Apache——漏标一次就是替别人做了一个我们没资格做的承诺。
# 读 `[dependencies]`（含 `[dependencies.epaint]` 那种拆开写的），不去读 Cargo.lock：
# 子树由上面那行点名，逐条钉会把门禁变成锁死的清单，升级一次版本就得改一次脚本。
deps=$(awk '
  /^[[]dependencies[]]$/            {f=1; next}   # 只有这一行标题：块里的 `name = "1"` 才是依赖
  /^[[]dependencies\.[a-z0-9_-]+[]]$/ {sub(/^\[dependencies\./, ""); sub(/\].*/, ""); print; f=0; next}
  /^[[]/                             {f=0}          # 其它任何表（含 dev-dependencies）都不算
  f && /^[A-Za-z0-9_-]+[ \t]*=/      {sub(/[ \t]*=.*/, ""); print}
' Cargo.toml | sort -u)
for dep in $deps; do
  # 按「反引号 + 名字」找，认的就是那张表里的写法（`ureq` 2.12），也顺手挡住注释里顺口提到。
  if ! grep -qF -- "\`$dep" ATTRIBUTION.md; then
    printf '\033[31m✗ 直接依赖 %s 没在 ATTRIBUTION.md 里点名（引进依赖=引进条款，就在这里补一行）\033[0m\n' "$dep"
    failed=1
  fi
done

# `rfd` 的原生面板是**嵌套事件循环**，只在界面层的 `eframe::App::ui` 里跑。核心层一旦碰它，
# 就等于让「跑一次 git」附带弹一个系统窗口——无头判据也立刻测不到那一层了。
if grep -rl "rfd::" src/ 2>/dev/null | grep -v "^src/ui/" | grep -q .; then
  printf '\033[31m✗ rfd 越过了界面层（核心层不许碰原生面板）：%s\033[0m\n' "$(grep -rl 'rfd::' src/ | grep -v '^src/ui/')"
  failed=1
fi

# 第三方界面截图只作本机测量对照，不许被提交进仓库（.gitignore 已挡住，这里再兜一道）。
if [ -n "$(git ls-files -- docs/reference)" ]; then
  printf '\033[31m✗ docs/reference 下的第三方截图被提交进仓库了（见 .gitignore 与 ATTRIBUTION.md）\033[0m\n'
  failed=1
fi

# 字体不许混进仓库：8.3 MB，而且它带着 OFL 的署名义务，只该由 README 指路去取。
if git ls-files --error-unmatch assets/fonts/NotoSansSC-Regular.otf >/dev/null 2>&1; then
  printf '\033[31m✗ 那份 CJK 字体被提交进仓库了（见 .gitignore 与 README）\033[0m\n'
  failed=1
fi

# 反过来：字体不在仓库里，就必须留一份能证明它是何种授权的声明文件。
audit "缺字体时要能说出它的许可证" assets/fonts/OFL.txt "SIL Open Font License"

if [ "$failed" -ne 0 ]; then
  printf '\033[31m素材审计没过\033[0m\n'
  exit 1
fi
printf '\033[32m素材审计全绿\033[0m\n'
