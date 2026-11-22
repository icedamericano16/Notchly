"""Renders the Notchly mark into the PNG logos the MSIX and sparse packages need.

Pure standard library (zlib + struct), 4x supersampled, so it runs anywhere Python 3 does:

    python packaging/make_assets.py

Writes packaging/msix/Assets/*.png and packaging/sparse/logo.png. The mark matches the
procedural app icon in build.rs: a black pill with a cyan -> blue -> violet glow line.
"""

import os
import struct
import zlib

ROOT = os.path.dirname(os.path.abspath(__file__))
SS = 4  # supersampling factor

STOPS = [(0.0, (25, 184, 255)), (0.55, (45, 91, 255)), (1.0, (123, 75, 255))]


def gradient(t):
    t = max(0.0, min(1.0, t))
    for (t0, c0), (t1, c1) in zip(STOPS, STOPS[1:]):
        if t <= t1:
            u = (t - t0) / (t1 - t0)
            return tuple(c0[i] + (c1[i] - c0[i]) * u for i in range(3))
    return STOPS[-1][1]


def mark_sample(x, y, cx, cy, size):
    """Colour (r, g, b, a) of the mark at a point; `size` is the mark width."""
    hw, hh = size * 0.5, size * 0.32
    r = hh
    dx = max(abs(x - cx) - (hw - r), 0.0)
    dy = max(abs(y - cy) - (hh - r), 0.0)
    d = (dx * dx + dy * dy) ** 0.5 - r
    if d > 0.5:
        return (0, 0, 0, 0)
    a = max(0.0, min(1.0, 0.5 - d))
    # body: near-black with a faint top sheen
    sheen = max(0.0, 1.0 - (y - (cy - hh)) / (hh * 1.2)) * 18
    col = (8 + sheen, 9 + sheen, 12 + sheen)
    # glow line: a capsule along the lower third
    ly = cy + hh * 0.38
    half = hw * 0.52
    lw = max(size * 0.055, 1.0)
    px = max(-half, min(half, x - cx))
    ld = ((x - cx - px) ** 2 + (y - ly) ** 2) ** 0.5 - lw / 2
    if ld < 1.0:
        la = max(0.0, min(1.0, 0.5 - ld))
        g = gradient((x - (cx - half)) / (2 * half))
        col = tuple(col[i] + (g[i] - col[i]) * la for i in range(3))
    # soft glow under the line
    glow = max(0.0, 1.0 - abs(y - ly) / (size * 0.12)) * max(0.0, 1.0 - abs(x - cx) / (half * 1.2))
    if glow > 0:
        g = gradient((x - (cx - half)) / (2 * half))
        col = tuple(col[i] + (g[i] - col[i]) * glow * 0.18 for i in range(3))
    return (col[0], col[1], col[2], a * 255)


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
