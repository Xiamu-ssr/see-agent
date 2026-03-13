import { useState } from 'react'
import { Outlet } from 'react-router-dom'
import { useTheme } from '@/hooks/useTheme'
import Sidebar from './Sidebar'

export default function Layout() {
  useTheme()
  const [sidebarOpen, setSidebarOpen] = useState(false)

  return (
    <div className="h-screen flex flex-col" style={{ background: 'var(--bg)' }}>
      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />
      <main className="lg:pl-[72px] flex-1 min-h-0 flex flex-col">
        <div className="flex-1 min-h-0">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
