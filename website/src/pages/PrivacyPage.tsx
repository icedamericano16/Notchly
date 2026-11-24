import { PagePlaceholder } from '@/components/common/PagePlaceholder'
import { Seo } from '@/components/common/Seo'
import { routeMeta } from '@/content/site'

export default function PrivacyPage() {
  return (
    <>
      <Seo page="privacy" />
      <PagePlaceholder
        title="Everything runs locally."
        description={routeMeta.privacy.description}
        step={6}
      />
    </>
  )
}
