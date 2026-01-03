#!/usr/bin/env python3
"""
Convert images (PNG, JPG, SVG) to ASCII art with optional color.
Usage: python png_to_ascii.py <image> [options]

SVG support requires: pip install cairosvg

Options:
  -w, --width WIDTH          Output width in characters (default: 100)
  --h-scale SCALE           Horizontal scale factor (default: 1.0)
  --v-scale SCALE           Vertical scale factor (default: 1.0)
  -b, --brightness ADJUST   Brightness adjustment -255 to 255 (default: 0)
  -c, --contrast FACTOR     Contrast factor 0.0 to 3.0 (default: 1.0)
  -s, --sharpness FACTOR    Sharpness factor 0.0 to 3.0 (default: 1.0)
  -m, --mode MODE           Rendering mode: full, block, shade, or ascii (default: full)
  -o, --output FILE         Output file (default: <input>_ascii.txt)
  --no-color, -mono         Disable ANSI color codes (plain monochrome output)
  -t, --threshold VALUE     Brightness threshold 0-255 for full/block modes (default: 128)
  -i, --invert              Invert brightness (dark pixels become blocks)
"""

import sys
import argparse
import io
from PIL import Image, ImageEnhance

try:
    import cairosvg
    HAS_CAIROSVG = True
except ImportError:
    HAS_CAIROSVG = False


# Unicode block characters for detailed rendering
BLOCK_CHARS = {
    0b0000: ' ',   # Empty
    0b0001: '▗',   # Lower right
    0b0010: '▖',   # Lower left
    0b0011: '▄',   # Lower half
    0b0100: '▝',   # Upper right
    0b0101: '▐',   # Right half
    0b0110: '▞',   # Diagonal
    0b0111: '▟',   # Missing upper left
    0b1000: '▘',   # Upper left
    0b1001: '▚',   # Diagonal inverse
    0b1010: '▌',   # Left half
    0b1011: '▙',   # Missing upper right
    0b1100: '▀',   # Upper half
    0b1101: '▜',   # Missing lower left
    0b1110: '▛',   # Missing lower right
    0b1111: '█',   # Full block
}

# Simple ASCII characters from darkest to brightest
ASCII_CHARS = " .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$"


def adjust_brightness_contrast(image, brightness=0, contrast=1.0, sharpness=1.0):
    """Adjust image brightness, contrast, and sharpness."""
    # Adjust brightness
    if brightness != 0:
        enhancer = ImageEnhance.Brightness(image)
        # Convert brightness adjustment to multiplier
        factor = 1.0 + (brightness / 255.0)
        image = enhancer.enhance(factor)

    # Adjust contrast
    if contrast != 1.0:
        enhancer = ImageEnhance.Contrast(image)
        image = enhancer.enhance(contrast)

    # Adjust sharpness
    if sharpness != 1.0:
        enhancer = ImageEnhance.Sharpness(image)
        image = enhancer.enhance(sharpness)

    return image


def resize_image(image, new_width=100, h_scale=1.0, v_scale=1.0):
    """Resize image with custom scaling factors."""
    width, height = image.size
    aspect_ratio = height / width

    # Apply scaling factors
    final_width = int(new_width * h_scale)
    # Terminal characters are roughly twice as tall as wide
    final_height = int(aspect_ratio * new_width * 0.5 * v_scale)

    return image.resize((final_width, final_height))


def get_brightness(pixel):
    """Calculate brightness using luminosity formula."""
    r, g, b = pixel[:3]
    return int(0.299 * r + 0.587 * g + 0.114 * b)


def pixel_to_ascii(pixel):
    """Convert a pixel's brightness to an ASCII character."""
    brightness = get_brightness(pixel)
    char_index = min(brightness * len(ASCII_CHARS) // 256, len(ASCII_CHARS) - 1)
    return ASCII_CHARS[char_index]


def get_block_char(pixels_2x2, threshold=128, invert=False, has_alpha=False):
    """
    Convert a 2x2 pixel block to a unicode block character.
    pixels_2x2: list of 4 pixels in order [top-left, top-right, bottom-left, bottom-right]
    """
    # Get brightness values and track which pixels are transparent
    brightnesses = []
    is_transparent = []

    for pixel in pixels_2x2:
        if pixel is None:
            brightnesses.append(0)
            is_transparent.append(True)
        elif has_alpha and len(pixel) >= 4 and pixel[3] < 128:
            # Transparent pixel
            brightnesses.append(0)
            is_transparent.append(True)
        else:
            brightnesses.append(get_brightness(pixel))
            is_transparent.append(False)

    if invert:
        brightnesses = [255 - b for b in brightnesses]

    # After inversion, set transparent pixels back to 0 (won't reach threshold)
    for i, is_trans in enumerate(is_transparent):
        if is_trans:
            brightnesses[i] = 0

    # Calculate which quadrants are "on" based on fixed threshold
    # This preserves maximum edge detail by using all 16 block patterns
    pattern = 0
    for i, brightness in enumerate(brightnesses):
        if brightness >= threshold:
            # Build pattern: bit 3=top-left, 2=top-right, 1=bottom-left, 0=bottom-right
            pattern |= (1 << (3 - i))

    return BLOCK_CHARS[pattern]


def get_shade_char(avg_brightness):
    """Convert average brightness to a shading character."""
    # Using block shading characters for smooth gradients
    if avg_brightness < 32:
        return ' '
    elif avg_brightness < 64:
        return '░'
    elif avg_brightness < 128:
        return '▒'
    elif avg_brightness < 192:
        return '▓'
    else:
        return '█'


def rgb_to_ansi(r, g, b):
    """Convert RGB values to ANSI 24-bit color escape sequence."""
    return f"\033[38;2;{r};{g};{b}m"


def average_color(pixels):
    """Calculate average color from a list of pixels."""
    valid_pixels = [p for p in pixels if p]
    if not valid_pixels:
        return (0, 0, 0)

    avg_r = sum(p[0] for p in valid_pixels) // len(valid_pixels)
    avg_g = sum(p[1] for p in valid_pixels) // len(valid_pixels)
    avg_b = sum(p[2] for p in valid_pixels) // len(valid_pixels)
    return (avg_r, avg_g, avg_b)


def image_to_colored_ascii(image_path, width=100, h_scale=1.0, v_scale=1.0,
                          brightness=0, contrast=1.0, sharpness=1.0, mode='full',
                          no_color=False, threshold=128, invert=False):
    """Convert image to colored ASCII art.

    mode: 'full' for full blocks only, 'block' for 2x2 pattern blocks, 'shade' for smooth shading, 'ascii' for text
    """
    try:
        # Load image - handle SVG specially
        if image_path.lower().endswith('.svg'):
            if not HAS_CAIROSVG:
                print("Error: cairosvg required for SVG support. Install with: pip install cairosvg")
                sys.exit(1)
            # Read SVG and replace currentColor with black (common in icon libraries)
            with open(image_path, 'r') as f:
                svg_content = f.read()
            svg_content = svg_content.replace('currentColor', 'black')
            # Render SVG to PNG at high resolution for quality
            png_data = cairosvg.svg2png(bytestring=svg_content.encode(), output_width=width * 4)
            image = Image.open(io.BytesIO(png_data))
        else:
            image = Image.open(image_path)

        # Handle alpha channel - convert to RGBA first to check for transparency
        if image.mode in ('RGBA', 'LA', 'PA'):
            has_alpha = True
            image = image.convert('RGBA')
        else:
            has_alpha = False
            image = image.convert('RGB')

        # Apply brightness, contrast, and sharpness adjustments
        image = adjust_brightness_contrast(image, brightness, contrast, sharpness)

        # Resize with scaling
        image = resize_image(image, width, h_scale, v_scale)

        pixels = image.load()
        img_width, img_height = image.size

        ascii_art = []

        if mode == 'full':
            # Simple full blocks - just use █ or space based on brightness
            for y in range(img_height):
                line = ""
                for x in range(img_width):
                    pixel = pixels[x, y]

                    # Check alpha - transparent pixels become spaces
                    if has_alpha and len(pixel) >= 4 and pixel[3] < 128:
                        line += ' '
                        continue

                    brightness_val = get_brightness(pixel)
                    if invert:
                        brightness_val = 255 - brightness_val

                    # Use full block or space based on threshold
                    char = '█' if brightness_val >= threshold else ' '

                    if no_color:
                        line += char
                    else:
                        r, g, b = pixel[:3]
                        color_code = rgb_to_ansi(r, g, b)
                        line += f"{color_code}{char}"

                if not no_color:
                    line += "\033[0m"
                ascii_art.append(line)

        elif mode == 'shade':
            # Use shading blocks for smooth gradients
            for y in range(img_height):
                line = ""
                for x in range(img_width):
                    pixel = pixels[x, y]

                    # Check alpha - transparent pixels become spaces
                    if has_alpha and len(pixel) >= 4 and pixel[3] < 128:
                        line += ' '
                        continue

                    # Get shade character based on brightness
                    brightness_val = get_brightness(pixel)
                    if invert:
                        brightness_val = 255 - brightness_val
                    char = get_shade_char(brightness_val)

                    if no_color:
                        line += char
                    else:
                        r, g, b = pixel[:3]
                        color_code = rgb_to_ansi(r, g, b)
                        line += f"{color_code}{char}"

                if not no_color:
                    line += "\033[0m"
                ascii_art.append(line)

        elif mode == 'block':
            # Process in 2x2 pixel blocks for unicode pattern characters
            # Each character represents a 2x2 pixel area
            for y in range(0, img_height, 2):
                line = ""
                for x in range(0, img_width, 2):
                    # Get 2x2 block of pixels [top-left, top-right, bottom-left, bottom-right]
                    pixels_2x2 = []
                    coords = [(x, y), (x+1, y), (x, y+1), (x+1, y+1)]

                    for px, py in coords:
                        if px < img_width and py < img_height:
                            pixels_2x2.append(pixels[px, py])
                        else:
                            pixels_2x2.append(None)

                    # Get block character based on threshold
                    char = get_block_char(pixels_2x2, threshold, invert, has_alpha)

                    if no_color:
                        line += char
                    else:
                        color = average_color(pixels_2x2)
                        color_code = rgb_to_ansi(*color)
                        line += f"{color_code}{char}"

                if not no_color:
                    line += "\033[0m"
                ascii_art.append(line)
        else:
            # Simple ASCII mode
            for y in range(img_height):
                line = ""
                for x in range(img_width):
                    pixel = pixels[x, y]

                    # Check alpha - transparent pixels become spaces
                    if has_alpha and len(pixel) >= 4 and pixel[3] < 128:
                        line += ' '
                        continue

                    # Get ASCII character based on brightness
                    brightness_val = get_brightness(pixel)
                    if invert:
                        brightness_val = 255 - brightness_val
                    char_index = min(brightness_val * len(ASCII_CHARS) // 256, len(ASCII_CHARS) - 1)
                    char = ASCII_CHARS[char_index]

                    if no_color:
                        line += char
                    else:
                        r, g, b = pixel[:3]
                        color_code = rgb_to_ansi(r, g, b)
                        line += f"{color_code}{char}"

                if not no_color:
                    line += "\033[0m"
                ascii_art.append(line)

        return "\n".join(ascii_art)

    except FileNotFoundError:
        print(f"Error: File '{image_path}' not found.")
        sys.exit(1)
    except Exception as e:
        print(f"Error processing image: {e}")
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(
        description='Convert PNG images to colored ASCII art with terminal color sequences.',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )

    parser.add_argument('image', help='Input image file (PNG, JPG, etc.)')
    parser.add_argument('-w', '--width', type=int, default=100,
                       help='Output width in characters (default: 100)')
    parser.add_argument('--h-scale', type=float, default=1.0,
                       help='Horizontal scale factor (default: 1.0)')
    parser.add_argument('--v-scale', type=float, default=1.0,
                       help='Vertical scale factor (default: 1.0)')
    parser.add_argument('-b', '--brightness', type=int, default=0,
                       help='Brightness adjustment -255 to 255 (default: 0)')
    parser.add_argument('-c', '--contrast', type=float, default=1.0,
                       help='Contrast factor 0.0 to 3.0 (default: 1.0)')
    parser.add_argument('-s', '--sharpness', type=float, default=1.0,
                       help='Sharpness factor 0.0 to 3.0 (default: 1.0, <1=blur, >1=sharpen)')
    parser.add_argument('-m', '--mode', choices=['full', 'block', 'shade', 'ascii'], default='full',
                       help='Rendering mode: full=solid blocks (█), block=2x2 patterns (▀▄▌▐), shade=smooth shading (░▒▓█), ascii=text (default: full)')
    parser.add_argument('-o', '--output', help='Output file (default: <input>_ascii.txt)')
    parser.add_argument('--no-color', '--monochrome', '-mono', action='store_true',
                       dest='no_color',
                       help='Disable ANSI color codes (plain monochrome output)')
    parser.add_argument('-t', '--threshold', type=int, default=128,
                       help='Brightness threshold 0-255 for full/block modes (default: 128)')
    parser.add_argument('-i', '--invert', action='store_true',
                       help='Invert brightness (dark pixels become blocks)')

    args = parser.parse_args()

    # Validate parameters
    if args.brightness < -255 or args.brightness > 255:
        print("Error: Brightness must be between -255 and 255")
        sys.exit(1)

    if args.contrast < 0 or args.contrast > 3.0:
        print("Error: Contrast must be between 0.0 and 3.0")
        sys.exit(1)

    if args.sharpness < 0 or args.sharpness > 3.0:
        print("Error: Sharpness must be between 0.0 and 3.0")
        sys.exit(1)

    if args.threshold < 0 or args.threshold > 255:
        print("Error: Threshold must be between 0 and 255")
        sys.exit(1)

    # Generate ASCII art
    ascii_art = image_to_colored_ascii(
        args.image,
        width=args.width,
        h_scale=args.h_scale,
        v_scale=args.v_scale,
        brightness=args.brightness,
        contrast=args.contrast,
        sharpness=args.sharpness,
        mode=args.mode,
        no_color=args.no_color,
        threshold=args.threshold,
        invert=args.invert
    )

    # Print to terminal
    print(ascii_art)

    # Save to file
    if args.output:
        output_file = args.output
    else:
        output_file = args.image.rsplit('.', 1)[0] + '_ascii.txt'

    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(ascii_art)

    print(f"\n\nSaved to: {output_file}", file=sys.stderr)


if __name__ == "__main__":
    main()
