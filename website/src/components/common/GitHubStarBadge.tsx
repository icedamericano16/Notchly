import { Star } from 'lucide-react'
import { site } from '@/content/site'
import { useRepoInfo } from '@/hooks/useRepoInfo'
import { cn } from '@/lib/cn'
import { formatCompact } from '@/lib/utils'
import { GitHubIcon } from './BrandIcons'

interface GitHubStarBadgeProps {
  className?: string
}

/** "GitHub ★ 12": links to the repo; the count space is reserved so nothing shifts. */
export function GitHubStarBadge({ className }: GitHubStarBadgeProps) {
  const repo = useRepoInfo()
  const stars = repo.status === 'success' ? repo.data.stars : null
  const label =
    stars === null
      ? 'Notchly on GitHub (opens in a new tab)'
      : `Notchly on GitHub, ${stars} stars (opens in a new tab)`

  return (
    <a
      href={site.repo.url}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={label}
      className={cn(
        'h-9 items-center gap-2 rounded-full border border-hairline bg-surface-1/60 pr-1.5 pl-3 text-[13px] text-fg-muted transition-colors duration-200 hover:border-hairline-strong hover:text-fg',
        className,
      )}
    >
      <GitHubIcon className="size-4" />
      <span>GitHub</span>
      {repo.status !== 'error' && (
        <span className="inline-flex h-6 min-w-[3.25rem] items-center justify-center gap-1 rounded-full bg-white/[0.06] px-2 font-mono text-xs text-fg tabular-nums">
          <Star className="size-3" aria-hidden="true" />
          {stars === null ? (
            <span className="h-2 w-4 animate-pulse rounded-sm bg-white/10" />
          ) : stars > 0 ? (
            formatCompact(stars)
          ) : (
            <span className="font-sans">Star</span>
          )}
        </span>
      )}
    </a>
  )
}
