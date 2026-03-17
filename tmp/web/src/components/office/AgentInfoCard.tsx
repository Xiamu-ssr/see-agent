import { X } from 'lucide-react'

interface AgentInfoCardProps {
  agentId: string
  agentName: string
  role?: string
  status?: string
  currentTask?: string
  onClose: () => void
  onViewDetail: () => void
}

const statusColor: Record<string, string> = {
  idle: 'var(--muted)',
  working: 'var(--ok)',
  busy: 'var(--warn)',
  error: 'var(--warn)',
}

export default function AgentInfoCard({
  agentName,
  role,
  status,
  currentTask,
  onClose,
  onViewDetail,
}: AgentInfoCardProps) {
  const initial = agentName.charAt(0).toUpperCase()
  const badgeColor = statusColor[status ?? ''] ?? 'var(--muted)'

  return (
    <div
      className="relative w-[250px] rounded-[var(--radius-lg)] border p-4"
      style={{
        background: 'var(--bg-elevated)',
        borderColor: 'var(--border)',
        boxShadow: '0 4px 24px rgba(0,0,0,.12)',
      }}
    >
      {/* Close button */}
      <button
        onClick={onClose}
        className="absolute right-2 top-2 rounded-[var(--radius-sm)] p-1 transition-colors hover:bg-[var(--accent-subtle)]"
        style={{ color: 'var(--muted)' }}
        aria-label="Close"
      >
        <X size={14} />
      </button>

      {/* Avatar + name row */}
      <div className="flex items-center gap-3">
        <div
          className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-sm font-semibold"
          style={{
            background: 'var(--accent-subtle)',
            color: 'var(--accent)',
          }}
        >
          {initial}
        </div>

        <div className="min-w-0">
          <p
            className="truncate text-sm font-semibold leading-tight"
            style={{ color: 'var(--text-strong)' }}
          >
            {agentName}
          </p>
          {role && (
            <p
              className="truncate text-xs leading-tight"
              style={{ color: 'var(--muted)' }}
            >
              {role}
            </p>
          )}
        </div>
      </div>

      {/* Status badge */}
      {status && (
        <div className="mt-3 flex items-center gap-1.5">
          <span
            className="inline-block h-2 w-2 rounded-full"
            style={{ background: badgeColor }}
          />
          <span
            className="text-xs capitalize"
            style={{ color: badgeColor }}
          >
            {status}
          </span>
        </div>
      )}

      {/* Current task */}
      {currentTask && (
        <div className="mt-2">
          <p
            className="text-[11px] font-medium uppercase tracking-wider"
            style={{ color: 'var(--muted)' }}
          >
            Current Task
          </p>
          <p
            className="mt-0.5 line-clamp-2 text-xs leading-snug"
            style={{ color: 'var(--text-strong)' }}
          >
            {currentTask}
          </p>
        </div>
      )}

      {/* View detail link */}
      <button
        onClick={onViewDetail}
        className="mt-3 text-xs font-medium transition-colors hover:underline"
        style={{ color: 'var(--accent)' }}
      >
        View Detail
      </button>
    </div>
  )
}
