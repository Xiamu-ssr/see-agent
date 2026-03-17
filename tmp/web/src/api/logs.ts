import type { LogEntry } from '@/types'
import client from './client'

export interface LogParams {
  date?: string
  level?: string
  limit?: number
  offset?: number
}

export async function getLogs(params: LogParams = {}): Promise<LogEntry[]> {
  const res = await client.get<LogEntry[]>('/api/logs', { params })
  return res.data
}
