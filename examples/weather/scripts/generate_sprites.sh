#!/usr/bin/env bash
set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPRITES_DIR="$SCRIPT_DIR/../sprites"
PYTHON="${PYTHON:-../../../memtui/.venv/bin/python}"

# Function to generate sprites for a directory
# Handles multi-layer sprites: SVGs named {name}_{color}.svg generate {size}_{color}.txt
generate_sprites() {
    local dir_name="$1"
    local h_scale="${2:-1.0}"
    local dir_path="$SPRITES_DIR/$dir_name"

    if [ ! -d "$dir_path" ]; then
        echo "Error: Directory $dir_path does not exist"
        return 1
    fi

    echo "Processing $dir_name..."
    local found_svg=false

    # Process all SVG files in directory
    for svg_file in "$dir_path"/*.svg; do
        [ -f "$svg_file" ] || continue
        found_svg=true

        # Extract color from filename: name_color.svg -> color
        local basename=$(basename "$svg_file" .svg)
        local color="${basename##*_}"  # Get part after last underscore

        # If no underscore in name, use original name as color
        if [ "$color" = "$basename" ]; then
            color="default"
        fi

        echo "  Layer: $basename → $color"

        # Generate each size with color suffix
        for size_spec in "small:48" "medium:64" "large:96"; do
            local size="${size_spec%%:*}"
            local width="${size_spec##*:}"
            local output="$dir_path/${size}_${color}.txt"

            $PYTHON "$SCRIPT_DIR/png_to_ascii.py" "$svg_file" \
                -m block -mono -i -w "$width" --h-scale "$h_scale" \
                -o "$output" 2>/dev/null
            echo "    ✓ ${size}_${color}.txt ($width chars)"
        done
    done

    if [ "$found_svg" = false ]; then
        echo "  Warning: No SVG files found, skipping..."
    fi
}

# Main logic
if [ $# -eq 0 ]; then
    # No args: process all directories with default scale
    echo "Generating sprites for all directories..."
    for dir in "$SPRITES_DIR"/*; do
        if [ -d "$dir" ]; then
            dir_name=$(basename "$dir")
            generate_sprites "$dir_name" "1.0"
        fi
    done
    echo ""
    echo "Done! Generated sprites in all directories."
elif [ $# -eq 1 ]; then
    # Check if arg is a number (scale for all dirs) or directory name
    if [[ "$1" =~ ^[0-9]+\.?[0-9]*$ ]]; then
        # It's a number - apply to all directories
        H_SCALE="$1"
        echo "Generating sprites for all directories with h-scale=$H_SCALE..."
        for dir in "$SPRITES_DIR"/*; do
            if [ -d "$dir" ]; then
                dir_name=$(basename "$dir")
                generate_sprites "$dir_name" "$H_SCALE"
            fi
        done
        echo ""
        echo "Done! Generated sprites in all directories."
    else
        # It's a directory name
        DIR_NAME="$1"
        echo "Generating sprites for '$DIR_NAME' with h-scale=1.0"
        generate_sprites "$DIR_NAME" "1.0"
    fi
else
    # Two args: directory name and scale
    DIR_NAME="$1"
    H_SCALE="$2"
    echo "Generating sprites for '$DIR_NAME' with h-scale=$H_SCALE"
    generate_sprites "$DIR_NAME" "$H_SCALE"
fi
