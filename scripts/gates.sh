#!/usr/bin/env bash
# 四条门禁一次跑完。**改代码时不要跑它**：本仓库的惯例是改动阶段只跑受影响的那一条窄门禁
# （见 CONTRIBUTING.md），一批做完再跑这个。慢的那一步是 eframe 的链接。
set -uo pipefail
cd "$(dirname "$0")/.."
failed=0

step() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }
run() { # run <标签> <命令...>
  local label=$1
  shift
  step "$label"
  if ! "$@"; then
    printf '\033[31m✗ %s\033[0m\n' "$label"
    failed=1
  fi
}

run "核心层（lib 单测）" cargo test --lib
run "egui 无头门禁" cargo test --bin gitx

# 界面层单独再查一次警告：`cargo test` 的输出里混着测试代码的诊断，
# 「产品代码零警告」要按 `cargo build` 的输出来判。（`cargo build` 无事可做时也会打一行
# "Finished"，所以只认以 warning/error 开头的那些行。）
step "零警告构建"
diagnostics=$(cargo build 2>&1 | grep -E "^(warning|error)" || true)
if [ -n "$diagnostics" ]; then
  printf '%s\n' "$diagnostics"
  printf '\033[31m✗ 构建有警告\033[0m\n'
  failed=1
fi

printf '\n'

# 第四条：许可证与素材审计。它读的是一堆文本文件，不编译任何东西，所以放在最后（也最便宜）。
# 实现只在 scripts/license-audit.sh 里有一份，CI 调的是同一个文件。
step "许可证与素材审计"
if ! ./scripts/license-audit.sh; then
  failed=1
fi

printf '\n'
if [ "$failed" -ne 0 ]; then
  printf '\033[31m门禁没过（上面每一段都要么绿、要么指出问题）\033[0m\n'
  exit 1
fi
printf '\033[32m四条门禁全绿\033[0m\n'
