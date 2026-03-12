import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate, Link } from 'react-router-dom'
import { getTeamStatus, getMessages, sendMessage, runTeam, stopTeam, updateTeam } from '@/api/teams'
import type { TeamStatus, TeamMessage } from '@/types'
import { ArrowLeft, Square, Settings, Plus, ExternalLink } from 'lucide-react'
import TeamSettings from '@/components/team/TeamSettings'

export default function TeamDetailPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [team, setTeam] = useState<TeamStatus | null>(null)
  const [messages, setMessages] = useState<TeamMessage[]>([])
  const [loading, setLoading] = useState(true)
  const [msgInput, setMsgInput] = useState('')
  const [recipient, setRecipient] = useState('')
  const [steer, setSteer] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [actionLoading, setActionLoading] = useState(false)
  const [showTaskModal, setShowTaskModal] = useState(false)
  const [taskInput, setTaskInput] = useState('')

  const loadData = useCallback(async () => {
    if (!id) return
    try {
      const [t, msgs] = await Promise.all([getTeamStatus(id), getMessages(id)])
      setTeam(t)
      setMessages(msgs)
      if (!recipient && t.members.length > 0) {
        setRecipient(t.leader || t.members[0].id)
      }
    } catch {
      setTeam(null)
    } finally {
      setLoading(false)
    }
  }, [id, recipient])

  useEffect(() => {
    loadData()
    const interval = setInterval(loadData, 5000)
    return () => clearInterval(interval)
  }, [loadData])

  const handleSend = async () => {
    if (!id || !msgInput.trim() || !recipient) return
    await sendMessage(id, recipient, msgInput)
    setMsgInput('')
    loadData()
  }

  const handleSendTask = async () => {
    if (!id || !taskInput.trim()) return
    setActionLoading(true)
    try {
      await runTeam(id, taskInput)
      setShowTaskModal(false)
      setTaskInput('')
      loadData()
    } finally {
      setActionLoading(false)
    }
  }

  const handleStop = async () => {
    if (!id) return
    setActionLoading(true)
    try {
      await stopTeam(id)
      loadData()
    } finally {
      setActionLoading(false)
    }
  }

  const handleSettingsSave = async (updates: { name?: string; members?: { id: string; role: string }[]; leader?: string }) => {
    if (!id) return
    await updateTeam(id, updates)
    setShowSettings(false)
    loadData()
  }

  if (loading) return <div style={{ color: '#7d8590' }}>Loading...</div>
  if (!team) return <div style={{ color: '#f85149' }}>Team not found</div>

  const statusColor = team.status === 'running' ? '#3fb950' : team.status === 'failed' ? '#f85149' : '#7d8590'

  const tasksByStatus = {
    todo: team.tasks.filter((t) => t.status === 'pending'),
    doing: team.tasks.filter((t) => t.status === 'claimed' || t.status === 'in_progress'),
    done: team.tasks.filter((t) => t.status === 'done' || t.status === 'completed'),
  }

  return (
    <div>
      {/* Header: ← Back + Team Name + Badge + Actions */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <button onClick={() => navigate('/teams')} className="rounded-md p-1.5 hover:bg-[#21262d]">
            <ArrowLeft size={16} style={{ color: '#e6edf3' }} />
          </button>
          <h1 className="text-2xl font-bold" style={{ color: '#e6edf3' }}>{team.name}</h1>
          <span
            className="inline-flex items-center gap-1.5 text-xs font-medium rounded-full px-2.5 py-1"
            style={{ color: statusColor, background: `${statusColor}20` }}
          >
            <span className="h-2 w-2 rounded-full" style={{ background: statusColor }} />
            [{team.status}]
          </span>
        </div>
        <div className="flex gap-2">
          <button onClick={() => setShowSettings(true)} className="rounded-md p-2 hover:bg-[#21262d]">
            <Settings size={16} style={{ color: '#7d8590' }} />
          </button>
          {team.status === 'running' ? (
            <button onClick={handleStop} disabled={actionLoading}
              className="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium text-white"
              style={{ background: '#f85149', opacity: actionLoading ? 0.6 : 1 }}>
              <Square size={14} /> Stop
            </button>
          ) : null}
        </div>
      </div>

      {/* Main: Left Members (40%) + Right Content (60%) */}
      <div className="flex gap-4" style={{ minHeight: 'calc(100vh - 200px)' }}>
        
        {/* LEFT: Members */}
        <div className="w-[40%] shrink-0">
          <div className="rounded-lg border p-5" style={{ background: '#161b22', borderColor: '#30363d' }}>
            <h2 className="text-lg font-semibold mb-4" style={{ color: '#e6edf3' }}>Members</h2>
            <div className="space-y-3">
              {team.members.map((m) => (
                <div key={m.id} className="flex items-center gap-3 rounded-lg p-3" style={{ background: '#0d1117' }}>
                  {/* Avatar circle */}
                  <div className="rounded-full flex items-center justify-center shrink-0"
                    style={{ width: 44, height: 44, background: '#30363d' }}>
                    <span className="text-sm font-medium" style={{ color: '#e6edf3' }}>
                      {m.id.charAt(0).toUpperCase()}
                    </span>
                  </div>
                  {/* Name + Role */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium" style={{ color: '#e6edf3' }}>{m.id}</span>
                      {m.id === team.leader && (
                        <span className="text-[10px] font-bold rounded px-1.5 py-0.5"
                          style={{ background: '#ff5c5c', color: 'white' }}>
                          LEADER
                        </span>
                      )}
                      <span className="text-[10px] rounded px-1.5 py-0.5" style={{ color: '#7d8590', background: '#21262d' }}>
                        {m.role}
                      </span>
                    </div>
                    <div className="flex items-center gap-1.5 mt-0.5">
                      <span className="h-1.5 w-1.5 rounded-full" style={{ background: '#3fb950' }} />
                      <span className="text-xs" style={{ color: '#7d8590' }}>Running</span>
                    </div>
                  </div>
                  {/* View link */}
                  <Link to={`/agents/${m.id}`} className="flex items-center gap-1 text-xs shrink-0"
                    style={{ color: '#7d8590' }}>
                    View <ExternalLink size={12} />
                  </Link>
                </div>
              ))}
            </div>
            {/* Add Member */}
            <button className="flex items-center gap-1.5 mt-4 text-sm" style={{ color: '#ff5c5c' }}>
              <Plus size={14} /> Add Member
            </button>
          </div>
        </div>

        {/* RIGHT: TaskBoard (top) + Messages (bottom) */}
        <div className="flex-1 flex flex-col gap-4">
          
          {/* Task Board */}
          <div className="rounded-lg border p-5" style={{ background: '#161b22', borderColor: '#30363d' }}>
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-lg font-semibold" style={{ color: '#e6edf3' }}>Task Board</h2>
              <button onClick={() => setShowTaskModal(true)}
                className="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium"
                style={{ color: '#ff5c5c', border: '1px solid #ff5c5c' }}>
                <Plus size={14} /> Send Task
              </button>
            </div>
            <div className="grid grid-cols-3 gap-3">
              {(['todo', 'doing', 'done'] as const).map((col) => (
                <div key={col}>
                  <p className="text-xs font-medium uppercase tracking-wide mb-2" style={{ color: '#7d8590' }}>
                    {col === 'doing' ? 'Doing' : col === 'todo' ? 'Todo' : 'Done'}
                  </p>
                  <div className="space-y-2">
                    {tasksByStatus[col].map((t) => (
                      <div key={t.id} className="rounded-lg border p-3"
                        style={{ background: '#0d1117', borderColor: '#30363d' }}>
                        <p className="text-sm" style={{ color: '#e6edf3' }}>{t.title}</p>
                        {t.assigned_to && (
                          <p className="text-xs mt-1" style={{ color: '#7d8590' }}>→ {t.assigned_to}</p>
                        )}
                      </div>
                    ))}
                    {tasksByStatus[col].length === 0 && (
                      <div className="rounded-lg border border-dashed p-4 text-center"
                        style={{ borderColor: '#30363d' }}>
                        <p className="text-xs" style={{ color: '#7d8590' }}>Empty</p>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Team Messages */}
          <div className="rounded-lg border p-5 flex-1 flex flex-col" style={{ background: '#161b22', borderColor: '#30363d' }}>
            <h2 className="text-lg font-semibold mb-3" style={{ color: '#e6edf3' }}>Team Messages</h2>
            
            {/* Message list */}
            <div className="flex-1 overflow-y-auto space-y-2 mb-4 min-h-[80px]">
              {messages.map((m, i) => (
                <p key={i} className="text-sm" style={{ color: '#e6edf3' }}>
                  <span style={{ color: '#7d8590' }}>{m.sender} to {m.recipient}:</span>{' '}
                  {m.content}
                </p>
              ))}
              {messages.length === 0 && (
                <p className="text-sm" style={{ color: '#7d8590' }}>No messages yet.</p>
              )}
            </div>

            {/* Input: To dropdown + message + Steer + Send */}
            <div className="flex items-center gap-2">
              <span className="text-sm shrink-0" style={{ color: '#7d8590' }}>To:</span>
              <select value={recipient} onChange={(e) => setRecipient(e.target.value)}
                className="rounded-md border px-2 py-1.5 text-sm shrink-0"
                style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3' }}>
                {team.members.map((m) => (
                  <option key={m.id} value={m.id}>{m.id} {m.id === team.leader ? '(leader)' : ''}</option>
                ))}
              </select>
              <input value={msgInput} onChange={(e) => setMsgInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                placeholder="Type a message..."
                className="flex-1 rounded-md border px-3 py-1.5 text-sm outline-none"
                style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3' }} />
              <label className="flex items-center gap-1 text-xs shrink-0" style={{ color: '#7d8590' }}>
                <input type="checkbox" checked={steer} onChange={(e) => setSteer(e.target.checked)}
                  style={{ accentColor: '#ff5c5c' }} />
                Steer
              </label>
              <button onClick={handleSend}
                className="rounded-md px-3 py-1.5 text-sm font-medium shrink-0"
                style={{ color: '#7d8590', border: '1px solid #30363d' }}>
                Send
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Send Task modal */}
      {showTaskModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="w-full max-w-md rounded-lg border p-6" style={{ background: '#161b22', borderColor: '#30363d' }}>
            <h2 className="text-base font-semibold mb-4" style={{ color: '#e6edf3' }}>Send Task</h2>
            <input placeholder="Describe the task..." value={taskInput}
              onChange={(e) => setTaskInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSendTask()}
              className="w-full rounded-md border px-3 py-2 text-sm outline-none mb-4"
              style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3' }} />
            <div className="flex gap-2 justify-end">
              <button onClick={() => { setShowTaskModal(false); setTaskInput('') }}
                className="px-3 py-1.5 text-sm" style={{ color: '#7d8590' }}>Cancel</button>
              <button onClick={handleSendTask} disabled={!taskInput.trim() || actionLoading}
                className="rounded-md px-3 py-1.5 text-sm font-medium text-white"
                style={{ background: '#ff5c5c', opacity: !taskInput.trim() || actionLoading ? 0.5 : 1 }}>Send</button>
            </div>
          </div>
        </div>
      )}

      <TeamSettings open={showSettings} onClose={() => setShowSettings(false)}
        team={team} onSave={handleSettingsSave} />
    </div>
  )
}
