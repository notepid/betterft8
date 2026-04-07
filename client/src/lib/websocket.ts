import type { ClientMessage, ServerMessage } from './messages'
import { connected, lastMessage, waterfallLine, addDecodes, radioStatus, qsoUpdate, myRole, operatorStatus, needsAuth, authError } from './stores'

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

        if (msg.type === 'hello') {
          if (msg.needs_viewer_auth) {
            needsAuth.set(true)
          } else {
            myRole.set('viewer')
          }
        } else if (msg.type === 'auth_result') {
          if (msg.success) {
            needsAuth.set(false)
            myRole.set('viewer')
            authError.set(null)
          } else {
            authError.set('Wrong password')
          }
        } else if (msg.type === 'operator_status') {
          operatorStatus.set(msg)
          myRole.update((current) => {
            if (msg.you_are_operator) return 'operator'
            if (current === 'operator') return 'viewer'
            return current
          })
        } else if (msg.type === 'waterfall') {
          waterfallLine.set(msg)
        } else if (msg.type === 'decode') {
          addDecodes(msg.period, msg.messages)
        } else if (msg.type === 'radio_status') {
          radioStatus.set(msg)
        } else if (msg.type === 'qso_update') {
          qsoUpdate.set(msg)
        } else if (msg.type === 'error') {
          // Surface server errors for auth UI
          authError.set(msg.message)
          lastMessage.set(msg)
        } else {
          lastMessage.set(msg)
        }
      } catch (e) {
        console.error('Failed to parse message', e)
      }
    }

    ws.onclose = () => {
      console.log('Disconnected')
      connected.set(false)
      myRole.set('unauthenticated')
      needsAuth.set(false)
      operatorStatus.set(null)
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
