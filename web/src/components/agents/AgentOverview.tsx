import type { AgentDetail } from '@/types'
import { Card } from '@/components/ui/card'

interface Props {
  agent: AgentDetail
}

function InfoCard({ title, value, accent }: { title: string; value: string; accent?: boolean }) {
  return (
    <Card className="p-4">
      <p className="text-[11px] font-medium uppercase tracking-wide mb-1.5 text-[var(--muted)]">
        {title}
      </p>
      <p className={`text-sm font-medium ${accent ? 'text-[var(--accent)]' : 'text-[var(--text-strong)]'}`}>
        {value}
      </p>
    </Card>
  )
}

export default function AgentOverview({ agent }: Props) {
  const status = agent.status || 'idle'
  const model = (agent as any).llm?.model || 'default'

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
        <InfoCard title="ID" value={agent.id} />
        <InfoCard title="Status" value={status} accent={status === 'running' || status === 'busy'} />
        <InfoCard title="Team" value={agent.team_name || '—'} />
        <InfoCard title="Model" value={model} />
      </div>

      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide mb-2 text-[var(--muted)]">
          Workspace
        </h3>
        <Card className="p-3 text-sm font-mono text-[var(--text-strong)]">
          ~/.see-agent/agents/{agent.id}/
        </Card>
      </div>

      {agent.has_soul && (
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide mb-2 text-[var(--muted)]">
            SOUL.md
          </h3>
          <Card className="p-3 text-sm leading-relaxed text-[var(--text-strong)]">
            SOUL.md configured
          </Card>
        </div>
      )}
    </div>
  )
}
