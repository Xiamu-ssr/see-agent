import { useState, useEffect } from 'react'
import { getConfig, updateConfig, getSchema } from '@/api/config'
import { Save, Eye, EyeOff } from 'lucide-react'

export default function ConfigPage() {
  const [config, setConfig] = useState<Record<string, unknown> | null>(null)
  const [schema, setSchema] = useState<Record<string, unknown> | null>(null)
  const [jsonText, setJsonText] = useState('')
  const [editing, setEditing] = useState(false)
  const [saving, setSaving] = useState(false)
  const [message, setMessage] = useState('')

  useEffect(() => {
    Promise.all([getConfig(), getSchema('config')]).then(([cfg, sch]) => {
      setConfig(cfg)
      setSchema(sch)
      setJsonText(JSON.stringify(cfg, null, 2))
    })
  }, [])

  const handleSave = async () => {
    setSaving(true)
    setMessage('')
    try {
      const toSave = editing ? JSON.parse(jsonText) : config
      await updateConfig(toSave)
      setMessage('Saved')
      // Refresh
      const fresh = await getConfig()
      setConfig(fresh)
      setJsonText(JSON.stringify(fresh, null, 2))
    } catch (e) {
      setMessage(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setSaving(false)
    }
  }

  if (!config || !schema) {
    return <div style={{ color: 'var(--muted)' }}>Loading...</div>
  }

  const properties = (schema as { properties?: Record<string, Record<string, unknown>> }).properties || {}

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
          Config
        </h1>
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
            onClick={handleSave}
            disabled={saving}
            className="flex items-center gap-1.5 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white"
            style={{ background: 'var(--accent)', opacity: saving ? 0.6 : 1 }}
          >
            <Save size={14} />
            Save
          </button>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        {/* Left: Form */}
        <div
          className="rounded-[var(--radius-lg)] border p-5 space-y-4"
          style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
        >
          {Object.entries(properties).map(([key, prop]) => (
            <ConfigField
              key={key}
              name={key}
              prop={prop}
              value={(config as Record<string, unknown>)[key]}
              onChange={(val) => {
                const updated = { ...config, [key]: val }
                setConfig(updated)
                setJsonText(JSON.stringify(updated, null, 2))
              }}
            />
          ))}
        </div>

        {/* Right: JSON preview */}
        <div
          className="rounded-[var(--radius-lg)] border p-5"
          style={{ background: 'var(--card)', borderColor: 'var(--border)' }}
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-medium" style={{ color: 'var(--text-strong)' }}>
              JSON
            </span>
            <button
              onClick={() => setEditing(!editing)}
              className="flex items-center gap-1 text-xs rounded px-2 py-1 hover:bg-[var(--bg-hover)]"
              style={{ color: 'var(--muted)' }}
            >
              {editing ? <EyeOff size={12} /> : <Eye size={12} />}
              {editing ? 'Read-only' : 'Edit'}
            </button>
          </div>
          {editing ? (
            <textarea
              value={jsonText}
              onChange={(e) => setJsonText(e.target.value)}
              className="w-full h-[500px] text-xs rounded-[var(--radius-sm)] border p-3 outline-none resize-none"
              style={{
                background: 'var(--bg)',
                borderColor: 'var(--border)',
                color: 'var(--text)',
                fontFamily: 'var(--mono)',
              }}
            />
          ) : (
            <pre
              className="text-xs overflow-auto max-h-[500px]"
              style={{ color: 'var(--text)', fontFamily: 'var(--mono)' }}
            >
              {jsonText}
            </pre>
          )}
        </div>
      </div>
    </div>
  )
}

function ConfigField({
  name,
  prop,
  value,
  onChange,
}: {
  name: string
  prop: Record<string, unknown>
  value: unknown
  onChange: (v: unknown) => void
}) {
  const type = prop.type as string | string[]
  const isObject = type === 'object' || (Array.isArray(type) && type.includes('object'))
  const isBoolean = type === 'boolean'
  const isNumber = type === 'integer' || type === 'number'
  const enumValues = prop.enum as string[] | undefined

  // For nested objects, just show a JSON preview
  if (isObject) {
    return (
      <div>
        <label className="block text-xs font-medium mb-1" style={{ color: 'var(--muted)' }}>
          {name}
        </label>
        <pre className="text-xs p-2 rounded" style={{ background: 'var(--bg)', color: 'var(--text)', fontFamily: 'var(--mono)' }}>
          {JSON.stringify(value, null, 2)}
        </pre>
      </div>
    )
  }

  if (isBoolean) {
    return (
      <div className="flex items-center justify-between">
        <label className="text-xs font-medium" style={{ color: 'var(--muted)' }}>
          {name}
        </label>
        <button
          onClick={() => onChange(!value)}
          className="relative w-9 h-5 rounded-full transition-colors"
          style={{ background: value ? 'var(--accent)' : 'var(--border-strong)' }}
        >
          <span
            className="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
            style={{ left: value ? '18px' : '2px' }}
          />
        </button>
      </div>
    )
  }

  if (enumValues) {
    return (
      <div>
        <label className="block text-xs font-medium mb-1" style={{ color: 'var(--muted)' }}>
          {name}
        </label>
        <select
          value={String(value || '')}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded-[var(--radius-sm)] border px-3 py-1.5 text-sm outline-none"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
        >
          {enumValues.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      </div>
    )
  }

  return (
    <div>
      <label className="block text-xs font-medium mb-1" style={{ color: 'var(--muted)' }}>
        {name}
      </label>
      <input
        type={isNumber ? 'number' : 'text'}
        value={value == null ? '' : String(value)}
        onChange={(e) => onChange(isNumber ? Number(e.target.value) : e.target.value)}
        className="w-full rounded-[var(--radius-sm)] border px-3 py-1.5 text-sm outline-none"
        style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
      />
    </div>
  )
}
