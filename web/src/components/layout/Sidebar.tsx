import { NavLink } from 'react-router-dom'
import {
  Bot,
  Users,
  Sparkles,
  Plug,
  Settings,
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
  { to: '/config', icon: Settings, label: 'Config' },
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
          fixed top-12 left-0 z-50 h-[calc(100vh-48px)] w-[200px]
          border-r bg-[var(--bg-elevated)] transition-transform duration-200
          lg:translate-x-0
          ${open ? 'translate-x-0' : '-translate-x-full'}
        `}
        style={{ borderColor: 'var(--border)' }}
      >
        <nav className="flex flex-col gap-1 p-3">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              onClick={onClose}
              className={({ isActive }) =>
                `flex items-center gap-2 rounded-[var(--radius-sm)] px-2 py-1.5 text-sm transition-colors ${
                  isActive
                    ? 'bg-[var(--accent-subtle)] font-medium'
                    : 'hover:bg-[var(--bg-hover)]'
                }`
              }
              style={({ isActive }) => ({
                color: isActive ? 'var(--accent)' : 'var(--text)',
              })}
            >
              <item.icon size={16} />
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>
    </>
  )
}
