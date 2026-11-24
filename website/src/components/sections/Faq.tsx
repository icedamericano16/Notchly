import { Reveal } from '@/components/common/Reveal'
import { SectionHeading } from '@/components/common/SectionHeading'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { faqs } from '@/content/faq'

export function Faq() {
  return (
    <section aria-labelledby="faq-title" className="container-page pt-32 sm:pt-40">
      <div className="grid gap-10 lg:grid-cols-[1fr_1.6fr]">
        <SectionHeading id="faq-title" align="start" eyebrow="FAQ" title="Questions, answered." />
        <Reveal>
          <Accordion
            type="single"
            collapsible
            className="rounded-card border border-hairline bg-surface-1/60 px-5"
          >
            {faqs.map((f) => (
              <AccordionItem key={f.id} value={f.id} className="border-hairline">
                <AccordionTrigger className="py-5 text-base font-medium hover:no-underline">
                  {f.question}
                </AccordionTrigger>
                <AccordionContent className="pb-5 text-[15px] leading-relaxed text-fg-muted">
                  {f.answer}
                </AccordionContent>
              </AccordionItem>
            ))}
          </Accordion>
        </Reveal>
      </div>
    </section>
  )
}
