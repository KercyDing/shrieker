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

## 环境依赖

### Rust

需要 Rust 1.89+：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Linux 系统库

```sh
# Debian / Ubuntu
sudo apt-get install \
  libxkbcommon-dev \
  libwayland-dev \
  libgl1-mesa-dev \
  libglib2.0-dev

# Fedora / RHEL
sudo dnf install \
  libxkbcommon-devel \
  wayland-devel \
  mesa-libGL-devel \
  glib2-devel
```

## 构建

```sh
cargo build --release
```

Windows 产物为 `target/release/shrieker.exe`，可直接双击运行。

## 使用

1. **建房**：填写 MC 端口、可选密码和最大人数 → 点击 **Start Host** → 分享票据
2. **加入**：粘贴票据，填写本地端口和密码 → 点击 **Join**

日志显示在底部面板，票据可通过 **Copy to Clipboard** 按钮复制。

## 许可证

[GPL-3.0](LICENSE)
