import { NavLink } from 'react-router-dom'
import {
  Bot,
  Users,
  Sparkles,
  SlidersHorizontal,
  FileText,
} from 'lucide-react'

const tabs = [
  { to: '/agents', icon: Bot, label: 'Agents' },
  { to: '/teams', icon: Users, label: 'Teams' },
  { to: '/skills', icon: Sparkles, label: 'Skills' },
  { to: '/config', icon: SlidersHorizontal, label: 'Config' },
  { to: '/logs', icon: FileText, label: 'Logs' },
]

export default function MobileTabBar() {
  return (
    <nav className="fixed bottom-0 left-0 right-0 z-50 md:hidden border-t border-[var(--border)] bg-[var(--bg-elevated)]">
      <div className="flex items-center justify-around h-14">
        {tabs.map((tab) => (
          <NavLink
            key={tab.to}
            to={tab.to}
            className="flex flex-col items-center justify-center gap-0.5 flex-1 h-full transition-colors duration-150"
            style={({ isActive }) => ({
              color: isActive ? 'var(--accent)' : 'var(--muted)',
            })}
          >
            {({ isActive }) => (
              <>
                <tab.icon size={18} strokeWidth={isActive ? 2 : 1.5} />
                <span className="text-[10px]" style={{ fontWeight: isActive ? 500 : 400 }}>
                  {tab.label}
                </span>
              </>
            )}
          </NavLink>
        ))}
      </div>
    </nav>
  )
}
