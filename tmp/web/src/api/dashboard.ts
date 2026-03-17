import type { DashboardResponse } from '@/types'
import client from './client'

export async function getDashboard(): Promise<DashboardResponse> {
  const res = await client.get<DashboardResponse>('/api/dashboard')
  return res.data
}
