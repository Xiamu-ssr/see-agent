import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { usePolling } from '@/hooks/usePolling'
import { listTeams, createTeam } from '@/api/teams'
import { listAgents } from '@/api/agents'
import type { TeamSummary, AgentSummary } from '@/types'
import { Plus, Users } from 'lucide-react'

const statusColors: Record<string, string> = {
  created: 'var(--muted)',
  running: 'var(--ok)',
  completed: 'var(--accent-2)',
  failed: 'var(--danger)',
  stopped: 'var(--warn)',
}

function TeamCard({ team, onClick }: { team: TeamSummary; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="w-full text-left rounded-[var(--radius-lg)] border p-4 transition-colors hover:bg-[var(--bg-hover)]"
      style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
    >
      <div className="flex items-center justify-between mb-2">
        <h3 className="font-medium" style={{ color: 'var(--text-strong)' }}>
          {team.name}
        </h3>
        <span
          className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs"
          style={{
            color: statusColors[team.status] || 'var(--muted)',
            background: 'var(--bg-hover)',
          }}
        >
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{ background: statusColors[team.status] || 'var(--muted)' }}
          />
          {team.status}
        </span>
      </div>
      <div className="flex items-center gap-1 text-xs" style={{ color: 'var(--muted)' }}>
        <Users size={12} />
        {team.members.length} members
      </div>
    </button>
  )
}

export default function TeamsPage() {
  const navigate = useNavigate()
  const fetchTeams = useCallback(() => listTeams(), [])
  const { data: teams, loading, refresh } = usePolling<TeamSummary[]>(fetchTeams, 10000)
  const [showCreate, setShowCreate] = useState(false)
  const [teamName, setTeamName] = useState('')
  const [availableAgents, setAvailableAgents] = useState<AgentSummary[]>([])
  const [selectedMembers, setSelectedMembers] = useState<string[]>([])
  const [leader, setLeader] = useState('')

  useEffect(() => {
    if (showCreate) {
      listAgents().then((agents) => setAvailableAgents(agents || []))
    }
  }, [showCreate])

  const toggleMember = (id: string) => {
    setSelectedMembers((prev) => {
      const next = prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
      if (leader && !next.includes(leader)) setLeader('')
      return next
    })
  }

  const handleCreate = async () => {
    if (!teamName || selectedMembers.length === 0) return
    await createTeam({ name: teamName, members: selectedMembers, leader: leader || undefined })
    setShowCreate(false)
    setTeamName('')
    setSelectedMembers([])
    setLeader('')
    refresh()
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
          Teams
        </h1>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-1.5 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white transition-colors"
          style={{ background: 'var(--accent)' }}
        >
          <Plus size={14} />
          New Team
        </button>
      </div>

      {loading && !teams ? (
        <div style={{ color: 'var(--muted)' }}>Loading...</div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {teams?.map((t) => (
            <TeamCard key={t.id} team={t} onClick={() => navigate(`/teams/${t.id}`)} />
          ))}
          {teams?.length === 0 && (
            <p className="col-span-full text-sm" style={{ color: 'var(--muted)' }}>
              No teams yet. Create one to get started.
            </p>
          )}
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
              Create Team
            </h2>
            <div className="flex flex-col gap-3">
              <input
                placeholder="Team name"
                value={teamName}
                onChange={(e) => setTeamName(e.target.value)}
                className="rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
                style={{
                  background: 'var(--bg)',
                  borderColor: 'var(--border)',
                  color: 'var(--text)',
                }}
              />

              {/* Members multi-select */}
              <div>
                <label className="text-xs font-medium" style={{ color: 'var(--muted)' }}>
                  Members
                </label>
                <div
                  className="mt-1 max-h-40 overflow-y-auto rounded border p-2"
                  style={{ borderColor: 'var(--border)', background: 'var(--bg)' }}
                >
                  {availableAgents.map((a) => (
                    <label
                      key={a.id}
                      className="flex items-center gap-2 py-1 text-sm cursor-pointer"
                      style={{ color: 'var(--text)' }}
                    >
                      <input
                        type="checkbox"
                        checked={selectedMembers.includes(a.id)}
                        onChange={() => toggleMember(a.id)}
                        style={{ accentColor: 'var(--accent)' }}
                      />
                      {a.name} ({a.id})
                    </label>
                  ))}
                  {availableAgents.length === 0 && (
                    <p className="text-xs py-2" style={{ color: 'var(--muted)' }}>
                      No agents available. Create agents first.
                    </p>
                  )}
                </div>
              </div>

              {/* Leader dropdown */}
              {selectedMembers.length > 0 && (
                <div>
                  <label className="text-xs font-medium" style={{ color: 'var(--muted)' }}>
                    Leader
                  </label>
                  <select
                    value={leader}
                    onChange={(e) => setLeader(e.target.value)}
                    className="mt-1 w-full rounded border px-3 py-2 text-sm"
                    style={{
                      background: 'var(--bg)',
                      borderColor: 'var(--border)',
                      color: 'var(--text)',
                    }}
                  >
                    <option value="">Select leader...</option>
                    {selectedMembers.map((id) => (
                      <option key={id} value={id}>
                        {id}
                      </option>
                    ))}
                  </select>
                </div>
              )}

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
