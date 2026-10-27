#!/usr/bin/env bash
# Compile GPUI's DirectX 11 shaders with a Wine-runnable FXC replacement.
set -euo pipefail

if (($# != 3)); then
    echo "Usage: $0 GPUI_SOURCE_DIR OUTPUT_DIR FXC_EXE" >&2
    exit 2
fi

source_dir="$(cd "$1" && pwd -P)"
output_dir="$2"
fxc="$3"
mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd -P)"
fxc="$(cd "$(dirname "$fxc")" && pwd -P)/$(basename "$fxc")"

wine_command="${WINE:-}"
if [[ -z "$wine_command" ]]; then
    if command -v wine64 >/dev/null; then wine_command=wine64
    elif command -v wine >/dev/null; then wine_command=wine
    else echo 'Wine is required to compile GPUI DirectX shaders.' >&2; exit 1
    fi
fi
command -v winepath >/dev/null || { echo 'Missing Wine tool: winepath' >&2; exit 1; }

fxc_windows="$(winepath -w "$fxc")"
modules=(quad shadow path_rasterization path_sprite underline monochrome_sprite subpixel_sprite polychrome_sprite)
for module in "${modules[@]}"; do
    for shader_target in vertex fragment; do
        case "$shader_target" in vertex) profile=vs_4_1; suffix=vs ;; fragment) profile=ps_4_1; suffix=ps ;; esac
        output="$output_dir/${module}_${suffix}.bin"
        "$wine_command" "$fxc_windows" /nologo /T "$profile" \
            /E "${module}_${shader_target}" /Fo "$(winepath -w "$output")" /O3 \
            "$(winepath -w "$source_dir/shaders.hlsl")"
    done
done

for shader_target in vertex fragment; do
    case "$shader_target" in vertex) profile=vs_4_1; suffix=vs ;; fragment) profile=ps_4_1; suffix=ps ;; esac
    output="$output_dir/emoji_rasterization_${suffix}.bin"
    "$wine_command" "$fxc_windows" /nologo /T "$profile" \
        /E "emoji_rasterization_${shader_target}" /Fo "$(winepath -w "$output")" /O3 \
        "$(winepath -w "$source_dir/color_text_raster.hlsl")"
done
