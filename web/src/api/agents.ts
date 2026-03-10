import type { Agent, AgentDetail, CreateAgentPayload } from '@/types/agent'
import client from './client'

export async function listAgents(): Promise<Agent[]> {
  const res = await client.get<Agent[]>('/api/agents/')
  return res.data
}

export async function getAgent(id: string): Promise<AgentDetail> {
  const res = await client.get<AgentDetail>(`/api/agents/${id}`)
  return res.data
}

export async function createAgent(payload: CreateAgentPayload): Promise<AgentDetail> {
  const res = await client.post<AgentDetail>('/api/agents/', payload)
  return res.data
}

export async function updateAgent(id: string, payload: Partial<CreateAgentPayload>): Promise<AgentDetail> {
  const res = await client.put<AgentDetail>(`/api/agents/${id}`, payload)
  return res.data
}
