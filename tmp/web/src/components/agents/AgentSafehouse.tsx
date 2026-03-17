import type { AgentDetail } from '@/types'
import { Shield, Wifi, Monitor, Clipboard, FolderOpen } from 'lucide-react'
import { Badge } from '@/components/ui/badge'

interface Props {
  agent: AgentDetail
}

function PermRow({ icon: Icon, label, enabled }: { icon: typeof Shield; label: string; enabled: boolean }) {
  return (
    <div className="flex items-center justify-between rounded-lg px-3 py-2.5 bg-[var(--bg)]">
      <div className="flex items-center gap-3">
        <Icon size={16} className={enabled ? 'text-[var(--ok)]' : 'text-[var(--muted)]'} />
        <span className="text-sm text-[var(--text-strong)]">{label}</span>
      </div>
      <Badge variant={enabled ? 'success' : 'secondary'}>
        {enabled ? 'Allowed' : 'Blocked'}
      </Badge>
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
      <div className="flex items-center gap-3">
        <Shield size={20} className={isEnabled ? 'text-[var(--ok)]' : 'text-[var(--muted)]'} />
        <div>
          <p className="text-sm font-medium text-[var(--text-strong)]">
            Sandbox {isEnabled ? 'Enabled' : 'Disabled'}
          </p>
          <p className="text-xs text-[var(--muted)]">
            {isEnabled ? 'Agent runs in sandbox-exec isolated environment' : 'Agent runs without sandbox isolation'}
          </p>
        </div>
      </div>

      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide mb-2 text-[var(--muted)]">
          Permissions
        </h3>
        <div className="space-y-1">
          <PermRow icon={Wifi} label="Network Access" enabled={sandbox.network as boolean ?? false} />
          <PermRow icon={Monitor} label="Screen Access" enabled={sandbox.screen_access as boolean ?? false} />
          <PermRow icon={Clipboard} label="Clipboard" enabled={sandbox.clipboard as boolean ?? true} />
        </div>
      </div>

      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide mb-2 text-[var(--muted)]">
          File Access
        </h3>
        <div className="space-y-1">
          <div className="flex items-center gap-3 rounded-lg px-3 py-2.5 bg-[var(--bg)]">
            <FolderOpen size={16} className="text-[var(--accent)]" />
            <div>
              <p className="text-sm text-[var(--text-strong)]">Agent workspace</p>
              <p className="text-xs text-[var(--muted)] font-mono">
                ~/.see-agent/agents/{agent.id}/
              </p>
            </div>
            <Badge className="ml-auto">read-write</Badge>
          </div>

          {(extraRead.length ?? 0) > 0 && (
            <div className="rounded-lg px-3 py-2.5 bg-[var(--bg)]">
              <p className="text-xs font-medium mb-1 text-[var(--muted)]">Extra read paths:</p>
              {extraRead.map((p: string, i: number) => (
                <p key={i} className="text-xs text-[var(--text-strong)] font-mono">{p}</p>
              ))}
            </div>
          )}

          {(extraWrite.length ?? 0) > 0 && (
            <div className="rounded-lg px-3 py-2.5 bg-[var(--bg)]">
              <p className="text-xs font-medium mb-1 text-[var(--muted)]">Extra write paths:</p>
              {extraWrite.map((p: string, i: number) => (
                <p key={i} className="text-xs text-[var(--text-strong)] font-mono">{p}</p>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
