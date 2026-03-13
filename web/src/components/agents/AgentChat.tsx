import { useState, useEffect, useCallback, useRef } from 'react'
import { getAgentChat, sendAgentMessage } from '@/api/agents'
import type { ChatMessage } from '@/types'
import { Send, ChevronRight, ChevronDown, Wrench } from 'lucide-react'
import Markdown from 'react-markdown'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'

interface Props {
  agentId: string
}

interface ToolCall {
  id: string
  name: string
  arguments: string
  result?: string | null
}

export default function AgentChat({ agentId }: Props) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [steer, setSteer] = useState(false)
  const [expandedTools, setExpandedTools] = useState<Set<string>>(new Set())
  const messagesEndRef = useRef<HTMLDivElement>(null)

  const loadChat = useCallback(async () => {
    const msgs = await getAgentChat(agentId)
    setMessages(msgs)
  }, [agentId])

  useEffect(() => {
    loadChat()
    const interval = setInterval(loadChat, 3000)
    return () => clearInterval(interval)
  }, [loadChat])

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleSend = async () => {
    if (!input.trim()) return
    await sendAgentMessage(agentId, input, steer ? 'steer' : 'normal')
    setInput('')
    loadChat()
  }

  const toggleTool = (toolId: string) => {
    setExpandedTools((prev) => {
      const next = new Set(prev)
      if (next.has(toolId)) next.delete(toolId)
      else next.add(toolId)
      return next
    })
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto space-y-3 mb-4 px-2">
        {messages.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No messages yet.</p>
        )}
        {messages.map((m, i) => {
          const isUser = m.role === 'user'
          const toolCalls = (m as any).tool_calls as ToolCall[] | null | undefined

          return (
            <div key={i} className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
              <div
                className={`text-sm rounded-[var(--radius)] max-w-[80%] ${isUser ? 'bg-[var(--accent-subtle)]' : 'bg-[var(--bg)]'} text-[var(--text)]`}
              >
                {m.content && (
                  <div className="px-4 py-2.5 prose prose-invert prose-sm max-w-none text-[14px] leading-[1.6]">
                    <Markdown>{m.content}</Markdown>
                  </div>
                )}

                {toolCalls && toolCalls.length > 0 && (
                  <div className="border-t border-[rgba(255,255,255,0.06)] px-3 py-1.5">
                    {toolCalls.map((tc) => {
                      const isExpanded = expandedTools.has(tc.id)
                      return (
                        <div key={tc.id} className="my-1">
                          <button
                            onClick={() => toggleTool(tc.id)}
                            className="flex items-center gap-1.5 text-xs py-0.5 w-full text-left transition-colors hover:opacity-80 text-[var(--warn)]"
                          >
                            {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                            <Wrench size={11} />
                            <span className="font-mono font-medium">{tc.name}</span>
                            {tc.result && !isExpanded && (
                              <span className="ml-1 text-[10px] text-[var(--ok)]">✓</span>
                            )}
                          </button>
                          {isExpanded && (
                            <div className="ml-5 mt-1 rounded text-xs font-mono p-2 overflow-x-auto bg-black/30 text-[var(--muted)]">
                              <div className="mb-1">
                                <span className="text-[var(--muted)]">args: </span>
                                {tc.arguments}
                              </div>
                              {tc.result && (
                                <div className="mt-1 pt-1 border-t border-[rgba(255,255,255,0.05)]">
                                  <span className="text-[var(--ok)]">result: </span>
                                  {tc.result}
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      )
                    })}
                  </div>
                )}

                {!m.content && (!toolCalls || toolCalls.length === 0) && !isUser && (
                  <div className="px-4 py-2 text-xs text-[var(--muted)]">
                    (thinking...)
                  </div>
                )}
              </div>
            </div>
          )
        })}
        <div ref={messagesEndRef} />
      </div>

      <div className="flex items-center gap-2 border-t border-[var(--border)] pt-3">
        <Input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.metaKey || e.ctrlKey) && handleSend()}
          placeholder="Type a message..."
          className="flex-1"
        />
        <label className="flex items-center gap-1.5 text-xs whitespace-nowrap text-[var(--muted)]">
          <Switch checked={steer} onCheckedChange={setSteer} />
          Steer
        </label>
        <Button onClick={handleSend} size="sm">
          <Send size={14} /> Send
        </Button>
      </div>
    </div>
  )
}
