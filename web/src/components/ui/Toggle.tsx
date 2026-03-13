/**
 * Small iOS-style toggle switch.
 */
export default function Toggle({
  enabled,
  onChange,
}: {
  enabled: boolean
  onChange: () => void
}) {
  return (
    <button
      onClick={onChange}
      className="relative shrink-0 rounded-full transition-colors"
      style={{
        width: 32,
        height: 18,
        background: enabled ? 'var(--accent)' : '#3b3b3b',
      }}
    >
      <span
        className="absolute rounded-full bg-white transition-all"
        style={{
          width: 14,
          height: 14,
          top: 2,
          left: enabled ? 16 : 2,
        }}
      />
    </button>
  )
}
