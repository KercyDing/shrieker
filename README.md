# Shrieker

Shrieker 是一个基于 [sculk](https://github.com/KercyDing/sculk) P2P 隧道库的 Minecraft Java 版联机工具，界面使用 [egui](https://github.com/emilk/egui) 构建。

无需公网 IP 或路由器端口转发。房主创建房间并分享链接，其他玩家粘贴链接后即可加入。

## 界面预览

<div style="display: flex; gap: 40px;">
  <img height="360" alt="创建房间" src="https://github.com/user-attachments/assets/47ca19d7-39ae-4b15-9d8f-1d1c07c0f685" />
  <img height="360" alt="加入房间" src="https://github.com/user-attachments/assets/818edaa8-8174-4dca-bc06-c5a90f7cd34a" />
</div>

## 功能

- **创建房间**：自动寻找已“对局域网开放”的 Minecraft Java 版世界，也支持手动填写服务端端口
- **分享链接**：生成带访问令牌的 `sculk://join/v1/...` 链接，并自动复制到剪贴板
- **链接刷新**：支持每次创建时刷新、永久复用、定时刷新和手动刷新
- **加入房间**：粘贴分享链接后自动建立连接，并在 Minecraft 多人游戏页面广播房间
- **连接状态**：显示在线玩家、连接方式、延迟和流量等信息
- **配置保存**：自动保存身份、链接、端口和应用偏好

链接刷新后，旧链接无法建立新的连接，但不会断开已经加入的玩家。

## 安装

| 系统      | 架构            | 下载最新版                                                                                                                                                                                                       |
| ------- | ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows | x86_64        | [安装程序（.exe）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-windows-amd64-setup.exe)                                                                                               |
| Windows | ARM64         | [安装程序（.exe）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-windows-arm64-setup.exe)                                                                                               |
| macOS   | Intel         | [磁盘映像（.dmg）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-darwin-amd64.dmg)                                                                                                      |
| macOS   | Apple Silicon | [磁盘映像（.dmg）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-darwin-arm64.dmg)                                                                                                      |
| Linux   | x86_64        | [软件包（.deb）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-linux-amd64.deb) / [软件包（.rpm）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-linux-amd64.rpm) |
| Linux   | ARM64         | [软件包（.deb）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-linux-arm64.deb) / [软件包（.rpm）](https://github.com/KercyDing/shrieker/releases/latest/download/shrieker-linux-arm64.rpm) |

全部版本和更新说明可在 [Releases](https://github.com/KercyDing/shrieker/releases) 页面查看。

### Windows

下载对应架构的安装程序，运行后按照安装向导完成安装。

大多数 Windows 电脑使用 `x86_64`；搭载 ARM 处理器的设备使用 ARM64 版本。

### MacOS

Apple Silicon 设备下载 `shrieker-darwin-arm64.dmg`，Intel 设备下载 `shrieker-darwin-amd64.dmg`。

打开 `.dmg` 文件后，将 `shrieker.app` 拖入 `Applications` 文件夹。

当前发布的应用未进行 Apple 签名和公证，MacOS 可能提示应用已损坏。此时可在终端执行：

```sh
xattr -dr com.apple.quarantine /Applications/shrieker.app
```

若仍被拦截，可前往“系统设置 → 隐私与安全性”选择“仍要打开”，或右键应用后选择“打开”。

> 买不起 Apple 开发者账号 🥹

### Arch Linux

```sh
paru -S shrieker-bin
```

或：

```sh
yay -S shrieker-bin
```

## 使用方法

### 创建房间

1. 进入 Minecraft Java 版单人世界，并选择“对局域网开放”
2. 打开 Shrieker 的“创建”页面，等待自动识别端口
3. 设置最大人数和分享链接有效期
4. 点击“创建”，将自动复制的链接发送给其他玩家

如果自动识别失败，可切换到手动端口模式，填写 Minecraft 聊天栏中显示的端口。

### 加入房间

1. 在 Shrieker 的“加入”页面粘贴分享链接
2. 本地端口保持自动选择，点击“加入”
3. 连接成功后，在 Minecraft 多人游戏页面选择 `shrieker` 房间

如果房间没有出现在列表中，可使用界面显示的 Minecraft 地址手动连接。

## Linux 防火墙

自动寻找 Minecraft LAN 世界需要接收发送到 `224.0.2.60:4445` 的局域网公告。

启用 UFW 后，如果界面一直显示“等待世界开放到局域网”，可先查看当前网络接口：

```sh
ip route get 224.0.2.60
```

假设输出中的接口名为 `wlan0`，执行：

```sh
sudo ufw allow in on wlan0 proto udp to 224.0.2.60 port 4445
```

接口名需按实际输出替换。这条规则只放行 Minecraft LAN 公告，不会开放 Minecraft TCP 服务端口。

也可以切换到手动端口模式，跳过自动搜寻。

## 常见问题

### Q: 为什么必须让房主保持 Shrieker 运行？

Shrieker 负责在房主和其他玩家之间转发 Minecraft 网络连接。

关闭 Shrieker 后，隧道也会停止，其他玩家将无法继续通过 Shrieker 访问房间。

### Q: 应该选择自动端口还是固定端口？

大多数情况下选择“自动”即可。

当 Minecraft 无法发现局域网房间，并且希望使用固定的本地地址手动连接时，可以考虑采用固定端口。

### Q: 分享链接可以公开发布吗？

不建议。

分享链接中包含用于加入房间的访问令牌。任何获得有效链接的人都可能尝试连接房间，因此请只将链接发送给你信任的玩家。

如果链接被意外公开，可以在 Shrieker 中立即刷新链接。刷新后，旧链接将不能建立新的连接。

### Q: 为什么刷新链接后，已经加入的玩家没有断开？

这是正常现象。

刷新链接只会阻止旧链接继续建立新连接，不会主动断开已经建立的会话。

需要让已加入的玩家断开时，可以停止当前房间或关闭 Shrieker。

### Q: 为什么 Minecraft 多人游戏列表里没有出现房间？

可以先等待几秒，然后重新进入多人游戏页面。

如果仍然没有出现，请直接使用 Shrieker 界面中显示的 Minecraft 地址进行连接。

Minecraft 的局域网发现机制偶尔可能无法显示 Shrieker 发送的本机公告；这不代表隧道连接失败，通常使用地址手动连接即可解决。

### Q: 什么是中继服务器？

Shrieker 会优先尝试让两台电脑直接建立 P2P 连接。

如果双方的网络环境无法直接连接，流量会通过中继服务器转发。中继可以提高连接成功率，但延迟和速度可能受到中继服务器位置与负载的影响。

不清楚如何选择时，保持默认设置即可。

中继具体搭建详见 [iroh-relay](https://github.com/KercyDing/iroh-relay)。

### Q: 支持基岩版吗？

目前面向 Minecraft Java 版设计，不支持 Minecraft 基岩版。

## 许可证

[GPL-3.0](LICENSE)
