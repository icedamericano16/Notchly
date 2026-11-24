import { Menu } from 'lucide-react'
import { Link } from 'react-router'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet'
import { primaryNav } from '@/content/site'
import { WindowsIcon } from './BrandIcons'
import { GitHubStarBadge } from './GitHubStarBadge'
import { Logo } from './Logo'
import { SmartLink } from './SmartLink'

/**
 * Mobile navigation sheet. Lazy-loaded by the Navbar so the dialog stack isn't on the
 * critical path; keep the trigger identical to the Navbar's Suspense fallback.
 */
export default function MobileMenu() {
  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button
          variant="ghost"
          size="icon-lg"
          className="rounded-full lg:hidden"
          aria-label="Open menu"
        >
          <Menu />
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-[85%] border-hairline bg-surface-1 p-6">
        <SheetTitle className="sr-only">Menu</SheetTitle>
        <SheetDescription className="sr-only">Site navigation</SheetDescription>
        <Logo className="mb-6" />
        <nav aria-label="Mobile">
          <ul className="flex flex-col">
            {primaryNav.map((link) => (
              <li key={link.href}>
                <SheetClose asChild>
                  <SmartLink
                    href={link.href}
                    className="flex items-center justify-between border-b border-hairline py-4 text-lg tracking-tightish text-fg-muted transition-colors hover:text-fg aria-[current=page]:text-fg"
                  >
                    {link.label}
                  </SmartLink>
                </SheetClose>
              </li>
            ))}
          </ul>
        </nav>
        <div className="mt-auto flex flex-col gap-3">
          <GitHubStarBadge className="inline-flex self-start" />
          <SheetClose asChild>
            <Button asChild size="lg" className="h-11 rounded-full">
              <Link to="/download">
                <WindowsIcon className="size-4" />
                Download for Windows
              </Link>
            </Button>
          </SheetClose>
        </div>
      </SheetContent>
    </Sheet>
  )
}
