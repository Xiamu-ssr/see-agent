import { useState } from 'react'
import { Outlet, useLocation } from 'react-router-dom'
import { useTheme } from '@/hooks/useTheme'
import { Menu } from 'lucide-react'
import Sidebar from './Sidebar'
import MobileTabBar from './MobileTabBar'

const pageTitle: Record<string, string> = {
  '/agents': 'Agents',
  '/teams': 'Teams',
  '/skills': 'Skills',
  '/mcp': 'MCP',
  '/config': 'Config',
  '/logs': 'Logs',
  '/dashboard': 'Dashboard',
}

export default function Layout() {
  useTheme()
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const location = useLocation()

  const title = Object.entries(pageTitle).find(([path]) =>
    location.pathname.startsWith(path),
  )?.[1] ?? 'see-agent'

  return (
    <div className="h-screen flex flex-col" style={{ background: 'var(--bg)' }}>
      {/* Mobile header — visible below lg */}
      <header className="lg:hidden sticky top-0 z-40 flex items-center h-12 px-4 border-b border-[var(--border)] bg-[var(--bg-elevated)]">
        <button
          onClick={() => setSidebarOpen(true)}
          className="p-1.5 -ml-1 rounded-lg hover:bg-[var(--bg-hover)] transition-colors"
        >
          <Menu size={20} style={{ color: 'var(--text)' }} />
        </button>
        <span className="ml-3 text-sm font-semibold text-[var(--text-strong)]">
          {title}
        </span>
      </header>

      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

      <main className="lg:pl-[72px] flex-1 min-h-0 flex flex-col pb-14 md:pb-0">
        <div className="flex-1 min-h-0">
          <Outlet />
        </div>
      </main>

      {/* Mobile tab bar — phone only */}
      <MobileTabBar />
    </div>
  )
}
