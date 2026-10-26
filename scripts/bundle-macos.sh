#!/usr/bin/env bash
# 把 release 产物包成 macOS 的 `GitX.app`，并压成能直接分发的 zip。
#
# 用法：
#   ./scripts/bundle-macos.sh                  # 本机架构
#   ./scripts/bundle-macos.sh aarch64-apple-darwin
#   ./scripts/bundle-macos.sh x86_64-apple-darwin
#
# 图标只有一份来源：`assets/icon.png`（窗口图标由 `src/ui/icon.rs` 直接嵌进二进制，
# 这里再把它切成 `.icns` 给 Dock / Finder 用），版本号也只有一份：`Cargo.toml`。
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=${1:-}
if [ -n "$TARGET" ]; then
  cargo build --release --locked --target "$TARGET"
  BIN="target/$TARGET/release/gitx"
  ARCH=${TARGET%%-*}
else
  cargo build --release --locked
  BIN="target/release/gitx"
  ARCH=$(uname -m)
fi

VERSION=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
APP="dist/GitX.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# icns：iconutil 只吃 .iconset 目录，10 档尺寸必须齐全（16..512 各带 @2x）。
ICONSET="dist/AppIcon.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" assets/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  double=$((s * 2))
  sips -z "$double" "$double" assets/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

cp "$BIN" "$APP/Contents/MacOS/gitx"
chmod +x "$APP/Contents/MacOS/gitx"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>GitX</string>
  <key>CFBundleDisplayName</key><string>GitX</string>
  <key>CFBundleIdentifier</key><string>com.superlbr.gitx</string>
  <key>CFBundleExecutable</key><string>gitx</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT OR Apache-2.0</string>
</dict>
</plist>
PLIST

# 本地临时签名（ad-hoc）：不签的话，同机打开也会被 Gatekeeper 判成「已损坏」。
# 正式分发要 Developer ID + 公证，那是另一件事，release 里会写明这一点。
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true

ZIP="dist/GitX-${VERSION}-${ARCH}.zip"
rm -f "$ZIP"
# ditto 保住资源分叉与权限；zip 命令会丢掉 .icns 的某些元数据。
ditto -c -k --keepParent "$APP" "$ZIP"

printf '打包完成：%s（版本 %s，架构 %s）\n' "$APP" "$VERSION" "$ARCH"
printf '分发包：%s\n' "$ZIP"
