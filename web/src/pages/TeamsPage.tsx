import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { usePolling } from '@/hooks/usePolling'
import { listTeams, createTeam } from '@/api/teams'
import { listAgents } from '@/api/agents'
import type { TeamSummary, AgentSummary } from '@/types'
import { Plus, Users } from 'lucide-react'

const statusColors: Record<string, string> = {
  created: '#7d8590',
  running: '#3fb950',
  completed: '#7d8590',
  failed: '#f85149',
  stopped: '#d29922',
  idle: '#7d8590',
}

function TeamCard({ team, onClick }: { team: TeamSummary; onClick: () => void }) {
  const color = statusColors[team.status] || '#7d8590'
  return (
    <button
      onClick={onClick}
      className="w-full text-left rounded-lg border p-5 transition-all hover:border-[#ff5c5c]/30"
      style={{ background: '#161b22', borderColor: '#30363d' }}
    >
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold" style={{ color: '#e6edf3' }}>
          {team.name}
        </h3>
        <span
          className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium"
          style={{ color, background: `${color}15` }}
        >
          <span className="h-1.5 w-1.5 rounded-full" style={{ background: color }} />
          {team.status}
        </span>
      </div>
      <div className="flex items-center gap-1.5 text-xs" style={{ color: '#7d8590' }}>
        <Users size={12} />
        {team.members.length} members
      </div>
      {(team as any).leader && (
        <div className="text-xs mt-1" style={{ color: '#7d8590' }}>
          Leader: {(team as any).leader}
        </div>
      )}
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
    const memberObjs = selectedMembers.map((id) => ({
      id,
      role: id === leader ? 'leader' : 'worker',
    }))
    await createTeam({ name: teamName, members: memberObjs, leader: leader || undefined })
    setShowCreate(false)
    setTeamName('')
    setSelectedMembers([])
    setLeader('')
    refresh()
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold" style={{ color: '#e6edf3' }}>
          Teams
        </h1>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-1.5 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white transition-colors"
          style={{ background: '#ff5c5c' }}
        >
          <Plus size={14} />
          New Team
        </button>
      </div>

      {loading && !teams ? (
        <div style={{ color: '#7d8590' }}>Loading...</div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {teams?.map((t) => (
            <TeamCard key={t.id} team={t} onClick={() => navigate(`/teams/${t.id}`)} />
          ))}
          {teams?.length === 0 && (
            <p className="col-span-full text-sm" style={{ color: '#7d8590' }}>
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
            style={{ background: '#161b22', borderColor: '#30363d' }}
          >
            <h2 className="text-base font-semibold mb-4" style={{ color: '#e6edf3' }}>
              Create Team
            </h2>
            <div className="flex flex-col gap-3">
              <input
                placeholder="Team name"
                value={teamName}
                onChange={(e) => setTeamName(e.target.value)}
                className="rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
                style={{
                  background: '#0d1117',
                  borderColor: '#30363d',
                  color: '#e6edf3',
                }}
              />

              {/* Members multi-select */}
              <div>
                <label className="text-xs font-medium" style={{ color: '#7d8590' }}>
                  Members
                </label>
                <div
                  className="mt-1 max-h-40 overflow-y-auto rounded border p-2"
                  style={{ borderColor: '#30363d', background: '#0d1117' }}
                >
                  {availableAgents.map((a) => (
                    <label
                      key={a.id}
                      className="flex items-center gap-2 py-1 text-sm cursor-pointer"
                      style={{ color: '#e6edf3' }}
                    >
                      <input
                        type="checkbox"
                        checked={selectedMembers.includes(a.id)}
                        onChange={() => toggleMember(a.id)}
                        style={{ accentColor: '#ff5c5c' }}
                      />
                      {a.id}
                    </label>
                  ))}
                  {availableAgents.length === 0 && (
                    <p className="text-xs py-2" style={{ color: '#7d8590' }}>
                      No agents available. Create agents first.
                    </p>
                  )}
                </div>
              </div>

              {/* Leader dropdown */}
              {selectedMembers.length > 0 && (
                <div>
                  <label className="text-xs font-medium" style={{ color: '#7d8590' }}>
                    Leader
                  </label>
                  <select
                    value={leader}
                    onChange={(e) => setLeader(e.target.value)}
                    className="mt-1 w-full rounded border px-3 py-2 text-sm"
                    style={{
                      background: '#0d1117',
                      borderColor: '#30363d',
                      color: '#e6edf3',
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
                  style={{ color: '#7d8590' }}
                >
                  Cancel
                </button>
                <button
                  onClick={handleCreate}
                  className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm font-medium text-white"
                  style={{ background: '#ff5c5c' }}
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
