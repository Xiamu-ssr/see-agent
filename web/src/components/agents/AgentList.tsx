import type { AgentSummary } from '@/types'
import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'

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
    <div className="w-[160px] shrink-0 border-r border-[var(--border)] flex flex-col h-full">
      <div className="px-3 pt-4 pb-2">
        <h2 className="text-lg font-semibold text-[var(--text-strong)]">
          Agents
        </h2>
        <p className="text-xs text-[var(--muted)]">
          {agents?.length ?? 0} agents
        </p>
      </div>

      <ScrollArea className="flex-1 px-2 pb-2">
        {agents?.map((a) => (
          <button
            key={a.id}
            onClick={() => onSelect(a.id)}
            className="w-full text-left rounded-[var(--radius)] px-2 py-1.5 mb-0.5 transition-colors"
            style={{
              background: selectedId === a.id ? 'var(--accent-subtle)' : 'transparent',
              borderLeft: selectedId === a.id ? '3px solid var(--accent)' : '3px solid transparent',
            }}
          >
            <div className="flex items-center gap-1.5">
              <span className="text-[14px]">{a.emoji || '🤖'}</span>
              <span
                className="text-sm font-medium truncate"
                style={{ color: selectedId === a.id ? 'var(--accent)' : 'var(--text-strong)' }}
              >
                {a.name || a.id}
              </span>
              <span
                className="ml-auto h-1.5 w-1.5 rounded-full shrink-0"
                style={{ background: statusColors[a.status] || 'var(--muted)' }}
              />
              <span className="text-[10px] shrink-0" style={{ color: statusColors[a.status] || 'var(--muted)' }}>
                {a.status}
              </span>
            </div>
          </button>
        ))}
      </ScrollArea>

      <div className="px-2 pb-3">
        <Button
          variant="ghost"
          onClick={onNewAgent}
          className="w-full"
          size="sm"
        >
          <Plus size={14} />
          New Agent
        </Button>
      </div>
    </div>
  )
}
