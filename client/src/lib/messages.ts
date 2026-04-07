export type WaterfallMessage = {
  type: 'waterfall'
  timestamp: number
  freq_min: number
  freq_max: number
  data: string // base64
}

export type DecodedEntry = {
  snr: number
  dt: number
  freq: number
  message: string
}

export type DecodeMessage = {
  type: 'decode'
  period: number
  messages: DecodedEntry[]
}

export type RadioStatusMessage = {
  type: 'radio_status'
  connected: boolean
  freq: number
  mode: string
  ptt: boolean
}

export type QsoStateValue =
  | { state: 'idle' }
  | { state: 'calling_cq'; my_call: string; my_grid: string; tx_freq: number }
  | {
      state: 'in_qso'
      their_call: string
      their_grid: string | null
      their_report: number | null
      my_report: number | null
      step: 'sent_grid' | 'sent_report' | 'sent_roger_report' | 'sent_rr73' | 'sent_73'
      tx_freq: number
    }
  | { state: 'complete'; their_call: string; their_report: number | null; my_report: number | null }

export type QsoUpdateMessage = {
  type: 'qso_update'
  state: QsoStateValue
  next_tx: string | null
  tx_enabled: boolean
  tx_queued: boolean
}

export type HelloMessage = {
  type: 'hello'
  needs_viewer_auth: boolean
}

export type AuthResultMessage = {
  type: 'auth_result'
  success: boolean
}

export type OperatorStatusMessage = {
  type: 'operator_status'
  operator_client_id: string | null
  you_are_operator: boolean
  client_count: number
}

export type ServerMessage =
  | { type: 'echo'; payload: unknown }
  | { type: 'error'; message: string }
  | HelloMessage
  | AuthResultMessage
  | OperatorStatusMessage
  | WaterfallMessage
  | DecodeMessage
  | RadioStatusMessage
  | QsoUpdateMessage

export type ClientMessage =
  | { type: 'ping' }
  | { type: 'auth'; password: string }
  | { type: 'claim_operator'; password: string }
  | { type: 'release_operator' }
  | { type: 'set_frequency'; freq: number }
  | { type: 'set_mode'; mode: string; passband: number }
  | { type: 'call_cq'; freq: number }
  | { type: 'respond_to'; their_call: string; their_freq: number; tx_freq: number }
  | { type: 'queue_tx'; message: string; freq: number }
  | { type: 'halt_tx' }
  | { type: 'enable_tx'; enabled: boolean }
  | { type: 'set_tx_parity'; parity: number }
  | { type: 'reset_qso' }
