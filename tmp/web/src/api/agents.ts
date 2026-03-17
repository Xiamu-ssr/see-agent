import type {
  AgentSummary,
  AgentDetail,
  AgentCreateResponse,
  CreateAgentRequest,
  StatusResponse,
  WorkspaceFileItem,
  WorkspaceFileContent,
  ChatMessage,
} from '@/types'
import client from './client'

export async function listAgents(): Promise<AgentSummary[]> {
  const res = await client.get<AgentSummary[]>('/api/agents')
  return res.data
}

export async function getAgent(id: string): Promise<AgentDetail> {
  const res = await client.get<AgentDetail>(`/api/agents/${id}`)
  return res.data
}

export async function createAgent(payload: CreateAgentRequest): Promise<AgentCreateResponse> {
  const res = await client.post<AgentCreateResponse>('/api/agents', payload)
  return res.data
}

export async function updateAgent(
  id: string,
  payload: Partial<CreateAgentRequest>,
): Promise<AgentCreateResponse> {
  const res = await client.put<AgentCreateResponse>(`/api/agents/${id}`, payload)
  return res.data
}

// v3.5: Agent messaging
export async function sendAgentMessage(
  id: string,
  content: string,
  priority: string = 'normal',
): Promise<StatusResponse> {
  const res = await client.post<StatusResponse>(`/api/agents/${id}/message`, {
    content,
    priority,
  })
  return res.data
}

export async function getAgentChat(id: string): Promise<ChatMessage[]> {
  const res = await client.get<ChatMessage[]>(`/api/agents/${id}/chat`)
  return res.data
}

// v3.5: Agent lifecycle
export async function startAgent(id: string): Promise<StatusResponse> {
  const res = await client.post<StatusResponse>(`/api/agents/${id}/start`)
  return res.data
}

export async function stopAgent(id: string): Promise<StatusResponse> {
  const res = await client.post<StatusResponse>(`/api/agents/${id}/stop`)
  return res.data
}

// v3.5: Workspace files
export async function getWorkspaceFiles(id: string): Promise<WorkspaceFileItem[]> {
  const res = await client.get<WorkspaceFileItem[]>(`/api/agents/${id}/workspace`)
  return res.data
}

export async function getWorkspaceFile(
  id: string,
  filename: string,
): Promise<WorkspaceFileContent> {
  const res = await client.get<WorkspaceFileContent>(
    `/api/agents/${id}/workspace/${filename}`,
  )
  return res.data
}

export async function updateWorkspaceFile(
  id: string,
  filename: string,
  content: string,
): Promise<StatusResponse> {
  const res = await client.put<StatusResponse>(
    `/api/agents/${id}/workspace/${filename}`,
    { content },
  )
  return res.data
}
