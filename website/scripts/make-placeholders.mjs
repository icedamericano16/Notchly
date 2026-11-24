// Generates the labelled placeholder images in public/media/placeholders/: a neutral
// graphite "desktop" with a notch outline and a label. Replace them with real captures
// (see README › Swapping media).
// Run: node scripts/make-placeholders.mjs   (safe to re-run; real media lives elsewhere)
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const out = path.join(root, 'public', 'media', 'placeholders')
fs.mkdirSync(out, { recursive: true })

const items = [
  ['hero-demo', 'Demo video', 1920, 1080],
  ['install-1', 'Installer', 1200, 750],
  ['install-2', 'SmartScreen: More info › Run anyway', 1200, 750],
]

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;')

for (const [id, title, w, h] of items) {
  const nw = Math.round(w * 0.2)
  const nh = Math.round(h * 0.045)
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <defs>
    <radialGradient id="a" cx="50%" cy="0%" r="75%"><stop offset="0" stop-color="#1b1c21"/><stop offset="1" stop-color="#0c0d10"/></radialGradient>
  </defs>
  <rect width="100%" height="100%" fill="url(#a)"/>
  <rect x="0" y="0" width="100%" height="1" fill="#ffffff26"/>
  <rect x="${(w - nw) / 2}" y="0" width="${nw}" height="${nh}" rx="${nh / 2}" fill="#000" stroke="#ffffff1f"/>
  <text x="50%" y="52%" text-anchor="middle" font-family="Segoe UI, system-ui, sans-serif" font-size="${Math.round(w * 0.032)}" font-weight="600" fill="#e6e6e6">${esc(title)}</text>
  <text x="50%" y="${52 + 7}%" text-anchor="middle" font-family="Consolas, monospace" font-size="${Math.round(w * 0.014)}" fill="#ffffff80">PLACEHOLDER · replace with a real capture · ${w}×${h}</text>
</svg>
`
  fs.writeFileSync(path.join(out, `${id}.svg`), svg)
}
console.log(`wrote ${items.length} placeholders to ${path.relative(root, out)}`)
