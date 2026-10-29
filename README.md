# Bandix GUI

English | [简体中文](README.zh.md)

<img src="assets/icons/bandix-128.png" width="96" height="96" alt="Bandix icon">

A desktop client for [Bandix](https://github.com/timsaya/bandix), built with Rust and GPUI Kit. Connect it to the Bandix service on your router to monitor network activity and manage device policies.

Traffic collection, history storage, rate limiting and quota enforcement run on the Bandix server. Installing the GUI does not install the server or require LuCI on the desktop.

## Screenshots

Traffic dashboard

![Bandix traffic dashboard with real-time charts and device usage](doc/traffic.png)

Connection and display settings

![Bandix settings with server connection, language, theme and speed units](doc/settings.png)

## Installation

### Linux

Packages are available for `x86_64` and `aarch64` and require glibc 2.35 or later.

Debian / Ubuntu:

```sh
sudo apt install ./bandix-gui-0.1.0-linux-x86_64.deb
```

Fedora / RPM-based distributions:

```sh
sudo dnf install ./bandix-gui-0.1.0-linux-x86_64.rpm
```

For the portable AppImage:

```sh
chmod +x bandix-gui-0.1.0-linux-x86_64.AppImage
./bandix-gui-0.1.0-linux-x86_64.AppImage
```

### Windows

Download the ZIP matching your architecture (`x86_64` or `arm64`), extract it, and run `bandix-gui.exe`.

### macOS

Download the DMG matching your Mac (`arm64`, `x86_64` or `universal`), open it, and drag **Bandix GUI.app** into Applications. A ZIP containing the app is also available.

### First connection

1. In the router's LuCI Bandix settings, set the listening Host to `0.0.0.0` or the router's LAN IPv4 address, then save and apply.
2. Open Bandix GUI. Linux, macOS and Windows will try to discover the router automatically (on Windows the IPv4 default routes from every interface are probed, so LAN and VPN setups both work). If needed, enter the service address in Settings, for example `http://192.168.1.1:8686`.
