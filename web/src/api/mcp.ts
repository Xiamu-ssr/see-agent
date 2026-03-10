import type { InstallMcpRequest } from '@/types'
import client from './client'

export async function installMcp(payload: InstallMcpRequest): Promise<void> {
  await client.post('/api/mcp/install', payload)
}

export async function deleteMcp(name: string): Promise<void> {
  // Remove from config by sending an update with the server removed
  const res = await client.get<Record<string, unknown>>('/api/config')
  const config = res.data
  const servers = (config.mcp_servers || {}) as Record<string, unknown>
  delete servers[name]
  await client.put('/api/config', { config: { mcp_servers: servers } })
}
