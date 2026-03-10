import type { SkillInfo } from '@/types'
import client from './client'

export async function listSkills(): Promise<SkillInfo[]> {
  const res = await client.get<SkillInfo[]>('/api/skills')
  return res.data
}

export async function installSkill(name: string): Promise<void> {
  await client.post('/api/skills/install', { name })
}
