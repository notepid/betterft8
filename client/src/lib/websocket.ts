import type { ClientMessage, CompleteSetupPayload, ServerMessage } from './messages'
import {
  addDecodes,
  alertEnabled,
  authError,
  commandError,
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
// Once the session is streaming (past auth/setup), the server pushes waterfall
// lines ~10x/sec. If none arrive within this window the connection is treated
// as half-open and force-closed so the reconnect logic can recover it. The
// watchdog is NOT armed while awaiting viewer auth or first-run setup, when the
// server is legitimately silent until the user acts.
const WATCHDOG_TIMEOUT_MS = 15000
const WATCHDOG_INTERVAL_MS = 5000
// A connection that stays open this long is considered healthy enough to reset
// the reconnect backoff — long enough that a crash-looping server (which dies
// shortly after the handshake) never resets it.
const STABLE_AFTER_MS = 3000

class BetterFT8Client {
  private ws: WebSocket | null = null
  private retryDelay = BASE_DELAY_MS
  private retryTimer: ReturnType<typeof setTimeout> | null = null
  private shouldConnect = false
  private lastMessageAt = 0
  private streaming = false
  private watchdogTimer: ReturnType<typeof setInterval> | null = null
  private cmdErrorTimer: ReturnType<typeof setTimeout> | null = null
  private stableTimer: ReturnType<typeof setTimeout> | null = null

  connect() {
    this.shouldConnect = true
    this.open()
  }

  private open() {
    if (this.ws) return

    const ws = new WebSocket(WS_URL)
    this.ws = ws
    this.streaming = false

    ws.onopen = () => {
      console.log('Connected')
      connected.set(true)
      commandError.set(null)
      this.lastMessageAt = Date.now()
      // Do NOT reset the backoff on open or on the first message — the server
      // always sends `hello` first, so a crash-looping server would reset it.
      // Reset only after the connection has survived a few seconds.
      this.stableTimer = setTimeout(() => {
        this.retryDelay = BASE_DELAY_MS
      }, STABLE_AFTER_MS)
    }

    ws.onmessage = (event) => {
      this.lastMessageAt = Date.now()
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
          // Open viewing (no auth, no setup): the server streams immediately, so
          // arm the liveness watchdog. Otherwise stay unarmed until the user
          // authenticates — the server is silent until then.
          if (!msg.needs_viewer_auth && !msg.needs_setup) {
            this.startWatchdog()
          }
        } else if (msg.type === 'auth_result') {
          if (msg.success) {
            needsAuth.set(false)
            myRole.set('viewer')
            authError.set(null)
            // Authenticated — the server now streams, so arm the watchdog.
            this.startWatchdog()
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
      this.stopWatchdog()
      if (this.stableTimer) {
        clearTimeout(this.stableTimer)
        this.stableTimer = null
      }
      connected.set(false)
      myRole.set('unauthenticated')
      needsAuth.set(false)
      operatorStatus.set(null)
      // Clear live radio/QSO state so the UI does not keep showing a frozen
      // (but plausible) transmitter/QSO status during the outage.
      qsoUpdate.set(null)
      radioStatus.set(null)
      this.ws = null
      if (this.shouldConnect) {
        this.scheduleReconnect()
      }
    }

    ws.onerror = () => {
      ws.close()
    }
  }

  private startWatchdog() {
    // Idempotent: arming again (e.g. auth after an open-viewing hello) just
    // refreshes the baseline rather than stacking intervals.
    this.streaming = true
    this.lastMessageAt = Date.now()
    if (this.watchdogTimer) return
    this.watchdogTimer = setInterval(() => {
      if (this.streaming && Date.now() - this.lastMessageAt > WATCHDOG_TIMEOUT_MS) {
        console.warn('Watchdog: no inbound traffic — forcing reconnect')
        // close() triggers onclose, which schedules the reconnect.
        this.ws?.close()
      }
    }, WATCHDOG_INTERVAL_MS)
  }

  private stopWatchdog() {
    this.streaming = false
    if (this.watchdogTimer) {
      clearInterval(this.watchdogTimer)
      this.watchdogTimer = null
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

  send(msg: ClientMessage): boolean {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg))
      // A successful send clears any stale "command not sent" notice.
      if (this.cmdErrorTimer) {
        clearTimeout(this.cmdErrorTimer)
        this.cmdErrorTimer = null
      }
      commandError.set(null)
      return true
    }
    // Socket is down — surface the failure instead of silently dropping, and
    // auto-dismiss it so it doesn't stay pinned after the link recovers.
    commandError.set('Not connected — command not sent')
    if (this.cmdErrorTimer) clearTimeout(this.cmdErrorTimer)
    this.cmdErrorTimer = setTimeout(() => {
      commandError.set(null)
      this.cmdErrorTimer = null
    }, 4000)
    return false
  }

  getSerialPorts() {
    this.send({ type: 'get_serial_ports' })
  }

  completeSetup(payload: CompleteSetupPayload) {
    this.send({ type: 'complete_setup', ...payload })
  }
}

// Single reused AudioContext — creating one per alert leaks contexts and hits
// Chrome's ~6-context cap.
let alertCtx: AudioContext | null = null

function playAlert() {
  try {
    if (!alertCtx) alertCtx = new AudioContext()
    const ctx = alertCtx
    if (ctx.state === 'suspended') void ctx.resume()
    const osc = ctx.createOscillator()
    const gain = ctx.createGain()
    osc.connect(gain)
    gain.connect(ctx.destination)
    osc.frequency.value = 800
    gain.gain.setValueAtTime(0.3, ctx.currentTime)
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3)
    osc.start()
    osc.stop(ctx.currentTime + 0.3)
    // Release the nodes once the beep finishes so they don't accumulate.
    osc.onended = () => {
      osc.disconnect()
      gain.disconnect()
    }
  } catch {
    // AudioContext may be blocked; ignore
  }
}

export const client = new BetterFT8Client()
