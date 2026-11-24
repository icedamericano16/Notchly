// Prerenders every route to static HTML after `vite build` (see the "build" script).
// Each page ships its full markup, so it paints before any JavaScript runs, then hydrates.
// Head tags rendered by <Seo> (title, description, canonical, Open Graph) are moved into
// <head>, replacing the template's defaults.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const dist = path.join(root, 'dist')
const ssrDir = path.join(root, 'dist-ssr')
const base = (process.env.BASE_PATH ?? '/').replace(/\/?$/, '/')

const { render } = await import(pathToFileURL(path.join(ssrDir, 'entry-server.js')).href)
const template = fs.readFileSync(path.join(dist, 'index.html'), 'utf8')

const pages = [
  ['', 'index.html'],
  ['features', 'features/index.html'],
  ['download', 'download/index.html'],
  ['changelog', 'changelog/index.html'],
  ['privacy', 'privacy/index.html'],
  ['this-page-does-not-exist', '404.html'],
]

// Tags the template provides as defaults; each page's own versions replace them.
const defaultHead = [
  /\s*<title>[\s\S]*?<\/title>/,
  /\s*<meta name="description"[^>]*>/,
  /\s*<meta property="og:[^"]+"[^>]*>/g,
  /\s*<meta name="twitter:[^"]+"[^>]*>/g,
]
// Preload the Latin Geist face used above the fold, so text rarely swaps after first paint.
const fonts = fs
  .readdirSync(path.join(dist, 'assets'))
  .filter((f) => /^geist-latin-wght-normal-.*\.woff2$/.test(f))
  .map(
    (f) => `<link rel="preload" href="${base}assets/${f}" as="font" type="font/woff2" crossorigin>`,
  )
  .join('\n    ')

/*
 * The page is complete HTML, so JavaScript isn't needed to see it. Start the app (and its
 * module preloads) after the first frame instead of competing with it: Vite's entry
 * <script> and <link rel="modulepreload"> tags become a loader that runs after paint.
 */
function deferApp(html) {
  const entry = html.match(/<script type="module" crossorigin src="([^"]+)"><\/script>/)
  if (!entry) return html
  const preloads = [...html.matchAll(/<link rel="modulepreload" crossorigin href="([^"]+)">/g)].map(
    (m) => m[1],
  )
  const loader = `<script>(function(){function go(){${JSON.stringify(preloads)}.forEach(function(h){var l=document.createElement('link');l.rel='modulepreload';l.crossOrigin='';l.href=h;document.head.appendChild(l)});var s=document.createElement('script');s.type='module';s.crossOrigin='';s.src=${JSON.stringify(entry[1])};document.head.appendChild(s)}requestAnimationFrame(function(){setTimeout(go,0)})})()</script>`
  return html
    .replace(entry[0], '')
    .replace(/\s*<link rel="modulepreload" crossorigin href="[^"]+">/g, '')
    .replace('</body>', `  ${loader}\n  </body>`)
}

const hoistable =
  /<title>[\s\S]*?<\/title>|<meta (?:name|property)="[^"]+"[^>]*>|<link rel="(?:canonical|preload)"[^>]*>/g

for (const [route, file] of pages) {
  const { html } = await render(base + route)
  const head = (html.match(hoistable) ?? []).join('\n    ')
  const body = html.replace(hoistable, '')

  let page = template
  for (const re of defaultHead) page = page.replace(re, '')
  page = deferApp(
    page
      .replace('</head>', `    ${fonts}\n    ${head}\n  </head>`)
      .replace('<div id="root"></div>', `<div id="root">${body}</div>`),
  )

  const out = path.join(dist, file)
  fs.mkdirSync(path.dirname(out), { recursive: true })
  fs.writeFileSync(out, page)
  console.log(`prerendered /${route} → ${file} (${(page.length / 1024).toFixed(0)} KB)`)
}

fs.rmSync(ssrDir, { recursive: true, force: true })
