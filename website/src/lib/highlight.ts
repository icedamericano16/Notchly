import type { HighlighterCore } from 'shiki/core'
import type { SnippetLang } from '@/content/developers'

/*
 * Shiki, fine-grained: only the core, the JavaScript regex engine (no WASM), one theme and
 * the three languages the site shows. Loaded on demand, once.
 */
let highlighter: Promise<HighlighterCore> | null = null

function getHighlighter(): Promise<HighlighterCore> {
  highlighter ??= Promise.all([import('shiki/core'), import('shiki/engine/javascript')]).then(
    ([{ createHighlighterCore }, { createJavaScriptRegexEngine }]) =>
      createHighlighterCore({
        themes: [import('@shikijs/themes/github-dark-default')],
        langs: [
          import('@shikijs/langs/python'),
          import('@shikijs/langs/javascript'),
          import('@shikijs/langs/powershell'),
        ],
        engine: createJavaScriptRegexEngine(),
      }),
  )
  return highlighter
}

/** Highlighted HTML for trusted, site-authored code. */
export async function highlight(code: string, lang: SnippetLang): Promise<string> {
  const h = await getHighlighter()
  return h.codeToHtml(code, {
    lang,
    theme: 'github-dark-default',
    colorReplacements: { '#0d1117': 'transparent' },
  })
}
