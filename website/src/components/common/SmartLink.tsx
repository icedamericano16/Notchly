import type { AnchorHTMLAttributes, ReactNode } from 'react'
import { NavLink } from 'react-router'
import { isExternal } from '@/lib/utils'

interface SmartLinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> {
  href: string
  children: ReactNode
}

/**
 * A router <NavLink> for internal paths (it sets aria-current="page" on the active route);
 * a new-tab <a> with a screen-reader hint for external URLs.
 */
export function SmartLink({ href, children, ...rest }: SmartLinkProps) {
  if (isExternal(href)) {
    return (
      <a href={href} target="_blank" rel="noopener noreferrer" {...rest}>
        {children}
        <span className="sr-only"> (opens in a new tab)</span>
      </a>
    )
  }
  return (
    <NavLink to={href} {...rest}>
      {children}
    </NavLink>
  )
}
