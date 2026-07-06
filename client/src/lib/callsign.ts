/**
 * Extract the caller's callsign from an FT8 CQ message.
 *
 * Standard CQ format is either:
 *   "CQ <call> [grid]"           e.g. "CQ K1ABC FN42"
 *   "CQ <modifier> <call> [grid]" e.g. "CQ DX JA1ABC PM95", "CQ POTA W1AW FN31"
 *
 * A directed-CQ modifier (DX, NA, EU, POTA, TEST, a US state, ...) is a
 * short token that is NOT a callsign, so a naive `words[1]` would return the
 * modifier ("DX") instead of the actual caller. Real callsigns always contain
 * a digit (or a slash for compound calls), while modifiers do not — that is
 * the discriminator used here.
 *
 * For non-CQ messages this returns the first token, which is conventionally
 * the addressed station.
 */
export function callerCall(message: string): string {
  const words = message.trim().split(/\s+/)
  if (words.length === 0 || words[0] === '') return message
  if (words[0].toUpperCase() !== 'CQ') return words[0]

  const first = words[1] ?? ''
  // If the token right after CQ has no digit and no slash it is a
  // direction/modifier (DX, POTA, NA, EU, TEST, ...); the real call follows.
  if (first && !/[0-9/]/.test(first) && words[2]) {
    return words[2]
  }
  return first
}
