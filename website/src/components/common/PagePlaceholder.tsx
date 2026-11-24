interface PagePlaceholderProps {
  title: string
  description: string
  /** Which delivery step fills this page in. */
  step: number
}

/** Temporary page body until the real page lands. Removed in step 6. */
export function PagePlaceholder({ title, description, step }: PagePlaceholderProps) {
  return (
    <section className="container-page pt-40 pb-24">
      <p className="font-mono text-xs tracking-wider text-fg-muted uppercase">
        Coming in step {step}
      </p>
      <h1 className="mt-4 text-5xl text-display sm:text-6xl">{title}</h1>
      <p className="mt-5 max-w-xl text-lg text-fg-muted">{description}</p>
    </section>
  )
}
