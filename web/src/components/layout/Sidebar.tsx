import { NavLink } from 'react-router-dom'
import {
  Bot,
  Users,
  Sparkles,
  Plug,
  SlidersHorizontal,
  FileText,
  Sun,
  Moon,
  Monitor,
} from 'lucide-react'
import { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider } from '@/components/ui/tooltip'
import { useTheme } from '@/hooks/useTheme'

type Theme = 'dark' | 'light' | 'system'

const themeIcons: Record<Theme, typeof Sun> = {
  dark: Moon,
  light: Sun,
  system: Monitor,
}

const themeLabels: Record<Theme, string> = {
  dark: 'Dark',
  light: 'Light',
  system: 'System',
}

const themeOrder: Theme[] = ['dark', 'light', 'system']

interface SidebarProps {
  open: boolean
  onClose: () => void
}

const navItems = [
  { to: '/agents', icon: Bot, label: 'Agents' },
  { to: '/teams', icon: Users, label: 'Teams' },
  { to: '/skills', icon: Sparkles, label: 'Skills' },
  { to: '/mcp', icon: Plug, label: 'MCP' },
  { to: '/config', icon: SlidersHorizontal, label: 'Config' },
  { to: '/logs', icon: FileText, label: 'Logs' },
]

export default function Sidebar({ open, onClose }: SidebarProps) {
  const { theme, setTheme } = useTheme()

  const nextTheme = () => {
    const idx = themeOrder.indexOf(theme)
    setTheme(themeOrder[(idx + 1) % themeOrder.length])
  }

  const ThemeIcon = themeIcons[theme]

  return (
    <TooltipProvider delayDuration={300}>
      {open && (
        <div
          className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={onClose}
        />
      )}

      <aside
        className={`
          fixed top-0 left-0 z-50 h-screen w-[72px]
          border-r border-[var(--border)] flex flex-col items-center
          bg-[var(--bg)] transition-transform duration-200
          lg:translate-x-0
          ${open ? 'translate-x-0' : '-translate-x-full'}
        `}
      >
        {/* Logo */}
        <div className="flex flex-col items-center pt-4 pb-3">
          <div className="flex items-center justify-center rounded-lg w-9 h-9 bg-[var(--accent)]">
            <span className="text-white text-base font-bold">S</span>
          </div>
          <span className="mt-1 text-[9px] text-[var(--muted)] tracking-[0.02em]">
            see-agent
          </span>
        </div>

        {/* Nav items */}
        <nav className="flex flex-col items-center gap-0.5 flex-1 pt-1">
          {navItems.map((item) => (
            <Tooltip key={item.to}>
              <TooltipTrigger asChild>
                <NavLink
                  to={item.to}
                  onClick={onClose}
                  className="flex flex-col items-center justify-center rounded-lg transition-all duration-150 w-14 h-11"
                  style={({ isActive }) => ({
                    background: isActive ? 'var(--accent-subtle)' : 'transparent',
                    color: isActive ? 'var(--accent)' : 'var(--muted)',
                  })}
                >
                  {({ isActive }) => (
                    <>
                      <item.icon size={17} strokeWidth={isActive ? 2 : 1.5} />
                      <span
                        className="text-[10px] mt-0.5"
                        style={{ fontWeight: isActive ? 500 : 400 }}
                      >
                        {item.label}
                      </span>
                    </>
                  )}
                </NavLink>
              </TooltipTrigger>
              <TooltipContent side="right">{item.label}</TooltipContent>
            </Tooltip>
          ))}
        </nav>

        {/* Theme toggle */}
        <div className="pb-4 pt-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={nextTheme}
                className="flex flex-col items-center justify-center rounded-lg transition-all duration-150 w-14 h-11 hover:bg-[var(--bg-hover)]"
                style={{ color: 'var(--muted)' }}
              >
                <ThemeIcon size={17} strokeWidth={1.5} />
                <span className="text-[10px] mt-0.5">
                  {themeLabels[theme]}
                </span>
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              Theme: {themeLabels[theme]}
            </TooltipContent>
          </Tooltip>
        </div>
      </aside>
    </TooltipProvider>
  )
}
