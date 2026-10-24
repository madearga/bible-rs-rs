#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 madearga
"""Renders the macOS application icon from the bundled Literata font.

Constraints: needs Pillow and iconutil, reads assets/fonts/Literata.ttf, and
writes only under assets/icons/. It never runs cargo and never touches dist/.
Run from the repository root: python3 assets/icons/make-icons.py
"""

from __future__ import annotations

import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

REPO = Path(__file__).resolve().parent.parent.parent
FONT_PATH = REPO / "assets" / "fonts" / "Literata.ttf"
ICONS_DIR = REPO / "assets" / "icons"
ICONSET_DIR = ICONS_DIR / "AppIcon.iconset"
ICNS_PATH = ICONS_DIR / "AppIcon.icns"

MASTER = 1024
# The tile is the rounded square itself; macOS wants transparent margins
# around it so the system can place and scale the icon.
TILE = 824
CORNER_RADIUS = round(TILE * 0.22)

PAPER_TOP = (246, 241, 231)  # #F6F1E7
PAPER_BOTTOM = (239, 231, 216)  # #EFE7D8
INK = (34, 29, 23)  # #221D17
ACCENT = (138, 85, 37)  # #8A5525

# Cap height of the letter, as a fraction of the tile. Large enough to read at
# 16x16, small enough to keep clear space inside the tile.
CAP_HEIGHT_FRAC = 0.57
BAR_WIDTH_FRAC = 0.18
BAR_HEIGHT_FRAC = 0.024
GAP_FRAC = 0.032


def tile_image() -> Image.Image:
    """Builds the rounded-square paper tile with a subtle vertical gradient."""
    gradient = Image.new("RGB", (TILE, TILE))
    gradient_draw = ImageDraw.Draw(gradient)
    for y in range(TILE):
        t = y / (TILE - 1)
        row = tuple(
            round(top + (bottom - top) * t)
            for top, bottom in zip(PAPER_TOP, PAPER_BOTTOM)
        )
        gradient_draw.line((0, y, TILE, y), fill=row)

    mask = Image.new("L", (TILE, TILE), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, TILE - 1, TILE - 1), radius=CORNER_RADIUS, fill=255
    )

    tile = Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0))
    tile.paste(gradient, (0, 0), mask)
    return tile


def font_for_cap_height(target: int) -> tuple[ImageFont.FreeTypeFont, int, int]:
    """Finds the font size whose capital-B height matches the target."""
    low, high = 1, MASTER * 4
    best: tuple[ImageFont.FreeTypeFont, int, int] | None = None
    while low <= high:
        size = (low + high) // 2
        font = ImageFont.truetype(str(FONT_PATH), size)
        x0, y0, x1, y1 = font.getbbox("B")
        cap = y1 - y0
        if cap < target:
            low = size + 1
        elif cap > target:
            best = (font, x0, y0)
            high = size - 1
        else:
            return font, x0, y0
    assert best is not None
    return best


def render_master() -> Image.Image:
    tile = tile_image()

    cap_target = round(TILE * CAP_HEIGHT_FRAC)
    font, bbox_x0, bbox_y0 = font_for_cap_height(cap_target)
    bbox_x1, bbox_y1 = font.getbbox("B")[2], font.getbbox("B")[3]
    glyph_w = bbox_x1 - bbox_x0
    glyph_h = bbox_y1 - bbox_y0

    bar_w = round(TILE * BAR_WIDTH_FRAC)
    bar_h = round(TILE * BAR_HEIGHT_FRAC)
    gap = round(TILE * GAP_FRAC)

    # Center the letter-plus-rule group inside the tile. The bar is narrower
    # than the letter, so both are centered on the tile's vertical axis.
    group_h = glyph_h + gap + bar_h
    group_top = (TILE - group_h) // 2
    glyph_left = (TILE - glyph_w) // 2 - bbox_x0
    glyph_top = group_top - bbox_y0

    draw = ImageDraw.Draw(tile)
    draw.text((glyph_left, glyph_top), "B", font=font, fill=INK)

    bar_left = (TILE - bar_w) // 2
    bar_top = group_top + glyph_h + gap
    draw.rounded_rectangle(
        (bar_left, bar_top, bar_left + bar_w, bar_top + bar_h),
        radius=bar_h // 2,
        fill=ACCENT,
    )

    master = Image.new("RGBA", (MASTER, MASTER), (0, 0, 0, 0))
    offset = (MASTER - TILE) // 2
    master.paste(tile, (offset, offset), tile)
    return master


SIZES = [16, 32, 128, 256, 512]


def main() -> None:
    master = render_master()
    ICONSET_DIR.mkdir(parents=True, exist_ok=True)
    master.save(ICONSET_DIR / "icon_512x512@2x.png")

    # Every size comes from the 1024 master with Lanczos resampling, so the
    # small sizes keep the letter's edges as clean as the format allows.
    for size in SIZES:
        for scale, suffix in ((1, ""), (2, "@2x")):
            target = size * scale
            master.resize((target, target), Image.LANCZOS).save(
                ICONSET_DIR / f"icon_{size}x{size}{suffix}.png"
            )

    subprocess.run(
        ["iconutil", "-c", "icns", str(ICONSET_DIR), "-o", str(ICNS_PATH)],
        check=True,
    )
    print(f"wrote {ICNS_PATH} and {ICONSET_DIR}")


if __name__ == "__main__":
    main()
