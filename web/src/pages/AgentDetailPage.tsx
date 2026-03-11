import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { getAgent, getWorkspaceFiles, getWorkspaceFile, updateWorkspaceFile, getAgentChat, sendAgentMessage, startAgent, stopAgent } from '@/api/agents'
import type { AgentDetail, WorkspaceFileItem, ChatMessage } from '@/types'
import { ArrowLeft, Bot, Play, Square, Save, Send, FileText, Shield, Wrench, MessageSquare, Info } from 'lucide-react'

type Tab = 'overview' | 'files' | 'tools' | 'safehouse' | 'chat'

export default function AgentDetailPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [agent, setAgent] = useState<AgentDetail | null>(null)
  const [loading, setLoading] = useState(true)
  const [tab, setTab] = useState<Tab>('overview')

  useEffect(() => {
    if (!id) return
    getAgent(id)
      .then(setAgent)
      .catch(() => setAgent(null))
      .finally(() => setLoading(false))
  }, [id])

  if (loading) return <div style={{ color: 'var(--muted)' }}>Loading...</div>
  if (!agent || !id) return <div style={{ color: 'var(--danger)' }}>Agent not found</div>

  const tabs: { key: Tab; label: string; icon: typeof Info }[] = [
    { key: 'overview', label: 'Overview', icon: Info },
    { key: 'files', label: 'Files', icon: FileText },
    { key: 'tools', label: 'Tools', icon: Wrench },
    { key: 'safehouse', label: 'Safehouse', icon: Shield },
    { key: 'chat', label: 'Chat', icon: MessageSquare },
  ]

  return (
    <div>
      <button
        onClick={() => navigate('/agents')}
        className="flex items-center gap-1 text-sm mb-4 hover:underline"
        style={{ color: 'var(--accent)' }}
      >
        <ArrowLeft size={14} />
        Back
      </button>

      {/* Header */}
      <div className="rounded-[var(--radius-lg)] border p-5 mb-4" style={{ background: 'var(--card)', borderColor: 'var(--border)' }}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="rounded-full p-2" style={{ background: 'var(--accent-subtle)' }}>
              <Bot size={20} style={{ color: 'var(--accent)' }} />
            </div>
            <div>
              <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>{agent.name}</h1>
              <p className="text-sm" style={{ color: 'var(--muted)' }}>
                {agent.role} &middot; {agent.team_name || 'No team'}
              </p>
            </div>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => startAgent(id)}
              className="flex items-center gap-1 rounded-[var(--radius-sm)] px-3 py-1.5 text-sm"
              style={{ color: 'var(--ok)' }}
            >
              <Play size={14} /> Start
            </button>
            <button
              onClick={() => stopAgent(id)}
              className="flex items-center gap-1 rounded-[var(--radius-sm)] px-3 py-1.5 text-sm"
              style={{ color: 'var(--danger)' }}
            >
              <Square size={14} /> Stop
            </button>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-4">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-[var(--radius-sm)] transition-colors"
            style={{
              background: tab === t.key ? 'var(--accent-subtle)' : 'transparent',
              color: tab === t.key ? 'var(--accent)' : 'var(--muted)',
            }}
          >
            <t.icon size={14} />
            {t.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="rounded-[var(--radius-lg)] border p-5" style={{ background: 'var(--card)', borderColor: 'var(--border)' }}>
        {tab === 'overview' && <OverviewTab agent={agent} />}
        {tab === 'files' && <FilesTab agentId={id} />}
        {tab === 'tools' && <ToolsTab agent={agent} />}
        {tab === 'safehouse' && <SafehouseTab agent={agent} />}
        {tab === 'chat' && <ChatTab agentId={id} />}
      </div>
    </div>
  )
}

/* ── Overview Tab ─────────────────────────────────────────────────── */

function OverviewTab({ agent }: { agent: AgentDetail }) {
  return (
    <div className="space-y-3 text-sm">
      <Row label="ID" value={agent.id} />
      <Row label="Name" value={agent.name} />
      <Row label="Role" value={agent.role} />
      <Row label="Team" value={agent.team_name || '\u2014'} />
      <Row label="Location" value={agent.location} />
      <Row label="Has SOUL" value={agent.has_soul ? 'Yes' : 'No'} />
    </div>
  )
}

/* ── Files Tab (workspace) ────────────────────────────────────────── */

function FilesTab({ agentId }: { agentId: string }) {
  const [files, setFiles] = useState<WorkspaceFileItem[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [content, setContent] = useState('')
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState('')

  const loadFiles = useCallback(async () => {
    const f = await getWorkspaceFiles(agentId)
    setFiles(f)
  }, [agentId])

  useEffect(() => { loadFiles() }, [loadFiles])

  const loadFile = async (name: string) => {
    const f = await getWorkspaceFile(agentId, name)
    setSelected(name)
    setContent(f.content)
    setDirty(false)
    setMsg('')
  }

  const handleSave = async () => {
    if (!selected) return
    setSaving(true)
    try {
      await updateWorkspaceFile(agentId, selected, content)
      setMsg('Saved')
      setDirty(false)
    } catch {
      setMsg('Error saving')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex gap-4" style={{ minHeight: '400px' }}>
      {/* File list */}
      <div className="w-48 shrink-0 border-r pr-4" style={{ borderColor: 'var(--border)' }}>
        <p className="text-xs font-medium uppercase mb-2" style={{ color: 'var(--muted)' }}>workspace/</p>
        {files.map((f) => (
          <button
            key={f.name}
            onClick={() => loadFile(f.name)}
            className="block w-full text-left text-sm px-2 py-1 rounded-[var(--radius-sm)] mb-0.5 transition-colors"
            style={{
              background: selected === f.name ? 'var(--accent-subtle)' : 'transparent',
              color: selected === f.name ? 'var(--accent)' : 'var(--text)',
            }}
          >
            {f.name}
          </button>
        ))}
        {files.length === 0 && (
          <p className="text-xs" style={{ color: 'var(--muted)' }}>No files</p>
        )}
      </div>

      {/* Editor */}
      <div className="flex-1">
        {selected ? (
          <>
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>{selected}</span>
              <div className="flex items-center gap-2">
                {msg && <span className="text-xs" style={{ color: msg === 'Saved' ? 'var(--ok)' : 'var(--danger)' }}>{msg}</span>}
                <button
                  onClick={handleSave}
                  disabled={!dirty || saving}
                  className="flex items-center gap-1 rounded-[var(--radius-sm)] px-3 py-1 text-sm font-medium text-white"
                  style={{ background: 'var(--accent)', opacity: !dirty || saving ? 0.5 : 1 }}
                >
                  <Save size={12} /> Save
                </button>
              </div>
            </div>
            <textarea
              value={content}
              onChange={(e) => { setContent(e.target.value); setDirty(true); setMsg('') }}
              className="w-full h-[360px] text-sm rounded-[var(--radius-sm)] border p-4 outline-none resize-none"
              style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}
            />
          </>
        ) : (
          <p className="text-sm" style={{ color: 'var(--muted)' }}>Select a file to edit</p>
        )}
      </div>
    </div>
  )
}

/* ── Tools Tab ────────────────────────────────────────────────────── */

function ToolsTab({ agent }: { agent: AgentDetail }) {
  return (
    <div>
      <p className="text-sm mb-3" style={{ color: 'var(--muted)' }}>Tool configuration for this agent.</p>
      <pre className="text-xs overflow-auto p-3 rounded-[var(--radius-sm)] border" style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}>
        {JSON.stringify({ tools_config: agent.tools_config, skills_config: agent.skills_config, mcp_config: agent.mcp_config }, null, 2)}
      </pre>
    </div>
  )
}

/* ── Safehouse Tab ────────────────────────────────────────────────── */

function SafehouseTab({ agent }: { agent: AgentDetail }) {
  const sandbox = agent.sandbox || {}
  return (
    <div className="space-y-3 text-sm">
      <Row label="Enabled" value={sandbox.enabled ? 'Yes' : 'No'} />
      <Row label="Network" value={sandbox.network ? 'Yes' : 'No'} />
      <Row label="Screen" value={sandbox.screen_access ? 'Yes' : 'No'} />
      <div>
        <p className="mb-1" style={{ color: 'var(--muted)' }}>Extra read paths:</p>
        <pre className="text-xs p-2 rounded-[var(--radius-sm)] border" style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}>
          {JSON.stringify(sandbox.extra_read || [], null, 2)}
        </pre>
      </div>
      <div>
        <p className="mb-1" style={{ color: 'var(--muted)' }}>Extra write paths:</p>
        <pre className="text-xs p-2 rounded-[var(--radius-sm)] border" style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}>
          {JSON.stringify(sandbox.extra_write || [], null, 2)}
        </pre>
      </div>
    </div>
  )
}

/* ── Chat Tab ─────────────────────────────────────────────────────── */

function ChatTab({ agentId }: { agentId: string }) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [steer, setSteer] = useState(false)

  const loadChat = useCallback(async () => {
    const msgs = await getAgentChat(agentId)
    setMessages(msgs)
  }, [agentId])

  useEffect(() => {
    loadChat()
    const interval = setInterval(loadChat, 5000)
    return () => clearInterval(interval)
  }, [loadChat])

  const handleSend = async () => {
    if (!input.trim()) return
    await sendAgentMessage(agentId, input, steer ? 'steer' : 'normal')
    setInput('')
    loadChat()
  }

  return (
    <div className="flex flex-col" style={{ height: '400px' }}>
      {/* Messages */}
      <div className="flex-1 overflow-y-auto space-y-2 mb-3">
        {messages.length === 0 && (
          <p className="text-sm" style={{ color: 'var(--muted)' }}>No messages yet.</p>
        )}
        {messages.map((m, i) => (
          <div
            key={i}
            className="text-sm px-3 py-2 rounded-[var(--radius-sm)]"
            style={{
              background: m.role === 'assistant' ? 'var(--accent-subtle)' : 'var(--bg)',
              color: 'var(--text)',
            }}
          >
            <span className="text-xs font-medium mr-2" style={{ color: 'var(--muted)' }}>
              {m.role}
            </span>
            {m.content || '(no content)'}
          </div>
        ))}
      </div>

      {/* Input */}
      <div className="flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleSend()}
          placeholder="Type a message..."
          className="flex-1 rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
        />
        <label className="flex items-center gap-1 text-xs" style={{ color: 'var(--muted)' }}>
          <input type="checkbox" checked={steer} onChange={(e) => setSteer(e.target.checked)} />
          Steer
        </label>
        <button
          onClick={handleSend}
          className="flex items-center gap-1 rounded-[var(--radius-sm)] px-3 py-2 text-sm font-medium text-white"
          style={{ background: 'var(--accent)' }}
        >
          <Send size={14} /> Send
        </button>
      </div>
    </div>
  )
}

/* ── Shared ────────────────────────────────────────────────────────── */

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex">
      <span className="w-28 shrink-0" style={{ color: 'var(--muted)' }}>{label}</span>
      <span style={{ color: 'var(--text)' }}>{value}</span>
    </div>
  )
}
