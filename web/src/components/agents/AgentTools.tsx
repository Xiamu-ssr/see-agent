import { useState, useEffect } from 'react'
import type { AgentDetail } from '@/types'
import {
  MousePointer2, Keyboard, ScrollText, GripVertical, Command,
  Camera, Terminal, Clock, CheckCircle, Phone, Search, PenTool,
} from 'lucide-react'
import Toggle from '@/components/ui/Toggle'

interface Props {
  agent: AgentDetail
}

const ICON_MAP: Record<string, typeof Camera> = {
  screenshot: Camera,
  click: MousePointer2,
  type_text: Keyboard,
  scroll: ScrollText,
  drag: GripVertical,
  hotkey: Command,
  shell: Terminal,
  wait: Clock,
  memory_search: Search,
  write_memory: PenTool,
  finished: CheckCircle,
  call_user: Phone,
}

interface ToolItem {
  name: string
  description: string
  disabled: boolean
}

export default function AgentTools({ agent }: Props) {
  const [tools, setTools] = useState<ToolItem[]>([])
  const [disabledList, setDisabledList] = useState<string[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    fetch(`/api/agents/${agent.id}/tools`)
      .then(r => r.json())
      .then((data: { tools: ToolItem[]; disabled: string[] }) => {
        setTools(data.tools)
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
      await fetch(`/api/agents/${agent.id}/tools`, {
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

  return (
    <div className="space-y-1">
      {saving && (
        <div className="text-xs mb-2" style={{ color: 'var(--muted)' }}>Saving...</div>
      )}
      {tools.map(tool => {
        const enabled = !disabledList.includes(tool.name)
        const Icon = ICON_MAP[tool.name] || Terminal
        return (
          <div
            key={tool.name}
            className="flex items-center gap-3 rounded-lg px-3 py-2 transition-colors"
            style={{ background: 'var(--bg-deeper)' }}
          >
            <Icon size={15} style={{ color: enabled ? 'var(--accent)' : 'var(--muted)', flexShrink: 0 }} />
            <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>
              {tool.name}
            </span>
            <span className="text-xs flex-1 min-w-0 truncate" style={{ color: 'var(--muted)' }}>
              {tool.description}
            </span>
            <Toggle enabled={enabled} onChange={() => toggle(tool.name)} />
          </div>
        )
      })}
    </div>
  )
}
