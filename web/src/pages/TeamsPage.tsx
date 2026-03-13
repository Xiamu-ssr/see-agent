import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { usePolling } from '@/hooks/usePolling'
import { listTeams, createTeam } from '@/api/teams'
import { listAgents } from '@/api/agents'
import type { TeamSummary, AgentSummary } from '@/types'
import { Plus, Users } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'

const statusVariant: Record<string, 'success' | 'secondary' | 'destructive' | 'warning'> = {
  running: 'success',
  completed: 'secondary',
  failed: 'destructive',
  stopped: 'warning',
  created: 'secondary',
  idle: 'secondary',
}

function TeamCard({ team, onClick }: { team: TeamSummary; onClick: () => void }) {
  return (
    <Card
      className="p-5 cursor-pointer transition-all hover:border-[var(--accent)]/30"
      onClick={onClick}
    >
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold text-[var(--text-strong)]">
          {team.name}
        </h3>
        <Badge variant={statusVariant[team.status] || 'secondary'}>
          <span className="h-1.5 w-1.5 rounded-full bg-current" />
          {team.status}
        </Badge>
      </div>
      <div className="flex items-center gap-1.5 text-xs text-[var(--muted)]">
        <Users size={12} />
        {team.members.length} members
      </div>
      {(team as any).leader && (
        <div className="text-xs mt-1 text-[var(--muted)]">
          Leader: {(team as any).leader}
        </div>
      )}
    </Card>
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
        <h1 className="text-xl font-semibold text-[var(--text-strong)]">
          Teams
        </h1>
        <Button onClick={() => setShowCreate(true)} size="sm">
          <Plus size={14} />
          New Team
        </Button>
      </div>

      {loading && !teams ? (
        <div className="text-[var(--muted)]">Loading...</div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {teams?.map((t) => (
            <TeamCard key={t.id} team={t} onClick={() => navigate(`/teams/${t.id}`)} />
          ))}
          {teams?.length === 0 && (
            <p className="col-span-full text-sm text-[var(--muted)]">
              No teams yet. Create one to get started.
            </p>
          )}
        </div>
      )}

      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Team</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <Input
              placeholder="Team name"
              value={teamName}
              onChange={(e) => setTeamName(e.target.value)}
            />

            <div>
              <label className="text-xs font-medium text-[var(--muted)]">
                Members
              </label>
              <div className="mt-1 max-h-40 overflow-y-auto rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] p-2">
                {availableAgents.map((a) => (
                  <label
                    key={a.id}
                    className="flex items-center gap-2 py-1 text-sm cursor-pointer text-[var(--text-strong)]"
                  >
                    <input
                      type="checkbox"
                      checked={selectedMembers.includes(a.id)}
                      onChange={() => toggleMember(a.id)}
                      className="accent-[var(--accent)]"
                    />
                    {a.id}
                  </label>
                ))}
                {availableAgents.length === 0 && (
                  <p className="text-xs py-2 text-[var(--muted)]">
                    No agents available. Create agents first.
                  </p>
                )}
              </div>
            </div>

            {selectedMembers.length > 0 && (
              <div>
                <label className="text-xs font-medium text-[var(--muted)]">
                  Leader
                </label>
                <select
                  value={leader}
                  onChange={(e) => setLeader(e.target.value)}
                  className="mt-1 w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm text-[var(--text-strong)]"
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
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowCreate(false)}>
              Cancel
            </Button>
            <Button onClick={handleCreate}>
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
