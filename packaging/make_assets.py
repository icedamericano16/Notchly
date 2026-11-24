"""Renders the Notchly logo into the PNG logos the MSIX and sparse packages need.

Pure standard library (zlib + struct), 4x supersampled, so it runs anywhere Python 3 does:

    python packaging/make_assets.py

Writes packaging/msix/Assets/*.png and packaging/sparse/logo.png. The logo is
assets/brand/notchly-logo.svg (a violet squircle, a black notch and two eyes); build.rs
draws the same mark for the app icon.
"""

import os
import struct
import zlib

ROOT = os.path.dirname(os.path.abspath(__file__))
SS = 4  # supersampling factor


def in_rrect(x, y, x0, y0, w, h, r):
    if x < x0 or x > x0 + w or y < y0 or y > y0 + h:
        return False
    cx = min(max(x, x0 + r), x0 + w - r)
    cy = min(max(y, y0 + r), y0 + h - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def logo(x, y):
    """Colour (r, g, b) of the logo at a point in its 1024x1024 design space, or None."""
    u, v = abs((x - 512) / 512), abs((y - 512) / 512)
    if u**7 + v**7 > 1:  # squircle (superellipse matching the SVG's corners)
        return None
    if in_rrect(x, y, 364, 383.32, 84, 114, 42) or in_rrect(x, y, 576, 383.32, 84, 114, 42):
        return (233, 252, 248)  # eyes #E9FCF8
    if in_rrect(x, y, 123, 302.82, 778, 275, 137.5):
        return (8, 8, 12)  # notch #08080C
    t = max(0.0, min(1.0, (x + y) / 2048))  # gradient #8438FF -> #5420FF
    return (132 - 48 * t, 56 - 24 * t, 255)


def mark_sample(x, y, cx, cy, size):
    """Colour (r, g, b, a) of the logo drawn `size` wide, centred on (cx, cy)."""
    lx = (x - (cx - size / 2)) / size * 1024
    ly = (y - (cy - size / 2)) / size * 1024
    c = logo(lx, ly)
    return (0, 0, 0, 0) if c is None else (c[0], c[1], c[2], 255)


def render(w, h, mark_w):
    W, H = w * SS, h * SS
    cx, cy, size = W / 2, H / 2, mark_w * SS
    rows = []
    for py in range(h):
        row = bytearray([0])  # filter: none
        for px in range(w):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(SS):
                for sx in range(SS):
                    r, g, b, a = mark_sample(px * SS + sx + 0.5, py * SS + sy + 0.5, cx, cy, size)
                    acc[0] += r * a
                    acc[1] += g * a
                    acc[2] += b * a
                    acc[3] += a
            n = SS * SS
            a = acc[3] / n
            if a > 0:
                row += bytes(
                    [
                        int(min(255, acc[0] / acc[3])),
                        int(min(255, acc[1] / acc[3])),
                        int(min(255, acc[2] / acc[3])),
                        int(min(255, a)),
                    ]
                )
            else:
                row += b"\0\0\0\0"
        rows.append(bytes(row))
    return png(w, h, b"".join(rows))


def png(w, h, raw):
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


ASSETS = [
    # (path, width, height, mark width)
    ("msix/Assets/StoreLogo.png", 50, 50, 44),
    ("msix/Assets/Square44x44Logo.png", 44, 44, 40),
    ("msix/Assets/Square44x44Logo.targetsize-24_altform-unplated.png", 24, 24, 23),
    ("msix/Assets/Square44x44Logo.targetsize-48_altform-unplated.png", 48, 48, 46),
    ("msix/Assets/Square44x44Logo.targetsize-256_altform-unplated.png", 256, 256, 244),
    ("msix/Assets/Square71x71Logo.png", 71, 71, 56),
    ("msix/Assets/Square150x150Logo.png", 150, 150, 110),
    ("msix/Assets/Wide310x150Logo.png", 310, 150, 132),
    ("sparse/logo.png", 150, 150, 110),
]

if __name__ == "__main__":
    for rel, w, h, m in ASSETS:
        path = os.path.join(ROOT, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as f:
            f.write(render(w, h, m))
        print(f"wrote {rel} ({w}x{h})")
