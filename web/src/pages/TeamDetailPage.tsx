import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { getTeamStatus, getMessages, sendMessage, runTeam, stopTeam, updateTeam } from '@/api/teams'
import type { TeamStatus, TeamMessage } from '@/types'
import { ArrowLeft, Send, Play, Square, Settings } from 'lucide-react'
import PixelOffice from '@/components/office/PixelOffice'
import AgentInfoCard from '@/components/office/AgentInfoCard'
import TeamSettings from '@/components/team/TeamSettings'

export default function TeamDetailPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [team, setTeam] = useState<TeamStatus | null>(null)
  const [messages, setMessages] = useState<TeamMessage[]>([])
  const [loading, setLoading] = useState(true)
  const [msgInput, setMsgInput] = useState('')
  const [recipient, setRecipient] = useState('')
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null)
  const [showSettings, setShowSettings] = useState(false)
  const [actionLoading, setActionLoading] = useState(false)

  const loadData = useCallback(async () => {
    if (!id) return
    try {
      const [t, msgs] = await Promise.all([getTeamStatus(id), getMessages(id)])
      setTeam(t)
      setMessages(msgs)
      if (!recipient && t.leader) setRecipient(t.leader)
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

  const handleRun = async () => {
    if (!id) return
    setActionLoading(true)
    try {
      const task = prompt('Enter task description:')
      if (task) {
        await runTeam(id, task)
        loadData()
      }
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

  const handleSettingsSave = async (updates: { name?: string; members?: string[]; leader?: string }) => {
    if (!id) return
    await updateTeam(id, updates)
    setShowSettings(false)
    loadData()
  }

  if (loading) return <div style={{ color: 'var(--muted)' }}>Loading...</div>
  if (!team) return <div style={{ color: 'var(--danger)' }}>Team not found</div>

  const statusColor =
    team.status === 'running'
      ? 'var(--ok)'
      : team.status === 'failed'
        ? 'var(--danger)'
        : 'var(--muted)'

  const tasksByStatus = {
    pending: team.tasks.filter((t) => t.status === 'pending'),
    claimed: team.tasks.filter((t) => t.status === 'claimed' || t.status === 'in_progress'),
    done: team.tasks.filter((t) => t.status === 'done' || t.status === 'completed'),
  }

  return (
    <div className="flex flex-col h-[calc(100vh-96px)]">
      {/* Sub-topbar */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <button
            onClick={() => navigate('/teams')}
            className="hover:bg-[var(--bg-hover)] rounded p-1"
          >
            <ArrowLeft size={16} style={{ color: 'var(--text)' }} />
          </button>
          <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
            {team.name}
          </h1>
          <span
            className="inline-flex items-center gap-1 text-xs rounded-full px-2 py-0.5"
            style={{ color: statusColor, background: 'var(--bg-hover)' }}
          >
            <span className="h-1.5 w-1.5 rounded-full" style={{ background: statusColor }} />
            {team.status}
          </span>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setShowSettings(true)}
            className="flex items-center gap-1 rounded-[var(--radius)] px-3 py-1.5 text-sm hover:bg-[var(--bg-hover)]"
            style={{ color: 'var(--muted)' }}
          >
            <Settings size={14} />
          </button>
          {team.status === 'running' ? (
            <button
              onClick={handleStop}
              disabled={actionLoading}
              className="flex items-center gap-1 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white"
              style={{ background: 'var(--danger)', opacity: actionLoading ? 0.6 : 1 }}
            >
              <Square size={14} />
              Stop
            </button>
          ) : (
            <button
              onClick={handleRun}
              disabled={actionLoading}
              className="flex items-center gap-1 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white"
              style={{ background: 'var(--ok)', opacity: actionLoading ? 0.6 : 1 }}
            >
              <Play size={14} />
              Run
            </button>
          )}
        </div>
      </div>

      {/* Pixel office */}
      <div className="relative mb-4">
        <PixelOffice
          members={team.members}
          seating={{}}
          onAgentClick={(agentId) => setSelectedAgent(agentId === selectedAgent ? null : agentId)}
        />
        {/* Agent info card overlay */}
        {selectedAgent && (
          <div className="absolute top-4 right-4 z-10">
            <AgentInfoCard
              agentId={selectedAgent}
              agentName={selectedAgent}
              status="idle"
              onClose={() => setSelectedAgent(null)}
              onViewDetail={() => navigate(`/agents/${selectedAgent}`)}
            />
          </div>
        )}
      </div>

      {/* Bottom: TaskBoard + Messages */}
      <div className="grid grid-cols-1 lg:grid-cols-5 gap-4 flex-1 min-h-0">
        {/* Task Board */}
        <div
          className="lg:col-span-3 rounded-[var(--radius-lg)] border p-4 overflow-auto"
          style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
        >
          <h3 className="text-sm font-medium mb-3" style={{ color: 'var(--text-strong)' }}>
            Task Board
          </h3>
          <div className="grid grid-cols-3 gap-3">
            {(['pending', 'claimed', 'done'] as const).map((col) => (
              <div key={col}>
                <p className="text-xs font-medium uppercase mb-2" style={{ color: 'var(--muted)' }}>
                  {col === 'claimed' ? 'In Progress' : col} ({tasksByStatus[col].length})
                </p>
                <div className="space-y-2">
                  {tasksByStatus[col].map((t) => (
                    <div
                      key={t.id}
                      className="rounded-[var(--radius-sm)] border p-2 text-xs"
                      style={{ background: 'var(--bg)', borderColor: 'var(--border)' }}
                    >
                      <p style={{ color: 'var(--text-strong)' }}>{t.title}</p>
                      {t.assigned_to && (
                        <p className="mt-1" style={{ color: 'var(--muted)' }}>
                          &rarr; {t.assigned_to}
                        </p>
                      )}
                    </div>
                  ))}
                  {tasksByStatus[col].length === 0 && (
                    <p className="text-[10px] text-center py-4" style={{ color: 'var(--muted)' }}>
                      Empty
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Messages */}
        <div
          className="lg:col-span-2 rounded-[var(--radius-lg)] border p-4 flex flex-col"
          style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
        >
          <div className="flex items-center gap-2 mb-3">
            <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>
              Messages
            </span>
            <select
              value={recipient}
              onChange={(e) => setRecipient(e.target.value)}
              className="text-xs rounded border px-2 py-1 outline-none"
              style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
            >
              {team.members.map((m) => (
                <option key={m} value={m}>
                  {m} {m === team.leader ? '(leader)' : ''}
                </option>
              ))}
            </select>
          </div>

          <div className="flex-1 overflow-auto space-y-2 mb-3 min-h-[100px]">
            {messages.map((m, i) => (
              <div
                key={i}
                className={`text-xs p-2 rounded-[var(--radius-sm)] max-w-[85%] ${
                  m.sender === 'owner' ? 'ml-auto' : ''
                }`}
                style={{
                  background: m.sender === 'owner' ? 'var(--accent-subtle)' : 'var(--bg)',
                  color: 'var(--text)',
                }}
              >
                <span className="font-medium" style={{ color: 'var(--muted)' }}>
                  {m.sender}:
                </span>{' '}
                {m.content}
              </div>
            ))}
            {messages.length === 0 && (
              <p className="text-xs text-center" style={{ color: 'var(--muted)' }}>
                No messages yet
              </p>
            )}
          </div>

          <div className="flex gap-2">
            <input
              value={msgInput}
              onChange={(e) => setMsgInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSend()}
              placeholder="Type a message..."
              className="flex-1 rounded-[var(--radius-sm)] border px-3 py-1.5 text-sm outline-none"
              style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
            />
            <button
              onClick={handleSend}
              className="rounded-[var(--radius-sm)] p-2 text-white"
              style={{ background: 'var(--accent)' }}
            >
              <Send size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* Team Settings Drawer */}
      <TeamSettings
        open={showSettings}
        onClose={() => setShowSettings(false)}
        team={team}
        onSave={handleSettingsSave}
      />
    </div>
  )
}
