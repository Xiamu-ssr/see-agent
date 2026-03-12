import { NavLink } from 'react-router-dom'
import {
  Bot,
  Users,
  Sparkles,
  Plug,
  SlidersHorizontal,
  FileText,
} from 'lucide-react'

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
  return (
    <>
      {/* Mobile overlay */}
      {open && (
        <div
          className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={onClose}
        />
      )}

      <aside
        className={`
          fixed top-0 left-0 z-50 h-screen w-[72px]
          border-r flex flex-col items-center
          transition-transform duration-200
          lg:translate-x-0
          ${open ? 'translate-x-0' : '-translate-x-full'}
        `}
        style={{
          background: '#0d1117',
          borderColor: 'var(--border)',
        }}
      >
        {/* Logo */}
        <div className="flex flex-col items-center pt-4 pb-4">
          <div
            className="flex items-center justify-center rounded-lg"
            style={{
              width: 36,
              height: 36,
              background: '#ff5c5c',
            }}
          >
            <span className="text-white text-base font-bold">S</span>
          </div>
          <span
            className="mt-1"
            style={{
              fontSize: 9,
              color: '#7d8590',
              letterSpacing: '0.02em',
            }}
          >
            see-agent
          </span>
        </div>

        {/* Nav items */}
        <nav className="flex flex-col items-center gap-1 flex-1 pt-2">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              onClick={onClose}
              className="flex flex-col items-center justify-center rounded-lg transition-all"
              style={({ isActive }) => ({
                width: 56,
                height: 52,
                background: isActive ? 'rgba(255, 92, 92, 0.12)' : 'transparent',
                color: isActive ? '#ff5c5c' : '#7d8590',
              })}
            >
              {({ isActive }) => (
                <>
                  <item.icon size={18} strokeWidth={isActive ? 2 : 1.5} />
                  <span
                    style={{
                      fontSize: 10,
                      marginTop: 3,
                      fontWeight: isActive ? 500 : 400,
                    }}
                  >
                    {item.label}
                  </span>
                </>
              )}
            </NavLink>
          ))}
        </nav>

        {/* Bottom: Avatar */}
        <div className="flex flex-col items-center gap-2 pb-4">
          <div
            className="rounded-full"
            style={{
              width: 28,
              height: 28,
              background: '#30363d',
            }}
          />
        </div>
      </aside>
    </>
  )
}
