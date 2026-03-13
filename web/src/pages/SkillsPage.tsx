import { useState, useEffect } from 'react'
import { listSkills, installSkill } from '@/api/skills'
import type { SkillInfo } from '@/types'
import { Sparkles, Check, X, Plus, Download, FolderOpen } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'

export default function SkillsPage() {
  const [skills, setSkills] = useState<SkillInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [showInstall, setShowInstall] = useState(false)
  const [installMode, setInstallMode] = useState<'clawhub' | 'manual'>('clawhub')
  const [skillName, setSkillName] = useState('')
  const [installing, setInstalling] = useState(false)
  const [installMsg, setInstallMsg] = useState('')

  const refresh = () => { listSkills().then(setSkills).finally(() => setLoading(false)) }
  useEffect(() => { refresh() }, [])

  const handleInstall = async () => {
    if (!skillName.trim()) return
    setInstalling(true); setInstallMsg('')
    try {
      await installSkill(skillName.trim())
      setInstallMsg('Installed successfully'); setSkillName(''); refresh()
    } catch (e) {
      setInstallMsg(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally { setInstalling(false) }
  }

  if (loading) return <div className="px-3 py-3 md:px-4 md:py-4 text-[var(--muted)]">Loading...</div>

  return (
    <div className="px-3 py-3 md:px-4 md:py-4">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold text-[var(--text-strong)]">Skills</h1>
        <Button onClick={() => { setShowInstall(true); setInstallMsg('') }} size="sm">
          <Plus size={14} /> Install Skill
        </Button>
      </div>

      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        {skills.map((s) => (
          <Card key={s.name} className="p-4">
            <div className="flex items-center gap-2 mb-2">
              <Sparkles size={16} className="text-[var(--accent)]" />
              <span className="text-sm font-medium text-[var(--text-strong)]">{s.name}</span>
            </div>
            <p className="text-xs mb-3 leading-relaxed text-[var(--muted)]">
              {s.description || 'No description'}
            </p>
            <Badge variant={s.available ? 'success' : 'destructive'}>
              {s.available ? <Check size={12} /> : <X size={12} />}
              {s.available ? 'Installed' : 'Unavailable'}
            </Badge>
          </Card>
        ))}
        {skills.length === 0 && (
          <p className="col-span-full text-sm text-[var(--muted)]">No skills installed.</p>
        )}
      </div>

      <Dialog open={showInstall} onOpenChange={setShowInstall}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Install Skill</DialogTitle>
          </DialogHeader>
          <div className="flex gap-1 mb-4">
            {([['clawhub', Download, 'From ClawHub'], ['manual', FolderOpen, 'Manual']] as const).map(([mode, Icon, label]) => (
              <Button
                key={mode}
                variant={installMode === mode ? 'default' : 'ghost'}
                size="sm"
                onClick={() => setInstallMode(mode as 'clawhub' | 'manual')}
              >
                <Icon size={14} /> {label}
              </Button>
            ))}
          </div>
          {installMode === 'clawhub' ? (
            <div className="space-y-3">
              <Input
                placeholder="Skill name (e.g. open-browser)"
                value={skillName} onChange={(e) => setSkillName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleInstall()}
              />
              {installMsg && <p className={`text-xs ${installMsg.startsWith('Error') ? 'text-[var(--danger)]' : 'text-[var(--ok)]'}`}>{installMsg}</p>}
              <DialogFooter>
                <Button variant="ghost" onClick={() => setShowInstall(false)}>Cancel</Button>
                <Button onClick={handleInstall} disabled={installing}>
                  {installing ? 'Installing...' : 'Install'}
                </Button>
              </DialogFooter>
            </div>
          ) : (
            <div className="space-y-3">
              <p className="text-sm text-[var(--text-strong)]">Place your SKILL.md in:</p>
              <code className="block text-xs p-3 rounded-[var(--radius-sm)] bg-[var(--bg)] text-[var(--accent)] font-mono">
                ~/.see-agent/skills/your-skill-name/SKILL.md
              </code>
              <DialogFooter>
                <Button variant="ghost" onClick={() => setShowInstall(false)}>Close</Button>
              </DialogFooter>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}
