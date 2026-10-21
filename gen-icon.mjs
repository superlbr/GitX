// Generates the GitCraft app icon (512x512 PNG) without external dependencies.
//
// The design is a commit graph whose two lanes cross into an "X" — the glyph
// says both "git history" and the X in GitCraft. Everything is rasterized here
// with signed-distance functions (rounded-square plate, gradient, two stroke
// segments, five commit nodes), so anti-aliasing comes from the SDF itself
// and there is still nothing to install. Run: node gen-icon.mjs
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

const SIZE = 512;

// CRC32 table
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const body = Buffer.concat([typeBuf, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

// --- SDF helpers (all distances in icon pixels) -----------------------------

const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
const mix = (a, b, t) => a + (b - a) * t;
// 1 inside the shape, 0 outside, ramping over `aa` pixels around the edge.
const cover = (d, aa) => clamp(0.5 - d / aa, 0, 1);

function roundedBox(x, y, half, radius) {
  const qx = Math.abs(x) - (half - radius);
  const qy = Math.abs(y) - (half - radius);
  const ox = Math.max(qx, 0);
  const oy = Math.max(qy, 0);
  return Math.sqrt(ox * ox + oy * oy) + Math.min(Math.max(qx, qy), 0) - radius;
}

function segment(x, y, ax, ay, bx, by, halfWidth) {
  const abx = bx - ax;
  const aby = by - ay;
  const apx = x - ax;
  const apy = y - ay;
  const t = clamp((apx * abx + apy * aby) / (abx * abx + aby * aby), 0, 1);
  const dx = apx - abx * t;
  const dy = apy - aby * t;
  return Math.sqrt(dx * dx + dy * dy) - halfWidth;
}

function circle(x, y, cx, cy, radius) {
  const dx = x - cx;
  const dy = y - cy;
  return Math.sqrt(dx * dx + dy * dy) - radius;
}

// --- palette -----------------------------------------------------------------
// Brand purple, eased toward white at the top-left and deep at the bottom-right
// so the plate reads as one surface instead of a flat fill.
const LIGHT = [150, 128, 255];
const BRAND = [124, 92, 255];
const DEEP = [88, 60, 216];
const GLYPH = [255, 255, 255];

// --- geometry -----------------------------------------------------------------
// Two lanes crossing at the center; end nodes sit slightly inside so the dots
// never fight the plate's rounded corners. Coordinates are centered on the
// plate (x/y in the loop below are too), so ±(HALF - INSET) keeps the ends
// INSET pixels away from the plate edge.
const HALF = SIZE / 2;
const INSET = 152;
const P = HALF - INSET; // 104
const A = [-P, P, P, -P]; // bottom-left → top-right
const B = [P, P, -P, -P]; // bottom-right → top-left
const STROKE = 23;
const END_NODE = 33;
const MERGE_NODE = 43;

// --- rasterize ----------------------------------------------------------------
const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
let o = 0;
for (let py = 0; py < SIZE; py++) {
  raw[o++] = 0; // PNG filter: none
  for (let px = 0; px < SIZE; px++) {
    // Icon-space coordinates centered on the plate.
    const x = px + 0.5 - HALF;
    const y = py + 0.5 - HALF;

    const plate = roundedBox(x, y, HALF, 116);
    const plateA = cover(plate, 1.5);

    // Diagonal gradient, lighter at the top-left, plus a soft top-left glow.
    const g = clamp((px + py) / (2 * SIZE), 0, 1);
    let r = mix(LIGHT[0], DEEP[0], g);
    let gg = mix(LIGHT[1], DEEP[1], g);
    let b = mix(LIGHT[2], DEEP[2], g);
    const glow = clamp(1 - Math.sqrt((x + 140) ** 2 + (y + 140) ** 2) / 460, 0, 1) * 0.16;
    r = mix(r, 255, glow);
    gg = mix(gg, 255, glow);
    b = mix(b, 255, glow);

    // The X: two strokes and their commit nodes, unioned by min().
    const glyph = Math.min(
      segment(x, y, A[0], A[1], A[2], A[3], STROKE),
      segment(x, y, B[0], B[1], B[2], B[3], STROKE),
      circle(x, y, A[0], A[1], END_NODE),
      circle(x, y, A[2], A[3], END_NODE),
      circle(x, y, B[0], B[1], END_NODE),
      circle(x, y, B[2], B[3], END_NODE),
      circle(x, y, 0, 0, MERGE_NODE),
    );
    const glyphA = cover(glyph, 1.5);

    r = mix(r, GLYPH[0], glyphA);
    gg = mix(gg, GLYPH[1], glyphA);
    b = mix(b, GLYPH[2], glyphA);

    raw[o++] = Math.round(r);
    raw[o++] = Math.round(gg);
    raw[o++] = Math.round(b);
    raw[o++] = Math.round(255 * plateA);
  }
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const png = Buffer.concat([
  sig,
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);

mkdirSync("assets", { recursive: true });
writeFileSync("assets/icon.png", png);
console.log("wrote assets/icon.png", png.length, "bytes");
