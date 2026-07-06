import type { ClientMessage, CompleteSetupPayload, ServerMessage } from './messages'
import {
  addDecodes,
  alertEnabled,
  authError,
  commandError,
  configUpdateResult,
  connected,
  connectionState,
  dataStale,
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
  notify,
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

  // ---- Operator relock (survives reconnects) --------------------------------
  // Cached ONLY in memory — never localStorage/sessionStorage. `operatorPassword`
  // holds the credential of a claim that has succeeded; `wantOperator` records
  // that we intend to hold the lock so we can re-claim after a reconnect.
  private operatorPassword: string | null = null
  private wantOperator = false
  // Password of a claim_operator we have sent but not yet had confirmed.
  private pendingOperatorPassword: string | null = null
  // A claim_operator is in flight awaiting its operator_status / error reply.
  private claimPending = false
  // The in-flight claim is an automatic reconnect re-claim (drives success/lost
  // toasts and distinguishes it from a manual claim in the login widget).
  private reclaimInFlight = false
  // Guard: at most one automatic re-claim per connection, so a repeatedly
  // failing claim can never loop.
  private reclaimAttempted = false

  connect() {
    this.shouldConnect = true
    this.open()
  }

  private open() {
    if (this.ws) return

    const ws = new WebSocket(WS_URL)
    this.ws = ws
    this.streaming = false
    // Fresh socket: void any claim that was in flight on the previous one and
    // re-arm the single-shot auto-reclaim for this connection. Note we KEEP
    // `operatorPassword` / `wantOperator` — that intent is what survives.
    this.pendingOperatorPassword = null
    this.claimPending = false
    this.reclaimInFlight = false
    this.reclaimAttempted = false

    ws.onopen = () => {
      console.log('Connected')
      connected.set(true)
      connectionState.set('connected')
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
            this.maybeReclaimOperator()
          }
        } else if (msg.type === 'auth_result') {
          if (msg.success) {
            needsAuth.set(false)
            myRole.set('viewer')
            authError.set(null)
            // Authenticated — the server now streams, so arm the watchdog.
            this.startWatchdog()
            // If we held (or were seeking) the operator lock before the drop,
            // reclaim it now that we are authenticated again.
            this.maybeReclaimOperator()
          } else {
            // messages.ts AuthResultMessage carries no reason field today; prefer
            // one if the server ever adds it, else fall back to the generic text.
            const reason = (msg as { reason?: string }).reason
            authError.set(reason ?? 'Wrong password')
          }
        } else if (msg.type === 'operator_status') {
          operatorStatus.set(msg)
          myRole.update((current) => {
            if (msg.you_are_operator) return 'operator'
            if (current === 'operator') return 'viewer'
            return current
          })
          if (msg.you_are_operator && this.claimPending) {
            // This confirms a claim we initiated. Commit the credential + intent
            // so we can restore the lock across future reconnects.
            if (this.pendingOperatorPassword !== null) {
              this.operatorPassword = this.pendingOperatorPassword
            }
            this.wantOperator = true
            this.claimPending = false
            this.pendingOperatorPassword = null
            if (this.reclaimInFlight) {
              this.reclaimInFlight = false
              notify('success', 'Operator lock restored')
            }
          }
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
          if (this.reclaimInFlight) {
            // Our automatic re-claim was rejected (e.g. operator password
            // changed) — we are a viewer now. Clear intent so we don't loop.
            this.reclaimInFlight = false
            this.claimPending = false
            this.wantOperator = false
            this.operatorPassword = null
            this.pendingOperatorPassword = null
            notify('error', 'Operator lock lost — you are now a viewer')
          } else if (this.claimPending) {
            // A manual operator claim failed — keep it inline in the login form.
            this.claimPending = false
            this.pendingOperatorPassword = null
            authError.set(msg.message)
          } else {
            // Generic server error → transient toast, not the auth widget.
            notify('error', msg.message)
          }
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
      // Keep the last-known radio/QSO snapshot on screen but flag it as stale so
      // the UI can DIM (rather than blank) it during the outage. It is refreshed
      // and un-dimmed once the server resends live state after reconnect.
      dataStale.set(true)
      this.ws = null
      if (this.shouldConnect) {
        this.scheduleReconnect()
      } else {
        // Intentional stop / not reconnecting.
        connectionState.set('disconnected')
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
    // Streaming has (re)started — the server is pushing live state again, so the
    // on-screen snapshot is no longer stale.
    dataStale.set(false)
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

  private maybeReclaimOperator() {
    // At most one automatic re-claim per connection (loop protection) and only
    // if we actually hold cached operator intent + credential.
    if (this.reclaimAttempted) return
    if (!this.wantOperator || this.operatorPassword === null) return
    this.reclaimAttempted = true
    this.reclaimInFlight = true
    // send() caches pendingOperatorPassword + sets claimPending.
    this.send({ type: 'claim_operator', password: this.operatorPassword })
  }

  private scheduleReconnect() {
    connectionState.set('reconnecting')
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
      // Track operator-lock intent as it goes out on the wire.
      if (msg.type === 'claim_operator') {
        this.pendingOperatorPassword = msg.password
        this.claimPending = true
      } else if (msg.type === 'release_operator') {
        // Explicit release drops the cached credential + intent so we do NOT
        // silently re-grab the lock on the next reconnect.
        this.wantOperator = false
        this.operatorPassword = null
        this.pendingOperatorPassword = null
        this.claimPending = false
        this.reclaimInFlight = false
      }
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
