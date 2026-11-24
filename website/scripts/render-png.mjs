// Renders public/og.png and the PNG icons from scripts/og.html and scripts/icon.html with a
// local headless Chromium (Edge or Chrome) over the DevTools protocol. No dependencies.
//   node scripts/render-png.mjs            (set BROWSER=path\to\chrome.exe to override)
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const candidates = [
  process.env.BROWSER,
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
].filter(Boolean)
const browser = candidates.find((p) => fs.existsSync(p))
if (!browser) throw new Error('No Chromium browser found; set BROWSER=<path to chrome>')

const jobs = [
  ['scripts/og.html', 'public/og.png', 1200, 630],
  ['scripts/icon.html', 'public/icon-512.png', 512, 512, true],
  ['scripts/icon.html', 'public/icon-192.png', 192, 192, true],
  ['scripts/icon.html?opaque', 'public/apple-touch-icon.png', 180, 180],
  ['scripts/grain.html', 'public/grain.png', 100, 100, true],
]

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const port = 9400 + Math.floor(Math.random() * 400)
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'render-'))
const proc = spawn(browser, [
  '--headless=new',
  '--disable-gpu',
  '--hide-scrollbars',
  `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`,
  'about:blank',
])

let target
for (let i = 0; i < 50 && !target; i++) {
  await sleep(200)
  try {
    target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find(
      (t) => t.type === 'page',
    )
  } catch {
    /* not up yet */
  }
}
const ws = new WebSocket(target.webSocketDebuggerUrl)
await new Promise((r) => ws.addEventListener('open', r))
let id = 0
const pending = new Map()
ws.addEventListener('message', (e) => {
  const msg = JSON.parse(e.data)
  pending.get(msg.id)?.(msg)
})
const send = (method, params = {}) =>
  new Promise((r) => {
    pending.set(++id, r)
    ws.send(JSON.stringify({ id, method, params }))
  })

await send('Page.enable')
for (const [src, out, w, h, transparent] of jobs) {
  await send(
    'Emulation.setDefaultBackgroundColorOverride',
    transparent ? { color: { r: 0, g: 0, b: 0, a: 0 } } : {},
  )
  await send('Emulation.setDeviceMetricsOverride', {
    width: w,
    height: h,
    deviceScaleFactor: 1,
    mobile: false,
  })
  const [file, query = ''] = src.split('?')
  const url = pathToFileURL(path.join(root, file)).href + (query ? `?${query}` : '')
  await send('Page.navigate', { url })
  await sleep(700)
  const shot = await send('Page.captureScreenshot', { format: 'png' })
  fs.writeFileSync(path.join(root, out), Buffer.from(shot.result.data, 'base64'))
  console.log('wrote', out)
}
ws.close()
proc.kill()
