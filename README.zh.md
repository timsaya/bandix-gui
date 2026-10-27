# Bandix GUI

[English](README.md) | 简体中文

<img src="assets/icons/bandix-128.png" width="96" height="96" alt="Bandix 图标">

[Bandix](https://github.com/timsaya/bandix) 的桌面客户端，使用 Rust 和 GPUI Kit 构建。连接路由器上的 Bandix 服务后，可以查看网络活动并管理设备策略。

流量采集、历史存储、限速和配额执行均由 Bandix 服务端完成。安装 GUI 不会安装服务端，也不需要在桌面系统上安装 LuCI。

## 界面预览

流量监控

![Bandix 流量页面：实时曲线与设备用量](doc/traffic.png)

服务连接与偏好设置

![Bandix 设置页面：服务连接、语言、主题与速度单位](doc/settings.png)

## 安装教程

### Linux

安装包支持 `x86_64` 和 `aarch64`，需要 glibc 2.35 或更高版本。

Debian / Ubuntu：

```sh
sudo apt install ./bandix-gui-0.1.0-linux-x86_64.deb
```

Fedora / RPM 系统：

```sh
sudo dnf install ./bandix-gui-0.1.0-linux-x86_64.rpm
```

使用便携版 AppImage：

```sh
chmod +x bandix-gui-0.1.0-linux-x86_64.AppImage
./bandix-gui-0.1.0-linux-x86_64.AppImage
```

### Windows

下载与设备架构对应的 ZIP（`x86_64` 或 `arm64`），解压后运行 `bandix-gui.exe`。

### macOS

下载与 Mac 架构对应的 DMG（`arm64`、`x86_64` 或 `universal`），打开后将 **Bandix GUI.app** 拖入 Applications。也可以下载包含应用程序的 ZIP。

### 首次连接

1. 在路由器的 LuCI Bandix 设置中，将监听 Host 设为 `0.0.0.0` 或路由器的局域网 IPv4 地址，然后保存并应用。
2. 打开 Bandix GUI。Linux 和 macOS 会尝试自动发现路由器；如未发现，请在设置中手动填写服务地址，例如 `http://192.168.1.1:8686`。
