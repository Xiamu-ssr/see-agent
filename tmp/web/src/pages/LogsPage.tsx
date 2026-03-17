import { useState, useEffect, useCallback } from 'react'
import { getLogs } from '@/api/logs'
import type { LogEntry } from '@/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

const levelVariant: Record<string, 'success' | 'secondary' | 'warning' | 'destructive'> = {
  INFO: 'success',
  DEBUG: 'secondary',
  WARNING: 'warning',
  WARN: 'warning',
  ERROR: 'destructive',
  CRITICAL: 'destructive',
}

export default function LogsPage() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState('ALL')

  const refresh = useCallback(() => {
    getLogs().then(setLogs).finally(() => setLoading(false))
  }, [])

  useEffect(() => {
    refresh()
    const interval = setInterval(refresh, 5000)
    return () => clearInterval(interval)
  }, [refresh])

  const filtered = filter === 'ALL' ? logs : logs.filter(l => l.level === filter)

  return (
    <div className="px-3 py-3 md:px-4 md:py-4">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold text-[var(--text-strong)]">Logs</h1>
        <div className="flex items-center gap-2">
          <select
            value={filter}
            onChange={e => setFilter(e.target.value)}
            className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg)] px-2.5 py-1.5 text-xs text-[var(--text-strong)]"
          >
            {['ALL', 'DEBUG', 'INFO', 'WARNING', 'ERROR'].map(l => (
              <option key={l} value={l}>{l}</option>
            ))}
          </select>
          <Button variant="outline" size="sm" onClick={refresh}>
            Refresh
          </Button>
        </div>
      </div>

      {loading ? (
        <div className="text-[var(--muted)]">Loading...</div>
      ) : (
        <Card className="overflow-x-auto">
          <table className="w-full text-[11px] md:text-xs font-mono">
            <thead>
              <tr className="bg-[var(--bg-elevated)]">
                <th className="text-left px-3 py-2.5 font-medium text-[var(--muted)] hidden md:table-cell" style={{ width: 180 }}>Timestamp</th>
                <th className="text-left px-3 py-2.5 font-medium text-[var(--muted)]" style={{ width: 70 }}>Level</th>
                <th className="text-left px-3 py-2.5 font-medium text-[var(--muted)] hidden md:table-cell" style={{ width: 120 }}>Source</th>
                <th className="text-left px-3 py-2.5 font-medium text-[var(--muted)]">Message</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((log, i) => (
                <tr
                  key={i}
                  className="border-t border-[var(--border)]"
                  style={{ background: i % 2 === 0 ? 'var(--bg)' : 'var(--bg-elevated)' }}
                >
                  <td className="px-3 py-2 text-[var(--muted)] hidden md:table-cell">{log.time}</td>
                  <td className="px-3 py-2">
                    <Badge variant={levelVariant[log.level] || 'secondary'}>{log.level}</Badge>
                  </td>
                  <td className="px-3 py-2 text-[var(--text-strong)] hidden md:table-cell">{log.logger || 'server'}</td>
                  <td className="px-3 py-2 text-[var(--text-strong)]">{log.message}</td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-3 py-8 text-center text-[var(--muted)]">
                    No logs found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  )
}
