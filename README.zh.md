# Bandix GUI

[English](README.md) | 简体中文

<img src="assets/icons/bandix-128.png" width="96" height="96" alt="Bandix 图标">

[Bandix](https://github.com/timsaya/bandix) 的桌面客户端，使用 Rust 和 GPUI Kit 构建。连接路由器上的 Bandix 服务，即可查看网络活动并管理设备策略。

GUI 是独立客户端。流量采集、历史存储、限速和配额执行均由 Bandix 服务端完成。安装 GUI 不会安装服务端，也不需要在桌面系统上安装 LuCI。

## 功能

- 实时上传、下载曲线，历史流量、设备用量和流量排行。
- 设备搜索、排序、名称修改，以及单个和批量删除。
- 定时上传、下载限速，全局默认限速及白名单。
- 合计配额或独立上传、下载配额，支持分钟、小时、日、周、月和累计额度。两种模式互斥，默认使用合计配额。
- 连接监控与筛选，显示服务端提供的 GeoIP 归属地信息。
- DNS 统计与查询记录，支持筛选和分页。
- Linux、macOS 默认网关自动发现，以及手动连接服务端。
- 浅色、深色及跟随系统主题，可调整刷新间隔和速度单位。
- 简体中文、英语、繁体中文、日语、法语、西班牙语、波兰语和俄语。

## 界面预览

流量监控

![Bandix 流量页面：实时曲线与设备用量](doc/traffic.png)

服务连接与偏好设置

![Bandix 设置页面：服务连接、语言、主题与速度单位](doc/settings.png)

## 安装与运行

### Linux

打包支持 AppImage、tar.gz、DEB 和 RPM，架构为 `x86_64` 和 `aarch64`。安装环境需要 **glibc 2.35 或更高版本**及可用的图形桌面。显卡驱动和部分桌面库由系统提供。

以下示例使用 `0.1.0` 版本和 `x86_64` 架构，请按实际下载的文件替换。

**Debian / Ubuntu：**

```sh
sudo apt install ./bandix-0.1.0-linux-x86_64.deb
```

**Fedora / 使用 DNF 的 RPM 系统：**

```sh
sudo dnf install ./bandix-0.1.0-linux-x86_64.rpm
```

安装后从应用菜单启动 Bandix，或运行 `bandix-gui`。

**AppImage：**

```sh
chmod +x bandix-0.1.0-linux-x86_64.AppImage
./bandix-0.1.0-linux-x86_64.AppImage
```

系统没有 FUSE 时，添加 `--appimage-extract-and-run`。若要复制到当前用户的应用目录，并注册菜单入口和图标：

```sh
./bandix-0.1.0-linux-x86_64.AppImage --appimage-extract-and-run --install
```

此可选安装方式需要 Python 3，无需 root，也支持用新版重复安装。tar.gz 包解压后，在解压目录运行 `./AppRun`；运行 `./AppRun --install` 可安装到当前用户目录。

依赖、校验和、安装位置及兼容性细节见 [Linux 打包说明](packaging/linux/README.md)。

### macOS

打包脚本可生成 Apple Silicon 和 Intel 的 `.app`、`.zip`、`.dmg`，支持分别打包或生成通用包。打开 DMG 后将 Bandix 拖入 Applications，或解压 ZIP 后将 `Bandix.app` 复制到该目录。


## 连接 Bandix

1. 在路由器的 LuCI Bandix 设置界面，将监听 Host 改为 `0.0.0.0`，或路由器局域网网卡对应的 IPv4 地址（例如 `192.168.1.1`），保存并应用。这里填写 IP 地址，不是 `br-lan` 等网卡名称。
2. 启动 Bandix GUI 自动发现服务。也可以在 GUI 设置中填写路由器地址，例如 `http://192.168.1.1:8686`，端口以实际配置为准。


## 相关项目与许可证

- [Bandix 服务端](https://github.com/timsaya/bandix)
- [LuCI 应用](https://github.com/timsaya/luci-app-bandix)
- [OpenWrt 服务软件包](https://github.com/timsaya/openwrt-bandix)
- 维护者：[timsaya](https://github.com/timsaya)
