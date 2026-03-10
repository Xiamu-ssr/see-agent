import { Menu, Sun, Moon, Monitor } from 'lucide-react'

type Theme = 'dark' | 'light' | 'system'

interface TopbarProps {
  theme: Theme
  onThemeChange: (t: Theme) => void
  onMenuToggle: () => void
}

const themeIcons: Record<Theme, typeof Sun> = {
  dark: Moon,
  light: Sun,
  system: Monitor,
}

const themeOrder: Theme[] = ['dark', 'light', 'system']

export default function Topbar({ theme, onThemeChange, onMenuToggle }: TopbarProps) {
  const next = () => {
    const idx = themeOrder.indexOf(theme)
    onThemeChange(themeOrder[(idx + 1) % themeOrder.length])
  }

  const Icon = themeIcons[theme]

  return (
    <header
      className="fixed top-0 left-0 right-0 z-50 flex h-12 items-center justify-between border-b px-4"
      style={{
        background: 'var(--bg-elevated)',
        borderColor: 'var(--border)',
      }}
    >
      <div className="flex items-center gap-3">
        <button
          onClick={onMenuToggle}
          className="lg:hidden p-1 rounded hover:bg-[var(--bg-hover)]"
        >
          <Menu size={20} style={{ color: 'var(--text)' }} />
        </button>
        <span className="text-sm font-semibold" style={{ color: 'var(--text-strong)' }}>
          see-agent
        </span>
        <span className="text-xs" style={{ color: 'var(--muted)' }}>
          v3.0
        </span>
      </div>

      <button
        onClick={next}
        className="rounded-[var(--radius-sm)] p-1.5 hover:bg-[var(--bg-hover)] transition-colors"
        title={`Theme: ${theme}`}
      >
        <Icon size={16} style={{ color: 'var(--text)' }} />
      </button>
    </header>
  )
}
