// Draws the icon of the VS Code extension (a faceted gem on a rounded square) as a PNG,
// without dependencies:  node scripts/icon.mjs editors/vscode/icon.png
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
const SIZE = 256, SS = 4, N = SIZE * SS;
const hex = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
const A = [80, 80], B = [128, 80], C = [176, 80], L = [42, 118], M1 = [102, 118], M2 = [154, 118], R = [214, 118], P = [128, 214];
const facets = [
  [[L, A, M1], "#F7A072"], [[A, B, M1], "#FCCBAB"], [[B, M2, M1], "#F4844C"], [[B, C, M2], "#F9B086"], [[C, R, M2], "#E86A33"],
  [[L, M1, P], "#D9541E"], [[M1, M2, P], "#F0733A"], [[M2, R, P], "#B8410F"],
];
const background = hex("#2B2D5B"), radius = 52;
const inRound = (x, y) => {
  const dx = Math.max(radius - x, x - (SIZE - radius), 0), dy = Math.max(radius - y, y - (SIZE - radius), 0);
  return dx * dx + dy * dy <= radius * radius;
};
const inTriangle = ([a, b, c], x, y) => {
  const s = (p, q) => (x - q[0]) * (p[1] - q[1]) - (p[0] - q[0]) * (y - q[1]);
  const d1 = s(a, b), d2 = s(b, c), d3 = s(c, a);
  return !((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0));
};
const big = new Float32Array(N * N * 4);
for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
  const x = (i + 0.5) / SS, y = (j + 0.5) / SS, o = (j * N + i) * 4;
  if (!inRound(x, y)) continue;
  let color = background;
  for (const [triangle, fill] of facets) if (inTriangle(triangle, x, y)) { color = hex(fill); break; }
  big[o] = color[0]; big[o + 1] = color[1]; big[o + 2] = color[2]; big[o + 3] = 255;
}
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0;
  for (let x = 0; x < SIZE; x++) {
    let r = 0, g = 0, b = 0, a = 0;
    for (let j = 0; j < SS; j++) for (let i = 0; i < SS; i++) {
      const o = ((y * SS + j) * N + x * SS + i) * 4, alpha = big[o + 3];
      r += big[o] * alpha; g += big[o + 1] * alpha; b += big[o + 2] * alpha; a += alpha;
    }
    const at = y * (SIZE * 4 + 1) + 1 + x * 4;
    raw[at] = a ? Math.round(r / a) : 0; raw[at + 1] = a ? Math.round(g / a) : 0; raw[at + 2] = a ? Math.round(b / a) : 0;
    raw[at + 3] = Math.round(a / (SS * SS));
  }
}
const table = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc = (buf) => { let c = 0xffffffff; for (const byte of buf) c = table[(c ^ byte) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
const chunk = (type, data) => {
  const head = Buffer.alloc(8); head.writeUInt32BE(data.length, 0); head.write(type, 4, "ascii");
  const tail = Buffer.alloc(4); tail.writeUInt32BE(crc(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, tail]);
};
const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(SIZE, 0); ihdr.writeUInt32BE(SIZE, 4); ihdr[8] = 8; ihdr[9] = 6;
writeFileSync(process.argv[2], Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0))]));
