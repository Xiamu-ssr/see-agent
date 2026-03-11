import { useState, useEffect, useCallback } from 'react'
import { getWorkspaceFiles, getWorkspaceFile, updateWorkspaceFile } from '@/api/agents'
import type { WorkspaceFileItem } from '@/types'
import { Save } from 'lucide-react'

interface Props {
  agentId: string
}

export default function AgentFiles({ agentId }: Props) {
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
