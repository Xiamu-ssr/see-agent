import type { ToolInfo } from '@/types'
import client from './client'

export async function listTools(): Promise<ToolInfo[]> {
  const res = await client.get<ToolInfo[]>('/api/tools')
  return res.data
}
