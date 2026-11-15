// Generates the source app icon (1024x1024 PNG) without any external
// dependencies. Run `npm run icons` to regenerate every Tauri icon size.
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const SIZE = 1024;

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function mix(a, b, t) {
  return a + (b - a) * t;
}

function roundedRectDistance(x, y, cx, cy, halfW, halfH, radius) {
  const dx = Math.abs(x - cx) - (halfW - radius);
  const dy = Math.abs(y - cy) - (halfH - radius);
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
  const inside = Math.min(Math.max(dx, dy), 0);
  return outside + inside - radius;
}

function segmentDistance(px, py, ax, ay, bx, by) {
  const dx = bx - ax;
  const dy = by - ay;
  const lengthSq = dx * dx + dy * dy;
  const t = lengthSq === 0 ? 0 : clamp(((px - ax) * dx + (py - ay) * dy) / lengthSq, 0, 1);
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}

// The "W" badge: three segments forming a W, plus the little dot from the logo.
const strokeWidth = 46;
const segments = [
  [300, 405, 404, 660],
  [404, 660, 512, 470],
  [512, 470, 620, 660],
  [620, 660, 724, 405],
];

const pixels = Buffer.alloc(SIZE * SIZE * 4);

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const index = (y * SIZE + x) * 4;

    // Outer squircle
    const outer = roundedRectDistance(x, y, 512, 512, 468, 468, 210);
    const outerAlpha = clamp(0.5 - outer, 0, 1);

    // Background gradient
    const bgT = y / SIZE;
    let r = mix(26, 11, bgT);
    let g = mix(31, 14, bgT);
    let b = mix(43, 21, bgT);

    // Inner glowing tile
    const inner = roundedRectDistance(x, y, 512, 512, 344, 344, 118);
    const innerAlpha = clamp(0.5 - inner, 0, 1);
    if (innerAlpha > 0) {
      const t = clamp((x - 168 + (y - 168)) / 688, 0, 1);
      const r2 = mix(126, 132, t);
      const g2 = mix(158, 104, t);
      const b2 = mix(255, 255, t);
      const highlight = clamp(1 - Math.hypot(x - 380, y - 330) / 620, 0, 1) * 0.35;
      r = mix(r, clamp(r2 + highlight * 255, 0, 255), innerAlpha);
      g = mix(g, clamp(g2 + highlight * 255, 0, 255), innerAlpha);
      b = mix(b, clamp(b2 + highlight * 255, 0, 255), innerAlpha);
    }

    // W mark
    let markDist = Infinity;
    for (const [ax, ay, bx, by] of segments) {
      markDist = Math.min(markDist, segmentDistance(x, y, ax, ay, bx, by));
    }
    const markAlpha = clamp(0.5 + (strokeWidth / 2 - markDist) / 1.6, 0, 1);
    const dotDist = Math.hypot(x - 512, y - 296);
    const dotAlpha = clamp(0.5 + (30 - dotDist) / 1.6, 0, 1);
    const white = Math.max(markAlpha, dotAlpha);
    r = mix(r, 255, white);
    g = mix(g, 255, white);
    b = mix(b, 255, white);

    // Subtle inner top highlight on the outer squircle
    const sheen = clamp(1 - Math.abs(y - 120) / 220, 0, 1) * 0.06 * outerAlpha;
    r = mix(r, 255, sheen);
    g = mix(g, 255, sheen);
    b = mix(b, 255, sheen);

    pixels[index] = Math.round(r);
    pixels[index + 1] = Math.round(g);
    pixels[index + 2] = Math.round(b);
    pixels[index + 3] = Math.round(outerAlpha * 255);
  }
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const typeBuffer = Buffer.from(type, "ascii");
  const crc = crc32(Buffer.concat([typeBuffer, data]));
  const crcBuffer = Buffer.alloc(4);
  crcBuffer.writeUInt32BE(crc >>> 0, 0);
  return Buffer.concat([length, typeBuffer, data, crcBuffer]);
}

const crcTable = new Int32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c;
});

function crc32(buffer) {
  let crc = -1;
  for (const byte of buffer) crc = (crc >>> 8) ^ crcTable[(crc ^ byte) & 0xff];
  return crc ^ -1;
}

const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0;
  pixels.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8;
ihdr[9] = 6;
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

const target = "src-tauri/icons/source.png";
mkdirSync(dirname(target), { recursive: true });
writeFileSync(target, png);
console.log(`Wrote ${target} (${(png.length / 1024).toFixed(1)} KB)`);
