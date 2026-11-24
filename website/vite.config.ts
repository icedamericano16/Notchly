import { fileURLToPath, URL } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig, loadEnv, type Plugin } from 'vite'

/** Routes listed in sitemap.xml (keep in sync with src/app/router.tsx). */
const ROUTES = ['/', '/features', '/download', '/changelog', '/privacy']

/** Emits sitemap.xml and robots.txt for the configured site URL. */
function seoFiles(siteUrl: string): Plugin {
  const origin = siteUrl.replace(/\/$/, '')
  return {
    name: 'notchly-seo-files',
    apply: 'build',
    generateBundle() {
      const today = new Date().toISOString().slice(0, 10)
      const urls = ROUTES.map(
        (r) =>
          `  <url><loc>${origin}${r === '/' ? '/' : r}</loc><lastmod>${today}</lastmod><priority>${r === '/' ? '1.0' : '0.8'}</priority></url>`,
      ).join('\n')
      this.emitFile({
        type: 'asset',
        fileName: 'sitemap.xml',
        source: `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`,
      })
      this.emitFile({
        type: 'asset',
        fileName: 'robots.txt',
        source: `User-agent: *\nAllow: /\n\nSitemap: ${origin}/sitemap.xml\n`,
      })
    },
  }
}

/*
 * VITE_SITE_URL: the canonical origin (canonical links, Open Graph image, sitemap, robots).
 * Override it per deploy, e.g. `VITE_SITE_URL=https://notchly.app npm run build`, or in a
 * git-ignored website/.env.local. Keep the fallback in src/content/site.ts in sync.
 */
process.env.VITE_SITE_URL ||= 'https://icedamericano16.github.io/Notchly'

// `BASE_PATH` lets the GitHub Pages workflow serve the site from /<repo>/.
// Vercel / Netlify serve from the root, so it defaults to '/'.
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), 'VITE_')
  return {
    base: process.env.BASE_PATH ?? '/',
    plugins: [react(), tailwindcss(), seoFiles(env.VITE_SITE_URL ?? '')],
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url)),
      },
    },
  }
})
