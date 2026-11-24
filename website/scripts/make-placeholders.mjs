// Generates the labelled placeholder images in public/media/placeholders/.
// Run: node scripts/make-placeholders.mjs   (safe to re-run; real media lives elsewhere)
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const out = path.join(root, 'public', 'media', 'placeholders')
fs.mkdirSync(out, { recursive: true })

const items = [
  ['hero-demo', 'Demo video', 1920, 1080],
  ['install-1', 'Installer', 1200, 800],
  ['install-2', 'SmartScreen: More info › Run anyway', 1200, 800],
  ['install-3', 'Hover the notch', 1200, 800],
]

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;')

for (const [id, title, w, h] of items) {
  const nw = Math.round(w * 0.2)
  const nh = Math.round(h * 0.045)
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <defs>
    <radialGradient id="a" cx="25%" cy="20%" r="80%"><stop offset="0" stop-color="#1d2b5c"/><stop offset="1" stop-color="#05060a"/></radialGradient>
    <radialGradient id="b" cx="85%" cy="90%" r="60%"><stop offset="0" stop-color="#3a1f63" stop-opacity=".9"/><stop offset="1" stop-color="#05060a" stop-opacity="0"/></radialGradient>
    <linearGradient id="g" x1="0" x2="1"><stop offset="0" stop-color="#3b7bff"/><stop offset="1" stop-color="#8b5cf6"/></linearGradient>
  </defs>
  <rect width="100%" height="100%" fill="url(#a)"/>
  <rect width="100%" height="100%" fill="url(#b)"/>
  <rect x="${(w - nw) / 2}" y="${Math.round(h * 0.03)}" width="${nw}" height="${nh}" rx="${nh / 2}" fill="#000" stroke="#ffffff22"/>
  <rect x="${(w - nw * 0.3) / 2}" y="${Math.round(h * 0.03 + nh * 0.62)}" width="${nw * 0.3}" height="${Math.max(3, nh * 0.1)}" rx="2" fill="url(#g)"/>
  <text x="50%" y="52%" text-anchor="middle" font-family="Segoe UI, system-ui, sans-serif" font-size="${Math.round(w * 0.034)}" font-weight="600" fill="#fafafa">${esc(title)}</text>
  <text x="50%" y="${52 + 7}%" text-anchor="middle" font-family="Consolas, monospace" font-size="${Math.round(w * 0.014)}" fill="#ffffff99">PLACEHOLDER · replace with a real capture (notchly --demo) · ${w}×${h}</text>
</svg>
`
  fs.writeFileSync(path.join(out, `${id}.svg`), svg)
}
console.log(`wrote ${items.length} placeholders to ${path.relative(root, out)}`)
