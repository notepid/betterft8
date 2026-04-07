import { writable } from 'svelte/store'
import type { QsoUpdateMessage, RadioStatusMessage, ServerMessage, WaterfallMessage } from './messages'

export const connected = writable(false)
export const lastMessage = writable<ServerMessage | null>(null)
export const waterfallLine = writable<WaterfallMessage | null>(null)
export const radioStatus = writable<RadioStatusMessage | null>(null)
export const qsoUpdate = writable<QsoUpdateMessage | null>(null)

export type Decode = {
  period: number
  snr: number
  dt: number
  freq: number
  message: string
  utcTime: string // HH:MM:SS formatted from period timestamp
}

// Newest decodes at the front; trimmed to 500 entries max.
export const decodes = writable<Decode[]>([])

export function addDecodes(period: number, entries: Array<{ snr: number; dt: number; freq: number; message: string }>) {
  const utcTime = new Date(period * 1000).toISOString().slice(11, 19)
  const newItems: Decode[] = entries.map((e) => ({ period, utcTime, ...e }))
  decodes.update((prev) => {
    const next = [...newItems, ...prev]
    return next.length > 500 ? next.slice(0, 500) : next
  })
}

// Decode that the user has clicked to respond to
export const selectedDecode = writable<Decode | null>(null)
