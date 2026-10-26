#!/usr/bin/env bash
# Build an AppImage and an extracted, runnable tar.gz from the same AppDir.
# Tool documentation: https://github.com/linuxdeploy/linuxdeploy
#                    https://github.com/AppImage/appimagetool
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
[[ "$(uname -s)" == Linux ]] || { echo 'Build this package on Linux.' >&2; exit 1; }
arch="$(uname -m)"
case "$arch" in x86_64|aarch64) ;; *) echo "Unsupported architecture: $arch" >&2; exit 1;; esac
for tool in cargo curl python3 desktop-file-validate readelf ldconfig tar sha256sum; do
    command -v "$tool" >/dev/null || { echo "Missing build tool: $tool" >&2; exit 1; }
done

version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
target_dir="$(realpath -m "$target_dir")"
out="$target_dir/dist"
tools="$target_dir/packaging-tools"
name="bandix-${version}-linux-${arch}"
appdir="$out/$name"
mkdir -p "$out" "$tools"

prepare_tool() {
    local tool_name="$1" url="$2" binary="$tools/$1.AppImage"
    if [[ ! -x "$binary" ]]; then
        curl -fL --retry 3 "$url" -o "$binary.part"
        chmod +x "$binary.part"
        mv "$binary.part" "$binary"
    fi
    if [[ ! -x "$tools/$tool_name/squashfs-root/AppRun" ]]; then
        mkdir -p "$tools/$tool_name"
        (cd "$tools/$tool_name" && "$binary" --appimage-extract > extract.log)
    fi
}
prepare_tool linuxdeploy "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${arch}.AppImage"
prepare_tool appimagetool "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${arch}.AppImage"

cargo build --release --locked
desktop-file-validate packaging/linux/bandix-gui.desktop
# Only replace the generated staging directory, never source assets.
rm -rf -- "$appdir"
mkdir -p "$appdir/usr/share/icons/hicolor" "$appdir/usr/share/applications"
install -m644 packaging/linux/bandix-gui.desktop "$appdir/usr/share/applications/"
for size in 32 48 64 128 256 512; do
    icon_dir="$appdir/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$icon_dir"
    install -m644 "assets/icons/bandix-${size}.png" "$icon_dir/bandix-gui.png"
done

# GPUI also loads these at runtime; ldd alone cannot discover them.
extra_libraries=()
for library in libfontconfig.so.1 libfreetype.so.6 libwayland-client.so.0 libwayland-cursor.so.0; do
    library_path="$(ldconfig -p | awk -v lib="$library" '$1 == lib && !found {print $NF; found=1}')"
    [[ -n "$library_path" ]] || { echo "Missing runtime library: $library" >&2; exit 1; }
    extra_libraries+=(--library "$library_path")
done
NO_STRIP=1 "$tools/linuxdeploy/squashfs-root/AppRun" \
    --appdir "$appdir" --executable "$target_dir/release/bandix-gui" \
    --desktop-file packaging/linux/bandix-gui.desktop \
    --icon-file assets/bandix.png --icon-filename bandix-gui \
    --custom-apprun packaging/linux/AppRun "${extra_libraries[@]}"
# Use the full-resolution icon for AppImage launchers and file managers.
ln -sfn usr/share/icons/hicolor/512x512/apps/bandix-gui.png "$appdir/bandix-gui.png"
ln -sfn bandix-gui.png "$appdir/.DirIcon"
install -m755 packaging/linux/install.sh "$appdir/install.sh"

required_glibc="$(find "$appdir/usr" -type f \( -name '*.so*' -o -name bandix-gui \) \
    -exec readelf --version-info {} + | sed -nE 's/.*Name: GLIBC_([0-9.]+).*/\1/p' | sort -Vu | tail -1)"
cat > "$appdir/README.txt" <<EOF
Bandix $version - Linux $arch

Run: ./AppRun
Optional per-user installation (requires Python 3): ./install.sh
The installer adds Bandix to the application menu without root access.

Built on: $(. /etc/os-release; echo "$PRETTY_NAME")
Required glibc: $required_glibc or later.
Both X11 and Wayland are supported. The host supplies graphics drivers,
Vulkan/EGL support and fonts. Connect to your Bandix service in Settings.
The complete extracted directory must stay together.
EOF
{
    printf 'Version: %s\nArchitecture: %s\nMinimum glibc: %s\n' "$version" "$arch" "$required_glibc"
    rustc --version
    git rev-parse HEAD
    sha256sum "$tools/linuxdeploy.AppImage" "$tools/appimagetool.AppImage"
} > "$appdir/BUILD-INFO.txt"

tar -czf "$out/$name.tar.gz" -C "$out" "$name"
ARCH="$arch" "$tools/appimagetool/squashfs-root/AppRun" --no-appstream \
    "$appdir" "$out/$name.AppImage"
chmod +x "$out/$name.AppImage"
(cd "$out" && sha256sum "$name.AppImage" "$name.tar.gz" > "$name.sha256")
printf '\nPackages saved to %s\n' "$out"
ls -lh "$out/$name.AppImage" "$out/$name.tar.gz" "$out/$name.sha256"
