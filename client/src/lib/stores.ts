import { writable } from 'svelte/store'
import type {
  LogEntryMessage,
  OperatorStatusMessage,
  QsoUpdateMessage,
  RadioStatusMessage,
  RigctldTestResultMessage,
  ServerMessage,
  WaterfallMessage,
} from './messages'

export const connected = writable(false)
export const lastMessage = writable<ServerMessage | null>(null)
export const waterfallLine = writable<WaterfallMessage | null>(null)
export const radioStatus = writable<RadioStatusMessage | null>(null)
export const qsoUpdate = writable<QsoUpdateMessage | null>(null)

export type Role = 'unauthenticated' | 'viewer' | 'operator'
export const myRole = writable<Role>('unauthenticated')
export const operatorStatus = writable<OperatorStatusMessage | null>(null)
export const needsAuth = writable(false)
/** Last auth-related error message (viewer password or operator claim failure). */
export const authError = writable<string | null>(null)

/** Station callsign received from server Hello. */
export const myCall = writable<string>('')
/** Station grid received from server Hello. */
export const myGrid = writable<string>('')
/** Log file path received from server Hello. */
export const logFile = writable<string>('ft8.adi')

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

// ---- Settings ---------------------------------------------------------------

/** Whether the Settings panel is open. */
export const settingsOpen = writable<boolean>(false)

/** Alert sound enabled (when callsign is heard). */
export const alertEnabled = writable<boolean>(true)

/** Waterfall color scheme. */
export const waterfallScheme = writable<'classic' | 'greyscale' | 'heat'>('classic')

/** Waterfall display floor in dB (-120 to 0). Values below this → black. */
export const waterfallFloor = writable<number>(-120)

/** Waterfall display ceiling in dB (-120 to 0). Values above this → full color. */
export const waterfallCeiling = writable<number>(0)

/** Available audio devices from server. */
export const deviceList = writable<{ inputs: string[]; outputs: string[] }>({ inputs: [], outputs: [] })

/** Recent QSO log entries. */
export const logEntries = writable<LogEntryMessage[]>([])

/** Result of the last rigctld test. */
export const rigctldTestResult = writable<RigctldTestResultMessage | null>(null)

/** Result of the last config update (shown in Settings). */
export const configUpdateResult = writable<{ success: boolean; message: string | null; requires_restart: boolean } | null>(null)

/** rigctld connection info from Hello (pre-populate Settings form). */
export const rigHost = writable<string>('localhost')
export const rigPort = writable<number>(4532)

// ---- Setup wizard -----------------------------------------------------------

/** True when server reports no config file exists. */
export const needsSetup = writable<boolean>(false)
/** OS type string from server ("windows" | "linux" | "raspberry_pi" | "macos"). */
export const osType = writable<string>('')
/** Whether the Hamlib direct backend feature was compiled in. */
export const hamlibAvailable = writable<boolean>(false)
/** Serial ports returned by GetSerialPorts. */
export const serialPorts = writable<string[]>([])
/** Controls whether the setup wizard overlay is shown. */
export const wizardOpen = writable<boolean>(false)
