import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { getAgent, updateAgent } from '@/api/agents'
import type { AgentDetail } from '@/types'
import { ArrowLeft, Bot, Save } from 'lucide-react'

type Tab = 'info' | 'soul' | 'config'

export default function AgentDetailPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [agent, setAgent] = useState<AgentDetail | null>(null)
  const [loading, setLoading] = useState(true)
  const [tab, setTab] = useState<Tab>('info')
  const [soulText, setSoulText] = useState('')
  const [soulDirty, setSoulDirty] = useState(false)
  const [saving, setSaving] = useState(false)
  const [message, setMessage] = useState('')

  useEffect(() => {
    if (!id) return
    getAgent(id)
      .then((a) => {
        setAgent(a)
        setSoulText((a as AgentDetailWithSoul).soul_content || '')
      })
      .catch(() => setAgent(null))
      .finally(() => setLoading(false))
  }, [id])

  const handleSaveSoul = async () => {
    if (!id) return
    setSaving(true)
    setMessage('')
    try {
      await updateAgent(id, { soul: soulText })
      setMessage('Saved')
      setSoulDirty(false)
    } catch (e) {
      setMessage(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setSaving(false)
    }
  }

  if (loading) return <div style={{ color: 'var(--muted)' }}>Loading...</div>
  if (!agent) return <div style={{ color: 'var(--danger)' }}>Agent not found</div>

  const tabs: Tab[] = ['info', 'soul', 'config']

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

      {/* Header card */}
      <div
        className="rounded-[var(--radius-lg)] border p-5 mb-4"
        style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
      >
        <div className="flex items-center gap-3">
          <div className="rounded-full p-2" style={{ background: 'var(--accent-subtle)' }}>
            <Bot size={20} style={{ color: 'var(--accent)' }} />
          </div>
          <div>
            <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
              {agent.name}
            </h1>
            <p className="text-sm" style={{ color: 'var(--muted)' }}>
              {agent.role} &middot; {agent.team_name || 'No team'}
              {agent.has_soul && ' \u00b7 Has SOUL'}
            </p>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-4">
        {tabs.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className="px-3 py-1.5 text-sm rounded-[var(--radius-sm)] capitalize transition-colors"
            style={{
              background: tab === t ? 'var(--accent-subtle)' : 'transparent',
              color: tab === t ? 'var(--accent)' : 'var(--muted)',
            }}
          >
            {t === 'soul' ? 'SOUL' : t}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div
        className="rounded-[var(--radius-lg)] border p-5"
        style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
      >
        {tab === 'info' && (
          <div className="space-y-3 text-sm">
            <Row label="ID" value={agent.id} />
            <Row label="Name" value={agent.name} />
            <Row label="Role" value={agent.role} />
            <Row label="Team" value={agent.team_name || '\u2014'} />
            <Row label="Location" value={agent.location} />
            <Row label="Has SOUL" value={agent.has_soul ? 'Yes' : 'No'} />
          </div>
        )}

        {tab === 'soul' && (
          <div>
            <div className="flex items-center justify-between mb-3">
              <p className="text-sm" style={{ color: 'var(--muted)' }}>
                SOUL.md defines this agent&apos;s personality and behavior.
              </p>
              <div className="flex items-center gap-2">
                {message && (
                  <span
                    className="text-xs"
                    style={{ color: message.startsWith('Error') ? 'var(--danger)' : 'var(--ok)' }}
                  >
                    {message}
                  </span>
                )}
                <button
                  onClick={handleSaveSoul}
                  disabled={saving || !soulDirty}
                  className="flex items-center gap-1 rounded-[var(--radius-sm)] px-3 py-1 text-sm font-medium text-white"
                  style={{ background: 'var(--accent)', opacity: saving || !soulDirty ? 0.5 : 1 }}
                >
                  <Save size={12} />
                  Save
                </button>
              </div>
            </div>
            <textarea
              value={soulText}
              onChange={(e) => { setSoulText(e.target.value); setSoulDirty(true); setMessage('') }}
              placeholder="# Agent SOUL\n\nDescribe this agent's personality, expertise, and behavior guidelines..."
              className="w-full h-[400px] text-sm rounded-[var(--radius-sm)] border p-4 outline-none resize-none"
              style={{
                background: 'var(--bg)',
                borderColor: 'var(--border)',
                color: 'var(--text)',
                fontFamily: 'var(--mono)',
              }}
            />
          </div>
        )}

        {tab === 'config' && (
          <pre
            className="text-xs overflow-auto"
            style={{ color: 'var(--text)', fontFamily: 'var(--mono)' }}
          >
            {JSON.stringify(
              {
                config_overrides: agent.config_overrides,
                tools_config: agent.tools_config,
                skills_config: agent.skills_config,
                mcp_config: agent.mcp_config,
              },
              null,
              2,
            )}
          </pre>
        )}
      </div>
    </div>
  )
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex">
      <span className="w-24 shrink-0" style={{ color: 'var(--muted)' }}>
        {label}
      </span>
      <span style={{ color: 'var(--text)' }}>{value}</span>
    </div>
  )
}

// Extended type for SOUL content (backend may include it)
interface AgentDetailWithSoul extends AgentDetail {
  soul_content?: string
}
