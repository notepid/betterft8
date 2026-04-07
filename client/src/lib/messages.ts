export type WaterfallMessage = {
  type: 'waterfall'
  timestamp: number
  freq_min: number
  freq_max: number
  data: string // base64
}

export type ServerMessage =
  | { type: 'echo'; payload: unknown }
  | { type: 'error'; message: string }
  | WaterfallMessage

export type ClientMessage =
  | { type: 'ping' }
