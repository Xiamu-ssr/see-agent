import type { AgentDetail } from '@/types'

interface Props {
  agent: AgentDetail
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex">
      <span className="w-28 shrink-0" style={{ color: 'var(--muted)' }}>{label}</span>
      <span style={{ color: 'var(--text)' }}>{value}</span>
    </div>
  )
}

export default function AgentSafehouse({ agent }: Props) {
  const sandbox = agent.sandbox || {}
  return (
    <div className="space-y-3 text-sm">
      <Row label="Enabled" value={sandbox.enabled ? 'Yes' : 'No'} />
      <Row label="Network" value={sandbox.network ? 'Yes' : 'No'} />
      <Row label="Screen" value={sandbox.screen_access ? 'Yes' : 'No'} />
      <div>
        <p className="mb-1" style={{ color: 'var(--muted)' }}>Extra read paths:</p>
        <pre
          className="text-xs p-2 rounded-[var(--radius-sm)] border"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}
        >
          {JSON.stringify(sandbox.extra_read || [], null, 2)}
        </pre>
      </div>
      <div>
        <p className="mb-1" style={{ color: 'var(--muted)' }}>Extra write paths:</p>
        <pre
          className="text-xs p-2 rounded-[var(--radius-sm)] border"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}
        >
          {JSON.stringify(sandbox.extra_write || [], null, 2)}
        </pre>
      </div>
    </div>
  )
}
