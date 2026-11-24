// Converts PNG captures to WebP (and keeps a PNG fallback) using a local headless Chromium.
//   node scripts/to-webp.mjs <in-dir> <out-dir> [quality 0-1]
// Used for the real app captures in public/media/app/ (taken with `notchly --demo`).
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const [inDir, outDir, quality = '0.9'] = process.argv.slice(2)
const browser = [
  process.env.BROWSER,
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  '/usr/bin/google-chrome',
].find((p) => p && fs.existsSync(p))
if (!browser) throw new Error('No Chromium browser found; set BROWSER=<path to chrome>')

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const port = 9500 + Math.floor(Math.random() * 400)
const proc = spawn(browser, [
  '--headless=new',
  '--disable-gpu',
  '--allow-file-access-from-files',
  `--remote-debugging-port=${port}`,
  `--user-data-dir=${fs.mkdtempSync(path.join(os.tmpdir(), 'webp-'))}`,
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
  const m = JSON.parse(e.data)
  pending.get(m.id)?.(m)
})
const send = (method, params = {}) =>
  new Promise((r) => {
    pending.set(++id, r)
    ws.send(JSON.stringify({ id, method, params }))
  })

fs.mkdirSync(outDir, { recursive: true })
await send('Page.navigate', { url: pathToFileURL(path.resolve(inDir)).href })
await sleep(500)
for (const f of fs.readdirSync(inDir).filter((n) => n.endsWith('.png'))) {
  const url = pathToFileURL(path.resolve(inDir, f)).href
  const { result } = await send('Runtime.evaluate', {
    awaitPromise: true,
    returnByValue: true,
    expression: `new Promise((res, rej) => { const i = new Image(); i.onload = () => {
      const c = document.createElement('canvas'); c.width = i.width; c.height = i.height;
      c.getContext('2d').drawImage(i, 0, 0);
      res({ w: i.width, h: i.height, data: c.toDataURL('image/webp', ${Number(quality)}) }) };
      i.onerror = rej; i.src = ${JSON.stringify(url)} })`,
  })
  const { w, h, data } = result.result.value
  const name = f.replace(/\.png$/, '')
  fs.writeFileSync(path.join(outDir, `${name}.webp`), Buffer.from(data.split(',')[1], 'base64'))
  fs.copyFileSync(path.join(inDir, f), path.join(outDir, `${name}.png`))
  console.log(`${name}: ${w}x${h}`)
}
ws.close()
proc.kill()
