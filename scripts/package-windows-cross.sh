#!/usr/bin/env bash
# Cross-compile portable Windows packages from macOS or Linux with cargo-xwin.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

builder=auto
architecture=x86_64
output_dir=""
while (($#)); do
    case "$1" in
        --builder|--arch|--output-dir)
            (($# >= 2)) || { echo "Missing value for $1" >&2; exit 2; }
            case "$1" in
                --builder) builder="$2" ;;
                --arch) architecture="$2" ;;
                --output-dir) output_dir="$2" ;;
            esac
            shift 2
            ;;
        -h|--help)
            cat <<'EOF'
Usage: ./scripts/package-windows-cross.sh [OPTIONS]

Cross-compile Bandix GUI Windows packages from macOS or Linux.

Options:
  --builder auto|local|docker  Build with local cargo-xwin or Docker (default: auto).
  --arch x86_64|x64|arm64|all Target architecture (default: x86_64).
  --output-dir DIR             Output directory (default: $CARGO_TARGET_DIR/dist).
  -h, --help                   Show this help.

Local prerequisites:
  brew install llvm
  brew install --cask wine-stable  # optional when Docker Desktop is available
  cargo install --locked cargo-xwin
  rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc

Docker uses the official messense/cargo-xwin image. Override it with
CARGO_XWIN_IMAGE. BANDIX_DOCKER_PLATFORM may select a Docker platform such as
linux/amd64. Outputs include a portable directory, ZIP, build-info JSON and
SHA-256 checksum.
EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1 (see --help)" >&2
            exit 2
            ;;
    esac
done

case "$builder" in auto|local|docker) ;; *) echo "Unsupported builder: $builder" >&2; exit 2 ;; esac
case "$architecture" in
    x64) architecture=x86_64 ;;
    x86_64|arm64|all) ;;
    *) echo "Unsupported architecture: $architecture" >&2; exit 2 ;;
esac
case "$(uname -s)" in
    Darwin|Linux) ;;
    *) echo 'Run this cross-packaging script on macOS or Linux.' >&2; exit 1 ;;
esac

for tool in curl git python3 tar unzip zip; do
    command -v "$tool" >/dev/null || { echo "Missing packaging tool: $tool" >&2; exit 1; }
done
if command -v shasum >/dev/null; then
    sha256_file() { shasum -a 256 "$1" | awk '{print $1}'; }
elif command -v sha256sum >/dev/null; then
    sha256_file() { sha256sum "$1" | awk '{print $1}'; }
else
    echo 'Missing SHA-256 tool: install shasum or sha256sum.' >&2
    exit 1
fi

if [[ "$builder" == auto ]]; then
    if command -v cargo-xwin >/dev/null; then
        builder=local
    elif command -v docker >/dev/null; then
        builder=docker
    else
        echo 'Install cargo-xwin or Docker Desktop first.' >&2
        echo 'See ./scripts/package-windows-cross.sh --help for local setup.' >&2
        exit 1
    fi
fi

if [[ "$builder" == local ]]; then
    for tool in cargo cargo-xwin rustc; do
        command -v "$tool" >/dev/null || { echo "Missing local build tool: $tool" >&2; exit 1; }
    done
    # Homebrew intentionally does not link LLVM into the default PATH.
    if { ! command -v clang-cl >/dev/null || ! command -v llvm-rc >/dev/null; } \
        && command -v brew >/dev/null; then
        llvm_prefix="$(brew --prefix llvm 2>/dev/null || true)"
        if [[ -n "$llvm_prefix" && -x "$llvm_prefix/bin/clang-cl" ]]; then
            export PATH="$llvm_prefix/bin:$PATH"
        fi
    fi
    if ! command -v clang-cl >/dev/null || ! command -v llvm-rc >/dev/null; then
        echo 'Missing clang-cl or llvm-rc. On macOS, install them with: brew install llvm' >&2
        exit 1
    fi
else
    command -v docker >/dev/null || { echo 'Missing build tool: docker' >&2; exit 1; }
    docker info >/dev/null 2>&1 || { echo 'Docker is installed but its engine is not running.' >&2; exit 1; }
fi

shader_runner=""
if command -v winepath >/dev/null \
    && { command -v wine64 >/dev/null || command -v wine >/dev/null; }; then
    shader_runner=local
elif command -v docker >/dev/null && docker info >/dev/null 2>&1; then
    shader_runner=docker
else
    cat >&2 <<'EOF'
GPUI release builds need Wine to compile their DirectX shaders.
Install Docker Desktop, or install Wine locally, then run this script again.
EOF
    exit 1
fi

version="$(awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || { echo 'Unable to read the package version from Cargo.toml.' >&2; exit 1; }

target_dir="${CARGO_TARGET_DIR:-$root/target}"
case "$target_dir" in /*) ;; *) target_dir="$root/$target_dir" ;; esac
mkdir -p "$target_dir"
target_dir="$(cd "$target_dir" && pwd -P)"

if [[ -z "$output_dir" ]]; then
    out="$target_dir/dist"
else
    case "$output_dir" in /*) out="$output_dir" ;; *) out="$root/$output_dir" ;; esac
fi
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"

work="$(mktemp -d "$out/.bandix-windows-cross.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT

tools_dir="$target_dir/packaging-tools/windows-cross"
mkdir -p "$tools_dir"
efxc_version=0.0.14.278
efxc_archive="$tools_dir/efxc2-x86_64-win64-$efxc_version.zip"
efxc_sha256=d6e4861422756032584edd630a0e2dadea252e7179d4af70013805aee8544055
efxc_dir="$tools_dir/efxc2-$efxc_version"

prepare_efxc() {
    if [[ ! -f "$efxc_dir/fxc.exe" ]]; then
        if [[ ! -f "$efxc_archive" || "$(sha256_file "$efxc_archive")" != "$efxc_sha256" ]]; then
            echo "Downloading efxc2 $efxc_version for GPUI shader compilation..."
            curl -fL --retry 3 \
                "https://github.com/JPeterMugaas/efxc2/releases/download/$efxc_version/efxc2-x86_64-win64-$efxc_version.zip" \
                -o "$efxc_archive.part"
            [[ "$(sha256_file "$efxc_archive.part")" == "$efxc_sha256" ]] || {
                echo 'efxc2 checksum verification failed.' >&2
                exit 1
            }
            mv "$efxc_archive.part" "$efxc_archive"
        fi
        mkdir -p "$efxc_dir"
        unzip -qo "$efxc_archive" -d "$efxc_dir"
    fi
}

gpui_source_dir=""
prepare_gpui_sources() {
    local candidate cargo_home registry_root crate_version crate_checksum crate_archive
    read -r crate_version crate_checksum < <(python3 - Cargo.lock <<'PY_LOCK'
import pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
for section in text.split('[[package]]')[1:]:
    if re.search(r'^name = "gpui-pre-windows"$', section, re.MULTILINE):
        version = re.search(r'^version = "([^"]+)"$', section, re.MULTILINE).group(1)
        checksum = re.search(r'^checksum = "([^"]+)"$', section, re.MULTILINE).group(1)
        print(version, checksum)
        break
else:
    raise SystemExit('gpui-pre-windows is missing from Cargo.lock')
PY_LOCK
)
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    registry_root="$cargo_home/registry/src"
    if [[ -d "$registry_root" ]]; then
        for candidate in "$registry_root"/*/"gpui-pre-windows-$crate_version"/src; do
            if [[ -f "$candidate/shaders.hlsl" && -f "$candidate/color_text_raster.hlsl" ]]; then
                gpui_source_dir="$candidate"
                return
            fi
        done
    fi
    gpui_source_dir="$tools_dir/gpui-pre-windows-$crate_version/src"
    [[ -f "$gpui_source_dir/shaders.hlsl" ]] && return
    crate_archive="$tools_dir/gpui-pre-windows-$crate_version.crate"
    echo "Downloading gpui-pre-windows $crate_version shader sources..."
    curl -fL --retry 3 \
        "https://static.crates.io/crates/gpui-pre-windows/gpui-pre-windows-$crate_version.crate" \
        -o "$crate_archive.part"
    [[ "$(sha256_file "$crate_archive.part")" == "$crate_checksum" ]] || {
        echo 'gpui-pre-windows checksum verification failed.' >&2
        exit 1
    }
    mv "$crate_archive.part" "$crate_archive"
    tar -xzf "$crate_archive" -C "$tools_dir"
    [[ -f "$gpui_source_dir/shaders.hlsl" ]] || { echo 'Unable to extract GPUI shader sources.' >&2; exit 1; }
}

generate_gpui_shaders() {
    local generated="$work/gpui-shaders" image
    prepare_efxc
    prepare_gpui_sources
    mkdir -p "$generated"
    echo "Compiling GPUI DirectX shaders with $shader_runner Wine..."
    if [[ "$shader_runner" == local ]]; then
        scripts/compile-gpui-windows-shaders.sh "$gpui_source_dir" "$generated" "$efxc_dir/fxc.exe"
    else
        image="${CARGO_XWIN_IMAGE:-messense/cargo-xwin:latest}"
        docker run --rm --platform "${BANDIX_DOCKER_PLATFORM:-linux/amd64}" \
            -v "$root/scripts:/bandix-scripts:ro" \
            -v "$gpui_source_dir:/gpui-source:ro" \
            -v "$efxc_dir:/efxc:ro" -v "$generated:/shader-output:rw" \
            "$image" bash /bandix-scripts/compile-gpui-windows-shaders.sh \
            /gpui-source /shader-output /efxc/fxc.exe
    fi
    python3 - "$generated" "$generated/shaders_bytes.rs" <<'PY_SHADERS'
import pathlib, sys

source = pathlib.Path(sys.argv[1])
destination = pathlib.Path(sys.argv[2])
modules = [
    'quad', 'shadow', 'path_rasterization', 'path_sprite', 'underline',
    'monochrome_sprite', 'subpixel_sprite', 'polychrome_sprite',
    'emoji_rasterization',
]
with destination.open('w', encoding='utf-8') as output:
    for module in modules:
        for target, suffix in [('vertex', 'vs'), ('fragment', 'ps')]:
            data = (source / f'{module}_{suffix}.bin').read_bytes()
            name = f'{module}_{target}_bytes'.upper()
            output.write(f'const {name}: &[u8] = &[\n')
            for offset in range(0, len(data), 20):
                output.write('    ' + ', '.join(map(str, data[offset:offset + 20])) + ',\n')
            output.write('];\n')
PY_SHADERS
}

case "$architecture" in
    all) package_arches=(x86_64 arm64) ;;
    *) package_arches=("$architecture") ;;
esac

build_target() {
    local rust_target="$1"
    local shader_bindings="" shader_out_dir
    local -a shader_out_dirs=()
    echo "Building release binary for $rust_target with $builder cargo-xwin..."
    if ! find "$target_dir/$rust_target/release/build" -path '*gpui-pre-windows-*/out/shaders_bytes.rs' \
        -print -quit 2>/dev/null | grep -q .; then
        echo 'The first Cargo pass may report a missing GPUI shaders_bytes.rs; the script will generate it and resume.'
    fi
    if run_cross_build "$rust_target"; then
        return
    fi

    while IFS= read -r shader_out_dir; do
        shader_out_dirs+=("$shader_out_dir")
    done < <(find "$target_dir/$rust_target/release/build" -type d -path '*gpui-pre-windows-*/out' 2>/dev/null)
    if ((${#shader_out_dirs[@]} == 0)); then
        echo 'Cross-compilation failed before GPUI created its shader output directory.' >&2
        exit 1
    fi

    generate_gpui_shaders
    shader_bindings="$work/gpui-shaders/shaders_bytes.rs"
    for shader_out_dir in "${shader_out_dirs[@]}"; do
        cp "$shader_bindings" "$shader_out_dir/shaders_bytes.rs"
    done
    echo 'Resuming Windows release build with compiled GPUI shaders...'
    run_cross_build "$rust_target"
}

run_cross_build() {
    local rust_target="$1"
    if [[ "$builder" == local ]]; then
        local target_libdir
        target_libdir="$(rustc --print target-libdir --target "$rust_target" 2>/dev/null || true)"
        if [[ ! -d "$target_libdir" ]]; then
            echo "Missing Rust target '$rust_target'." >&2
            echo "Install it with: rustup target add $rust_target" >&2
            exit 1
        fi
        CARGO_TARGET_DIR="$target_dir" cargo xwin build --release --locked --target "$rust_target"
    else
        local image="${CARGO_XWIN_IMAGE:-messense/cargo-xwin:latest}"
        local platform_options=()
        if [[ -n "${BANDIX_DOCKER_PLATFORM:-}" ]]; then
            platform_options=(--platform "$BANDIX_DOCKER_PLATFORM")
        fi
        docker run --rm "${platform_options[@]}" \
            -v "$root:/io:ro" -v "$target_dir:/cargo-target:rw" \
            -w /io -e CARGO_TARGET_DIR=/cargo-target \
            -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
            "$image" sh -lc \
            'rustup target add "$1" && cargo xwin build --release --locked --target "$1"' \
            sh "$rust_target"
    fi
}

write_build_info() {
    local destination="$1" package_arch="$2" rust_target="$3" builder_name="$4"
    local source_commit source_dirty rustc_version xwin_version lock_hash built_at
    source_commit="$(git rev-parse HEAD)"
    [[ -z "$(git status --porcelain)" ]] && source_dirty=false || source_dirty=true
    lock_hash="$(sha256_file Cargo.lock)"
    built_at="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    if [[ "$builder_name" == local ]]; then
        rustc_version="$(rustc --version)"
        xwin_version="$(cargo xwin --version)"
    else
        rustc_version="recorded inside container build output"
        xwin_version="${CARGO_XWIN_IMAGE:-messense/cargo-xwin:latest}"
    fi
    python3 - "$destination" "$version" "$package_arch" "$rust_target" \
        "$builder_name" "$built_at" "$rustc_version" "$xwin_version" \
        "$source_commit" "$source_dirty" "$lock_hash" <<'PY_INFO'
import json
import pathlib
import sys

(destination, version, architecture, rust_target, builder, built_at, rustc,
 xwin, commit, dirty, lock_hash) = sys.argv[1:]
info = {
    "name": "bandix-gui",
    "version": version,
    "os": "windows",
    "architecture": architecture,
    "rust_target": rust_target,
    "format": "zip",
    "cross_compiled": True,
    "builder": builder,
    "built_at_utc": built_at,
    "rustc": rustc,
    "cargo_xwin": xwin,
    "source_commit": commit,
    "source_dirty": dirty == "true",
    "cargo_lock_sha256": lock_hash,
}
pathlib.Path(destination).write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
PY_INFO
}

outputs=()
for package_arch in "${package_arches[@]}"; do
    case "$package_arch" in
        x86_64) rust_target=x86_64-pc-windows-msvc ;;
        arm64) rust_target=aarch64-pc-windows-msvc ;;
    esac
    build_target "$rust_target"

    binary="$target_dir/$rust_target/release/bandix-gui.exe"
    [[ -f "$binary" ]] || { echo "Release binary was not created: $binary" >&2; exit 1; }

    name="bandix-gui-${version}-windows-${package_arch}"
    stage="$work/$name"
    mkdir -p "$stage"
    cp "$binary" "$stage/bandix-gui.exe"
    cp assets/icons/bandix-256.png "$stage/bandix-gui.png"
    cat > "$stage/README.txt" <<EOF
Bandix GUI $version - Windows $package_arch

Run bandix-gui.exe to start the application. Keep the files in this directory
together if you move the application. Windows 10 or later is recommended.

Connect to your Bandix service from Settings. Automatic default-gateway
discovery is not currently available on Windows, so enter the server address
manually, for example http://192.168.1.1:8686.
EOF
    write_build_info "$stage/BUILD-INFO.json" "$package_arch" "$rust_target" "$builder"

    temporary_zip="$work/$name.zip"
    (cd "$work" && zip -qry "$temporary_zip" "$name")
    zip_hash="$(sha256_file "$temporary_zip")"
    printf '%s  %s\n' "$zip_hash" "$name.zip" > "$work/$name.sha256"

    published_directory="$out/$name"
    rm -rf -- "$published_directory"
    mv "$stage" "$published_directory"
    mv -f -- "$temporary_zip" "$out/$name.zip"
    mv -f -- "$work/$name.sha256" "$out/$name.sha256"
    outputs+=("$name.zip" "$name.sha256")
done

echo
echo "Windows cross-compiled packages saved to $out"
for output in "${outputs[@]}"; do
    ls -lh "$out/$output"
done
