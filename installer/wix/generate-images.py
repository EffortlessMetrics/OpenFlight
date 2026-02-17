#!/usr/bin/env python3
"""
Generate placeholder BMP images for WiX installer.

This script creates the required banner.bmp (493x58) and dialog.bmp (374x316)
placeholder images with a simple branded appearance.

Requirements:
- Python 3.6+
- Pillow: pip install Pillow

Usage:
    python generate-images.py
"""

import struct
import sys


def create_bmp(width: int, height: int, filename: str, bg_color: tuple, text: str = "") -> None:
    """
    Create a simple BMP file with a solid background color.

    Args:
        width: Image width in pixels
        height: Image height in pixels
        filename: Output filename
        bg_color: Background color as (R, G, B) tuple
        text: Optional text (ignored in this simple implementation)
    """
    # BMP files store rows bottom-to-top, and each row is padded to 4-byte boundary
    row_size = (width * 3 + 3) & ~3  # 24-bit (3 bytes per pixel), padded to 4 bytes
    pixel_data_size = row_size * height
    file_size = 54 + pixel_data_size  # 54 byte header + pixel data

    with open(filename, 'wb') as f:
        # BMP Header (14 bytes)
        f.write(b'BM')                          # Signature
        f.write(struct.pack('<I', file_size))   # File size
        f.write(struct.pack('<HH', 0, 0))       # Reserved
        f.write(struct.pack('<I', 54))          # Pixel data offset

        # DIB Header (BITMAPINFOHEADER - 40 bytes)
        f.write(struct.pack('<I', 40))          # DIB header size
        f.write(struct.pack('<i', width))       # Width
        f.write(struct.pack('<i', height))      # Height (positive = bottom-up)
        f.write(struct.pack('<H', 1))           # Color planes
        f.write(struct.pack('<H', 24))          # Bits per pixel
        f.write(struct.pack('<I', 0))           # Compression (none)
        f.write(struct.pack('<I', pixel_data_size))  # Image size
        f.write(struct.pack('<i', 2835))        # X pixels per meter (72 DPI)
        f.write(struct.pack('<i', 2835))        # Y pixels per meter (72 DPI)
        f.write(struct.pack('<I', 0))           # Colors in color table
        f.write(struct.pack('<I', 0))           # Important colors

        # Pixel data (BGR format, bottom-to-top)
        r, g, b = bg_color
        row = bytes([b, g, r] * width)
        padding = bytes(row_size - width * 3)

        for _ in range(height):
            f.write(row + padding)

    print(f"Created {filename} ({width}x{height})")


def main():
    # Flight Hub brand colors
    PRIMARY_BLUE = (41, 98, 255)      # Vibrant blue
    DARK_BLUE = (26, 62, 161)         # Darker blue for dialog

    # Banner: 493x58 - shown at top of installer pages
    create_bmp(493, 58, "banner.bmp", PRIMARY_BLUE)

    # Dialog: 374x316 - shown on welcome/exit pages (left side)
    create_bmp(374, 316, "dialog.bmp", DARK_BLUE)

    print("\nPlaceholder images created successfully.")
    print("For a professional installer, replace these with proper branded images:")
    print("  - banner.bmp: 493x58 pixels, displayed at top of installer dialogs")
    print("  - dialog.bmp: 374x316 pixels, displayed on welcome/finish pages")


if __name__ == "__main__":
    main()
