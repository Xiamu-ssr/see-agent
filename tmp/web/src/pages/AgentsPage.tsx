import { useCallback, useState, useEffect } from 'react'
import { useNavigate, useParams, useLocation } from 'react-router-dom'
import { usePolling } from '@/hooks/usePolling'
import { listAgents, createAgent, getAgent } from '@/api/agents'
import type { AgentSummary, AgentDetail, CreateAgentRequest } from '@/types'
import { Bot, ArrowLeft } from 'lucide-react'
import AgentList from '@/components/agents/AgentList'
import AgentOverview from '@/components/agents/AgentOverview'
import AgentFiles from '@/components/agents/AgentFiles'
import AgentTools from '@/components/agents/AgentTools'
import AgentSkills from '@/components/agents/AgentSkills'
import AgentSafehouse from '@/components/agents/AgentSafehouse'
import AgentChat from '@/components/agents/AgentChat'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'

export default function AgentsPage() {
  const navigate = useNavigate()
  const { id } = useParams<{ id: string }>()
  const location = useLocation()
  const isChat = location.pathname.endsWith('/chat')

  const fetchAgents = useCallback(() => listAgents(), [])
  const { data: agents, refresh } = usePolling<AgentSummary[]>(fetchAgents, 10000)

  const [showCreate, setShowCreate] = useState(false)
  const [form, setForm] = useState<CreateAgentRequest>({})
  const [agent, setAgent] = useState<AgentDetail | null>(null)

  useEffect(() => {
    if (!id) {
      setAgent(null)
      return
    }
    getAgent(id)
      .then(setAgent)
      .catch(() => setAgent(null))
  }, [id])

  const handleCreate = async () => {
    if (!form.name) return
    await createAgent(form)
    setShowCreate(false)
    setForm({})
    refresh()
  }

  return (
    <div className="flex h-full">
      {/* Agent list — hidden on mobile when viewing detail */}
      <div className={`${id ? 'hidden md:flex' : 'flex'} flex-col h-full md:w-[200px] w-full shrink-0 md:border-r border-[var(--border)]`}>
        <AgentList
          agents={agents ?? undefined}
          selectedId={id}
          onSelect={(agentId) => navigate(`/agents/${agentId}/chat`)}
          onNewAgent={() => setShowCreate(true)}
        />
      </div>

      {/* Detail panel — hidden on mobile when no agent selected */}
      <div className={`${!id ? 'hidden md:flex' : 'flex'} flex-1 flex-col min-h-0 min-w-0 px-3 py-3 md:px-4 md:py-4`}>
        {!id ? (
          <div className="flex items-center justify-center flex-1">
            <div className="text-center">
              <Bot size={48} className="text-[var(--muted)] mx-auto mb-3" />
              <p className="text-sm text-[var(--muted)]">Select an agent</p>
            </div>
          </div>
        ) : !agent ? (
          <div className="text-[var(--muted)]">Loading...</div>
        ) : (
          <>
            {/* Agent header */}
            <div className="flex items-center gap-3 mb-3 shrink-0">
              {/* Mobile back button */}
              <button
                onClick={() => navigate('/agents')}
                className="md:hidden p-1 -ml-1 rounded-lg hover:bg-[var(--bg-hover)] transition-colors"
              >
                <ArrowLeft size={18} style={{ color: 'var(--muted)' }} />
              </button>

              <div className="flex rounded-full p-0.5 shrink-0 bg-[var(--bg-elevated)] border border-[var(--border)]">
                <Button
                  onClick={() => navigate(`/agents/${id}`)}
                  variant={!isChat ? 'default' : 'ghost'}
                  size="sm"
                  className="rounded-full"
                >
                  Details
                </Button>
                <Button
                  onClick={() => navigate(`/agents/${id}/chat`)}
                  variant={isChat ? 'default' : 'ghost'}
                  size="sm"
                  className="rounded-full"
                >
                  Chat
                </Button>
              </div>

              <h1 className="text-lg font-semibold text-[var(--text-strong)] truncate">
                {agent.id}
              </h1>
              <Badge variant={agent.status === 'busy' ? 'success' : 'secondary'}>
                <span className="h-1.5 w-1.5 rounded-full bg-current" />
                {agent.status === 'busy' ? 'Running' : 'Idle'}
              </Badge>
            </div>

            {isChat ? (
              <Card className="flex-1 min-h-0 p-3 md:p-4 flex flex-col">
                <AgentChat agentId={id} />
              </Card>
            ) : (
              <Tabs defaultValue="overview" className="flex-1 flex flex-col min-h-0">
                <TabsList className="shrink-0 overflow-x-auto">
                  <TabsTrigger value="overview">Overview</TabsTrigger>
                  <TabsTrigger value="files">Files</TabsTrigger>
                  <TabsTrigger value="tools">Tools</TabsTrigger>
                  <TabsTrigger value="skills">Skills</TabsTrigger>
                  <TabsTrigger value="safehouse">Safehouse</TabsTrigger>
                </TabsList>
                <TabsContent value="overview" className="overflow-y-auto">
                  <Card className="p-4"><AgentOverview agent={agent} /></Card>
                </TabsContent>
                <TabsContent value="files">
                  <Card className="h-full min-h-0 overflow-hidden">
                    <AgentFiles agentId={id} />
                  </Card>
                </TabsContent>
                <TabsContent value="tools" className="overflow-y-auto">
                  <Card className="p-4"><AgentTools agent={agent} /></Card>
                </TabsContent>
                <TabsContent value="skills" className="overflow-y-auto">
                  <Card className="p-4"><AgentSkills agent={agent} /></Card>
                </TabsContent>
                <TabsContent value="safehouse" className="overflow-y-auto">
                  <Card className="p-4"><AgentSafehouse agent={agent} /></Card>
                </TabsContent>
              </Tabs>
            )}
          </>
        )}
      </div>

      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Agent</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <Input
              placeholder="Name"
              value={form.name || ''}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
            <Input
              placeholder="Emoji (e.g. 🤖)"
              value={form.emoji || ''}
              onChange={(e) => setForm({ ...form, emoji: e.target.value })}
            />
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowCreate(false)}>
              Cancel
            </Button>
            <Button onClick={handleCreate}>
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
