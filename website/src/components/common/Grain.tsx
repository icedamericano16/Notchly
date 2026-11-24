/**
 * A static film-grain layer over the whole page. Decorative, never animated, and no blend
 * mode (a full-screen blend would be re-composited on every scroll frame).
 */
export function Grain() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 z-[100] bg-grain opacity-[0.045]"
    />
  )
}
