"""Measure where ink actually is in the SourceTree reference screenshots.

Read-only: decodes a PNG with zlib + the five scanline filters, then reports the
x-extents of dark/light runs inside a y band. Coordinates printed are in
*logical points* (the screenshots are @2x), so divide nothing further.
"""
import struct
import sys
import zlib


def load(path):
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a png"
    pos, idat, w = 8, bytearray(), None
    while pos < len(data):
        (ln,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + ln]
        if kind == b"IHDR":
            w, h, depth, color = struct.unpack(">IIBB", chunk[:10])
            assert depth == 8 and color in (2, 6), (depth, color)
            ch = 3 if color == 2 else 4
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
        pos += 12 + ln
    raw = zlib.decompress(bytes(idat))
    stride = w * ch
    rows = []
    prev = bytearray(stride)
    i = 0
    for _ in range(h):
        ft = raw[i]
        line = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        if ft == 1:
            for x in range(ch, stride):
                line[x] = (line[x] + line[x - ch]) & 255
        elif ft == 2:
            for x in range(stride):
                line[x] = (line[x] + prev[x]) & 255
        elif ft == 3:
            for x in range(stride):
                a = line[x - ch] if x >= ch else 0
                line[x] = (line[x] + ((a + prev[x]) >> 1)) & 255
        elif ft == 4:
            for x in range(stride):
                a = line[x - ch] if x >= ch else 0
                b = prev[x]
                c = prev[x - ch] if x >= ch else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        rows.append(line)
        prev = line
    return w, h, ch, rows


def lum(rows, ch, x, y):
    o = x * ch
    r, g, b = rows[y][o], rows[y][o + 1], rows[y][o + 2]
    return (r * 30 + g * 59 + b * 11) // 100


def runs(path, y0, y1, scale=2.0, thresh=70):
    w, h, ch, rows = load(path)
    cols = []
    for x in range(w):
        ink = any(lum(rows, ch, x, y) < 255 - thresh
                  for y in range(y0 * 2, y1 * 2) if y < h)
        cols.append(ink)
    out, start = [], None
    for x, ink in enumerate(cols):
        if ink and start is None:
            start = x
        elif not ink and start is not None:
            out.append((round(start / scale, 1), round((x - 1) / scale, 1)))
            start = None
    if start is not None:
        out.append((round(start / scale, 1), round((w - 1) / scale, 1)))
    print(f"--- {path.split('/')[-1]}  y {y0}..{y1}  (logical pt, x from left)")
    for a, b in out:
        print(f"  {a:7.1f} .. {b:7.1f}   w={round(b - a, 1):6.1f}")


if __name__ == "__main__":
    runs(sys.argv[1], int(sys.argv[2]), int(sys.argv[3]))
