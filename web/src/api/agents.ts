import type { AgentSummary, AgentDetail, AgentCreateResponse, CreateAgentRequest } from '@/types'
import client from './client'

export async function listAgents(): Promise<AgentSummary[]> {
  const res = await client.get<AgentSummary[]>('/api/agents/')
  return res.data
}

export async function getAgent(id: string): Promise<AgentDetail> {
  const res = await client.get<AgentDetail>(`/api/agents/${id}`)
  return res.data
}

export async function createAgent(payload: CreateAgentRequest): Promise<AgentCreateResponse> {
  const res = await client.post<AgentCreateResponse>('/api/agents/', payload)
  return res.data
}

export async function updateAgent(id: string, payload: Partial<CreateAgentRequest>): Promise<AgentCreateResponse> {
  const res = await client.put<AgentCreateResponse>(`/api/agents/${id}`, payload)
  return res.data
}
