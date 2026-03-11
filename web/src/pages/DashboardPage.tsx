import { useCallback } from 'react'
import { usePolling } from '@/hooks/usePolling'
import { getDashboard } from '@/api/dashboard'
import type { DashboardResponse } from '@/types'
import { Bot, Users, ClipboardList, Zap } from 'lucide-react'

function StatCard({
  icon: Icon,
  title,
  value,
  details,
  color,
}: {
  icon: typeof Bot
  title: string
  value: number
  details: string
  color: string
}) {
  return (
    <div className="rounded-lg border p-5" style={{ background: '#161b22', borderColor: '#30363d' }}>
      <div className="flex items-center gap-3 mb-3">
        <div className="rounded-lg p-2" style={{ background: `${color}15` }}>
          <Icon size={18} style={{ color }} />
        </div>
        <span className="text-sm" style={{ color: '#7d8590' }}>{title}</span>
      </div>
      <p className="text-3xl font-bold" style={{ color: '#e6edf3' }}>{value}</p>
      <p className="mt-1 text-xs" style={{ color: '#7d8590' }}>{details}</p>
    </div>
  )
}

function formatStatus(obj: Record<string, number>): string {
  return Object.entries(obj).map(([k, v]) => `${v} ${k}`).join(', ') || 'none'
}

export default function DashboardPage() {
  const fetchDashboard = useCallback(() => getDashboard(), [])
  const { data, loading } = usePolling<DashboardResponse>(fetchDashboard, 10000)

  if (loading && !data) return <div style={{ color: '#7d8590' }}>Loading...</div>
  const d = data!

  return (
    <div>
      <h1 className="text-xl font-semibold mb-6" style={{ color: '#e6edf3' }}>Dashboard</h1>
      
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 mb-8">
        <StatCard icon={Bot} title="Agents" value={d.agents_in_team + d.agents_idle} details={`${d.agents_in_team} in team, ${d.agents_idle} idle`} color="#ff5c5c" />
        <StatCard icon={Users} title="Teams" value={d.teams_count} details={formatStatus(d.teams_by_status)} color="#3fb950" />
        <StatCard icon={ClipboardList} title="Tasks" value={d.total_tasks} details={formatStatus(d.tasks_by_status)} color="#d29922" />
        <StatCard icon={Zap} title="Skills" value={0} details="0 active" color="#a371f7" />
      </div>

      {/* Recent Activity placeholder */}
      <div>
        <h2 className="text-sm font-medium uppercase tracking-wide mb-3" style={{ color: '#7d8590' }}>
          Recent Activity
        </h2>
        <div className="rounded-lg border" style={{ background: '#161b22', borderColor: '#30363d' }}>
          <div className="p-4 text-sm" style={{ color: '#7d8590' }}>
            No recent activity.
          </div>
        </div>
      </div>
    </div>
  )
}
