import client from './client'

export interface DashboardData {
  teams_count: number
  teams_by_status: Record<string, number>
  agents_in_team: number
  agents_idle: number
  total_tasks: number
  tasks_by_status: Record<string, number>
}

export async function getDashboard(): Promise<DashboardData> {
  const res = await client.get<DashboardData>('/api/dashboard')
  return res.data
}
