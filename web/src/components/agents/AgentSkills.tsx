import type { AgentDetail } from '@/types'

interface Props {
  agent: AgentDetail
}

export default function AgentSkills({ agent }: Props) {
  const skills = agent.skills_config || {}
  return (
    <div>
      <p className="text-sm mb-3" style={{ color: 'var(--muted)' }}>Skills configuration for this agent.</p>
      <pre
        className="text-xs overflow-auto p-3 rounded-[var(--radius-sm)] border"
        style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)', fontFamily: 'var(--mono)' }}
      >
        {JSON.stringify(skills, null, 2)}
      </pre>
    </div>
  )
}
