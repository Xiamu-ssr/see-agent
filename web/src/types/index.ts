import type { components } from './generated/api'

// Agent types
export type AgentSummary = components['schemas']['AgentSummary']
export type AgentDetail = components['schemas']['AgentDetail']
export type AgentCreateResponse = components['schemas']['AgentCreateResponse']

// Team types
export type TeamSummary = components['schemas']['TeamSummary']
export type TeamStatus = components['schemas']['TeamStatus']
export type TeamCreateResponse = components['schemas']['TeamCreateResponse']
export type TeamUpdateResponse = components['schemas']['TeamUpdateResponse']
export type TeamRunResponse = components['schemas']['TeamRunResponse']
export type TaskItem = components['schemas']['TaskItem']
export type TeamMessage = components['schemas']['TeamMessage']
export type UnreadResponse = components['schemas']['UnreadResponse']
export type MarkReadResponse = components['schemas']['MarkReadResponse']
export type StatusResponse = components['schemas']['StatusResponse']
export type AgentStatusResponse = components['schemas']['AgentStatusResponse']
export type TeamLogEntry = components['schemas']['TeamLogEntry']

// Dashboard / Skills / Logs / Health / Tools / MCP
export type DashboardResponse = components['schemas']['DashboardResponse']
export type SkillInfo = components['schemas']['SkillInfo']
export type SkillInstallResponse = components['schemas']['SkillInstallResponse']
export type LogEntry = components['schemas']['LogEntry']
export type HealthResponse = components['schemas']['HealthResponse']
export type ToolInfo = components['schemas']['ToolInfo']
export type McpInstallResponse = components['schemas']['McpInstallResponse']

// Agent workspace / chat (v3.5)
export type WorkspaceFileItem = components['schemas']['WorkspaceFileItem']
export type WorkspaceFileContent = components['schemas']['WorkspaceFileContent']
export type ChatMessage = components['schemas']['ChatMessage']

// Request types
export type CreateAgentRequest = components['schemas']['CreateAgentRequest']
export type CreateTeamRequest = components['schemas']['CreateTeamRequest']
export type InstallMcpRequest = components['schemas']['InstallMcpRequest']
