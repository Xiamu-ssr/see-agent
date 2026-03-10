import { useCallback, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { usePolling } from '@/hooks/usePolling'
import { listAgents, createAgent } from '@/api/agents'
import type { Agent, CreateAgentPayload } from '@/types/agent'
import { Plus, Bot } from 'lucide-react'

const statusColors: Record<string, string> = {
  idle: 'var(--accent-2)',
  busy: 'var(--ok)',
}

export default function AgentsPage() {
  const navigate = useNavigate()
  const fetchAgents = useCallback(() => listAgents(), [])
  const { data: agents, loading, refresh } = usePolling<Agent[]>(fetchAgents, 10000)
  const [showCreate, setShowCreate] = useState(false)
  const [form, setForm] = useState<CreateAgentPayload>({ id: '', name: '', role: 'general assistant' })

  const handleCreate = async () => {
    if (!form.id || !form.name) return
    await createAgent(form)
    setShowCreate(false)
    setForm({ id: '', name: '', role: 'general assistant' })
    refresh()
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
          Agents
        </h1>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-1.5 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white"
          style={{ background: 'var(--accent)' }}
        >
          <Plus size={14} />
          New Agent
        </button>
      </div>

      {loading && !agents ? (
        <div style={{ color: 'var(--muted)' }}>Loading...</div>
      ) : (
        <div
          className="overflow-hidden rounded-[var(--radius-lg)] border"
          style={{ borderColor: 'var(--border)' }}
        >
          <table className="w-full text-sm">
            <thead>
              <tr style={{ background: 'var(--bg-elevated)' }}>
                <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>
                  Name
                </th>
                <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>
                  Role
                </th>
                <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>
                  Team
                </th>
                <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>
                  Status
                </th>
                <th className="text-right px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {agents?.map((a) => (
                <tr
                  key={a.id}
                  className="border-t hover:bg-[var(--bg-hover)] transition-colors"
                  style={{ borderColor: 'var(--border)' }}
                >
                  <td className="px-4 py-2.5">
                    <div className="flex items-center gap-2">
                      <Bot size={14} style={{ color: 'var(--accent)' }} />
                      <span style={{ color: 'var(--text-strong)' }}>{a.name}</span>
                    </div>
                  </td>
                  <td className="px-4 py-2.5" style={{ color: 'var(--text)' }}>
                    {a.role}
                  </td>
                  <td className="px-4 py-2.5" style={{ color: 'var(--muted)' }}>
                    {a.team_name || '—'}
                  </td>
                  <td className="px-4 py-2.5">
                    <span
                      className="inline-flex items-center gap-1 text-xs"
                      style={{ color: statusColors[a.status] || 'var(--muted)' }}
                    >
                      <span
                        className="h-1.5 w-1.5 rounded-full"
                        style={{ background: statusColors[a.status] || 'var(--muted)' }}
                      />
                      {a.status}
                    </span>
                  </td>
                  <td className="px-4 py-2.5 text-right">
                    <button
                      onClick={() => navigate(`/agents/${a.id}`)}
                      className="text-xs px-2 py-1 rounded hover:bg-[var(--bg-hover)]"
                      style={{ color: 'var(--accent)' }}
                    >
                      Detail
                    </button>
                  </td>
                </tr>
              ))}
              {agents?.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-8 text-center" style={{ color: 'var(--muted)' }}>
                    No agents yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* Create modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div
            className="w-full max-w-md rounded-[var(--radius-lg)] border p-6"
            style={{ background: 'var(--bg-elevated)', borderColor: 'var(--border)' }}
          >
            <h2 className="text-base font-semibold mb-4" style={{ color: 'var(--text-strong)' }}>
              Create Agent
            </h2>
            <div className="flex flex-col gap-3">
              <input
                placeholder="Agent ID"
                value={form.id}
                onChange={(e) => setForm({ ...form, id: e.target.value })}
                className="rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
                style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
              />
              <input
                placeholder="Display name"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                className="rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
                style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
              />
              <input
                placeholder="Role"
                value={form.role || ''}
                onChange={(e) => setForm({ ...form, role: e.target.value })}
                className="rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
                style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
              />
              <div className="flex gap-2 justify-end mt-2">
                <button
                  onClick={() => setShowCreate(false)}
                  className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm"
                  style={{ color: 'var(--muted)' }}
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreate}
                  className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm font-medium text-white"
                  style={{ background: 'var(--accent)' }}
                >
                  Create
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
