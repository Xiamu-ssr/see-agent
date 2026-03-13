import { useCallback } from 'react'
import { usePolling } from '@/hooks/usePolling'
import { getDashboard } from '@/api/dashboard'
import type { DashboardResponse } from '@/types'
import { Bot, Users, ClipboardList, Zap } from 'lucide-react'
import { Card } from '@/components/ui/card'

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
    <Card className="p-5">
      <div className="flex items-center gap-3 mb-3">
        <div className="rounded-lg p-2" style={{ background: `${color}15` }}>
          <Icon size={18} style={{ color }} />
        </div>
        <span className="text-sm text-[var(--muted)]">{title}</span>
      </div>
      <p className="text-3xl font-bold text-[var(--text-strong)]">{value}</p>
      <p className="mt-1 text-xs text-[var(--muted)]">{details}</p>
    </Card>
  )
}

function formatStatus(obj: Record<string, number>): string {
  return Object.entries(obj).map(([k, v]) => `${v} ${k}`).join(', ') || 'none'
}

export default function DashboardPage() {
  const fetchDashboard = useCallback(() => getDashboard(), [])
  const { data, loading } = usePolling<DashboardResponse>(fetchDashboard, 10000)

  if (loading && !data) return <div className="text-[var(--muted)]">Loading...</div>
  const d = data!

  return (
    <div>
      <h1 className="text-xl font-semibold mb-6 text-[var(--text-strong)]">Dashboard</h1>
      
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 mb-8">
        <StatCard icon={Bot} title="Agents" value={d.agents_in_team + d.agents_idle} details={`${d.agents_in_team} in team, ${d.agents_idle} idle`} color="#ff5c5c" />
        <StatCard icon={Users} title="Teams" value={d.teams_count} details={formatStatus(d.teams_by_status)} color="#3fb950" />
        <StatCard icon={ClipboardList} title="Tasks" value={d.total_tasks} details={formatStatus(d.tasks_by_status)} color="#d29922" />
        <StatCard icon={Zap} title="Skills" value={0} details="0 active" color="#a371f7" />
      </div>

      <div>
        <h2 className="text-sm font-medium uppercase tracking-wide mb-3 text-[var(--muted)]">
          Recent Activity
        </h2>
        <Card>
          <div className="p-4 text-sm text-[var(--muted)]">
            No recent activity.
          </div>
        </Card>
      </div>
    </div>
  )
}
