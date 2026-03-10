export interface Team {
  id: string
  name: string
  members: string[]
  status: string
}

export interface TeamDetail extends Team {
  leader: string | null
  tasks: TaskItem[]
}

export interface TaskItem {
  id: string
  title: string
  status: string
  assigned_to: string | null
}

export interface TeamMessage {
  sender: string
  recipient: string
  content: string
  ts: string
}

export interface CreateTeamPayload {
  name: string
  members: string[]
  leader?: string
}
