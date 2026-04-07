import type { ClientMessage, ServerMessage } from './messages'
import { connected, lastMessage } from './stores'

const WS_URL = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`

const BASE_DELAY_MS = 1000
const MAX_DELAY_MS = 30000

class BetterFT8Client {
  private ws: WebSocket | null = null
  private retryDelay = BASE_DELAY_MS
  private retryTimer: ReturnType<typeof setTimeout> | null = null
  private shouldConnect = false

  connect() {
    this.shouldConnect = true
    this.open()
  }

  private open() {
    if (this.ws) return

    const ws = new WebSocket(WS_URL)
    this.ws = ws

    ws.onopen = () => {
      console.log('Connected')
      this.retryDelay = BASE_DELAY_MS
      connected.set(true)
    }

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as ServerMessage
        lastMessage.set(msg)
      } catch (e) {
        console.error('Failed to parse message', e)
      }
    }

    ws.onclose = () => {
      console.log('Disconnected')
      connected.set(false)
      this.ws = null
      if (this.shouldConnect) {
        this.scheduleReconnect()
      }
    }

    ws.onerror = () => {
      ws.close()
    }
  }

  private scheduleReconnect() {
    if (this.retryTimer) return
    console.log(`Reconnecting in ${this.retryDelay}ms`)
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null
      this.retryDelay = Math.min(this.retryDelay * 2, MAX_DELAY_MS)
      this.open()
    }, this.retryDelay)
  }

  send(msg: ClientMessage) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg))
    }
  }
}

export const client = new BetterFT8Client()
