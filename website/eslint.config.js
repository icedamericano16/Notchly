import js from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import { defineConfig, globalIgnores } from 'eslint/config'
import globals from 'globals'
import tseslint from 'typescript-eslint'

export default defineConfig([
  globalIgnores(['dist', 'node_modules']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.strict,
      reactHooks.configs.flat['recommended-latest'],
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2023,
      globals: globals.browser,
    },
    rules: {
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/consistent-type-imports': 'error',
      // Motion runs under <LazyMotion strict>: use `m.*` from motion/react-m.
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: 'motion/react',
              importNames: ['motion'],
              message:
                "Use `import * as m from 'motion/react-m'` (LazyMotion keeps the bundle lean).",
            },
          ],
        },
      ],
    },
  },
  {
    // shadcn primitives export variants next to components by design.
    // shadcn primitives, and the island registry (views + their sizes in one table).
    files: ['src/components/ui/**/*.tsx', 'src/components/island/activities.tsx'],
    rules: { 'react-refresh/only-export-components': 'off' },
  },
])
