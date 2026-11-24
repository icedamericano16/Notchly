import { ArrowUpRight, Check, Download, Package, TriangleAlert } from 'lucide-react'
import { useState, type RefObject } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { buildFromSource, installSteps } from '@/content/download'
import { site } from '@/content/site'
import type { LatestRelease } from '@/hooks/useLatestRelease'
import { formatBytes } from '@/lib/format'
import { CodeBlockPlain } from './CodeBlockPlain'

interface InstallDialogProps {
  latest: LatestRelease
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Where focus returns on close (the button that opened it). */
  returnFocusRef: RefObject<HTMLButtonElement | null>
}

const fallbackReasons: Record<LatestRelease['reason'], string> = {
  none: '',
  'no-release': "There's no published release yet.",
  'no-installer': "The latest release doesn't include an installer file.",
  'rate-limited':
    "GitHub's API limit for your network was reached, so the file can't be listed here.",
  error: "GitHub couldn't be reached just now.",
}

/**
 * The 3-step install guide, shown before the download starts. Lazy-loaded by
 * DownloadButton on first click, so the dialog stack stays off the critical path.
 */
export default function InstallDialog({
  latest,
  open,
  onOpenChange,
  returnFocusRef,
}: InstallDialogProps) {
  const [started, setStarted] = useState(false)
  const { release, primary, msix } = latest
  const version = release?.version ? ` ${release.version}` : ''

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) setStarted(false)
        onOpenChange(next)
      }}
    >
      <DialogContent
        className="max-h-[90dvh] gap-0 overflow-y-auto border-hairline bg-surface-1 p-0 sm:max-w-lg"
        onCloseAutoFocus={(e) => {
          e.preventDefault()
          returnFocusRef.current?.focus()
        }}
      >
        <DialogHeader className="border-b border-hairline p-6">
          <DialogTitle className="text-xl font-semibold tracking-tightish">
            Install Notchly{version}
          </DialogTitle>
          <DialogDescription className="text-fg-muted">
            Three steps, under a minute. Windows 10 (2004+) and 11, 64-bit.
          </DialogDescription>
        </DialogHeader>

        <ol className="flex flex-col gap-5 p-6">
          {installSteps.map((step, i) => (
            <li key={step.title} className="flex gap-4">
              <span
                aria-hidden="true"
                className="flex size-7 shrink-0 items-center justify-center rounded-full border border-hairline-strong font-mono text-xs text-fg"
              >
                {i + 1}
              </span>
              <div>
                <p className="text-[15px] font-medium">{step.title}</p>
                <p className="mt-1 text-sm leading-relaxed text-fg-muted">{step.body}</p>
              </div>
            </li>
          ))}
        </ol>

        <div className="flex flex-col gap-3 border-t border-hairline bg-canvas/40 p-6">
          {latest.status === 'ready' && primary ? (
            <>
              <a
                href={primary.url}
                onClick={() => setStarted(true)}
                className="inline-flex h-12 items-center justify-center gap-2 rounded-full bg-fg px-6 text-[15px] font-medium text-canvas transition-colors hover:bg-fg/90"
              >
                <Download className="size-4" aria-hidden="true" />
                Download Notchly{version}
                <span className="font-normal text-canvas/60">· {formatBytes(primary.size)}</span>
              </a>
              <p className="min-h-5 text-center text-sm text-fg-muted" aria-live="polite">
                {started && (
                  <span className="inline-flex items-center gap-1.5">
                    <Check className="size-4 text-live-green" aria-hidden="true" />
                    Your download is starting. Open it from your browser's downloads.
                  </span>
                )}
              </p>
              <div className="flex flex-wrap items-center justify-center gap-x-5 gap-y-2 text-sm">
                {msix && (
                  <a
                    href={msix.url}
                    className="inline-flex items-center gap-1.5 text-fg-muted underline-offset-4 hover:text-fg hover:underline"
                  >
                    <Package className="size-3.5" aria-hidden="true" />
                    MSIX package ({formatBytes(msix.size)})
                  </a>
                )}
                <ReleasesLink />
              </div>
            </>
          ) : (
            <Fallback latest={latest} />
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

function ReleasesLink() {
  return (
    <a
      href={site.repo.releasesUrl}
      target="_blank"
      rel="noopener noreferrer"
      className="inline-flex items-center gap-1 text-fg-muted underline-offset-4 hover:text-fg hover:underline"
    >
      All releases on GitHub
      <ArrowUpRight className="size-3.5" aria-hidden="true" />
      <span className="sr-only"> (opens in a new tab)</span>
    </a>
  )
}

/** No usable release (yet, or the API failed): never a broken button. */
function Fallback({ latest }: { latest: LatestRelease }) {
  const loading = latest.status === 'loading'
  return (
    <>
      {!loading && (
        <p className="flex items-start gap-2 text-sm text-fg-muted">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-live-orange" aria-hidden="true" />
          {fallbackReasons[latest.reason]} You can get every build from GitHub Releases, or build it
          yourself.
        </p>
      )}
      <a
        href={site.repo.releasesUrl}
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex h-12 items-center justify-center gap-2 rounded-full bg-fg px-6 text-[15px] font-medium text-canvas transition-colors hover:bg-fg/90"
      >
        Open GitHub Releases
        <ArrowUpRight className="size-4" aria-hidden="true" />
        <span className="sr-only"> (opens in a new tab)</span>
      </a>
      {!loading && (
        <details className="group rounded-xl border border-hairline bg-surface-1 p-4 text-sm">
          <summary className="cursor-pointer font-medium marker:text-fg-muted">
            Build from source
          </summary>
          <ul className="mt-3 list-disc space-y-1 pl-5 text-fg-muted">
            {buildFromSource.requirements.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
          <CodeBlockPlain className="mt-3" code={buildFromSource.commands} />
        </details>
      )}
    </>
  )
}
