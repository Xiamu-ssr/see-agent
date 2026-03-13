import { useState, useEffect } from 'react'
import { getConfig } from '@/api/config'
import { installMcp, deleteMcp } from '@/api/mcp'
import type { InstallMcpRequest } from '@/types'
import { Plug, Plus, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'

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

  useEffect(() => { refresh() }, [])

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

  const handlePackageChange = (pkg: string) => {
    setForm((prev) => {
      const name = prev.name || pkg.split('/').pop()?.replace(/^@/, '') || ''
      return { ...prev, package: pkg, name }
    })
  }

  if (loading) return <div className="text-[var(--muted)]">Loading...</div>

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold text-[var(--text-strong)]">
          MCP Servers
        </h1>
        <Button onClick={() => { setShowInstall(true); setInstallMsg('') }} size="sm">
          <Plus size={14} />
          Add MCP Server
        </Button>
      </div>

      <Card className="overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-[var(--bg-elevated)]">
              <th className="text-left px-4 py-2.5 font-medium text-[var(--muted)]">Name</th>
              <th className="text-left px-4 py-2.5 font-medium text-[var(--muted)]">Type</th>
              <th className="text-left px-4 py-2.5 font-medium text-[var(--muted)]">Command</th>
              <th className="text-right px-4 py-2.5 font-medium text-[var(--muted)]">Actions</th>
            </tr>
          </thead>
          <tbody>
            {servers.map((s) => (
              <tr key={s.name} className="border-t border-[var(--border)] hover:bg-[var(--bg-hover)]">
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <Plug size={14} className="text-[var(--accent-2)]" />
                    <span className="text-[var(--text-strong)]">{s.name}</span>
                  </div>
                </td>
                <td className="px-4 py-2.5 text-[var(--muted)]">{s.type}</td>
                <td className="px-4 py-2.5 text-[var(--text-strong)] font-mono">
                  {s.command} {s.args?.join(' ')}
                </td>
                <td className="px-4 py-2.5 text-right">
                  <Button variant="ghost" size="icon" onClick={() => handleDelete(s.name)} className="text-[var(--danger)]">
                    <Trash2 size={14} />
                  </Button>
                </td>
              </tr>
            ))}
            {servers.length === 0 && (
              <tr>
                <td colSpan={4} className="px-4 py-8 text-center text-[var(--muted)]">
                  No MCP servers configured.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </Card>

      <Dialog open={showInstall} onOpenChange={setShowInstall}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Add MCP Server</DialogTitle>
          </DialogHeader>

          <div className="flex gap-1 mb-4">
            {(['npm', 'pip', 'manual'] as const).map((t) => (
              <Button
                key={t}
                variant={installType === t ? 'default' : 'ghost'}
                size="sm"
                onClick={() => setInstallType(t)}
                className="capitalize"
              >
                {t === 'npm' ? 'npm Package' : t === 'pip' ? 'pip Package' : 'Manual'}
              </Button>
            ))}
          </div>

          <div className="space-y-3">
            {(installType === 'npm' || installType === 'pip') && (
              <>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">Package name</label>
                  <Input
                    placeholder={installType === 'npm' ? '@modelcontextprotocol/server-fs' : 'mcp-server-sqlite'}
                    value={form.package}
                    onChange={(e) => handlePackageChange(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">Parameters</label>
                  <Input
                    placeholder={installType === 'npm' ? '/Users/me/Documents' : '--db /path/to/db.sqlite'}
                    value={form.params}
                    onChange={(e) => setForm({ ...form, params: e.target.value })}
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">Server name</label>
                  <Input
                    placeholder="filesystem"
                    value={form.name}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                  />
                </div>
              </>
            )}

            {installType === 'manual' && (
              <>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">Server name</label>
                  <Input placeholder="my-server" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
                </div>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">Command</label>
                  <Input placeholder="node" value={form.command} onChange={(e) => setForm({ ...form, command: e.target.value })} />
                </div>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">Arguments (space-separated)</label>
                  <Input placeholder="server.js --port 3000" value={form.args} onChange={(e) => setForm({ ...form, args: e.target.value })} />
                </div>
                <div>
                  <label className="block text-xs font-medium mb-1 text-[var(--muted)]">
                    Environment Variables
                  </label>
                  {envRows.map((row, i) => (
                    <div key={i} className="flex gap-2 mb-1">
                      <Input
                        placeholder="KEY"
                        value={row.key}
                        onChange={(e) => {
                          const next = [...envRows]
                          next[i] = { ...row, key: e.target.value }
                          setEnvRows(next)
                        }}
                        className="flex-1 text-xs"
                      />
                      <Input
                        placeholder="VALUE"
                        value={row.value}
                        onChange={(e) => {
                          const next = [...envRows]
                          next[i] = { ...row, value: e.target.value }
                          setEnvRows(next)
                        }}
                        className="flex-1 text-xs"
                      />
                      <Button variant="ghost" size="sm" onClick={() => setEnvRows(envRows.filter((_, j) => j !== i))} className="text-[var(--danger)]">
                        x
                      </Button>
                    </div>
                  ))}
                  <button
                    onClick={() => setEnvRows([...envRows, { key: '', value: '' }])}
                    className="text-xs mt-1 text-[var(--accent)]"
                  >
                    + Add variable
                  </button>
                </div>
              </>
            )}

            {installMsg && (
              <p className={`text-xs ${installMsg.startsWith('Error') ? 'text-[var(--danger)]' : 'text-[var(--ok)]'}`}>
                {installMsg}
              </p>
            )}
          </div>

          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowInstall(false)}>
              Cancel
            </Button>
            <Button onClick={handleInstall} disabled={installing}>
              {installing ? 'Adding...' : installType === 'pip' ? 'Install & Add' : 'Add'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
