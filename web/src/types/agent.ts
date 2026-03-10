export interface Agent {
  id: string
  name: string
  role: string
  team_id: string | null
  team_name: string | null
  status: string
}

export interface AgentDetail extends Agent {
  config_overrides: Record<string, unknown>
  tools_config: Record<string, unknown>
  skills_config: Record<string, unknown>
  mcp_config: Record<string, unknown>
  has_soul: boolean
  location: string
}

export interface CreateAgentPayload {
  id: string
  name: string
  role?: string
  soul?: string
  config_overrides?: Record<string, unknown>
  tools_config?: Record<string, unknown>
  skills_config?: Record<string, unknown>
  mcp_config?: Record<string, unknown>
}
