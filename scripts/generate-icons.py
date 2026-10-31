"""Generate the app icon from the exact CreeperMark used in the UI.

The splash screen, titlebar, account card and launch card all render the same
16x16 creeper sprite. The Windows icon is generated from this sprite instead of
using a separate badge design, so the project keeps one visual identity from
the first paint down to the taskbar.
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ICON_DIR = ROOT / "src-tauri" / "icons"
PUBLIC = ROOT / "public"
DOCS = ROOT / "docs" / "screenshots"

# Keep these values in sync with src/components/CreeperMark.tsx.
TEXTURE = [
    "abacbbab",
    "bacabbca",
    "abbbacba",
    "cababbbc",
    "bbaabacb",
    "acbbabac",
    "bbacabba",
    "ababbcab",
]

SHADES = {
    "a": (125, 182, 75),
    "b": (139, 199, 90),
    "c": (107, 163, 63),
    "d": (155, 213, 107),
}

BASE = (10, 18, 12)
FACE_COLOR = (11, 20, 13)
FACE = [
    (2, 4), (3, 4), (4, 4), (5, 4),
    (2, 5), (3, 5), (4, 5), (5, 5),
    (10, 4), (11, 4), (12, 4), (13, 4),
    (10, 5), (11, 5), (12, 5), (13, 5),
    (6, 8), (7, 8), (8, 8), (9, 8),
    (4, 9), (5, 9), (6, 9), (7, 9),
    (8, 9), (9, 9), (10, 9), (11, 9),
    (4, 10), (5, 10), (6, 10), (7, 10),
    (8, 10), (9, 10), (10, 10), (11, 10),
    (4, 11), (5, 11), (6, 11), (7, 11),
    (8, 11), (9, 11), (10, 11), (11, 11),
    (4, 12), (5, 12), (10, 12), (11, 12),
    (4, 13), (5, 13), (10, 13), (11, 13),
]

SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def blend(base, color, alpha):
    return tuple(round(base[i] * (1 - alpha) + color[i] * alpha) for i in range(3))


def build_master():
    """Draw the same 8x8 sprite and 16-unit face geometry at 1024 px."""
    size = 1024
    image = Image.new("RGB", (size, size), BASE)
    draw = ImageDraw.Draw(image)
    cell = size / 8

    for y in range(8):
        for x in range(8):
            opacity = 0.86 if x == 0 or y == 0 else 0.92 if x == 7 or y == 7 else 1.0
            color = blend(BASE, SHADES[TEXTURE[y][x]], opacity)
            x0, y0 = x * cell, y * cell
            draw.rectangle((x0, y0, x0 + cell - 1, y0 + cell - 1), fill=color)

            # CreeperMark adds a faint lighter speck on every 17th texture cell.
            if (x + y * 3) % 17 == 0:
                x0, y0 = x * cell, y * cell
                draw.rectangle(
                    (x0, y0, x0 + cell - 1, y0 + cell - 1),
                    fill=blend(color, SHADES["d"], 0.2),
                )

    # FACE uses the 16-unit viewBox coordinates from the React component.
    unit = size / 16
    for x, y in FACE:
        draw.rectangle(
            (x * unit, y * unit, (x + 2) * unit - 1, (y + 2) * unit - 1),
            fill=FACE_COLOR,
        )

    return image


def make_sizes(master):
    # Nearest keeps the same pixel boundaries as the SVG's crispEdges rendering.
    return {
        size: master.resize((size, size), Image.Resampling.NEAREST)
        for size in SIZES
    }


def write_outputs(master, frames):
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    PUBLIC.mkdir(parents=True, exist_ok=True)
    DOCS.mkdir(parents=True, exist_ok=True)

    master.save(ICON_DIR / "icon.png")
    master.save(PUBLIC / "creeper-icon.png")
    master.save(DOCS / "icon.png")

    for size in (32, 48, 64, 128, 256):
        frames[size].save(ICON_DIR / f"{size}x{size}.png")
    frames[256].save(ICON_DIR / "128x128@2x.png")
    frames[128].save(ICON_DIR / "128x128.png")

    icon = frames[256].copy()
    icon.save(
        ICON_DIR / "icon.ico",
        format="ICO",
        sizes=[(size, size) for size in SIZES],
        append_images=[frames[size] for size in SIZES],
    )

    # Light contact sheet, matching the app background.
    tile = 136
    gap = 16
    preview = Image.new(
        "RGB",
        (len(SIZES[:6]) * (tile + gap) + gap, tile + gap * 2),
        (243, 245, 241),
    )
    x = gap
    for size in SIZES[:6]:
        enlarged = frames[size].resize((tile, tile), Image.Resampling.NEAREST)
        preview.paste(enlarged, (x, gap))
        x += tile + gap
    preview.save(DOCS / "icon-preview.png")


if __name__ == "__main__":
    master = build_master()
    frames = make_sizes(master)
    write_outputs(master, frames)
    print("wrote unified creeper icons")
