import { Check, Link2 } from 'lucide-react'
import { lazy, Suspense, useRef, useState } from 'react'
import { ShimmerButton } from '@/components/magicui/ShimmerButton'
import { otherOsCopy } from '@/content/download'
import { site } from '@/content/site'
import { useLatestRelease, type LatestRelease } from '@/hooks/useLatestRelease'
import { osNames, useOS } from '@/hooks/useOS'
import { cn } from '@/lib/cn'
import { formatBytes, formatDate, formatNumber } from '@/lib/format'
import { WindowsIcon } from './BrandIcons'
import { copyText } from '@/lib/clipboard'

const InstallDialog = lazy(() => import('./InstallDialog'))
/** Warm the chunk on hover/focus so the dialog opens instantly on click. */
const preloadDialog = () => void import('./InstallDialog')

interface DownloadButtonProps {
  label?: string
  size?: 'md' | 'lg'
  /** Version · date · size · downloads under the button. */
  showMeta?: boolean
  align?: 'center' | 'start'
  className?: string
}

/**
 * The one download entry point. Windows (or unknown): opens the install guide, whose button
 * downloads the latest installer, or links to GitHub Releases when there's no usable
 * release. Other systems: "Send link to my PC".
 */
export function DownloadButton({
  label = 'Download for Windows',
  size = 'lg',
  showMeta = false,
  align = 'center',
  className,
}: DownloadButtonProps) {
  const { os, isWindows } = useOS()
  const latest = useLatestRelease()
  const [open, setOpen] = useState(false)
  const [mounted, setMounted] = useState(false)
  const buttonRef = useRef<HTMLButtonElement>(null)

  return (
    <div
      className={cn(
        'flex flex-col gap-2.5',
        align === 'center' ? 'items-center' : 'items-start',
        className,
      )}
    >
      {isWindows ? (
        <>
          <ShimmerButton
            ref={buttonRef}
            size={size}
            aria-haspopup="dialog"
            aria-expanded={open}
            onPointerEnter={preloadDialog}
            onFocus={preloadDialog}
            onClick={() => {
              setMounted(true)
              setOpen(true)
            }}
          >
            <WindowsIcon />
            {label}
          </ShimmerButton>
          {mounted && (
            <Suspense fallback={null}>
              <InstallDialog
                latest={latest}
                open={open}
                onOpenChange={setOpen}
                returnFocusRef={buttonRef}
              />
            </Suspense>
          )}
        </>
      ) : (
        <SendLinkButton size={size} osName={osNames[os]} />
      )}
      {showMeta && isWindows && <ReleaseMeta latest={latest} />}
    </div>
  )
}

function SendLinkButton({ size, osName }: { size: 'md' | 'lg'; osName: string }) {
  const [copied, setCopied] = useState(false)
  const link = `${site.url}/download`

  return (
    <>
      <ShimmerButton
        size={size}
        onClick={async () => {
          const ok = await copyText(link)
          if (!ok && typeof navigator.share === 'function') {
            await navigator
              .share({ title: 'Notchly for Windows', url: link })
              .catch(() => undefined)
          }
          setCopied(ok)
        }}
      >
        {copied ? <Check /> : <Link2 />}
        {copied ? otherOsCopy.copied : otherOsCopy.button}
      </ShimmerButton>
      <p className="max-w-xs text-center text-xs text-fg-muted" aria-live="polite">
        {otherOsCopy.title}. You're on {osName}:{' '}
        {copied ? `paste ${link} on your PC.` : 'copy the link and open it on your PC.'}
      </p>
    </>
  )
}

/** "v0.3.0 · Nov 24, 2026 · 3.4 MB · 1,204 downloads" with a same-height skeleton. */
export function ReleaseMeta({ latest, className }: { latest: LatestRelease; className?: string }) {
  const base = cn('min-h-5 font-mono text-xs text-fg-muted', className)

  if (latest.status === 'loading') {
    return (
      <p className={base} aria-hidden="true">
        <span className="inline-block h-3 w-52 animate-pulse rounded bg-white/[0.06] align-middle" />
      </p>
    )
  }
  if (latest.status === 'fallback' || !latest.release || !latest.primary) {
    return <p className={base}>Builds on GitHub Releases</p>
  }

  const { release, primary } = latest
  const parts = [
    release.version ? `v${release.version}` : release.name,
    formatDate(release.publishedAt),
    formatBytes(primary.size),
    primary.downloadCount > 0 ? `${formatNumber(primary.downloadCount)} downloads` : null,
  ].filter(Boolean)

  return <p className={base}>{parts.join(' · ')}</p>
}
