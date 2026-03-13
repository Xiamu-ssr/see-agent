import { useState, useEffect, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { X } from "lucide-react"

type TeamMember = { id: string; role: string }

interface TeamSettingsProps {
  open: boolean
  onClose: () => void
  team: {
    id: string
    name: string
    members: TeamMember[]
    leader?: string | null
    status: string
  }
  onSave: (updates: { name?: string; members?: TeamMember[]; leader?: string }) => void
}

export default function TeamSettings({ open, onClose, team, onSave }: TeamSettingsProps) {
  const [name, setName] = useState(team.name)
  const [members, setMembers] = useState<TeamMember[]>(team.members)
  const [leader, setLeader] = useState(team.leader ?? "")
  const [newMember, setNewMember] = useState("")

  useEffect(() => {
    setName(team.name)
    setMembers(team.members)
    setLeader(team.leader ?? "")
  }, [team])

  const handleRemoveMember = useCallback((memberId: string) => {
    setMembers((prev) => prev.filter((m) => m.id !== memberId))
    setLeader((prev) => (prev === memberId ? "" : prev))
  }, [])

  const handleAddMember = useCallback(() => {
    const trimmed = newMember.trim()
    if (trimmed && !members.some((m) => m.id === trimmed)) {
      setMembers((prev) => [...prev, { id: trimmed, role: "worker" }])
      setNewMember("")
    }
  }, [newMember, members])

  const handleSave = useCallback(() => {
    const updates: { name?: string; members?: TeamMember[]; leader?: string } = {}
    if (name !== team.name) updates.name = name
    if (JSON.stringify(members) !== JSON.stringify(team.members)) updates.members = members
    if (leader && leader !== (team.leader ?? "")) updates.leader = leader
    onSave(updates)
  }, [name, members, leader, team, onSave])

  if (!open) return null

  return (
    <>
      <div className="fixed inset-0 z-[999] bg-black/50" onClick={onClose} />

      <div
        className="fixed top-0 right-0 bottom-0 w-full md:w-[380px] z-[1000] flex flex-col bg-[var(--bg-elevated)] border-l border-[var(--border)] transition-transform duration-250"
        style={{ transform: open ? "translateX(0)" : "translateX(100%)" }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
          <h2 className="text-lg font-semibold text-[var(--text-strong)]">
            Team Settings
          </h2>
          <Button variant="ghost" size="icon" onClick={onClose}>
            <X className="h-4 w-4" />
          </Button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5 space-y-6">
          <div>
            <label className="block mb-1.5 text-[13px] font-semibold text-[var(--text-strong)]">
              Team Name
            </label>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </div>

          <div>
            <label className="block mb-1.5 text-[13px] font-semibold text-[var(--text-strong)]">
              Members
            </label>
            <ul className="space-y-1 mb-2">
              {members.map((member) => (
                <li
                  key={member.id}
                  className="flex items-center justify-between px-2.5 py-1.5 rounded-[var(--radius-sm)] border border-[var(--border)] text-sm text-[var(--text-strong)]"
                >
                  <span>{member.id} <span className="text-[var(--muted)] text-xs">({member.role})</span></span>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6 text-[var(--danger)]"
                    onClick={() => handleRemoveMember(member.id)}
                  >
                    <X className="h-3 w-3" />
                  </Button>
                </li>
              ))}
            </ul>
            <Input
              placeholder="Add member..."
              value={newMember}
              onChange={(e) => setNewMember(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault()
                  handleAddMember()
                }
              }}
            />
          </div>

          <div>
            <label className="block mb-1.5 text-[13px] font-semibold text-[var(--text-strong)]">
              Leader
            </label>
            <select
              value={leader}
              onChange={(e) => setLeader(e.target.value)}
              className="w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2 text-sm text-[var(--text-strong)]"
            >
              <option value="">Select a leader</option>
              {members.map((member) => (
                <option key={member.id} value={member.id}>
                  {member.id}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Footer */}
        <div className="px-5 py-4 border-t border-[var(--border)]">
          <Button className="w-full" onClick={handleSave}>
            Save
          </Button>
        </div>
      </div>
    </>
  )
}
