import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate, Link } from 'react-router-dom'
import { getTeamStatus, getMessages, sendMessage, runTeam, stopTeam, updateTeam } from '@/api/teams'
import type { TeamStatus, TeamMessage } from '@/types'
import { ArrowLeft, Square, Settings, Plus, ExternalLink } from 'lucide-react'
import TeamSettings from '@/components/team/TeamSettings'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'

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

  if (loading) return <div className="px-3 py-3 md:px-4 md:py-4 text-[var(--muted)]">Loading...</div>
  if (!team) return <div className="px-3 py-3 md:px-4 md:py-4 text-[var(--danger)]">Team not found</div>

  const statusVariant = team.status === 'running' ? 'success' as const : team.status === 'failed' ? 'destructive' as const : 'secondary' as const

  const tasksByStatus = {
    todo: team.tasks.filter((t) => t.status === 'pending'),
    doing: team.tasks.filter((t) => t.status === 'claimed' || t.status === 'in_progress'),
    done: team.tasks.filter((t) => t.status === 'done' || t.status === 'completed'),
  }

  return (
    <div className="px-3 py-3 md:px-4 md:py-4">
      <div className="flex items-center justify-between mb-4 md:mb-6">
        <div className="flex items-center gap-2 md:gap-3 min-w-0">
          <Button variant="ghost" size="icon" onClick={() => navigate('/teams')}>
            <ArrowLeft size={16} />
          </Button>
          <h1 className="text-xl font-semibold text-[var(--text-strong)] truncate">{team.name}</h1>
          <Badge variant={statusVariant}>
            <span className="h-1.5 w-1.5 rounded-full bg-current" />
            {team.status}
          </Badge>
        </div>
        <div className="flex gap-2 shrink-0">
          <Button variant="ghost" size="icon" onClick={() => setShowSettings(true)}>
            <Settings size={16} />
          </Button>
          {team.status === 'running' && (
            <Button variant="destructive" size="sm" onClick={handleStop} disabled={actionLoading}>
              <Square size={14} /> Stop
            </Button>
          )}
        </div>
      </div>

      <div className="flex flex-col md:flex-row gap-4" style={{ minHeight: 'calc(100vh - 200px)' }}>
        {/* Members */}
        <div className="w-full md:w-[40%] shrink-0">
          <Card className="p-4">
            <h2 className="text-base font-semibold mb-3 text-[var(--text-strong)]">Members</h2>
            <div className="space-y-2">
              {team.members.map((m) => (
                <div key={m.id} className="flex items-center gap-3 rounded-lg p-3 bg-[var(--bg)]">
                  <div className="rounded-full flex items-center justify-center shrink-0 w-10 h-10 bg-[var(--border-strong)]">
                    <span className="text-sm font-medium text-[var(--text-strong)]">
                      {m.id.charAt(0).toUpperCase()}
                    </span>
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-sm font-medium text-[var(--text-strong)]">{m.id}</span>
                      {m.id === team.leader && (
                        <Badge>LEADER</Badge>
                      )}
                      <Badge variant="secondary">{m.role}</Badge>
                    </div>
                    <div className="flex items-center gap-1.5 mt-0.5">
                      <span className="h-1.5 w-1.5 rounded-full bg-[var(--ok)]" />
                      <span className="text-xs text-[var(--muted)]">Running</span>
                    </div>
                  </div>
                  <Link to={`/agents/${m.id}`} className="flex items-center gap-1 text-xs shrink-0 text-[var(--muted)]">
                    View <ExternalLink size={12} />
                  </Link>
                </div>
              ))}
            </div>
            <button className="flex items-center gap-1.5 mt-4 text-sm text-[var(--accent)]">
              <Plus size={14} /> Add Member
            </button>
          </Card>
        </div>

        {/* Right: task board + messages */}
        <div className="flex-1 flex flex-col gap-4">
          <Card className="p-4">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-base font-semibold text-[var(--text-strong)]">Task Board</h2>
              <Button variant="outline" size="sm" onClick={() => setShowTaskModal(true)}>
                <Plus size={14} /> Send Task
              </Button>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              {(['todo', 'doing', 'done'] as const).map((col) => (
                <div key={col}>
                  <p className="text-xs font-medium uppercase tracking-wide mb-2 text-[var(--muted)]">
                    {col === 'doing' ? 'Doing' : col === 'todo' ? 'Todo' : 'Done'}
                  </p>
                  <div className="space-y-2">
                    {tasksByStatus[col].map((t) => (
                      <Card key={t.id} className="p-3">
                        <p className="text-sm text-[var(--text-strong)]">{t.title}</p>
                        {t.assigned_to && (
                          <p className="text-xs mt-1 text-[var(--muted)]">→ {t.assigned_to}</p>
                        )}
                      </Card>
                    ))}
                    {tasksByStatus[col].length === 0 && (
                      <div className="rounded-[var(--radius-lg)] border border-dashed border-[var(--border)] p-4 text-center">
                        <p className="text-xs text-[var(--muted)]">Empty</p>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </Card>

          <Card className="p-4 flex-1 flex flex-col">
            <h2 className="text-base font-semibold mb-3 text-[var(--text-strong)]">Team Messages</h2>
            <div className="flex-1 overflow-y-auto space-y-2 mb-4 min-h-[80px]">
              {messages.map((m, i) => (
                <p key={i} className="text-sm text-[var(--text-strong)]">
                  <span className="text-[var(--muted)]">{m.sender} to {m.recipient}:</span>{' '}
                  {m.content}
                </p>
              ))}
              {messages.length === 0 && (
                <p className="text-sm text-[var(--muted)]">No messages yet.</p>
              )}
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm shrink-0 text-[var(--muted)]">To:</span>
              <select value={recipient} onChange={(e) => setRecipient(e.target.value)}
                className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] px-2 py-1.5 text-sm text-[var(--text-strong)] shrink-0">
                {team.members.map((m) => (
                  <option key={m.id} value={m.id}>{m.id} {m.id === team.leader ? '(leader)' : ''}</option>
                ))}
              </select>
              <Input value={msgInput} onChange={(e) => setMsgInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                placeholder="Type a message..."
                className="flex-1 min-w-[120px]" />
              <label className="flex items-center gap-1.5 text-xs shrink-0 text-[var(--muted)]">
                <Switch checked={steer} onCheckedChange={setSteer} />
                Steer
              </label>
              <Button variant="outline" size="sm" onClick={handleSend}>
                Send
              </Button>
            </div>
          </Card>
        </div>
      </div>

      <Dialog open={showTaskModal} onOpenChange={setShowTaskModal}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Send Task</DialogTitle>
          </DialogHeader>
          <Input placeholder="Describe the task..." value={taskInput}
            onChange={(e) => setTaskInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSendTask()} />
          <DialogFooter>
            <Button variant="ghost" onClick={() => { setShowTaskModal(false); setTaskInput('') }}>Cancel</Button>
            <Button onClick={handleSendTask} disabled={!taskInput.trim() || actionLoading}>Send</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <TeamSettings open={showSettings} onClose={() => setShowSettings(false)}
        team={team} onSave={handleSettingsSave} />
    </div>
  )
}
