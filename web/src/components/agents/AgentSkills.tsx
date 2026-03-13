import { useState, useEffect } from 'react'
import type { AgentDetail } from '@/types'
import { Sparkles } from 'lucide-react'
import Toggle from '@/components/ui/Toggle'

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
    <div className="space-y-1">
      {saving && (
        <div className="text-xs mb-2" style={{ color: 'var(--muted)' }}>Saving...</div>
      )}
      {skills.map(skill => {
        const enabled = !disabledList.includes(skill.name)
        return (
          <div
            key={skill.name}
            className="flex items-center gap-3 rounded-lg px-3 py-2 transition-colors"
            style={{ background: 'var(--bg-deeper)' }}
          >
            <Sparkles size={15} style={{ color: enabled ? 'var(--accent)' : 'var(--muted)', flexShrink: 0 }} />
            <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>
              {skill.name}
            </span>
            <span className="text-xs flex-1 min-w-0 truncate" style={{ color: 'var(--muted)' }}>
              {skill.description}
            </span>
            <Toggle enabled={enabled} onChange={() => toggle(skill.name)} />
          </div>
        )
      })}
    </div>
  )
}
