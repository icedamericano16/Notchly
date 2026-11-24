import { PagePlaceholder } from '@/components/common/PagePlaceholder'
import { Seo } from '@/components/common/Seo'
import { routeMeta } from '@/content/site'

export default function ChangelogPage() {
  return (
    <>
      <Seo page="changelog" />
      <PagePlaceholder title="Changelog" description={routeMeta.changelog.description} step={6} />
    </>
  )
}
