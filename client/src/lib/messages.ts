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

export type ServerMessage =
  | { type: 'echo'; payload: unknown }
  | { type: 'error'; message: string }
  | WaterfallMessage
  | DecodeMessage

export type ClientMessage =
  | { type: 'ping' }
