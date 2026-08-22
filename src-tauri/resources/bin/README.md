# Windows 运行时依赖 DLL 文件

此目录包含 Windows 平台运行所需的 WinPcap/Npcap 动态链接库文件。

## 所需文件

| 文件名 | 说明 | 来源 |
|--------|------|------|
| `packet.dll` | WinPcap 核心库 | Npcap SDK |
| `wpcap.dll` | WinPcap 核心库 | Npcap SDK |
| `Packet.dll` | WinPcap 核心库（大小写兼容） | Npcap SDK |
| `Wpcap.dll` | WinPcap 核心库（大小写兼容） | Npcap SDK |

## 获取方法

### 方法 1: 从 Npcap 官网下载（推荐）

1. 访问 [Npcap 官网](https://nmap.org/npcap/)
2. 下载最新版本的 Npcap SDK (`npcap-<version>.exe`)
3. 使用 7-Zip 解压安装包：
   ```bash
   7z x npcap-<version>.exe
   ```
4. 从解压后的文件中复制以下 DLL 到此目录：
   - `packet.dll`
   - `wpcap.dll`
   - `Packet.dll` (如存在)
   - `Wpcap.dll` (如存在)

### 方法 2: 从已安装 Npcap 的系统复制

如果开发机已安装 Npcap，可直接复制：
```bash
# Windows 系统目录通常位置
copy C:\Windows\System32\packet.dll src-tauri\resources\bin\
copy C:\Windows\System32\wpcap.dll src-tauri\resources\bin\
copy C:\Windows\System32\Packet.dll src-tauri\resources\bin\ 2>nul || echo "Packet.dll 不存在"
copy C:\Windows\System32\Wpcap.dll src-tauri\resources\bin\ 2>nul || echo "Wpcap.dll 不存在"
```

## CI/CD 自动化

GitHub Actions 工作流中可通过以下步骤自动下载和提取：

```yaml
- name: Download Npcap DLLs
  run: |
    wget https://nmap.org/npcap/dist/npcap-1.80.exe -O npcap.exe
    7z x npcap.exe -o./npcap_extract
    cp npcap_extract/packet.dll src-tauri/resources/bin/
    cp npcap_extract/wpcap.dll src-tauri/resources/bin/
    cp npcap_extract/Packet.dll src-tauri/resources/bin/ 2>/dev/null || true
    cp npcap_extract/Wpcap.dll src-tauri/resources/bin/ 2>/dev/null || true
```

## 注意事项

1. **版本兼容性**: 建议使用 Npcap 1.x 版本，确保与 `pnet` crate 兼容
2. **架构匹配**: 确保下载的 DLL 与目标架构匹配（x64 用于 x86_64 目标）
3. **版本控制**: 实际的 DLL 文件不应提交到 Git，应在 CI/CD 中动态获取
   - 当前目录下的 `.gitkeep` 和占位文件仅用于目录结构保留
   - 实际构建时请确保替换为真实的 DLL 文件

## 验证

构建后可通过以下方式验证 DLL 是否正确打包：

```bash
# 检查 MSI 包内容
lessmsi x homeTier-x86_64.msi output_dir
ls output_dir/ | grep -E "packet|wpcap"
```