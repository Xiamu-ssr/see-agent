import { useState, useEffect } from 'react'
import type { AgentDetail } from '@/types'
import {
  MousePointer2, Keyboard, ScrollText, GripVertical, Command,
  Camera, Terminal, Clock, CheckCircle, Phone, Search, PenTool,
} from 'lucide-react'

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
      // revert on error
      setDisabledList(disabledList)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-4">
      {saving && (
        <div className="text-xs" style={{ color: 'var(--muted)' }}>Saving...</div>
      )}
      <div className="space-y-1">
        {tools.map(tool => {
          const enabled = !disabledList.includes(tool.name)
          const Icon = ICON_MAP[tool.name] || Terminal
          return (
            <div
              key={tool.name}
              className="flex items-center justify-between rounded-lg px-3 py-2.5 transition-colors"
              style={{ background: 'var(--bg-deeper)' }}
            >
              <div className="flex items-center gap-3">
                <Icon size={16} style={{ color: enabled ? 'var(--accent)' : 'var(--muted)' }} />
                <div>
                  <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>
                    {tool.name}
                  </span>
                  <span className="text-xs ml-2" style={{ color: 'var(--muted)' }}>
                    {tool.description}
                  </span>
                </div>
              </div>
              <button
                onClick={() => toggle(tool.name)}
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
