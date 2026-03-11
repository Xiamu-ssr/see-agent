import type { AgentDetail } from '@/types'
import { Shield, Wifi, Monitor, Clipboard, FolderOpen } from 'lucide-react'

interface Props {
  agent: AgentDetail
}

function PermRow({ icon: Icon, label, enabled }: { icon: typeof Shield; label: string; enabled: boolean }) {
  return (
    <div
      className="flex items-center justify-between rounded-lg px-3 py-2.5"
      style={{ background: '#0d1117' }}
    >
      <div className="flex items-center gap-3">
        <Icon size={16} style={{ color: enabled ? '#3fb950' : '#7d8590' }} />
        <span className="text-sm" style={{ color: '#e6edf3' }}>{label}</span>
      </div>
      <span
        className="text-xs font-medium rounded-full px-2 py-0.5"
        style={{
          background: enabled ? 'rgba(63, 185, 80, 0.15)' : 'rgba(125, 133, 144, 0.15)',
          color: enabled ? '#3fb950' : '#7d8590',
        }}
      >
        {enabled ? 'Allowed' : 'Blocked'}
      </span>
    </div>
  )
}

export default function AgentSafehouse({ agent }: Props) {
  const sandbox = (agent.sandbox || {}) as Record<string, unknown>
  const isEnabled = (sandbox.enabled as boolean) ?? false
  const extraRead = (sandbox.extra_read as string[]) ?? []
  const extraWrite = (sandbox.extra_write as string[]) ?? []

  return (
    <div className="space-y-6">
      {/* Status */}
      <div className="flex items-center gap-3">
        <Shield size={20} style={{ color: isEnabled ? '#3fb950' : '#7d8590' }} />
        <div>
          <p className="text-sm font-medium" style={{ color: '#e6edf3' }}>
            Sandbox {isEnabled ? 'Enabled' : 'Disabled'}
          </p>
          <p className="text-xs" style={{ color: '#7d8590' }}>
            {isEnabled ? 'Agent runs in sandbox-exec isolated environment' : 'Agent runs without sandbox isolation'}
          </p>
        </div>
      </div>

      {/* Permissions */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide mb-2" style={{ color: '#7d8590' }}>
          Permissions
        </h3>
        <div className="space-y-1">
          <PermRow icon={Wifi} label="Network Access" enabled={sandbox.network as boolean ?? false} />
          <PermRow icon={Monitor} label="Screen Access" enabled={sandbox.screen_access as boolean ?? false} />
          <PermRow icon={Clipboard} label="Clipboard" enabled={sandbox.clipboard as boolean ?? true} />
        </div>
      </div>

      {/* File Access */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide mb-2" style={{ color: '#7d8590' }}>
          File Access
        </h3>
        <div className="space-y-1">
          <div className="flex items-center gap-3 rounded-lg px-3 py-2.5" style={{ background: '#0d1117' }}>
            <FolderOpen size={16} style={{ color: '#ff5c5c' }} />
            <div>
              <p className="text-sm" style={{ color: '#e6edf3' }}>Agent workspace</p>
              <p className="text-xs" style={{ color: '#7d8590', fontFamily: 'var(--mono, monospace)' }}>
                ~/.see-agent/agents/{agent.id}/
              </p>
            </div>
            <span className="ml-auto text-xs rounded-full px-2 py-0.5" style={{ background: 'rgba(255, 92, 92, 0.15)', color: '#ff5c5c' }}>
              read-write
            </span>
          </div>

          {(extraRead.length ?? 0) > 0 && (
            <div className="rounded-lg px-3 py-2.5" style={{ background: '#0d1117' }}>
              <p className="text-xs font-medium mb-1" style={{ color: '#7d8590' }}>Extra read paths:</p>
              {extraRead.map((p: string, i: number) => (
                <p key={i} className="text-xs" style={{ color: '#e6edf3', fontFamily: 'var(--mono, monospace)' }}>{p}</p>
              ))}
            </div>
          )}

          {(extraWrite.length ?? 0) > 0 && (
            <div className="rounded-lg px-3 py-2.5" style={{ background: '#0d1117' }}>
              <p className="text-xs font-medium mb-1" style={{ color: '#7d8590' }}>Extra write paths:</p>
              {extraWrite.map((p: string, i: number) => (
                <p key={i} className="text-xs" style={{ color: '#e6edf3', fontFamily: 'var(--mono, monospace)' }}>{p}</p>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
