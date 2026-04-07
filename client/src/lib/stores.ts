import { writable } from 'svelte/store'
import type { ServerMessage, WaterfallMessage } from './messages'

export const connected = writable(false)
export const lastMessage = writable<ServerMessage | null>(null)
export const waterfallLine = writable<WaterfallMessage | null>(null)
