import { useState, useEffect } from 'react'
import type { AgentDetail } from '@/types'
import { Sparkles } from 'lucide-react'

interface Props {
  agent: AgentDetail
}

interface SkillItem {
  name: string
  description: string
  disabled: boolean
}

export default function AgentSkills({ agent }: Props) {
  const [skills, setSkills] = useState<SkillItem[]>([])
  const [disabledList, setDisabledList] = useState<string[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    fetch(`/api/agents/${agent.id}/skills`)
      .then(r => r.json())
      .then((data: { skills: SkillItem[]; disabled: string[] }) => {
        setSkills(data.skills)
        setDisabledList(data.disabled)
      })
      .catch(() => {})
  }, [agent.id])

  const toggle = async (name: string) => {
    const newDisabled = disabledList.includes(name)
      ? disabledList.filter(n => n !== name)
      : [...disabledList, name]

    setDisabledList(newDisabled)
    setSaving(true)
    try {
      await fetch(`/api/agents/${agent.id}/skills`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ disabled: newDisabled }),
      })
    } catch {
      setDisabledList(disabledList)
    } finally {
      setSaving(false)
    }
  }

  if (skills.length === 0) {
    return (
      <div className="text-sm" style={{ color: 'var(--muted)' }}>
        No skills installed. Install skills from the Skills page.
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {saving && (
        <div className="text-xs" style={{ color: 'var(--muted)' }}>Saving...</div>
      )}
      <div className="space-y-1">
        {skills.map(skill => {
          const enabled = !disabledList.includes(skill.name)
          return (
            <div
              key={skill.name}
              className="flex items-center justify-between rounded-lg px-3 py-2.5 transition-colors"
              style={{ background: 'var(--bg-deeper)' }}
            >
              <div className="flex items-center gap-3">
                <Sparkles size={16} style={{ color: enabled ? 'var(--accent)' : 'var(--muted)' }} />
                <div>
                  <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>
                    {skill.name}
                  </span>
                  <span className="text-xs ml-2" style={{ color: 'var(--muted)' }}>
                    {skill.description}
                  </span>
                </div>
              </div>
              <button
                onClick={() => toggle(skill.name)}
                className="relative rounded-full transition-colors"
                style={{
                  width: 36,
                  height: 20,
                  background: enabled ? 'var(--accent)' : 'var(--border)',
                }}
              >
                <span
                  className="absolute top-0.5 rounded-full bg-white transition-transform"
                  style={{
                    width: 16,
                    height: 16,
                    transform: enabled ? 'translateX(18px)' : 'translateX(2px)',
                  }}
                />
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
