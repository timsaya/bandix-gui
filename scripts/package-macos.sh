#!/usr/bin/env bash
# Build signed macOS app bundles plus ZIP and DMG packages.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

[[ "$(uname -s)" == Darwin ]] || {
    echo 'This package must be built on macOS.' >&2
    exit 1
}

mode="separate"
make_dmg=true
while (($#)); do
    case "$1" in
        --separate)
            mode="separate"
            ;;
        --universal)
            mode="universal"
            ;;
        --native)
            mode="native"
            ;;
        --no-dmg)
            make_dmg=false
            ;;
        -h|--help)
            cat <<'EOF'
Usage: ./scripts/package-macos.sh [--separate|--universal|--native] [--no-dmg]

Modes:
  --separate  Build separate arm64 and x86_64 packages (default).
  --universal Build one package containing both architectures.
  --native    Only build for the current Mac's architecture.

Options:
  --no-dmg    Only produce app bundles and ZIP archives.

Environment:
  MACOS_CODESIGN_IDENTITY  Signing identity; defaults to ad-hoc signing (-).
  MACOS_BUNDLE_ID          Bundle identifier; defaults to com.bandix.gui.
  MACOSX_DEPLOYMENT_TARGET Minimum macOS version; defaults to 11.0.
  CARGO_TARGET_DIR         Cargo build/output directory; defaults to ./target.

For distribution outside your own Mac, provide an Apple Developer ID identity.
Notarization is intentionally separate because it requires App Store credentials.
EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            echo 'Run with --help for usage.' >&2
            exit 2
            ;;
    esac
    shift
done

for tool in cargo rustc xcrun sips iconutil plutil codesign ditto; do
    command -v "$tool" >/dev/null || {
        echo "Missing build tool: $tool" >&2
        exit 1
    }
done
if "$make_dmg"; then
    command -v hdiutil >/dev/null || {
        echo 'Missing build tool: hdiutil' >&2
        exit 1
    }
fi

version="$(awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || {
    echo 'Unable to read the package version from Cargo.toml.' >&2
    exit 1
}

case "$(uname -m)" in
    arm64)
        native_target="aarch64-apple-darwin"
        native_arch="arm64"
        ;;
    x86_64)
        native_target="x86_64-apple-darwin"
        native_arch="x86_64"
        ;;
    *)
        echo "Unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

arm_target="aarch64-apple-darwin"
intel_target="x86_64-apple-darwin"
case "$mode" in
    separate|universal) required_targets=("$arm_target" "$intel_target") ;;
    native) required_targets=("$native_target") ;;
esac

missing_targets=()
for target in "${required_targets[@]}"; do
    target_libdir="$(rustc --print target-libdir --target "$target" 2>/dev/null || true)"
    if [[ ! -d "$target_libdir" ]]; then
        missing_targets+=("$target")
    fi
done
if ((${#missing_targets[@]})); then
    echo 'Missing Rust compilation target(s).' >&2
    echo "Install them once with: rustup target add ${missing_targets[*]}" >&2
    exit 1
fi

target_dir="${CARGO_TARGET_DIR:-$root/target}"
case "$target_dir" in
    /*) ;;
    *) target_dir="$root/$target_dir" ;;
esac
mkdir -p "$target_dir"
target_dir="$(cd "$target_dir" && pwd -P)"

out="$target_dir/dist"
bundle_id="${MACOS_BUNDLE_ID:-com.bandix.gui}"
deployment_target="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
signing_identity="${MACOS_CODESIGN_IDENTITY:--}"
work="$(mktemp -d "${TMPDIR:-/tmp}/bandix-macos.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT

mkdir -p "$out"

build_target() {
    local target="$1"
    echo "Building release binary for $target..."
    MACOSX_DEPLOYMENT_TARGET="$deployment_target" \
        cargo build --release --locked --target "$target"
}

iconset="$work/Bandix.iconset"
mkdir -p "$iconset"
icon_source="$root/assets/bandix.png"
while read -r filename pixels; do
    sips -z "$pixels" "$pixels" "$icon_source" --out "$iconset/$filename" >/dev/null
done <<'EOF'
icon_16x16.png 16
icon_16x16@2x.png 32
icon_32x32.png 32
icon_32x32@2x.png 64
icon_128x128.png 128
icon_128x128@2x.png 256
icon_256x256.png 256
icon_256x256@2x.png 512
icon_512x512.png 512
icon_512x512@2x.png 1024
EOF
iconutil -c icns "$iconset" -o "$work/Bandix.icns"

outputs=()
package_binary() {
    local binary="$1"
    local package_arch="$2"
    local stage="$work/package-$package_arch"
    local app="$stage/Bandix GUI.app"
    local package_name="bandix-gui-${version}-macos-${package_arch}"
    local exported_app="$out/$package_name.app"
    local zip_path="$out/$package_name.zip"
    local plist="$app/Contents/Info.plist"

    [[ -x "$binary" ]] || {
        echo "Release binary was not created: $binary" >&2
        exit 1
    }

    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    install -m755 "$binary" "$app/Contents/MacOS/Bandix"
    install -m644 "$work/Bandix.icns" "$app/Contents/Resources/Bandix.icns"
    xcrun strip -x "$app/Contents/MacOS/Bandix"

    plutil -create xml1 "$plist"
    plutil -insert CFBundleDevelopmentRegion -string en "$plist"
    plutil -insert CFBundleDisplayName -string "Bandix GUI" "$plist"
    plutil -insert CFBundleExecutable -string Bandix "$plist"
    plutil -insert CFBundleIconFile -string Bandix "$plist"
    plutil -insert CFBundleIdentifier -string "$bundle_id" "$plist"
    plutil -insert CFBundleInfoDictionaryVersion -string 6.0 "$plist"
    plutil -insert CFBundleName -string "Bandix GUI" "$plist"
    plutil -insert CFBundlePackageType -string APPL "$plist"
    plutil -insert CFBundleShortVersionString -string "$version" "$plist"
    plutil -insert CFBundleVersion -string "$version" "$plist"
    plutil -insert LSApplicationCategoryType -string public.app-category.utilities "$plist"
    plutil -insert LSMinimumSystemVersion -string "$deployment_target" "$plist"
    plutil -insert NSHighResolutionCapable -bool true "$plist"

    if [[ "$signing_identity" == '-' ]]; then
        echo "Applying ad-hoc code signature to $package_arch app..."
        codesign --force --deep --sign - "$app"
    else
        echo "Signing $package_arch app with identity: $signing_identity"
        codesign --force --deep --options runtime --timestamp \
            --sign "$signing_identity" "$app"
    fi
    codesign --verify --deep --strict "$app"

    rm -rf -- "$exported_app"
    ditto "$app" "$exported_app"
    outputs+=("$exported_app")

    rm -f -- "$zip_path"
    ditto -c -k --sequesterRsrc --keepParent "$app" "$zip_path"
    outputs+=("$zip_path")

    if "$make_dmg"; then
        local dmg_root="$work/dmg-$package_arch"
        local dmg_path="$out/$package_name.dmg"
        mkdir -p "$dmg_root"
        ditto "$app" "$dmg_root/Bandix GUI.app"
        ln -s /Applications "$dmg_root/Applications"
        rm -f -- "$dmg_path"
        hdiutil create -quiet -volname "Bandix GUI" -srcfolder "$dmg_root" \
            -ov -format UDZO "$dmg_path"
        outputs+=("$dmg_path")
    fi
}

case "$mode" in
    separate)
        build_target "$arm_target"
        package_binary "$target_dir/$arm_target/release/bandix-gui" arm64
        build_target "$intel_target"
        package_binary "$target_dir/$intel_target/release/bandix-gui" x86_64
        ;;
    universal)
        build_target "$arm_target"
        build_target "$intel_target"
        universal_binary="$work/Bandix-universal"
        xcrun lipo -create \
            "$target_dir/$arm_target/release/bandix-gui" \
            "$target_dir/$intel_target/release/bandix-gui" \
            -output "$universal_binary"
        chmod +x "$universal_binary"
        package_binary "$universal_binary" universal
        ;;
    native)
        build_target "$native_target"
        package_binary "$target_dir/$native_target/release/bandix-gui" "$native_arch"
        ;;
esac

echo
echo "macOS packages saved to $out"
du -sh "${outputs[@]}"
