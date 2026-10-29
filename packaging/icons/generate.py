#!/usr/bin/env python3
"""Regenerate the package icons from the unmodified lazer logo.

Requires Pillow (`pip install pillow`). Run from anywhere:
    python3 packaging/icons/generate.py
Output is deterministic for a given Pillow version.
"""
import io
import struct
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parents[1] / "apps/osu-radio-qt/assets/logos/lazer.png"
# The source is 1024px, so every size below is a downscale.
PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def scaled(src: Image.Image, size: int) -> Image.Image:
    # Pillow resizes RGBA with premultiplied alpha, so edges get no dark halo.
    return src.resize((size, size), Image.LANCZOS)


def ico_frame(img: Image.Image) -> bytes:
    size = img.width
    if size >= 256:  # Windows stores 256px frames as PNG
        buf = io.BytesIO()
        img.save(buf, "PNG", optimize=True)
        return buf.getvalue()
    # BMP frames (32-bit BGRA, bottom-up, plus 1-bit AND mask) read everywhere.
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    px = img.convert("RGBA").tobytes("raw", "BGRA")
    rows = [px[y * size * 4:(y + 1) * size * 4] for y in range(size)][::-1]
    mask_row = b"\x00" * (((size + 31) // 32) * 4)  # alpha channel carries transparency
    return header + b"".join(rows) + mask_row * size


def write_ico(src: Image.Image, path: Path) -> None:
    frames = [ico_frame(scaled(src, s)) for s in ICO_SIZES]
    out = struct.pack("<HHH", 0, 1, len(frames))
    offset = 6 + 16 * len(frames)
    for s, data in zip(ICO_SIZES, frames):
        dim = 0 if s >= 256 else s
        out += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    path.write_bytes(out + b"".join(frames))


def main() -> None:
    src = Image.open(SOURCE).convert("RGBA")
    assert src.size == (1024, 1024), src.size
    for s in PNG_SIZES:
        d = HERE / "hicolor" / f"{s}x{s}" / "apps"
        d.mkdir(parents=True, exist_ok=True)
        scaled(src, s).save(d / "osu-radio.png", optimize=True)
    write_ico(src, HERE / "osu-radio.ico")


if __name__ == "__main__":
    main()
