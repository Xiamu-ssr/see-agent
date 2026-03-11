import type { AgentDetail } from '@/types'

interface Props {
  agent: AgentDetail
}

export default function AgentTools({ agent }: Props) {
  return (
    <div>
      <p className="text-sm mb-3" style={{ color: 'var(--muted)' }}>Tool configuration for this agent.</p>
      <pre
        className="text-xs overflow-auto p-3 rounded-[var(--radius-sm)] border"
        style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}
      >
        {JSON.stringify(
          { tools_config: agent.tools_config, skills_config: agent.skills_config, mcp_config: agent.mcp_config },
          null,
          2,
        )}
      </pre>
    </div>
  )
}
