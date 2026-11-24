import '@fontsource-variable/geist'
import '@fontsource-variable/geist-mono'
import '@/styles/globals.css'

import { StrictMode } from 'react'
import { createRoot, hydrateRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router/dom'
import { Providers } from '@/app/providers'
import { createAppRouter } from '@/app/router'

const root = document.getElementById('root')
if (!root) throw new Error('#root missing from index.html')

const router = await createAppRouter()
const app = (
  <StrictMode>
    <Providers>
      <RouterProvider router={router} />
    </Providers>
  </StrictMode>
)

// Production pages are prerendered (scripts/prerender.mjs): hydrate them. Dev renders fresh.
if (root.hasChildNodes()) hydrateRoot(root, app)
else createRoot(root).render(app)
