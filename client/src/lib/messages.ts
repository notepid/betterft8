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

export type ServerMessage =
  | { type: 'echo'; payload: unknown }
  | { type: 'error'; message: string }
  | WaterfallMessage
  | DecodeMessage
  | RadioStatusMessage

export type ClientMessage =
  | { type: 'ping' }
  | { type: 'set_frequency'; freq: number }
  | { type: 'set_mode'; mode: string; passband: number }
