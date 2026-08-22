#!/bin/bash
# 下载 Npcap SDK 并提取 Windows 运行时所需的 DLL 文件
# 用于解决 Windows 平台 "packet.dll 缺失" 问题

set -euo pipefail

NPCAP_VERSION="1.80"
NPCAP_URL="https://nmap.org/npcap/dist/npcap-${NPCAP_VERSION}.exe"
DEST_DIR="src-tauri/resources/bin"
TEMP_DIR=$(mktemp -d)

echo "=== 下载 Npcap SDK ==="
echo "URL: ${NPCAP_URL}"

# 创建目标目录
mkdir -p "${DEST_DIR}"

# 下载 Npcap 安装包
cd "${TEMP_DIR}"
wget -q --show-progress "${NPCAP_URL}" -O npcap.exe

# 使用 7z 解压（需要安装 p7zip-full）
echo "=== 解压 Npcap 安装包 ==="
7z x npcap.exe -o./npcap_extract > /dev/null

# 查找并复制所需的 DLL 文件
echo "=== 查找并复制 DLL 文件 ==="

# 定义需要的 DLL 文件
DLL_FILES=(
    "packet.dll"
    "wpcap.dll"
    "Packet.dll"
    "Wpcap.dll"
)

for dll in "${DLL_FILES[@]}"; do
    # 在解压目录中查找文件
    found=$(find "${TEMP_DIR}/npcap_extract" -name "${dll}" -type f 2>/dev/null | head -1)
    if [ -n "${found}" ]; then
        cp "${found}" "src-tauri/resources/bin/${dll}"
        echo "已复制: ${dll}"
    else
        echo "警告: 未找到 ${dll}"
    fi
done

# 验证复制结果
echo "=== 验证复制结果 ==="
for dll in "${DLL_FILES[@]}"; do
    if [ -f "src-tauri/resources/bin/${dll}" ]; then
        echo "✓ ${dll} - 已就绪"
        file "src-tauri/resources/bin/${dll}"
    else
        echo "✗ ${dll} - 缺失"
    fi
done

# 清理临时文件
rm -rf "${TEMP_DIR}"

echo "=== 完成 ==="
echo "Windows 所需的 DLL 文件已准备就绪在 src-tauri/resources/bin/"