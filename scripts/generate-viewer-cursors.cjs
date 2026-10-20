// Regenerate with Node.js and sharp: node scripts/generate-viewer-cursors.cjs
// Source geometry is the unmodified official Lucide SVG in assets/icons/.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
async function generate(name, hotspot) {
  const svg = fs.readFileSync(path.join(__dirname, '..', 'assets', 'icons', `${name}.svg`));
  const entries = [];
  for (const side of [32, 48, 64, 96]) {
    // Preserve the SVG geometry and antialiased coverage. Only tint the stroke
    // white; no dilation, outline or halo is added.
    const pixels = await sharp(svg, {density: side * 72 / 24})
      .resize(side, side).ensureAlpha().raw().toBuffer();
    const stride = Math.ceil(side / 32) * 4;
    const dib = Buffer.alloc(40 + side * side * 4 + stride * side);
    dib.writeUInt32LE(40, 0); dib.writeInt32LE(side, 4); dib.writeInt32LE(side * 2, 8);
    dib.writeUInt16LE(1, 12); dib.writeUInt16LE(32, 14);
    for (let y = 0; y < side; y++) for (let x = 0; x < side; x++) {
      const i = y * side + x, out = 40 + ((side - y - 1) * side + x) * 4;
      const a = pixels[i * 4 + 3];
      // Windows alpha cursors use premultiplied BGRA: white * alpha = alpha.
      dib[out] = dib[out + 1] = dib[out + 2] = a;
      dib[out + 3] = a;
      if (!a) dib[40 + side * side * 4 + (side - y - 1) * stride + (x >> 3)] |= 128 >> (x & 7);
    }
    entries.push({side, dib, hotspot: hotspot.map(n => Math.round(n * side / 24))});
  }
  const header = Buffer.alloc(6 + entries.length * 16);
  header.writeUInt16LE(2, 2); header.writeUInt16LE(entries.length, 4);
  let offset = header.length;
  entries.forEach(({side, dib, hotspot}, i) => {
    const at = 6 + i * 16;
    header[at] = header[at + 1] = side;
    header.writeUInt16LE(hotspot[0], at + 4); header.writeUInt16LE(hotspot[1], at + 6);
    header.writeUInt32LE(dib.length, at + 8); header.writeUInt32LE(offset, at + 12);
    offset += dib.length;
  });
  fs.writeFileSync(path.join(__dirname, '..', 'assets', 'cursors', `${name}.cur`), Buffer.concat([header, ...entries.map(e => e.dib)]));
}
Promise.all([generate('search', [11, 11]), generate('hand-grab', [12, 13])]).catch(error => { console.error(error); process.exitCode = 1; });
