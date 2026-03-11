import { useState, useEffect, useCallback } from 'react'
import { getAgentChat, sendAgentMessage } from '@/api/agents'
import type { ChatMessage } from '@/types'
import { Send } from 'lucide-react'

interface Props {
  agentId: string
}

export default function AgentChat({ agentId }: Props) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [steer, setSteer] = useState(false)

  const loadChat = useCallback(async () => {
    const msgs = await getAgentChat(agentId)
    setMessages(msgs)
  }, [agentId])

  useEffect(() => {
    loadChat()
    const interval = setInterval(loadChat, 5000)
    return () => clearInterval(interval)
  }, [loadChat])

  const handleSend = async () => {
    if (!input.trim()) return
    await sendAgentMessage(agentId, input, steer ? 'steer' : 'normal')
    setInput('')
    loadChat()
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
          return (
            <div
              key={i}
              className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}
            >
              <div
                className="text-sm px-4 py-2.5 rounded-[var(--radius)] max-w-[75%]"
                style={{
                  background: isUser ? 'var(--accent-subtle)' : 'var(--bg)',
                  color: 'var(--text)',
                }}
              >
                {!isUser && (
                  <span className="text-xs font-medium mr-2" style={{ color: 'var(--muted)' }}>
                    [{m.role}]
                  </span>
                )}
                {m.content || '(no content)'}
              </div>
            </div>
          )
        })}
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
