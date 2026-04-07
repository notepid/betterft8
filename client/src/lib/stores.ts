import { writable } from 'svelte/store'
import type { ServerMessage } from './messages'

export const connected = writable(false)
export const lastMessage = writable<ServerMessage | null>(null)
