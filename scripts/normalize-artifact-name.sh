#!/usr/bin/env bash
# 把 dist/ 里唯一的目标扩展名产物重命名为 {productName}-{os}-{arch}-{version}{suffix}.{ext}
#
# 命名规范（对齐上游 EasyTier 的 easytier-{os}-{arch}-{version}.zip）：
#   顺序    = 应用包名 → 操作系统 → 架构 → 版本号
#   桌面    = Rust target 前缀（x86_64 / aarch64），与 matrix artifact 名保持一致
#   Android = ABI 原名（arm64-v8a / armeabi-v7a / x86_64），用户需按 ABI 选包
#   -debug  = 仅当 inputs.debug_mode=true 时追加，避免 debug 产物伪装成正式版
#
# 版本号必须在 tauri build（含代码签名）之后读取来源并注入文件名。MSI/EXE 的 Windows
# 代码签名写在内嵌 Win32 证书表，签的是文件内容而非文件名，所以构建后改名安全；
# 若在签名前改名则失效。
#
# 用法: normalize-artifact-name.sh <dist目录> <扩展名> <操作系统token> <架构token> [后缀]
set -euo pipefail

DIST_DIR="${1:?用法: normalize-artifact-name.sh <dist目录> <扩展名> <操作系统token> <架构token> [后缀]}"
EXT="${2:?缺少扩展名，如 msi / dmg / deb / AppImage / apk}"
OS="${3:?缺少操作系统 token，如 linux / macos / windows}"
ARCH="${4:?缺少架构 token，如 x86_64 / aarch64 / arm64-v8a}"
SUFFIX="${5:-}"

CONF="${TAURI_CONF:-src-tauri/tauri.conf.json}"
APP_NAME="$(node -p "require('./$CONF').productName")"
VERSION="$(node -p "require('./$CONF').version")"

shopt -s nullglob
files=("$DIST_DIR"/*."$EXT")

# 数量断言：Stage 用宽松 find glob 收集，若 target/ 残留旧产物会误收集，这里直接失败
if [ "${#files[@]}" -eq 0 ]; then
  echo "ERROR: $DIST_DIR 下无 .$EXT 产物" >&2
  exit 1
fi
if [ "${#files[@]}" -gt 1 ]; then
  echo "ERROR: $DIST_DIR 下有 ${#files[@]} 个 .$EXT 产物（期望 1 个），拒绝改名:" >&2
  ls -lh "${files[@]}" >&2
  exit 1
fi

OUT="$DIST_DIR/${APP_NAME}-${OS}-${ARCH}-${VERSION}${SUFFIX}.${EXT}"
if [ "${files[0]}" = "$OUT" ]; then
  echo "[rename] 已符合规范: $(basename "$OUT")"
else
  mv -f "${files[0]}" "$OUT"
  echo "[rename] $(basename "${files[0]}") -> $(basename "$OUT")"
fi
