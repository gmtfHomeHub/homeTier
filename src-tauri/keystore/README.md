# Keystore 目录

此目录用于存放 Android 发布签名密钥库。

## 生成发布密钥库

```bash
keytool -genkey -v -keystore release.keystore -alias hometier -keyalg RSA -keysize 2048 -validity 10000
```

生成后将文件重命名为 `release.keystore` 并放入此目录。

## CI/CD 配置

在 GitHub Secrets 中配置以下密钥：

| Secret 名称 | 说明 |
|-------------|------|
| `KEYSTORE_BASE64` | `base64 -w 0 release.keystore` 的输出 |
| `KEYSTORE_PASSWORD` | keystore 密码 |
| `KEY_PASSWORD` | key 密码 |

工作流会在构建时自动解码并配置签名。