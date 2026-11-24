/**
 * Developer section snippets. They use the real SDK and CLI APIs (sdk/python/notchly.py,
 * sdk/node/notchly.js, `notchly live`); the build steps around them are illustrative.
 */

export type SnippetLang = 'python' | 'javascript' | 'powershell'

export interface Snippet {
  id: string
  label: string
  lang: SnippetLang
  file: string
  code: string
}

export const developer = {
  eyebrow: 'For developers',
  title: 'Build your own Live Activities.',
  body: 'Any app or script can post to the notch through a per-user named pipe, the notchly CLI, or tiny dependency-free SDKs. Payloads are validated and never executed, and the first message from a new app asks you to allow it.',
  limits: [
    '8 activities at once',
    '10 updates/s each',
    'Icons up to 64 KB',
    'Newline-delimited JSON',
  ],
}

export const snippets: Snippet[] = [
  {
    id: 'python',
    label: 'Python SDK',
    lang: 'python',
    file: 'build.py',
    code: `# example: show a build in the notch
from notchly import Live

with Live("build-42", title="Build", icon="hammer") as a:
    a.update(progress=0.4, subtitle="compiling")
    a.update(progress=0.8, subtitle="linking")
    a.end("Done in 2m 14s")
`,
  },
  {
    id: 'node',
    label: 'Node.js SDK',
    lang: 'javascript',
    file: 'deploy.js',
    code: `// example: a deploy with a Cancel button
const { live } = require("./notchly");

const a = await live(
  "deploy",
  { title: "Deploy", actions: [{ id: "cancel", label: "Cancel" }] },
  (action) => console.log("clicked", action),
);
a.update({ progress: 0.5, subtitle: "uploading" });
a.end("Deployed");
`,
  },
  {
    id: 'cli',
    label: 'CLI',
    lang: 'powershell',
    file: 'PowerShell',
    code: `# example: drive an activity from any script
notchly live start --id build-42 --title Build --subtitle compiling --icon hammer --progress 0.4 --action cancel:Cancel
notchly live update --id build-42 --progress 0.8
notchly live end --id build-42 --final "Done in 2m 14s"
`,
  },
]
