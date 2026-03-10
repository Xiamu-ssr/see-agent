import { useState, useEffect, useCallback } from 'react'
import { getLogs } from '@/api/logs'
import type { LogEntry } from '@/api/logs'
import { Search } from 'lucide-react'

const levelColors: Record<string, string> = {
  DEBUG: 'var(--muted)',
  INFO: 'var(--accent-2)',
  WARNING: 'var(--warn)',
  ERROR: 'var(--danger)',
  CRITICAL: 'var(--danger)',
}

const levels = ['ALL', 'DEBUG', 'INFO', 'WARNING', 'ERROR']

function today() {
  return new Date().toISOString().slice(0, 10)
}

export default function LogsPage() {
  const [date, setDate] = useState(today())
  const [level, setLevel] = useState('ALL')
  const [search, setSearch] = useState('')
  const [entries, setEntries] = useState<LogEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [offset, setOffset] = useState(0)
  const limit = 100

  const fetchLogs = useCallback(
    async (reset = false) => {
      setLoading(true)
      const newOffset = reset ? 0 : offset
      const result = await getLogs({
        date,
        level: level === 'ALL' ? '' : level,
        limit,
        offset: newOffset,
      })
      if (reset) {
        setEntries(result)
        setOffset(result.length)
      } else {
        setEntries((prev) => [...prev, ...result])
        setOffset(newOffset + result.length)
      }
      setLoading(false)
    },
    [date, level, offset],
  )

  useEffect(() => {
    setOffset(0)
    fetchLogs(true)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [date, level])

  const filtered = search
    ? entries.filter(
        (e) =>
          e.message.toLowerCase().includes(search.toLowerCase()) ||
          e.logger.toLowerCase().includes(search.toLowerCase()),
      )
    : entries

  return (
    <div>
      <h1 className="text-lg font-semibold mb-4" style={{ color: 'var(--text-strong)' }}>
        Logs
      </h1>

      <div className="flex flex-wrap gap-3 mb-4">
        <input
          type="date"
          value={date}
          onChange={(e) => setDate(e.target.value)}
          className="rounded-[var(--radius-sm)] border px-3 py-1.5 text-sm outline-none"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
        />
        <select
          value={level}
          onChange={(e) => setLevel(e.target.value)}
          className="rounded-[var(--radius-sm)] border px-3 py-1.5 text-sm outline-none"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
        >
          {levels.map((l) => (
            <option key={l}>{l}</option>
          ))}
        </select>
        <div className="relative flex-1 min-w-[200px]">
          <Search
            size={14}
            className="absolute left-2.5 top-1/2 -translate-y-1/2"
            style={{ color: 'var(--muted)' }}
          />
          <input
            placeholder="Search..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full rounded-[var(--radius-sm)] border pl-8 pr-3 py-1.5 text-sm outline-none"
            style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
          />
        </div>
      </div>

      <div
        className="rounded-[var(--radius-lg)] border overflow-hidden"
        style={{ borderColor: 'var(--border)' }}
      >
        <div className="max-h-[calc(100vh-220px)] overflow-auto">
          <table className="w-full text-xs" style={{ fontFamily: 'var(--mono)' }}>
            <tbody>
              {filtered.map((e, i) => (
                <tr
                  key={i}
                  className="border-t hover:bg-[var(--bg-hover)]"
                  style={{ borderColor: 'var(--border)' }}
                >
                  <td className="px-3 py-1.5 whitespace-nowrap" style={{ color: 'var(--muted)' }}>
                    {e.time}
                  </td>
                  <td
                    className="px-2 py-1.5 whitespace-nowrap font-medium"
                    style={{ color: levelColors[e.level] || 'var(--text)' }}
                  >
                    {e.level}
                  </td>
                  <td className="px-2 py-1.5 whitespace-nowrap" style={{ color: 'var(--muted)' }}>
                    {e.logger}
                  </td>
                  <td className="px-2 py-1.5" style={{ color: 'var(--text)' }}>
                    {e.message}
                  </td>
                </tr>
              ))}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-4 py-8 text-center text-sm" style={{ color: 'var(--muted)' }}>
                    {loading ? 'Loading...' : 'No log entries'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        {entries.length >= offset && entries.length > 0 && (
          <div className="border-t p-2 text-center" style={{ borderColor: 'var(--border)' }}>
            <button
              onClick={() => fetchLogs(false)}
              disabled={loading}
              className="text-xs px-3 py-1 rounded hover:bg-[var(--bg-hover)]"
              style={{ color: 'var(--accent)' }}
            >
              {loading ? 'Loading...' : 'Load more'}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
