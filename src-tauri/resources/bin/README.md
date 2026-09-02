# Windows 运行时依赖 DLL 文件

此目录包含 Windows 平台运行所需的 Npcap 与 Wintun 动态链接库文件（x64 运行时 DLL）。

## 所需文件

| 文件名 | 说明 | 当前状态 |
|--------|------|---------|
| `packet.dll` | Npcap 核心驱动库 `Packet_x64.dll`（221,056 字节） | ✅ 已提交 |
| `wpcap.dll` | Npcap 抓包库 `wpcap_x64.dll`（488,320 字节） | ✅ 已提交 |
| `wintun.dll` | Wintun 虚拟网卡驱动库（427,552 字节） | ✅ 已提交 |
| `WinDivert64.sys` | WinDivert 网络分流驱动（94,144 字节） | ✅ 已提交 |

> 两份 DLL 均为从 Npcap 官方安装器 `npcap-1.80.exe` 中提取的 **x64** 运行时库（MZ PE 头，有效）。
> 注意：安装器根目录的小写 `Packet.dll`/`wpcap.dll`（约 30–40 KB）是 **32 位**版本，不要误用；
> x64 版本的文件名带 `_x64` 后缀，复制时需重命名为小写 `packet.dll`/`wpcap.dll`。
>
> `wintun.dll` 与 `WinDivert64.sys` 来自 EasyTier 官方发布包（`easytier-windows-x86_64-v2.6.4.zip`），
> easytier-core.exe 在 Windows 上创建虚拟网卡（TUN）需要 wintun.dll 在其同目录，否则 `tun::create()` 失败导致
> `has_virtual_ip=false`。WinDivert64.sys 用于部分流量分流特性。

## 为什么直接提交，而不是 CI 下载

- **Npcap SDK zip（如 `npcap-sdk-1.16.zip`）只包含头文件和 `.lib` 导入库，不包含运行时 `packet.dll`/`wpcap.dll`。**
  运行时 DLL 只存在于 Npcap 安装器（`npcap-<version>.exe`）中（或已安装 Npcap 的系统 `C:\Windows\System32\Npcap\`）。
- 因此在 CI 中从 SDK 下载必然失败（`REAL packet.dll not found in Npcap SDK`），无法可靠取到真实 DLL。
- 改为直接提交两份真实 x64 DLL，彻底消除对 npcap.com 的网络依赖，MSI 打包时经
  `tauri.conf.json` 的 `resources/bin/*` 直接内嵌，运行时由
  `src-tauri/src/easytier/downloader.rs` 复制到 easytier-core.exe 目录。

## 更新 DLL（需要升级 Npcap 版本时）

```bash
# 1. 下载官方安装器
wget https://npcap.com/dist/npcap-1.80.exe -O /tmp/npcap.exe
# 2. 7z 解压（NSIS 自解压包）
7z x /tmp/npcap.exe -o/tmp/npcap_extract
# 3. 复制 x64 版本并重命名为小写
cp /tmp/npcap_extract/Packet_x64.dll src-tauri/resources/bin/packet.dll
cp /tmp/npcap_extract/wpcap_x64.dll src-tauri/resources/bin/wpcap.dll
# 4. 更新本文件中的体积/来源说明，并提交
```

## CI/CD 处理

GitHub Actions 中**不下载** DLL，仅校验提交的 DLL 有效（防止误回退为占位文件）：

```yaml
- name: Verify Npcap runtime DLLs are bundled
  shell: pwsh
  run: |
    $dlls = @("packet.dll", "wpcap.dll")
    foreach ($dll in $dlls) {
      $len = (Get-Item "$env:GITHUB_WORKSPACE/src-tauri/resources/bin/$dll").Length
      if ($len -lt 10000) { Write-Error "$dll is NOT a valid DLL (size=$len)"; exit 1 }
    }
```

## 注意事项

1. **版本兼容性**: 建议使用 Npcap 1.x 版本，确保与 `pnet` crate 兼容
2. **架构匹配**: 当前 Windows 构建目标为 x86_64，两份 DLL 均为 x64；若未来增加 32 位目标需补充 x86 版本
3. **运行期兜底**: `downloader.rs` 会跳过小于 10,000 字节的占位文件，避免把无效 DLL 复制到
   easytier-core.exe 目录导致其加载失败

## 验证

构建后可通过以下方式验证 DLL 是否正确打包：

```bash
# 检查 MSI 包内容
lessmsi x homeTier-x86_64.msi output_dir
ls output_dir/ | grep -E "packet|wpcap"
```