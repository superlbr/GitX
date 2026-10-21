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
audit "再分发时必须知道截图不是我们的" ATTRIBUTION.md "docs/reference"
audit "包要声明双许可" Cargo.toml 'license = "MIT OR Apache-2.0"'

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
