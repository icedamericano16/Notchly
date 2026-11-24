import type { ReactNode } from 'react'
import { Bento } from './Bento'
import { Customise } from './Customise'
import { Developer } from './Developer'
import { Faq } from './Faq'
import { FinalCta } from './FinalCta'
import { Gallery } from './Gallery'
import { PrivacySection } from './PrivacySection'
import { Stats } from './Stats'

/** Skips layout and paint until the block nears the viewport (see .cv-auto). */
function Lazy({ children }: { children: ReactNode }) {
  return <div className="cv-auto">{children}</div>
}

/** Everything after the scroll story, in one lazily loaded chunk. */
export default function BelowFold() {
  return (
    <>
      <Lazy>
        <Bento />
      </Lazy>
      <Lazy>
        <Customise />
      </Lazy>
      <Lazy>
        <Developer />
      </Lazy>
      {/* Not wrapped: its sticky column needs normal paint containment. */}
      <PrivacySection />
      <Lazy>
        <Stats />
      </Lazy>
      <Lazy>
        <Gallery />
      </Lazy>
      <Lazy>
        <Faq />
      </Lazy>
      <Lazy>
        <FinalCta />
      </Lazy>
    </>
  )
}
