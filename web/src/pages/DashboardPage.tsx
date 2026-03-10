import { useCallback } from 'react'
import { usePolling } from '@/hooks/usePolling'
import { getDashboard } from '@/api/dashboard'
import type { DashboardData } from '@/api/dashboard'
import { BarChart3, Bot, Users, ClipboardList } from 'lucide-react'

function StatCard({
  icon: Icon,
  title,
  value,
  details,
}: {
  icon: typeof BarChart3
  title: string
  value: number
  details: string
}) {
  return (
    <div
      className="rounded-[var(--radius-lg)] border p-5"
      style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
    >
      <div className="flex items-center gap-3 mb-3">
        <div
          className="rounded-[var(--radius-sm)] p-2"
          style={{ background: 'var(--accent-subtle)' }}
        >
          <Icon size={18} style={{ color: 'var(--accent)' }} />
        </div>
        <span className="text-sm" style={{ color: 'var(--muted)' }}>
          {title}
        </span>
      </div>
      <p className="text-3xl font-bold" style={{ color: 'var(--text-strong)' }}>
        {value}
      </p>
      <p className="mt-1 text-xs" style={{ color: 'var(--muted)' }}>
        {details}
      </p>
    </div>
  )
}

function formatStatus(obj: Record<string, number>): string {
  return Object.entries(obj)
    .map(([k, v]) => `${v} ${k}`)
    .join(', ') || 'none'
}

export default function DashboardPage() {
  const fetchDashboard = useCallback(() => getDashboard(), [])
  const { data, loading } = usePolling<DashboardData>(fetchDashboard, 10000)

  if (loading && !data) {
    return <div style={{ color: 'var(--muted)' }}>Loading...</div>
  }

  const d = data!

  return (
    <div>
      <h1 className="text-lg font-semibold mb-6" style={{ color: 'var(--text-strong)' }}>
        Dashboard
      </h1>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <StatCard
          icon={Users}
          title="Teams"
          value={d.teams_count}
          details={formatStatus(d.teams_by_status)}
        />
        <StatCard
          icon={Bot}
          title="Agents"
          value={d.agents_in_team + d.agents_idle}
          details={`${d.agents_in_team} in team, ${d.agents_idle} idle`}
        />
        <StatCard
          icon={ClipboardList}
          title="Tasks"
          value={d.total_tasks}
          details={formatStatus(d.tasks_by_status)}
        />
      </div>
    </div>
  )
}
