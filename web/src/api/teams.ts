import type { Team, TeamDetail, TeamMessage, CreateTeamPayload } from '@/types/team'
import client from './client'

export async function listTeams(): Promise<Team[]> {
  const res = await client.get<Team[]>('/api/teams/')
  return res.data
}

export async function getTeamStatus(id: string): Promise<TeamDetail> {
  const res = await client.get<TeamDetail>(`/api/teams/${id}/status`)
  return res.data
}

export async function createTeam(payload: CreateTeamPayload): Promise<Team> {
  const res = await client.post<Team>('/api/teams/', payload)
  return res.data
}

export async function updateTeam(id: string, payload: Partial<CreateTeamPayload>): Promise<Team> {
  const res = await client.put<Team>(`/api/teams/${id}`, payload)
  return res.data
}

export async function runTeam(id: string, task: string): Promise<{ status: string }> {
  const res = await client.post<{ status: string }>(`/api/teams/${id}/run`, { task })
  return res.data
}

export async function stopTeam(id: string): Promise<{ status: string }> {
  const res = await client.post<{ status: string }>(`/api/teams/${id}/stop`)
  return res.data
}

export async function getMessages(teamId: string, limit?: number): Promise<TeamMessage[]> {
  const res = await client.get<TeamMessage[]>(`/api/teams/${teamId}/messages`, {
    params: limit != null ? { limit } : undefined,
  })
  return res.data
}

export async function sendMessage(teamId: string, to: string, content: string): Promise<TeamMessage> {
  const res = await client.post<TeamMessage>(`/api/teams/${teamId}/message`, { to, content })
  return res.data
}

export async function getUnread(teamId: string): Promise<TeamMessage[]> {
  const res = await client.get<TeamMessage[]>(`/api/teams/${teamId}/unread`)
  return res.data
}

export async function markRead(teamId: string): Promise<{ status: string }> {
  const res = await client.post<{ status: string }>(`/api/teams/${teamId}/mark_read`)
  return res.data
}
