# Shrieker

基于 [sculk](https://github.com/KercyDing/sculk) P2P 隧道库的 GUI 客户端，使用 [egui](https://github.com/emilk/egui) 构建。

## 实机效果

<div style="display: flex; gap: 10px;">
  <img width="32%" alt="CleanShot 2026-03-23 at 14 48 53@2x" src="https://github.com/user-attachments/assets/0df1eb2e-9c77-4edc-9c7b-5fceec860df3" />
  <img width="32%" alt="CleanShot 2026-03-23 at 14 52 26@2x" src="https://github.com/user-attachments/assets/16b07fb2-9a58-4516-884e-8f4ecd25c342" />
  <img width="32%" alt="CleanShot 2026-03-23 at 14 54 24@2x" src="https://github.com/user-attachments/assets/e61703a9-04a5-4deb-8f87-fc61d914849e" />
</div>

## 功能

- **建房**：暴露本地 Minecraft 服务端，生成可分享的 `sculk://...` 票据
- **加入**：通过票据连接到房主隧道，转发流量到本地端口
- **中继配置**：支持默认中继或自建中继
- 跨重启的配置与密钥持久化

## 安装

由 CI 自动构建和发布，根据自己的平台选择对应并下载即可：

- 前往 [Releases](https://github.com/KercyDing/shrieker/releases) 下载对应系统的最新版本

### macOS

当前 Release 中的 `shrieker.app` 未进行 Apple 签名/公证，macOS 可能提示“‘shrieker.app’已损坏，无法打开”。

> 作为一个学生买不起 Apple 开发者账号呜呜🥹

先解压下载的 zip，再在当前文件夹下打开终端，并执行：

```sh
mv ./shrieker.app /Applications/
xattr -dr com.apple.quarantine /Applications/shrieker.app
```

若仍被拦截，可在“系统设置 -> 隐私与安全性”中点击“仍要打开”，或右键应用后选择“打开”。

### Arch Linux

```sh
yay -S shrieker-bin
# 或者
paru -S shrieker-bin
```

## 使用

1. **建房**：填写 MC 端口、可选密码和可选最大人数 → 点击 **开始建房** → 自动复制票据 → 分享
2. **加入**：粘贴票据，填写本地端口和密码 → 点击 **加入**

日志显示在底部面板，票据也可通过 **复制到剪贴板** 按钮复制。

## 许可证

[GPL-3.0](LICENSE)
