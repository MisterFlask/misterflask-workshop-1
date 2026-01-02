#!/usr/bin/env python3
"""
Sprite generation tool for FFH Simplified.

Reads .sprite files from assets/sprite_definitions/ and generates PNG files
to assets/sprites/.

Sprite file format:
    name: sprite_name
    size: 32x32
    palette:
      .: transparent
      x: #000000
      a: #ff0000
    pixels: |
      ................................
      ......xxxxxx....................
      (32 rows of 32 characters each)
"""

import os
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Pillow not installed. Install with: pip install Pillow")
    sys.exit(1)


def parse_sprite_file(filepath: Path) -> dict:
    """Parse a .sprite file and return its data."""
    with open(filepath, 'r') as f:
        content = f.read()

    result = {
        'name': '',
        'size': (32, 32),
        'palette': {},
        'pixels': []
    }

    lines = content.strip().split('\n')
    i = 0
    in_pixels = False
    in_palette = False

    while i < len(lines):
        line = lines[i]

        if in_pixels:
            # Reading pixel data
            stripped = line.strip()
            if stripped and not stripped.startswith('#'):
                result['pixels'].append(stripped)
            i += 1
            continue

        if in_palette:
            # Reading palette entries
            stripped = line.strip()
            if stripped.startswith('pixels:'):
                in_palette = False
                in_pixels = True
                i += 1
                continue
            if ':' in stripped and not stripped.startswith('#'):
                parts = stripped.split(':')
                if len(parts) == 2:
                    char = parts[0].strip()
                    color = parts[1].strip()
                    result['palette'][char] = color
            i += 1
            continue

        if line.startswith('name:'):
            result['name'] = line.split(':', 1)[1].strip()
        elif line.startswith('size:'):
            size_str = line.split(':', 1)[1].strip()
            w, h = size_str.split('x')
            result['size'] = (int(w), int(h))
        elif line.startswith('palette:'):
            in_palette = True
        elif line.startswith('pixels:'):
            in_pixels = True

        i += 1

    return result


def color_to_rgba(color_str: str) -> tuple:
    """Convert a color string to RGBA tuple."""
    if color_str == 'transparent':
        return (0, 0, 0, 0)

    if color_str.startswith('#'):
        hex_color = color_str[1:]
        if len(hex_color) == 6:
            r = int(hex_color[0:2], 16)
            g = int(hex_color[2:4], 16)
            b = int(hex_color[4:6], 16)
            return (r, g, b, 255)
        elif len(hex_color) == 8:
            r = int(hex_color[0:2], 16)
            g = int(hex_color[2:4], 16)
            b = int(hex_color[4:6], 16)
            a = int(hex_color[6:8], 16)
            return (r, g, b, a)

    return (255, 0, 255, 255)  # Magenta for errors


def generate_sprite(sprite_data: dict, output_path: Path):
    """Generate a PNG from sprite data."""
    width, height = sprite_data['size']
    palette = sprite_data['palette']
    pixels = sprite_data['pixels']

    img = Image.new('RGBA', (width, height), (0, 0, 0, 0))

    for y, row in enumerate(pixels):
        if y >= height:
            break
        for x, char in enumerate(row):
            if x >= width:
                break
            if char in palette:
                color = color_to_rgba(palette[char])
                img.putpixel((x, y), color)

    # Ensure output directory exists
    output_path.parent.mkdir(parents=True, exist_ok=True)

    img.save(output_path, 'PNG')
    print(f"Generated: {output_path}")


def main():
    # Find project root
    script_dir = Path(__file__).parent
    project_root = script_dir.parent

    definitions_dir = project_root / 'assets' / 'sprite_definitions'
    output_dir = project_root / 'assets' / 'sprites'

    if not definitions_dir.exists():
        print(f"No sprite definitions directory found at {definitions_dir}")
        print("Creating directory...")
        definitions_dir.mkdir(parents=True, exist_ok=True)
        return

    # Find all .sprite files
    sprite_files = list(definitions_dir.rglob('*.sprite'))

    if not sprite_files:
        print(f"No .sprite files found in {definitions_dir}")
        return

    print(f"Found {len(sprite_files)} sprite definition(s)")

    for sprite_file in sprite_files:
        try:
            sprite_data = parse_sprite_file(sprite_file)

            # Determine output path (preserve subdirectory structure)
            rel_path = sprite_file.relative_to(definitions_dir)
            output_path = output_dir / rel_path.with_suffix('.png')

            generate_sprite(sprite_data, output_path)

        except Exception as e:
            print(f"Error processing {sprite_file}: {e}")


if __name__ == '__main__':
    main()
