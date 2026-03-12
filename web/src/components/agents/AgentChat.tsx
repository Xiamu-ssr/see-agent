import { useState, useEffect, useCallback, useRef } from 'react'
import { getAgentChat, sendAgentMessage } from '@/api/agents'
import type { ChatMessage } from '@/types'
import { Send, ChevronRight, ChevronDown, Wrench } from 'lucide-react'
import Markdown from 'react-markdown'

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

  // Auto-scroll to bottom on new messages.
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
      {/* Messages */}
      <div className="flex-1 overflow-y-auto space-y-3 mb-4 px-2">
        {messages.length === 0 && (
          <p className="text-sm" style={{ color: 'var(--muted)' }}>No messages yet.</p>
        )}
        {messages.map((m, i) => {
          const isUser = m.role === 'user'
          const toolCalls = (m as any).tool_calls as ToolCall[] | null | undefined

          return (
            <div key={i} className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
              <div
                className="text-sm rounded-[var(--radius)] max-w-[80%]"
                style={{
                  background: isUser ? 'var(--accent-subtle)' : 'var(--bg)',
                  color: 'var(--text)',
                }}
              >
                {/* Text content with markdown */}
                {m.content && (
                  <div className="px-4 py-2.5 prose prose-invert prose-sm max-w-none"
                    style={{ fontSize: '14px', lineHeight: '1.6' }}
                  >
                    <Markdown>{m.content}</Markdown>
                  </div>
                )}

                {/* Tool calls (collapsible) */}
                {toolCalls && toolCalls.length > 0 && (
                  <div
                    className="border-t px-3 py-1.5"
                    style={{ borderColor: 'rgba(255,255,255,0.06)' }}
                  >
                    {toolCalls.map((tc) => {
                      const isExpanded = expandedTools.has(tc.id)
                      return (
                        <div key={tc.id} className="my-1">
                          <button
                            onClick={() => toggleTool(tc.id)}
                            className="flex items-center gap-1.5 text-xs py-0.5 w-full text-left transition-colors hover:opacity-80"
                            style={{ color: '#ff8c5c' }}
                          >
                            {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                            <Wrench size={11} />
                            <span className="font-mono font-medium">{tc.name}</span>
                            {tc.result && !isExpanded && (
                              <span className="ml-1 text-[10px]" style={{ color: '#3fb950' }}>✓</span>
                            )}
                          </button>
                          {isExpanded && (
                            <div
                              className="ml-5 mt-1 rounded text-xs font-mono p-2 overflow-x-auto"
                              style={{ background: 'rgba(0,0,0,0.3)', color: '#8b949e' }}
                            >
                              <div className="mb-1">
                                <span style={{ color: '#7d8590' }}>args: </span>
                                {tc.arguments}
                              </div>
                              {tc.result && (
                                <div className="mt-1 pt-1" style={{ borderTop: '1px solid rgba(255,255,255,0.05)' }}>
                                  <span style={{ color: '#3fb950' }}>result: </span>
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

                {/* Empty assistant with only tool calls — show a subtle label */}
                {!m.content && (!toolCalls || toolCalls.length === 0) && !isUser && (
                  <div className="px-4 py-2 text-xs" style={{ color: 'var(--muted)' }}>
                    (thinking...)
                  </div>
                )}
              </div>
            </div>
          )
        })}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="flex items-center gap-2 border-t pt-3" style={{ borderColor: 'var(--border)' }}>
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleSend()}
          placeholder="Type a message..."
          className="flex-1 rounded-[var(--radius-sm)] border px-3 py-2 text-sm outline-none"
          style={{ background: 'var(--bg)', borderColor: 'var(--border)', color: 'var(--text)' }}
        />
        <label className="flex items-center gap-1 text-xs whitespace-nowrap" style={{ color: 'var(--muted)' }}>
          <input type="checkbox" checked={steer} onChange={(e) => setSteer(e.target.checked)} />
          Steer
        </label>
        <button
          onClick={handleSend}
          className="flex items-center gap-1 rounded-[var(--radius-sm)] px-4 py-2 text-sm font-medium text-white"
          style={{ background: 'var(--accent)' }}
        >
          <Send size={14} /> Send
        </button>
      </div>
    </div>
  )
}
