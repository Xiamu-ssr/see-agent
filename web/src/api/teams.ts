import type { TeamSummary, TeamStatus, TeamMessage, TeamCreateResponse, TeamUpdateResponse, TeamRunResponse, StatusResponse, UnreadResponse, MarkReadResponse, CreateTeamRequest } from '@/types'
import client from './client'

export async function listTeams(): Promise<TeamSummary[]> {
  const res = await client.get<TeamSummary[]>('/api/teams/')
  return res.data
}

export async function getTeamStatus(id: string): Promise<TeamStatus> {
  const res = await client.get<TeamStatus>(`/api/teams/${id}/status`)
  return res.data
}

export async function createTeam(payload: CreateTeamRequest): Promise<TeamCreateResponse> {
  const res = await client.post<TeamCreateResponse>('/api/teams/', payload)
  return res.data
}

export async function updateTeam(id: string, payload: Partial<CreateTeamRequest>): Promise<TeamUpdateResponse> {
  const res = await client.put<TeamUpdateResponse>(`/api/teams/${id}`, payload)
  return res.data
}

export async function runTeam(id: string, task: string): Promise<TeamRunResponse> {
  const res = await client.post<TeamRunResponse>(`/api/teams/${id}/run`, { task })
  return res.data
}

export async function stopTeam(id: string): Promise<StatusResponse> {
  const res = await client.post<StatusResponse>(`/api/teams/${id}/stop`)
  return res.data
}

export async function getMessages(teamId: string, limit?: number): Promise<TeamMessage[]> {
  const res = await client.get<TeamMessage[]>(`/api/teams/${teamId}/messages`, {
    params: limit != null ? { limit } : undefined,
  })
  return res.data
}

export async function sendMessage(teamId: string, to: string, content: string): Promise<StatusResponse> {
  const res = await client.post<StatusResponse>(`/api/teams/${teamId}/message`, { to, content })
  return res.data
}

export async function getUnread(teamId: string): Promise<UnreadResponse> {
  const res = await client.get<UnreadResponse>(`/api/teams/${teamId}/unread`)
  return res.data
}

export async function markRead(teamId: string): Promise<MarkReadResponse> {
  const res = await client.post<MarkReadResponse>(`/api/teams/${teamId}/mark_read`)
  return res.data
}
