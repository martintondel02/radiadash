#!/usr/bin/env python3
"""Generate the Radiadash app icons (RGBA PNG, no external dependencies)."""
import struct, zlib, math, pathlib

BG = (10, 13, 16, 255)
RING = (79, 209, 197, 255)
DIM = (31, 111, 106, 180)

def blend(dst, src):
    a = src[3] / 255
    return tuple(int(src[i] * a + dst[i] * (1 - a)) for i in range(3)) + (255,)

def pixel(x, y, size):
    cx = cy = size / 2
    dx, dy = x - cx, y - cy
    r = math.hypot(dx, dy)
    ang = math.atan2(dy, dx) % (2 * math.pi)

    # rounded-square background
    radius = size * 0.42
    corner = size * 0.22
    inside = max(abs(dx), abs(dy)) < radius or math.hypot(max(abs(dx) - (radius - corner), 0), max(abs(dy) - (radius - corner), 0)) < corner
    if not inside:
        return (0, 0, 0, 0)
    color = BG

    if size * 0.20 < r < size * 0.235:
        color = blend(color, RING)
    if size * 0.055 < r < size * 0.085:
        color = blend(color, RING)
    if r <= size * 0.055:
        color = blend(color, RING)

    # three trefoil wedges
    for k in range(3):
        start = k * 2 * math.pi / 3 + math.pi / 2
        delta = (ang - start) % (2 * math.pi)
        if size * 0.26 < r < size * 0.36 and abs(delta - math.pi / 3) < 0.42:
            color = blend(color, DIM)
    return color

def render(size, ss=4):
    rows = []
    for y in range(size):
        row = bytearray()
        for x in range(size):
            acc = [0, 0, 0, 0]
            for sy in range(ss):
                for sx in range(ss):
                    px = pixel(x + (sx + 0.5) / ss, y + (sy + 0.5) / ss, size)
                    for i in range(4):
                        acc[i] += px[i]
            n = ss * ss
            row.extend(int(v / n) for v in acc)
        rows.append(bytes(row))
    return rows

def write_png(path, size):
    raw = b"".join(b"\x00" + row for row in render(size))
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    pathlib.Path(path).write_bytes(png)

if __name__ == "__main__":
    icons = pathlib.Path(__file__).resolve().parent.parent / "apps/desktop/src-tauri/icons"
    icons.mkdir(parents=True, exist_ok=True)
    for name, size in [("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)]:
        write_png(icons / name, size)
        print("wrote", icons / name)
