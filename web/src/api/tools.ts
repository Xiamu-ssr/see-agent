import client from './client'

export interface Tool {
  name: string
  description: string
  [key: string]: unknown
}

export async function listTools(): Promise<Tool[]> {
  const res = await client.get<Tool[]>('/api/tools')
  return res.data
}
