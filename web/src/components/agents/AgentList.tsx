import type { AgentSummary } from '@/types'
import { Plus } from 'lucide-react'

const statusColors: Record<string, string> = {
  idle: 'var(--accent-2)',
  busy: 'var(--ok)',
}

interface AgentListProps {
  agents: AgentSummary[] | undefined
  selectedId: string | undefined
  onSelect: (id: string) => void
  onNewAgent: () => void
}

export default function AgentList({ agents, selectedId, onSelect, onNewAgent }: AgentListProps) {
  return (
    <div
      className="w-[280px] shrink-0 border-r flex flex-col h-full"
      style={{ borderColor: 'var(--border)' }}
    >
      <div className="px-4 pt-4 pb-2">
        <h2 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
          Agents
        </h2>
        <p className="text-xs" style={{ color: 'var(--muted)' }}>
          {agents?.length ?? 0} agents
        </p>
      </div>

      <div className="flex-1 overflow-y-auto px-3 pb-2">
        {agents?.map((a) => (
          <button
            key={a.id}
            onClick={() => onSelect(a.id)}
            className="w-full text-left rounded-[var(--radius)] p-3 mb-1 transition-colors"
            style={{
              background: selectedId === a.id ? 'var(--accent-subtle)' : 'transparent',
              borderLeft: selectedId === a.id ? '3px solid var(--accent)' : '3px solid transparent',
            }}
          >
            <div className="flex items-center gap-2">
              <span style={{ fontSize: 16 }}>{a.emoji || '🤖'}</span>
              <span
                className="text-sm font-medium"
                style={{ color: selectedId === a.id ? 'var(--accent)' : 'var(--text-strong)' }}
              >
                {a.name || a.id}
              </span>
            </div>
            <div className="flex items-center gap-1 mt-0.5 ml-[22px]">
              <span
                className="h-1.5 w-1.5 rounded-full"
                style={{ background: statusColors[a.status] || 'var(--muted)' }}
              />
              <span className="text-xs" style={{ color: statusColors[a.status] || 'var(--muted)' }}>
                {a.status}
              </span>
            </div>
          </button>
        ))}
      </div>

      <div className="px-3 pb-3">
        <button
          onClick={onNewAgent}
          className="w-full flex items-center justify-center gap-1.5 rounded-[var(--radius)] py-2 text-sm font-medium transition-colors hover:bg-[var(--bg-hover)]"
          style={{ color: 'var(--text)' }}
        >
          <Plus size={14} />
          New Agent
        </button>
      </div>
    </div>
  )
}
