import { useState, useEffect, useCallback } from 'react'
import { getLogs } from '@/api/logs'
import type { LogEntry } from '@/types'

const levelColors: Record<string, string> = {
  INFO: '#3fb950',
  DEBUG: '#7d8590',
  WARNING: '#d29922',
  WARN: '#d29922',
  ERROR: '#f85149',
  CRITICAL: '#f85149',
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
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold" style={{ color: '#e6edf3' }}>Logs</h1>
        <div className="flex items-center gap-2">
          <select
            value={filter}
            onChange={e => setFilter(e.target.value)}
            className="rounded-md border px-2.5 py-1.5 text-xs"
            style={{ background: '#0d1117', borderColor: '#30363d', color: '#e6edf3' }}
          >
            {['ALL', 'DEBUG', 'INFO', 'WARNING', 'ERROR'].map(l => (
              <option key={l} value={l}>{l}</option>
            ))}
          </select>
          <button
            onClick={refresh}
            className="rounded-md border px-3 py-1.5 text-xs"
            style={{ borderColor: '#30363d', color: '#7d8590' }}
          >
            Refresh
          </button>
        </div>
      </div>

      {loading ? (
        <div style={{ color: '#7d8590' }}>Loading...</div>
      ) : (
        <div className="rounded-lg border overflow-hidden" style={{ borderColor: '#30363d' }}>
          <table className="w-full text-xs" style={{ fontFamily: 'var(--mono, monospace)' }}>
            <thead>
              <tr style={{ background: '#161b22' }}>
                <th className="text-left px-3 py-2.5 font-medium" style={{ color: '#7d8590', width: 180 }}>Timestamp</th>
                <th className="text-left px-3 py-2.5 font-medium" style={{ color: '#7d8590', width: 70 }}>Level</th>
                <th className="text-left px-3 py-2.5 font-medium" style={{ color: '#7d8590', width: 120 }}>Source</th>
                <th className="text-left px-3 py-2.5 font-medium" style={{ color: '#7d8590' }}>Message</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((log, i) => (
                <tr
                  key={i}
                  className="border-t"
                  style={{
                    borderColor: '#21262d',
                    background: i % 2 === 0 ? '#0d1117' : '#161b22',
                  }}
                >
                  <td className="px-3 py-2" style={{ color: '#7d8590' }}>{log.time}</td>
                  <td className="px-3 py-2">
                    <span style={{ color: levelColors[log.level] || '#7d8590' }}>{log.level}</span>
                  </td>
                  <td className="px-3 py-2" style={{ color: '#e6edf3' }}>{log.logger || 'server'}</td>
                  <td className="px-3 py-2" style={{ color: '#e6edf3' }}>{log.message}</td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-3 py-8 text-center" style={{ color: '#7d8590' }}>
                    No logs found.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
