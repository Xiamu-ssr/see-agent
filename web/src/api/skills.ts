import client from './client'

export interface Skill {
  name: string
  description: string
  available: boolean
}

export async function listSkills(): Promise<Skill[]> {
  const res = await client.get<Skill[]>('/api/skills')
  return res.data
}

export async function installSkill(name: string): Promise<void> {
  await client.post('/api/skills/install', { name })
}
