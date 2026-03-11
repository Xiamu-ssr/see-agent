import type { AgentDetail } from '@/types'

interface Props {
  agent: AgentDetail
}

function InfoCard({ title, value }: { title: string; value: string }) {
  return (
    <div
      className="rounded-[var(--radius)] border p-4"
      style={{ background: 'var(--bg)', borderColor: 'var(--border)' }}
    >
      <p className="text-xs font-medium mb-1" style={{ color: 'var(--muted)' }}>{title}</p>
      <p className="text-sm" style={{ color: 'var(--text-strong)' }}>{value}</p>
    </div>
  )
}

export default function AgentOverview({ agent }: Props) {
  return (
    <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
      <InfoCard title="ID" value={agent.id} />
      <InfoCard title="Name" value={agent.name} />
      <InfoCard title="Role" value={agent.role} />
      <InfoCard title="Team" value={agent.team_name || '\u2014'} />
      <InfoCard title="Location" value={agent.location} />
      <InfoCard title="Has SOUL" value={agent.has_soul ? 'Yes' : 'No'} />
    </div>
  )
}
