import type { ClientMessage, CompleteSetupPayload, ServerMessage } from './messages'
import {
  addDecodes,
  alertEnabled,
  authError,
  configUpdateResult,
  connected,
  deviceList,
  hamlibAvailable,
  lastMessage,
  logEntries,
  logFile,
  myCall,
  myGrid,
  myRole,
  needsAuth,
  needsSetup,
  operatorStatus,
  osType,
  qsoUpdate,
  radioStatus,
  rigctldTestResult,
  rigHost,
  rigPort,
  serialPorts,
  waterfallLine,
  wizardOpen,
} from './stores'
import { get } from 'svelte/store'

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
          myCall.set(msg.callsign)
          myGrid.set(msg.grid)
          logFile.set(msg.log_file)
          rigHost.set(msg.rig_host)
          rigPort.set(msg.rig_port)
          needsSetup.set(msg.needs_setup)
          osType.set(msg.os_type)
          hamlibAvailable.set(msg.hamlib_available)
          if (msg.needs_setup) {
            wizardOpen.set(true)
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
          // Check for callsign alert
          const call = get(myCall)
          const alert = get(alertEnabled)
          if (call && alert) {
            const upper = call.toUpperCase()
            if (msg.messages.some((m) => m.message.toUpperCase().includes(upper))) {
              playAlert()
            }
          }
        } else if (msg.type === 'radio_status') {
          radioStatus.set(msg)
        } else if (msg.type === 'qso_update') {
          qsoUpdate.set(msg)
        } else if (msg.type === 'log_entry') {
          logEntries.update((prev) => {
            const next = [msg, ...prev]
            return next.length > 100 ? next.slice(0, 100) : next
          })
        } else if (msg.type === 'device_list') {
          deviceList.set({ inputs: msg.inputs, outputs: msg.outputs })
        } else if (msg.type === 'config_update_result') {
          configUpdateResult.set(msg)
          if (msg.success && !msg.message) {
            // immediate callsign/grid update: refresh Hello values via next connection
            // or we can just trust the stores updated in Settings
          }
        } else if (msg.type === 'rigctld_test_result') {
          rigctldTestResult.set(msg)
        } else if (msg.type === 'serial_port_list') {
          serialPorts.set(msg.ports)
        } else if (msg.type === 'error') {
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

  getSerialPorts() {
    this.send({ type: 'get_serial_ports' })
  }

  completeSetup(payload: CompleteSetupPayload) {
    this.send({ type: 'complete_setup', ...payload })
  }
}

function playAlert() {
  try {
    const ctx = new AudioContext()
    const osc = ctx.createOscillator()
    const gain = ctx.createGain()
    osc.connect(gain)
    gain.connect(ctx.destination)
    osc.frequency.value = 800
    gain.gain.setValueAtTime(0.3, ctx.currentTime)
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3)
    osc.start()
    osc.stop(ctx.currentTime + 0.3)
  } catch {
    // AudioContext may be blocked; ignore
  }
}

export const client = new BetterFT8Client()
