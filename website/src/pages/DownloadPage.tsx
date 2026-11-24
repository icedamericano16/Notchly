import { PagePlaceholder } from '@/components/common/PagePlaceholder'
import { Seo } from '@/components/common/Seo'
import { routeMeta } from '@/content/site'

export default function DownloadPage() {
  return (
    <>
      <Seo page="download" />
      <PagePlaceholder
        title="Download Notchly"
        description={routeMeta.download.description}
        step={6}
      />
    </>
  )
}
