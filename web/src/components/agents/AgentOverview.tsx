import type { AgentDetail } from '@/types'

interface Props {
  agent: AgentDetail
}

function InfoCard({ title, value, accent }: { title: string; value: string; accent?: boolean }) {
  return (
    <div
      className="rounded-lg border p-4"
      style={{ background: '#0d1117', borderColor: '#30363d' }}
    >
      <p className="text-[11px] font-medium uppercase tracking-wide mb-1.5" style={{ color: '#7d8590' }}>
        {title}
      </p>
      <p className="text-sm font-medium" style={{ color: accent ? '#ff5c5c' : '#e6edf3' }}>
        {value}
      </p>
    </div>
  )
}

export default function AgentOverview({ agent }: Props) {
  const status = agent.status || 'idle'
  const model = (agent as any).llm?.model || 'default'

  return (
    <div className="space-y-6">
      {/* Info grid */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
        <InfoCard title="ID" value={agent.id} />
        <InfoCard title="Status" value={status} accent={status === 'running' || status === 'busy'} />
        <InfoCard title="Team" value={agent.team_name || '—'} />
        <InfoCard title="Model" value={model} />
      </div>

      {/* Workspace info */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide mb-2" style={{ color: '#7d8590' }}>
          Workspace
        </h3>
        <div
          className="rounded-lg border p-3 text-sm"
          style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3', fontFamily: 'var(--mono, monospace)' }}
        >
          ~/.see-agent/agents/{agent.id}/
        </div>
      </div>

      {/* SOUL preview */}
      {agent.has_soul && (
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide mb-2" style={{ color: '#7d8590' }}>
            SOUL.md
          </h3>
          <div
            className="rounded-lg border p-3 text-sm leading-relaxed"
            style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3' }}
          >
            SOUL.md configured
          </div>
        </div>
      )}
    </div>
  )
}
