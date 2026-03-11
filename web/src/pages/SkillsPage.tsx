import { useState, useEffect } from 'react'
import { listSkills, installSkill } from '@/api/skills'
import type { SkillInfo } from '@/types'
import { Sparkles, Check, X, Plus, Download, FolderOpen } from 'lucide-react'

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

  if (loading) return <div style={{ color: '#7d8590' }}>Loading...</div>

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold" style={{ color: '#e6edf3' }}>Skills</h1>
        <button
          onClick={() => { setShowInstall(true); setInstallMsg('') }}
          className="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium text-white"
          style={{ background: '#ff5c5c' }}
        >
          <Plus size={14} /> Install Skill
        </button>
      </div>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {skills.map((s) => (
          <div key={s.name} className="rounded-lg border p-4" style={{ background: '#161b22', borderColor: '#30363d' }}>
            <div className="flex items-center gap-2 mb-2">
              <Sparkles size={16} style={{ color: '#ff5c5c' }} />
              <span className="text-sm font-medium" style={{ color: '#e6edf3' }}>{s.name}</span>
            </div>
            <p className="text-xs mb-3 leading-relaxed" style={{ color: '#7d8590' }}>
              {s.description || 'No description'}
            </p>
            <span
              className="inline-flex items-center gap-1 text-xs font-medium rounded-full px-2 py-0.5"
              style={{
                color: s.available ? '#3fb950' : '#f85149',
                background: s.available ? 'rgba(63,185,80,0.15)' : 'rgba(248,81,73,0.15)',
              }}
            >
              {s.available ? <Check size={12} /> : <X size={12} />}
              {s.available ? 'Installed' : 'Unavailable'}
            </span>
          </div>
        ))}
        {skills.length === 0 && (
          <p className="col-span-full text-sm" style={{ color: '#7d8590' }}>No skills installed.</p>
        )}
      </div>

      {/* Install modal */}
      {showInstall && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="w-full max-w-md rounded-lg border p-6" style={{ background: '#161b22', borderColor: '#30363d' }}>
            <h2 className="text-base font-semibold mb-4" style={{ color: '#e6edf3' }}>Install Skill</h2>
            <div className="flex gap-1 mb-4">
              {([['clawhub', Download, 'From ClawHub'], ['manual', FolderOpen, 'Manual']] as const).map(([mode, Icon, label]) => (
                <button
                  key={mode}
                  onClick={() => setInstallMode(mode as 'clawhub' | 'manual')}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md"
                  style={{
                    background: installMode === mode ? 'rgba(255,92,92,0.12)' : 'transparent',
                    color: installMode === mode ? '#ff5c5c' : '#7d8590',
                  }}
                >
                  <Icon size={14} /> {label}
                </button>
              ))}
            </div>
            {installMode === 'clawhub' ? (
              <div className="space-y-3">
                <input
                  placeholder="Skill name (e.g. open-browser)"
                  value={skillName} onChange={(e) => setSkillName(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleInstall()}
                  className="w-full rounded-md border px-3 py-2 text-sm outline-none"
                  style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3' }}
                />
                {installMsg && <p className="text-xs" style={{ color: installMsg.startsWith('Error') ? '#f85149' : '#3fb950' }}>{installMsg}</p>}
                <div className="flex gap-2 justify-end">
                  <button onClick={() => setShowInstall(false)} className="px-3 py-1.5 text-sm" style={{ color: '#7d8590' }}>Cancel</button>
                  <button onClick={handleInstall} disabled={installing} className="rounded-md px-3 py-1.5 text-sm font-medium text-white" style={{ background: '#ff5c5c', opacity: installing ? 0.6 : 1 }}>
                    {installing ? 'Installing...' : 'Install'}
                  </button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <p className="text-sm" style={{ color: '#e6edf3' }}>Place your SKILL.md in:</p>
                <code className="block text-xs p-3 rounded-md" style={{ background: '#0d1117', color: '#ff5c5c', fontFamily: 'var(--mono, monospace)' }}>
                  ~/.see-agent/skills/your-skill-name/SKILL.md
                </code>
                <div className="flex justify-end">
                  <button onClick={() => setShowInstall(false)} className="px-3 py-1.5 text-sm" style={{ color: '#7d8590' }}>Close</button>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
