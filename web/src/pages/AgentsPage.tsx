import { useCallback, useState, useEffect } from 'react'
import { useNavigate, useParams, useLocation } from 'react-router-dom'
import { usePolling } from '@/hooks/usePolling'
import { listAgents, createAgent, getAgent } from '@/api/agents'
import type { AgentSummary, AgentDetail, CreateAgentRequest } from '@/types'
import { Bot } from 'lucide-react'
import AgentList from '@/components/agents/AgentList'
import AgentOverview from '@/components/agents/AgentOverview'
import AgentFiles from '@/components/agents/AgentFiles'
import AgentTools from '@/components/agents/AgentTools'
import AgentSkills from '@/components/agents/AgentSkills'
import AgentSafehouse from '@/components/agents/AgentSafehouse'
import AgentChat from '@/components/agents/AgentChat'

type DetailsTab = 'overview' | 'files' | 'tools' | 'skills' | 'safehouse'

export default function AgentsPage() {
  const navigate = useNavigate()
  const { id } = useParams<{ id: string }>()
  const location = useLocation()
  const isChat = location.pathname.endsWith('/chat')

  const fetchAgents = useCallback(() => listAgents(), [])
  const { data: agents, refresh } = usePolling<AgentSummary[]>(fetchAgents, 10000)

  const [showCreate, setShowCreate] = useState(false)
  const [form, setForm] = useState<CreateAgentRequest>({ name: '', role: 'general assistant' })
  const [agent, setAgent] = useState<AgentDetail | null>(null)
  const [detailsTab, setDetailsTab] = useState<DetailsTab>('overview')

  // Load agent detail when id changes
  useEffect(() => {
    if (!id) {
      setAgent(null)
      return
    }
    getAgent(id)
      .then(setAgent)
      .catch(() => setAgent(null))
  }, [id])

  const handleCreate = async () => {
    if (!form.name) return
    await createAgent(form)
    setShowCreate(false)
    setForm({ name: '', role: 'general assistant' })
    refresh()
  }

  const detailsTabs: { key: DetailsTab; label: string }[] = [
    { key: 'overview', label: 'Overview' },
    { key: 'files', label: 'Files' },
    { key: 'tools', label: 'Tools' },
    { key: 'skills', label: 'Skills' },
    { key: 'safehouse', label: 'Safehouse' },
  ]

  return (
    <div className="flex h-[calc(100vh-96px)]">
      {/* Left panel: Agent list */}
      <AgentList
        agents={agents ?? undefined}
        selectedId={id}
        onSelect={(agentId) => navigate(`/agents/${agentId}`)}
        onNewAgent={() => setShowCreate(true)}
      />

      {/* Right panel */}
      <div className="flex-1 overflow-y-auto p-6">
        {!id ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <Bot size={48} style={{ color: 'var(--muted)', margin: '0 auto 12px' }} />
              <p className="text-sm" style={{ color: 'var(--muted)' }}>Select an agent</p>
            </div>
          </div>
        ) : !agent ? (
          <div style={{ color: 'var(--muted)' }}>Loading...</div>
        ) : (
          <>
            {/* Agent header */}
            <div className="mb-5">
              <div className="flex items-center gap-3 mb-1">
                <h1 className="text-xl font-semibold" style={{ color: '#e6edf3' }}>
                  {agent.name}
                </h1>
                <span
                  className="text-xs font-medium rounded-full px-2.5 py-0.5"
                  style={{
                    background: agent.status === 'busy' ? 'rgba(63, 185, 80, 0.15)' : 'rgba(125, 133, 144, 0.15)',
                    color: agent.status === 'busy' ? '#3fb950' : '#7d8590',
                  }}
                >
                  {agent.status === 'busy' ? 'Running' : 'Idle'}
                </span>
              </div>
              <p className="text-sm mb-4" style={{ color: '#7d8590' }}>{agent.role}</p>

              {/* Details / Chat toggle */}
              <div className="flex gap-2">
                <button
                  onClick={() => navigate(`/agents/${id}`)}
                  className="rounded-full px-4 py-1.5 text-sm font-medium transition-all"
                  style={{
                    background: !isChat ? '#ff5c5c' : 'transparent',
                    color: !isChat ? 'white' : '#7d8590',
                    border: !isChat ? '1px solid #ff5c5c' : '1px solid #30363d',
                  }}
                >
                  Details
                </button>
                <button
                  onClick={() => navigate(`/agents/${id}/chat`)}
                  className="rounded-full px-4 py-1.5 text-sm font-medium transition-all"
                  style={{
                    background: isChat ? '#ff5c5c' : 'transparent',
                    color: isChat ? 'white' : '#7d8590',
                    border: isChat ? '1px solid #ff5c5c' : '1px solid #30363d',
                  }}
                >
                  Chat
                </button>
              </div>
            </div>

            {isChat ? (
              <div
                className="rounded-[var(--radius-lg)] border p-5"
                style={{ background: 'var(--card)', borderColor: 'var(--border)', height: 'calc(100% - 120px)' }}
              >
                <AgentChat agentId={id} />
              </div>
            ) : (
              <>
                {/* Details tabs */}
                <div className="flex gap-0 mb-4" style={{ borderBottom: '1px solid #30363d' }}>
                  {detailsTabs.map((t) => (
                    <button
                      key={t.key}
                      onClick={() => setDetailsTab(t.key)}
                      className="px-4 py-2 text-sm transition-colors relative"
                      style={{
                        color: detailsTab === t.key ? '#ff5c5c' : '#7d8590',
                        fontWeight: detailsTab === t.key ? 500 : 400,
                      }}
                    >
                      {t.label}
                      {detailsTab === t.key && (
                        <span
                          className="absolute bottom-0 left-0 right-0 h-0.5 rounded-full"
                          style={{ background: '#ff5c5c' }}
                        />
                      )}
                    </button>
                  ))}
                </div>

                {/* Tab content */}
                <div
                  className="rounded-[var(--radius-lg)] border p-5"
                  style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
                >
                  {detailsTab === 'overview' && <AgentOverview agent={agent} />}
                  {detailsTab === 'files' && <AgentFiles agentId={id} />}
                  {detailsTab === 'tools' && <AgentTools agent={agent} />}
                  {detailsTab === 'skills' && <AgentSkills agent={agent} />}
                  {detailsTab === 'safehouse' && <AgentSafehouse agent={agent} />}
                </div>
              </>
            )}
          </>
        )}
      </div>

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
