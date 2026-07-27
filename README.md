# Shrieker

基于 [sculk](https://github.com/KercyDing/sculk) P2P 隧道库的 GUI 客户端，使用 [egui](https://github.com/emilk/egui) 构建。

## 实机效果

<div style="display: flex; gap: 10px;">
  <img width="32%" alt="host" src="https://github.com/user-attachments/assets/e6cb514f-91c0-44c4-b1e6-39d0da984588" />
  <img width="32%" alt="join" src="https://github.com/user-attachments/assets/04bcdf1c-2c78-45f0-9cbd-8446b8dc7cb8" />
  <img width="32%" alt="relay" src="https://github.com/user-attachments/assets/dd1ff804-d1b0-4185-a879-7995bc1d01af" />
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

1. **建房**：填写 MC 端口、可选最大人数和分享链接有效期 → 点击 **开始建房** → 自动复制链接 → 分享
2. **加入**：粘贴分享链接 → 点击 **加入** → 在 Minecraft Java 版多人游戏页面选择 `shrieker` 房间

默认由系统自动选择本地端口；需要固定端口时可关闭“自动”。如果房间未出现在 Minecraft 的局域网列表中，可以手动连接界面显示的本地地址。日志显示在底部面板，分享链接和 Minecraft 地址都可手动复制。

定时刷新只会阻止旧链接建立新连接，不会断开已经加入的玩家。刷新后的新链接会自动复制到剪贴板。

## 许可证

[GPL-3.0](LICENSE)
