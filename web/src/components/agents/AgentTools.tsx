import { useState } from 'react'
import type { AgentDetail } from '@/types'
import {
  MousePointer2, Keyboard, ScrollText, GripVertical, Command,
  Camera, Terminal, Clock, CheckCircle, Phone, Search, PenTool,
} from 'lucide-react'

interface Props {
  agent: AgentDetail
}

interface ToolDef {
  name: string
  category: string
  description: string
  icon: typeof Camera
}

const ALL_TOOLS: ToolDef[] = [
  { name: 'screenshot', category: 'Screen', description: 'Capture screen screenshot', icon: Camera },
  { name: 'click', category: 'Screen', description: 'Mouse click at coordinates', icon: MousePointer2 },
  { name: 'type_text', category: 'Screen', description: 'Type text via keyboard', icon: Keyboard },
  { name: 'scroll', category: 'Screen', description: 'Scroll up/down', icon: ScrollText },
  { name: 'drag', category: 'Screen', description: 'Drag from point to point', icon: GripVertical },
  { name: 'hotkey', category: 'Screen', description: 'Press keyboard shortcut', icon: Command },
  { name: 'shell', category: 'System', description: 'Execute shell command', icon: Terminal },
  { name: 'wait', category: 'System', description: 'Wait for duration', icon: Clock },
  { name: 'memory_search', category: 'Memory', description: 'Search agent memory', icon: Search },
  { name: 'write_memory', category: 'Memory', description: 'Write to agent memory', icon: PenTool },
  { name: 'finished', category: 'Control', description: 'Mark task as complete', icon: CheckCircle },
  { name: 'call_user', category: 'Control', description: 'Ask user for help', icon: Phone },
]

export default function AgentTools({ agent }: Props) {
  const toolsConfig = agent.tools || {}
  const [enabled, setEnabled] = useState<Record<string, boolean>>(() => {
    const state: Record<string, boolean> = {}
    ALL_TOOLS.forEach(t => {
      state[t.name] = (toolsConfig as Record<string, Record<string, unknown>>)[t.name]?.enabled !== false
    })
    return state
  })

  const toggle = (name: string) => {
    setEnabled(prev => ({ ...prev, [name]: !prev[name] }))
  }

  const categories = [...new Set(ALL_TOOLS.map(t => t.category))]

  return (
    <div className="space-y-6">
      {/* Quick presets */}
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium uppercase tracking-wide" style={{ color: '#7d8590' }}>
          Quick Presets:
        </span>
        {['Minimal', 'Standard', 'Full'].map(preset => (
          <button
            key={preset}
            onClick={() => {
              if (preset === 'Full') setEnabled(Object.fromEntries(ALL_TOOLS.map(t => [t.name, true])))
              else if (preset === 'Minimal') setEnabled(Object.fromEntries(ALL_TOOLS.map(t => [t.name, ['screenshot', 'click', 'type_text', 'finished'].includes(t.name)])))
              else setEnabled(Object.fromEntries(ALL_TOOLS.map(t => [t.name, t.name !== 'drag'])))
            }}
            className="rounded-md px-2.5 py-1 text-xs font-medium transition-colors"
            style={{ background: '#161b22', color: '#e6edf3', border: '1px solid #30363d' }}
          >
            {preset}
          </button>
        ))}
      </div>

      {/* Tool list by category */}
      {categories.map(cat => (
        <div key={cat}>
          <h3 className="text-xs font-medium uppercase tracking-wide mb-2" style={{ color: '#7d8590' }}>
            {cat}
          </h3>
          <div className="space-y-1">
            {ALL_TOOLS.filter(t => t.category === cat).map(tool => (
              <div
                key={tool.name}
                className="flex items-center justify-between rounded-lg px-3 py-2.5 transition-colors"
                style={{ background: '#0d1117' }}
              >
                <div className="flex items-center gap-3">
                  <tool.icon size={16} style={{ color: enabled[tool.name] ? '#ff5c5c' : '#7d8590' }} />
                  <div>
                    <span className="text-sm font-medium" style={{ color: '#e6edf3' }}>
                      {tool.name}
                    </span>
                    <span className="text-xs ml-2" style={{ color: '#7d8590' }}>
                      {tool.description}
                    </span>
                  </div>
                </div>
                {/* Toggle switch */}
                <button
                  onClick={() => toggle(tool.name)}
                  className="relative rounded-full transition-colors"
                  style={{
                    width: 36,
                    height: 20,
                    background: enabled[tool.name] ? '#ff5c5c' : '#30363d',
                  }}
                >
                  <span
                    className="absolute top-0.5 rounded-full bg-white transition-transform"
                    style={{
                      width: 16,
                      height: 16,
                      transform: enabled[tool.name] ? 'translateX(18px)' : 'translateX(2px)',
                    }}
                  />
                </button>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
