import client from './client'

export async function getConfig(): Promise<Record<string, unknown>> {
  const res = await client.get<Record<string, unknown>>('/api/config')
  return res.data
}

export async function updateConfig(config: Record<string, unknown>): Promise<Record<string, unknown>> {
  const res = await client.put<Record<string, unknown>>('/api/config', { config })
  return res.data
}

export async function getSchema(type: string): Promise<Record<string, unknown>> {
  const res = await client.get<Record<string, unknown>>(`/api/schemas/${type}`)
  return res.data
}
