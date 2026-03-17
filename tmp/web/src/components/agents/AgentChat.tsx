import { useState, useEffect, useCallback, useRef } from 'react'
import { getAgentChat, sendAgentMessage } from '@/api/agents'
import type { ChatMessage } from '@/types'
import { Send, ChevronRight, ChevronDown, Wrench, ArrowDown } from 'lucide-react'
import Markdown from 'react-markdown'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
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

function formatTime(ts?: string | null): string {
  if (!ts) return ''
  try {
    const d = new Date(ts)
    return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  } catch {
    return ''
  }
}

export default function AgentChat({ agentId }: Props) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [steer, setSteer] = useState(false)
  const [expandedTools, setExpandedTools] = useState<Set<string>>(new Set())
  const [isNearBottom, setIsNearBottom] = useState(true)
  const scrollRef = useRef<HTMLDivElement>(null)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const loadChat = useCallback(async () => {
    const msgs = await getAgentChat(agentId)
    setMessages(msgs)
  }, [agentId])

  useEffect(() => {
    loadChat()
    const interval = setInterval(loadChat, 3000)
    return () => clearInterval(interval)
  }, [loadChat])

  // Only auto-scroll if user is near the bottom.
  useEffect(() => {
    if (isNearBottom) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages, isNearBottom])

  // Track scroll position to determine if near bottom.
  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return
    const threshold = 80
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < threshold
    setIsNearBottom(atBottom)
  }

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    setIsNearBottom(true)
  }

  const handleSend = async () => {
    if (!input.trim()) return
    await sendAgentMessage(agentId, input, steer ? 'steer' : 'normal')
    setInput('')
    setIsNearBottom(true)
    loadChat()
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
    }
  }

  const handleTextareaInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value)
    // Auto-resize textarea
    const el = e.target
    el.style.height = 'auto'
    const maxHeight = 6 * 24 // ~6 lines
    el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      handleSend()
    }
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
      {/* Messages area */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto space-y-2 mb-3 px-1 md:px-2 relative"
      >
        {messages.length === 0 && (
          <p className="text-sm text-[var(--muted)]">No messages yet.</p>
        )}
        {messages.map((m, i) => {
          const isUser = m.role === 'user'
          const toolCalls = (m as any).tool_calls as ToolCall[] | null | undefined
          const sender = (m as any).sender as string | null | undefined
          const priority = (m as any).priority as string | null | undefined
          const isSteer = priority === 'steer'
          const time = formatTime(m.timestamp)

          return (
            <div key={i} className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
              <div
                className={`text-sm rounded-[var(--radius)] max-w-[85%] md:max-w-[80%] ${
                  isSteer
                    ? 'bg-[rgba(255,92,92,0.12)] border border-[rgba(255,92,92,0.3)]'
                    : isUser
                      ? 'bg-[var(--accent-subtle)]'
                      : 'bg-[var(--bg)]'
                } text-[var(--text)]`}
              >
                {/* Sender + time header */}
                <div className="flex items-center gap-2 px-3 md:px-4 pt-2 pb-0">
                  {sender && (
                    <span className="text-[11px] font-medium" style={{
                      color: isUser ? 'var(--accent)' : '#58a6ff',
                    }}>
                      {sender}
                    </span>
                  )}
                  {isSteer && (
                    <span className="text-[9px] px-1.5 py-0.5 rounded-full font-medium"
                      style={{ background: 'rgba(255,92,92,0.2)', color: '#ff5c5c' }}>
                      STEER
                    </span>
                  )}
                  {time && (
                    <span className="text-[10px] text-[var(--muted)] ml-auto">{time}</span>
                  )}
                </div>

                {/* Text content with markdown */}
                {m.content && (
                  <div className="px-3 md:px-4 py-2 prose prose-invert prose-sm max-w-none text-[14px] leading-[1.6]">
                    <Markdown>{m.content}</Markdown>
                  </div>
                )}

                {/* Tool calls (collapsible) */}
                {toolCalls && toolCalls.length > 0 && (
                  <div className="border-t border-[rgba(255,255,255,0.06)] px-3 py-1.5">
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
                              <span className="ml-1 text-[10px] text-[var(--ok)]">✓</span>
                            )}
                          </button>
                          {isExpanded && (
                            <div className="ml-5 mt-1 rounded text-xs font-mono p-2 overflow-x-auto bg-black/30 text-[var(--muted)]">
                              <div className="mb-1">
                                <span style={{ color: '#7d8590' }}>args: </span>
                                <span style={{ color: '#c9d1d9' }}>
                                  {tc.arguments || '(none)'}
                                </span>
                              </div>
                              {tc.result && (
                                <div className="mt-1 pt-1 border-t border-[rgba(255,255,255,0.05)]">
                                  <span className="text-[var(--ok)]">result: </span>
                                  <span style={{ color: '#c9d1d9' }}>
                                    {tc.result}
                                  </span>
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      )
                    })}
                  </div>
                )}

                {/* Empty assistant — thinking */}
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

      {/* Scroll-to-bottom button */}
      {!isNearBottom && (
        <div className="flex justify-center -mt-2 mb-2">
          <button
            onClick={scrollToBottom}
            className="rounded-full p-1.5 transition-colors hover:bg-[var(--accent-subtle)]"
            style={{ background: 'var(--card)', border: '1px solid var(--border)' }}
          >
            <ArrowDown size={14} style={{ color: 'var(--muted)' }} />
          </button>
        </div>
      )}

      {/* Input area — textarea with controls */}
      <div className="border-t border-[var(--border)] pt-3">
        <div className="relative">
          <Textarea
            ref={textareaRef}
            value={input}
            onChange={handleTextareaInput}
            onKeyDown={handleKeyDown}
            placeholder="Type a message... (⌘+Enter to send)"
            className="min-h-[52px] max-h-[144px] resize-none pr-28 text-sm"
            rows={2}
          />
          <div className="absolute bottom-2 right-2 flex items-center gap-2">
            <label className="flex items-center gap-1.5 text-xs whitespace-nowrap text-[var(--muted)]">
              <Switch checked={steer} onCheckedChange={setSteer} />
              Steer
            </label>
            <Button onClick={handleSend} size="sm" className="h-7 px-2.5">
              <Send size={14} />
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
