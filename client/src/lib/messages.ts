export type ServerMessage =
  | { type: 'echo'; payload: unknown }
  | { type: 'error'; message: string }

export type ClientMessage =
  | { type: 'ping' }
