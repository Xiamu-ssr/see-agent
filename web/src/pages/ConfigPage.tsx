import { useState, useEffect } from 'react'
import { getConfig, updateConfig, getSchema } from '@/api/config'
import { Save, Eye, EyeOff } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'

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
    return <div className="px-3 py-3 md:px-4 md:py-4 text-[var(--muted)]">Loading...</div>
  }

  const properties = (schema as { properties?: Record<string, Record<string, unknown>> }).properties || {}

  return (
    <div className="px-3 py-3 md:px-4 md:py-4">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold text-[var(--text-strong)]">
          Config
        </h1>
        <div className="flex items-center gap-2">
          {message && (
            <span className={`text-xs ${message.startsWith('Error') ? 'text-[var(--danger)]' : 'text-[var(--ok)]'}`}>
              {message}
            </span>
          )}
          <Button onClick={handleSave} disabled={saving} size="sm">
            <Save size={14} />
            Save
          </Button>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card className="p-4 space-y-4">
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
        </Card>

        <Card className="p-4">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-medium text-[var(--text-strong)]">
              JSON
            </span>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setEditing(!editing)}
            >
              {editing ? <EyeOff size={12} /> : <Eye size={12} />}
              {editing ? 'Read-only' : 'Edit'}
            </Button>
          </div>
          {editing ? (
            <textarea
              value={jsonText}
              onChange={(e) => setJsonText(e.target.value)}
              className="w-full h-[400px] md:h-[500px] text-xs rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] p-3 outline-none resize-none text-[var(--text-strong)] font-mono"
              spellCheck={false}
            />
          ) : (
            <pre className="text-xs overflow-auto max-h-[400px] md:max-h-[500px] text-[var(--text-strong)] font-mono">
              {jsonText}
            </pre>
          )}
        </Card>
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

  if (isObject) {
    return (
      <div>
        <label className="block text-xs font-medium mb-1 text-[var(--muted)]">
          {name}
        </label>
        <pre className="text-xs p-2 rounded bg-[var(--bg)] text-[var(--text-strong)] font-mono overflow-x-auto">
          {JSON.stringify(value, null, 2)}
        </pre>
      </div>
    )
  }

  if (isBoolean) {
    return (
      <div className="flex items-center justify-between">
        <label className="text-xs font-medium text-[var(--muted)]">
          {name}
        </label>
        <Switch checked={!!value} onCheckedChange={() => onChange(!value)} />
      </div>
    )
  }

  if (enumValues) {
    return (
      <div>
        <label className="block text-xs font-medium mb-1 text-[var(--muted)]">
          {name}
        </label>
        <select
          value={String(value || '')}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] px-3 py-1.5 text-sm text-[var(--text-strong)] outline-none"
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
      <label className="block text-xs font-medium mb-1 text-[var(--muted)]">
        {name}
      </label>
      <Input
        type={isNumber ? 'number' : 'text'}
        value={value == null ? '' : String(value)}
        onChange={(e) => onChange(isNumber ? Number(e.target.value) : e.target.value)}
      />
    </div>
  )
}
