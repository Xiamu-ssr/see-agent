import { useEffect, useRef, useCallback, useState } from 'react'

export function useWebSocket(url: string | null) {
  const wsRef = useRef<WebSocket | null>(null)
  const [lastMessage, setLastMessage] = useState<unknown>(null)
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    if (!url) return

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}${url}`
    const ws = new WebSocket(wsUrl)
    wsRef.current = ws

    ws.onopen = () => setConnected(true)
    ws.onclose = () => setConnected(false)
    ws.onmessage = (e) => {
      try {
        setLastMessage(JSON.parse(e.data))
      } catch {
        setLastMessage(e.data)
      }
    }

    return () => {
      ws.close()
      setConnected(false)
    }
  }, [url])

  const send = useCallback((data: unknown) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(data))
    }
  }, [])

  return { lastMessage, connected, send }
}
