import { PagePlaceholder } from '@/components/common/PagePlaceholder'
import { Seo } from '@/components/common/Seo'
import { routeMeta } from '@/content/site'

export default function FeaturesPage() {
  return (
    <>
      <Seo page="features" />
      <PagePlaceholder
        title="Every feature"
        description={routeMeta.features.description}
        step={6}
      />
    </>
  )
}
