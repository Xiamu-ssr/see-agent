import { useState, useEffect } from 'react'
import { getConfig } from '@/api/config'
import { installMcp, deleteMcp } from '@/api/mcp'
import type { InstallMcpRequest } from '@/types'
import { Plug, Plus, Trash2 } from 'lucide-react'

interface McpServer {
  name: string
  type: string
  command: string
  args?: string[]
}

type InstallType = 'npm' | 'pip' | 'manual'

export default function McpPage() {
  const [servers, setServers] = useState<McpServer[]>([])
  const [loading, setLoading] = useState(true)
  const [showInstall, setShowInstall] = useState(false)
  const [installType, setInstallType] = useState<InstallType>('npm')
  const [form, setForm] = useState({ name: '', package: '', params: '', command: '', args: '' })
  const [envRows, setEnvRows] = useState<{ key: string; value: string }[]>([])
  const [installing, setInstalling] = useState(false)
  const [installMsg, setInstallMsg] = useState('')

  const refresh = () => {
    getConfig()
      .then((cfg) => {
        const mcp = (cfg.mcp_servers || {}) as Record<string, Record<string, unknown>>
        setServers(
          Object.entries(mcp).map(([name, v]) => ({
            name,
            type: String(v.type || 'stdio'),
            command: String(v.command || ''),
            args: (v.args as string[]) || [],
          })),
        )
      })
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    refresh()
  }, [])

  const handleInstall = async () => {
    if (!form.name.trim()) return
    setInstalling(true)
    setInstallMsg('')
    try {
      const payload: InstallMcpRequest = {
        name: form.name.trim(),
        install_type: installType,
      }
      if (installType === 'npm' || installType === 'pip') {
        payload.package = form.package.trim()
        if (form.params.trim()) payload.params = form.params.trim()
      } else {
        payload.command = form.command.trim()
        if (form.args.trim()) payload.args = form.args.split(/\s+/).filter(Boolean)
        const env: Record<string, string> = {}
        for (const row of envRows) {
          if (row.key.trim()) env[row.key.trim()] = row.value
        }
        if (Object.keys(env).length > 0) payload.env = env
      }
      await installMcp(payload)
      setInstallMsg('Added successfully')
      setForm({ name: '', package: '', params: '', command: '', args: '' })
      setEnvRows([])
      refresh()
    } catch (e) {
      setInstallMsg(`Error: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setInstalling(false)
    }
  }

  const handleDelete = async (name: string) => {
    if (!confirm(`Remove MCP server "${name}"?`)) return
    try {
      await deleteMcp(name)
      refresh()
    } catch {
      // ignore
    }
  }

  // Auto-infer name from package
  const handlePackageChange = (pkg: string) => {
    setForm((prev) => {
      const name = prev.name || pkg.split('/').pop()?.replace(/^@/, '') || ''
      return { ...prev, package: pkg, name }
    })
  }

  if (loading) return <div style={{ color: 'var(--muted)' }}>Loading...</div>

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-lg font-semibold" style={{ color: 'var(--text-strong)' }}>
          MCP Servers
        </h1>
        <button
          onClick={() => { setShowInstall(true); setInstallMsg('') }}
          className="flex items-center gap-1.5 rounded-[var(--radius)] px-3 py-1.5 text-sm font-medium text-white"
          style={{ background: 'var(--accent)' }}
        >
          <Plus size={14} />
          Add MCP Server
        </button>
      </div>

      <div
        className="overflow-hidden rounded-[var(--radius-lg)] border"
        style={{ borderColor: 'var(--border)' }}
      >
        <table className="w-full text-sm">
          <thead>
            <tr style={{ background: 'var(--bg-elevated)' }}>
              <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>Name</th>
              <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>Type</th>
              <th className="text-left px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>Command</th>
              <th className="text-right px-4 py-2.5 font-medium" style={{ color: 'var(--muted)' }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {servers.map((s) => (
              <tr
                key={s.name}
                className="border-t hover:bg-[var(--bg-hover)]"
                style={{ borderColor: 'var(--border)' }}
              >
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <Plug size={14} style={{ color: 'var(--accent-2)' }} />
                    <span style={{ color: 'var(--text-strong)' }}>{s.name}</span>
                  </div>
                </td>
                <td className="px-4 py-2.5" style={{ color: 'var(--muted)' }}>{s.type}</td>
                <td className="px-4 py-2.5" style={{ color: 'var(--text)', fontFamily: 'var(--mono)' }}>
                  {s.command} {s.args?.join(' ')}
                </td>
                <td className="px-4 py-2.5 text-right">
                  <button
                    onClick={() => handleDelete(s.name)}
                    className="text-xs p-1.5 rounded hover:bg-[var(--bg-hover)]"
                    style={{ color: 'var(--danger)' }}
                  >
                    <Trash2 size={14} />
                  </button>
                </td>
              </tr>
            ))}
            {servers.length === 0 && (
              <tr>
                <td colSpan={4} className="px-4 py-8 text-center" style={{ color: 'var(--muted)' }}>
                  No MCP servers configured.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Install modal */}
      {showInstall && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div
            className="w-full max-w-lg rounded-[var(--radius-lg)] border p-6"
            style={{ background: 'var(--bg-elevated)', borderColor: 'var(--border)' }}
          >
            <h2 className="text-base font-semibold mb-4" style={{ color: 'var(--text-strong)' }}>
              Add MCP Server
            </h2>

            {/* Type tabs */}
            <div className="flex gap-1 mb-4">
              {(['npm', 'pip', 'manual'] as const).map((t) => (
                <button
                  key={t}
                  onClick={() => setInstallType(t)}
                  className="px-3 py-1.5 text-sm rounded-[var(--radius-sm)] capitalize"
                  style={{
                    background: installType === t ? 'var(--accent-subtle)' : 'transparent',
                    color: installType === t ? 'var(--accent)' : 'var(--muted)',
                  }}
                >
                  {t === 'npm' ? 'npm Package' : t === 'pip' ? 'pip Package' : 'Manual'}
                </button>
              ))}
            </div>

            <div className="space-y-3">
              {(installType === 'npm' || installType === 'pip') && (
                <>
                  <Input
                    label="Package name"
                    placeholder={installType === 'npm' ? '@modelcontextprotocol/server-fs' : 'mcp-server-sqlite'}
                    value={form.package}
                    onChange={handlePackageChange}
                  />
                  <Input
                    label="Parameters"
                    placeholder={installType === 'npm' ? '/Users/me/Documents' : '--db /path/to/db.sqlite'}
                    value={form.params}
                    onChange={(v) => setForm({ ...form, params: v })}
                  />
                  <Input
                    label="Server name"
                    placeholder="filesystem"
                    value={form.name}
                    onChange={(v) => setForm({ ...form, name: v })}
                  />
                </>
              )}

              {installType === 'manual' && (
                <>
                  <Input
                    label="Server name"
                    placeholder="my-server"
                    value={form.name}
                    onChange={(v) => setForm({ ...form, name: v })}
                  />
                  <Input
                    label="Command"
                    placeholder="node"
                    value={form.command}
                    onChange={(v) => setForm({ ...form, command: v })}
                  />
                  <Input
                    label="Arguments (space-separated)"
                    placeholder="server.js --port 3000"
                    value={form.args}
                    onChange={(v) => setForm({ ...form, args: v })}
                  />
                  <div>
                    <label className="block text-xs font-medium mb-1" style={{ color: 'var(--muted)' }}>
                      Environment Variables
                    </label>
                    {envRows.map((row, i) => (
                      <div key={i} className="flex gap-2 mb-1">
                        <input
                          placeholder="KEY"
                          value={row.key}
                          onChange={(e) => {
                            const next = [...envRows]
                            next[i] = { ...row, key: e.target.value }
                            setEnvRows(next)
                          }}
                          className="flex-1 rounded-[var(--radius-sm)] border px-2 py-1 text-xs outline-none"
                          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
                        />
                        <input
                          placeholder="VALUE"
                          value={row.value}
                          onChange={(e) => {
                            const next = [...envRows]
                            next[i] = { ...row, value: e.target.value }
                            setEnvRows(next)
                          }}
                          className="flex-1 rounded-[var(--radius-sm)] border px-2 py-1 text-xs outline-none"
                          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
                        />
                        <button
                          onClick={() => setEnvRows(envRows.filter((_, j) => j !== i))}
                          className="text-xs px-1"
                          style={{ color: 'var(--danger)' }}
                        >
                          x
                        </button>
                      </div>
                    ))}
                    <button
                      onClick={() => setEnvRows([...envRows, { key: '', value: '' }])}
                      className="text-xs mt-1"
                      style={{ color: 'var(--accent)' }}
                    >
                      + Add variable
                    </button>
                  </div>
                </>
              )}

              {installMsg && (
                <p
                  className="text-xs"
                  style={{ color: installMsg.startsWith('Error') ? 'var(--danger)' : 'var(--ok)' }}
                >
                  {installMsg}
                </p>
              )}

              <div className="flex gap-2 justify-end mt-2">
                <button
                  onClick={() => setShowInstall(false)}
                  className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm"
                  style={{ color: 'var(--muted)' }}
                >
                  Cancel
                </button>
                <button
                  onClick={handleInstall}
                  disabled={installing}
                  className="rounded-[var(--radius-sm)] px-3 py-1.5 text-sm font-medium text-white"
                  style={{ background: 'var(--accent)', opacity: installing ? 0.6 : 1 }}
                >
                  {installing ? 'Adding...' : installType === 'pip' ? 'Install & Add' : 'Add'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function Input({
  label,
  placeholder,
  value,
  onChange,
}: {
  label: string
  placeholder: string
  value: string
  onChange: (v: string) => void
}) {
  return (
    <div>
      <label className="block text-xs font-medium mb-1" style={{ color: 'var(--muted)' }}>
        {label}
      </label>
      <input
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
        style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
      />
    </div>
  )
}
