import { useState, useEffect } from 'react'
import { listSkills, installSkill } from '@/api/skills'
import type { Skill } from '@/api/skills'
import { Sparkles, Check, X, Plus, Download, FolderOpen } from 'lucide-react'

export default function SkillsPage() {
  const [skills, setSkills] = useState<Skill[]>([])
  const [loading, setLoading] = useState(true)
  const [showInstall, setShowInstall] = useState(false)
  const [installMode, setInstallMode] = useState<'clawhub' | 'manual'>('clawhub')
  const [skillName, setSkillName] = useState('')
  const [installing, setInstalling] = useState(false)
  const [installMsg, setInstallMsg] = useState('')

  const refresh = () => {
    listSkills()
      .then(setSkills)
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    refresh()
  }, [])

  const handleInstall = async () => {
    if (!skillName.trim()) return
    setInstalling(true)
    setInstallMsg('')
    try {
      await installSkill(skillName.trim())
      setInstallMsg('Installed successfully')
      setSkillName('')
      refresh()
    } catch (e) {
      setInstallMsg(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setInstalling(false)
    }
  }

  if (loading) return <div style={{ color: 'var(--muted)' }}>Loading...</div>

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
          Skills
        </h1>
        <button
          onClick={() => { setShowInstall(true); setInstallMsg('') }}
          className="flex items-center gap-1.5 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white"
          style={{ background: 'var(--accent)' }}
        >
          <Plus size={14} />
          Install Skill
        </button>
      </div>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {skills.map((s) => (
          <div
            key={s.name}
            className="rounded-[var(--radius-lg)] border p-4"
            style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
          >
            <div className="flex items-center gap-2 mb-2">
              <Sparkles size={16} style={{ color: 'var(--accent)' }} />
              <span className="font-medium" style={{ color: 'var(--text-strong)' }}>
                {s.name}
              </span>
            </div>
            <p className="text-xs mb-3" style={{ color: 'var(--muted)' }}>
              {s.description || 'No description'}
            </p>
            <span
              className="inline-flex items-center gap-1 text-xs"
              style={{ color: s.available ? 'var(--ok)' : 'var(--danger)' }}
            >
              {s.available ? <Check size={12} /> : <X size={12} />}
              {s.available ? 'Available' : 'Blocked'}
            </span>
          </div>
        ))}
        {skills.length === 0 && (
          <p className="col-span-full text-sm" style={{ color: 'var(--muted)' }}>
            No skills installed.
          </p>
        )}
      </div>

      {/* Install modal */}
      {showInstall && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div
            className="w-full max-w-md rounded-[var(--radius-lg)] border p-6"
            style={{ background: 'var(--bg-elevated)', borderColor: 'var(--border)' }}
          >
            <h2 className="text-base font-semibold mb-4" style={{ color: 'var(--text-strong)' }}>
              Install Skill
            </h2>

            {/* Mode tabs */}
            <div className="flex gap-1 mb-4">
              <button
                onClick={() => setInstallMode('clawhub')}
                className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-[var(--radius-sm)]"
                style={{
                  background: installMode === 'clawhub' ? 'var(--accent-subtle)' : 'transparent',
                  color: installMode === 'clawhub' ? 'var(--accent)' : 'var(--muted)',
                }}
              >
                <Download size={14} />
                From ClawHub
              </button>
              <button
                onClick={() => setInstallMode('manual')}
                className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-[var(--radius-sm)]"
                style={{
                  background: installMode === 'manual' ? 'var(--accent-subtle)' : 'transparent',
                  color: installMode === 'manual' ? 'var(--accent)' : 'var(--muted)',
                }}
              >
                <FolderOpen size={14} />
                Manual
              </button>
            </div>

            {installMode === 'clawhub' ? (
              <div className="space-y-3">
                <input
                  placeholder="Skill name (e.g. open-browser)"
                  value={skillName}
                  onChange={(e) => setSkillName(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleInstall()}
                  className="w-full rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
                  style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
                />
                {installMsg && (
                  <p
                    className="text-xs"
                    style={{ color: installMsg.startsWith('Error') ? 'var(--danger)' : 'var(--ok)' }}
                  >
                    {installMsg}
                  </p>
                )}
                <div className="flex gap-2 justify-end">
                  <button
                    onClick={() => setShowInstall(false)}
                    className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm"
                    style={{ color: 'var(--muted)' }}
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleInstall}
                    disabled={installing}
                    className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm font-medium text-white"
                    style={{ background: 'var(--accent)', opacity: installing ? 0.6 : 1 }}
                  >
                    {installing ? 'Installing...' : 'Install'}
                  </button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <p className="text-sm" style={{ color: 'var(--text)' }}>
                  Place your SKILL.md file in a subdirectory under:
                </p>
                <code
                  className="block text-xs p-3 rounded-[var(--radius-sm)]"
                  style={{ background: 'var(--bg)', color: 'var(--accent)', fontFamily: 'var(--mono)' }}
                >
                  ~/.see-agent/skills/your-skill-name/SKILL.md
                </code>
                <p className="text-xs" style={{ color: 'var(--muted)' }}>
                  The skill will be automatically loaded on the next server restart.
                </p>
                <div className="flex justify-end">
                  <button
                    onClick={() => setShowInstall(false)}
                    className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm"
                    style={{ color: 'var(--muted)' }}
                  >
                    Close
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
