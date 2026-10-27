#!/usr/bin/env bash
# Build AppImage/tar.gz bundles and native DEB/RPM desktop packages.
# Tool documentation: https://github.com/linuxdeploy/linuxdeploy
#                    https://github.com/AppImage/appimagetool
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

format=all
output_dir=""
native=false
glibc_baseline=2.35
while (($#)); do
    case "$1" in
        --format|--output-dir)
            (($# >= 2)) || { echo "Missing value for $1" >&2; exit 2; }
            case "$1" in
                --format) format="$2" ;;
                --output-dir) output_dir="$2" ;;
            esac
            shift 2
            ;;
        --native)
            native=true
            shift
            ;;
        -h|--help)
            cat <<'EOF'
Usage: ./scripts/package-linux.sh [--format all|appimage|tar.gz|deb|rpm] [--output-dir DIR] [--native]

Build Linux x86_64 or aarch64 packages with a glibc 2.35 release baseline.
By default, compile and package in Ubuntu 22.04 using Podman or Docker.
--native runs directly in the current environment but still enforces the ABI limit.
Relative output paths are resolved from the repository root.
Output defaults to $CARGO_TARGET_DIR/dist, or target/dist.
Default format: all (AppImage, tar.gz, DEB and RPM).
Each build includes a build-info JSON file and SHA-256 checksums.
DEB/RPM install /usr/bin/bandix-gui, menu entries and icons; libraries are system-managed.
Build separately on each architecture; cross-packaging is not supported.
BANDIX_CONTAINER_ENGINE may select podman or docker.
CARGO_BUILD_JOBS defaults to 4 in the container to limit memory usage.
EOF
            exit 0
            ;;
        *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
    esac
done
case "$format" in all|appimage|tar.gz|deb|rpm) ;; *) echo "Unsupported format: $format" >&2; exit 2 ;; esac
[[ "$(uname -s)" == Linux ]] || { echo 'Build this package on Linux.' >&2; exit 1; }
arch="$(uname -m)"
case "$arch" in x86_64|aarch64) ;; *) echo "Unsupported architecture: $arch" >&2; exit 1;; esac
target_dir="$(realpath -m "${CARGO_TARGET_DIR:-$root/target}")"
out="$(realpath -m "${output_dir:-$target_dir/dist}")"
if ! "$native"; then
    engine="${BANDIX_CONTAINER_ENGINE:-}"
    if [[ -z "$engine" ]]; then
        if command -v podman >/dev/null; then engine=podman
        elif command -v docker >/dev/null; then engine=docker
        else echo 'Install Podman or Docker to build glibc 2.35 packages.' >&2; exit 1
        fi
    fi
    command -v "$engine" >/dev/null || { echo "Missing container engine: $engine" >&2; exit 1; }
    command -v rustup >/dev/null || { echo 'Install the repository Rust toolchain with rustup first.' >&2; exit 1; }
    rust_toolchain="$(dirname "$(dirname "$(rustup which rustc)")")"
    cache="$target_dir/linux-glibc-$glibc_baseline/$arch"
    mkdir -p "$out" "$cache/build" "$cache/cargo"
    image="localhost/bandix-gui-builder:glibc-$glibc_baseline"
    "$engine" build -t "$image" -f packaging/linux/Containerfile packaging/linux
    user_options=(--user "$(id -u):$(id -g)")
    [[ "$(basename "$engine")" != podman ]] || user_options+=(--userns=keep-id)
    "$engine" run --rm "${user_options[@]}" --security-opt label=disable \
        -v "$root:/workspace:ro" -v "$rust_toolchain:/rust-toolchain:ro" \
        -v "$cache/build:/build:rw" -v "$cache/cargo:/cargo:rw" -v "$out:/packages:rw" \
        -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
        -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/workspace \
        "$image" bash scripts/package-linux.sh --native --format "$format" --output-dir /packages
    printf '\nPackages on host: %s\n' "$out"
    exit 0
fi

for tool in cargo rustc curl python3 desktop-file-validate readelf ldconfig tar sha256sum realpath install find sort sed awk git; do
    command -v "$tool" >/dev/null || { echo "Missing build tool: $tool" >&2; exit 1; }
done
native_tools=()
if [[ "$format" == all || "$format" == deb ]]; then native_tools+=(dpkg-deb dpkg-shlibdeps strip); fi
if [[ "$format" == all || "$format" == rpm ]]; then native_tools+=(rpmbuild strip); fi
for tool in "${native_tools[@]}"; do
    command -v "$tool" >/dev/null || { echo "Missing native packaging tool: $tool" >&2; exit 1; }
done
version="$(python3 - <<'PY_VERSION'
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
with open('Cargo.toml', 'rb') as source:
    print(tomllib.load(source)['package']['version'])
PY_VERSION
)"
tools="$target_dir/packaging-tools"
name="bandix-${version}-linux-${arch}"
mkdir -p "$out" "$tools"
# Build in a temporary directory: failed builds leave previous packages intact.
work="$(mktemp -d "$out/.bandix-package.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT
appdir="$work/$name"

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
if [[ "$format" == all || "$format" == appimage ]]; then
    prepare_tool appimagetool "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${arch}.AppImage"
fi

# Ask Cargo for the actual executable path (also handles Cargo target config).
cargo build --release --locked --message-format=json-render-diagnostics > "$work/cargo.jsonl"
binary="$(python3 - "$work/cargo.jsonl" "$arch" <<'PY_BINARY'
import json, struct, sys
artifacts = [json.loads(line) for line in open(sys.argv[1])]
binaries = [a['executable'] for a in artifacts
            if a.get('reason') == 'compiler-artifact'
            and a.get('target', {}).get('name') == 'bandix-gui' and a.get('executable')]
if not binaries:
    raise SystemExit('Cargo did not produce the bandix-gui executable')
path = binaries[-1]
with open(path, 'rb') as source:
    header = source.read(20)
expected = {'x86_64': 62, 'aarch64': 183}[sys.argv[2]]
if header[:6] != b'\x7fELF\x02\x01' or struct.unpack('<H', header[18:20])[0] != expected:
    raise SystemExit('Executable architecture does not match this Linux host; build natively')
print(path)
PY_BINARY
)"
desktop-file-validate packaging/linux/bandix-gui.desktop
mkdir -p "$appdir/usr/share/icons/hicolor" "$appdir/usr/share/applications"
install -m644 packaging/linux/bandix-gui.desktop "$appdir/usr/share/applications/"
for size in 32 48 64 128 256 512; do
    icon_dir="$appdir/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$icon_dir"
    install -m644 "assets/icons/bandix-${size}.png" "$icon_dir/bandix-gui.png"
done

# GPUI also loads these at runtime; ldd alone cannot discover them.
extra_libraries=()
for library in libfontconfig.so.1 libfreetype.so.6; do
    library_path="$(ldconfig -p | awk -v lib="$library" '$1 == lib && !found {print $NF; found=1}')"
    [[ -n "$library_path" ]] || { echo "Missing runtime library: $library" >&2; exit 1; }
    extra_libraries+=(--library "$library_path")
done
# Keep compositor/keyboard libraries matched to the host's GPU drivers and
# XCompose/keymap data. Old bundled copies can break newer desktop systems.
NO_STRIP=1 "$tools/linuxdeploy/squashfs-root/AppRun" \
    --appdir "$appdir" --executable "$binary" \
    --exclude-library 'libwayland-*.so*' \
    --exclude-library 'libxkbcommon*.so*' \
    --desktop-file packaging/linux/bandix-gui.desktop \
    --icon-file assets/bandix.png --icon-filename bandix-gui \
    --custom-apprun packaging/linux/AppRun "${extra_libraries[@]}"
# Use the full-resolution icon for AppImage launchers and file managers.
ln -sfn usr/share/icons/hicolor/512x512/apps/bandix-gui.png "$appdir/bandix-gui.png"
ln -sfn bandix-gui.png "$appdir/.DirIcon"
install -m755 packaging/linux/install.sh "$appdir/install.sh"

python3 scripts/check-linux-abi.py "$appdir/usr" --max-glibc "$glibc_baseline" > "$work/abi.json"
required_glibc="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["minimum_glibc"])' "$work/abi.json")"
cat > "$appdir/README.txt" <<EOF
Bandix $version - Linux $arch

Run: ./AppRun
Optional per-user installation (requires Python 3):
  AppImage: ./$name.AppImage --install
  Without FUSE: ./$name.AppImage --appimage-extract-and-run --install
  Extracted bundle: ./AppRun --install (or ./install.sh)
The installer adds Bandix and its icon to the application menu without root.
AppImage installs use a stable copy in the user's data directory, so the
downloaded file can be moved or deleted afterwards. Reinstall to update it.

Built on: $(. /etc/os-release; echo "$PRETTY_NAME")
Supported glibc baseline: $glibc_baseline or later.
Detected required glibc symbols: $required_glibc (optional weak symbols excluded).
Both X11 and Wayland are supported. The host supplies graphics drivers,
Vulkan/EGL support, Wayland client/cursor and xkbcommon libraries, and fonts.
Connect to your Bandix service in Settings.
The complete extracted directory must stay together.
EOF
python3 - "$version" "$arch" "$required_glibc" "$format" "$tools" "$appdir/BUILD-INFO.json" "$work/abi.json" <<'PY_INFO'
import datetime, hashlib, json, pathlib, platform, subprocess, sys
version, arch, glibc, package_format, tools_dir, destination, abi_path = sys.argv[1:]
abi = json.loads(pathlib.Path(abi_path).read_text())
def command(*args):
    return subprocess.check_output(args, text=True).strip()
tool_names = ['linuxdeploy'] + (['appimagetool'] if package_format in ['all', 'appimage'] else [])
tool_hashes = {name: hashlib.sha256((pathlib.Path(tools_dir) / (name + '.AppImage')).read_bytes()).hexdigest()
               for name in tool_names}
os_release = platform.freedesktop_os_release()
info = {
    'name': 'bandix-gui', 'version': version, 'os': 'linux', 'architecture': arch,
    'formats': ['appimage', 'tar.gz', 'deb', 'rpm'] if package_format == 'all' else [package_format],
    'built_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'build_distribution': {'id': os_release.get('ID'), 'version': os_release.get('VERSION_ID'),
                           'name': os_release.get('PRETTY_NAME')},
    'glibc_baseline': abi['glibc_baseline'], 'glibc_requirements': abi['files'],
    'minimum_glibc': glibc, 'rustc': command('rustc', '--version'),
    'source_commit': command('git', 'rev-parse', 'HEAD'),
    'source_dirty': bool(command('git', 'status', '--porcelain')),
    'cargo_lock_sha256': hashlib.sha256(pathlib.Path('Cargo.lock').read_bytes()).hexdigest(),
    'packaging_tools_sha256': tool_hashes,
}
pathlib.Path(destination).write_text(json.dumps(info, indent=2) + '\n')
PY_INFO

outputs=()
if [[ "$format" == all || "$format" == tar.gz ]]; then
    tar -czf "$work/$name.tar.gz" -C "$work" "$name"
    outputs+=("$name.tar.gz")
fi
if [[ "$format" == all || "$format" == appimage ]]; then
    ARCH="$arch" "$tools/appimagetool/squashfs-root/AppRun" --no-appstream \
        "$appdir" "$work/$name.AppImage"
    chmod +x "$work/$name.AppImage"
    outputs+=("$name.AppImage")
fi
if [[ "$format" == all || "$format" == deb || "$format" == rpm ]]; then
    python3 scripts/package-linux-native.py --binary "$binary" \
        --build-info "$appdir/BUILD-INFO.json" --output-dir "$work" --format "$format"
    if [[ "$format" == all || "$format" == deb ]]; then outputs+=("$name.deb"); fi
    if [[ "$format" == all || "$format" == rpm ]]; then outputs+=("$name.rpm"); fi
fi
cp "$appdir/BUILD-INFO.json" "$work/$name.build-info.json"
outputs+=("$name.build-info.json")
(cd "$work" && sha256sum "${outputs[@]}" > "$name.sha256" && sha256sum -c "$name.sha256")
outputs+=("$name.sha256")

# Publish only after every selected format was successfully generated.
# Only replace the generated staging directory, never source assets.
rm -rf -- "$out/$name"
mv -- "$appdir" "$out/$name"
for output in "${outputs[@]}"; do
    mv -f -- "$work/$output" "$out/$output"
done
printf '\nLinux %s packages saved to %s (requires glibc %s or later)\n' "$arch" "$out" "$required_glibc"
for output in "${outputs[@]}"; do
    ls -lh "$out/$output"
done
