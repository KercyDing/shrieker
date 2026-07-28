# Shrieker

基于 [sculk](https://github.com/KercyDing/sculk) P2P 隧道库的 GUI 客户端，使用 [egui](https://github.com/emilk/egui) 构建。

## 实机效果

<div style="display: flex; gap: 40px;">
  <img height="360" alt="host" src="https://github.com/user-attachments/assets/47ca19d7-39ae-4b15-9d8f-1d1c07c0f685" />
  <img height="360" alt="join" src="https://github.com/user-attachments/assets/818edaa8-8174-4dca-bc06-c5a90f7cd34a" />
</div>

## 功能

- **建房**：暴露本地 Minecraft 服务端，生成带访问令牌的 `sculk://join/v1/...` 分享链接
- **链接期限**：支持每次开房、永久复用或按 1 至 24 小时自动刷新，也可手动立即刷新
- **加入**：通过分享链接连接到房主，默认自动选择可用的本地端口，并在 Minecraft Java 版的多人游戏页面广播房间
- **中继配置**：支持默认中继或自建中继
- 跨重启的身份、分享链接状态和配置持久化

## 安装

由 CI 自动构建和发布，根据自己的平台选择对应并下载即可：

- 前往 [Releases](https://github.com/KercyDing/shrieker/releases) 下载对应系统的最新版本

### macOS

根据设备架构下载 `shrieker-darwin-arm64.dmg`（Apple Silicon）或
`shrieker-darwin-amd64.dmg`（Intel），打开后将 `shrieker.app` 拖入 `Applications`。

当前 Release 中的 `shrieker.app` 未进行 Apple 签名/公证，macOS 可能提示
“‘shrieker.app’已损坏，无法打开”。

> 作为一个学生买不起 Apple 开发者账号 🥹

遇到该提示时，在终端中执行：

```sh
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

1. **建房**：填写 MC 端口、可选最大人数和分享链接有效期 → 点击 **开始建房** → 自动复制链接 → 分享
2. **加入**：粘贴分享链接 → 点击 **加入** → 在 Minecraft Java 版多人游戏页面选择 `shrieker` 房间

默认由系统自动选择本地端口；需要固定端口时可关闭“自动”。如果房间未出现在 Minecraft 的局域网列表中，可以手动连接界面显示的本地地址。日志显示在底部面板，分享链接和 Minecraft 地址都可手动复制。

定时刷新只会阻止旧链接建立新连接，不会断开已经加入的玩家。刷新后的新链接会自动复制到剪贴板。

## 许可证

[GPL-3.0](LICENSE)
