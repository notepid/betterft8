<script lang="ts">
  import { decodes, myCall, selectedDecode, type Decode } from '../lib/stores'
  import { callerCall } from '../lib/callsign'

  function isCq(d: Decode): boolean {
    const upper = d.message.toUpperCase()
    return upper.startsWith('CQ ') || upper === 'CQ'
  }

  function rowClass(d: Decode): string {
    const upper = d.message.toUpperCase()
    if ($myCall && upper.includes($myCall.toUpperCase())) return 'row-mycall'
    if (isCq(d)) return 'row-cq'
    return ''
  }

  // Accessible name so a screen reader announces what activating the row does.
  function ariaLabel(d: Decode): string {
    const verb = isCq(d) ? `Reply to ${callerCall(d.message)}` : 'Select'
    return `${verb}, "${d.message}", ${Math.round(d.freq)} hertz, SNR ${d.snr}`
  }

  function handleClick(d: Decode) {
    selectedDecode.set(d)
  }

  function handleKeydown(e: KeyboardEvent, d: Decode) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      handleClick(d)
    }
  }

  // Announce (once) the newest decode that mentions the operator's own call, so
  // a screen-reader user is alerted without narrating every line.
  let lastAnnouncedId = -1
  let announcement = ''
  $: if ($myCall) {
    const call = $myCall.toUpperCase()
    const mine = $decodes.find((d) => d.message.toUpperCase().includes(call))
    if (mine && mine.id !== lastAnnouncedId) {
      lastAnnouncedId = mine.id
      announcement = `Heard your call: ${mine.message} at ${Math.round(mine.freq)} hertz`
    }
  }
</script>

<div class="sr-only" aria-live="polite" aria-atomic="true">{announcement}</div>

<div class="decode-list">
  <table>
    <thead>
      <tr>
        <th>UTC</th>
        <th>dB</th>
        <th>DT</th>
        <th>Freq</th>
        <th>Message</th>
      </tr>
    </thead>
    <tbody>
      {#each $decodes as d (d.id)}
        <tr
          class={rowClass(d)}
          class:selected={$selectedDecode?.period === d.period && $selectedDecode?.freq === d.freq && $selectedDecode?.message === d.message}
          role="button"
          tabindex="0"
          aria-label={ariaLabel(d)}
          on:click={() => handleClick(d)}
          on:keydown={(e) => handleKeydown(e, d)}
        >
          <td class="col-utc">{d.utcTime}</td>
          <td class="col-snr">{d.snr > 0 ? '+' : ''}{d.snr}</td>
          <td class="col-dt">{d.dt >= 0 ? '+' : ''}{d.dt.toFixed(1)}</td>
          <td class="col-freq">{Math.round(d.freq)}</td>
          <td class="col-msg">
            {#if isCq(d)}<span class="cq-tag" aria-hidden="true">▸</span>{/if}{d.message}</td>
        </tr>
      {/each}
      {#if $decodes.length === 0}
        <tr class="row-empty">
          <td colspan="5">No decodes yet — waiting for next FT8 period…</td>
        </tr>
      {/if}
    </tbody>
  </table>
</div>

<style>
  .decode-list {
    height: 100%;
    min-height: 0;
    overflow-y: auto;
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-200);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  thead tr {
    position: sticky;
    top: 0;
    background: var(--surface-2);
    z-index: 1;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--surface-2);
    padding: var(--sp-1) var(--sp-2);
    text-align: left;
    color: var(--text-muted);
    font-family: var(--font-ui);
    font-weight: var(--fw-regular);
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  td {
    padding: 0.2rem var(--sp-2);
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  tr {
    cursor: pointer;
    color: var(--text-secondary);
  }

  tr:hover {
    background: var(--surface-3);
  }

  tr.selected {
    background: var(--surface-2);
    outline: 1px solid var(--accent);
  }

  .row-cq {
    color: var(--cq);
  }

  /* Non-colour cue for CQ rows so colour-blind users can distinguish them. */
  .cq-tag {
    color: var(--cq);
    font-weight: var(--fw-bold);
    margin-right: var(--sp-1);
  }

  /* Visually hidden but available to assistive tech (aria-live announcements). */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  tr[role="button"]:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .row-mycall {
    color: var(--mycall);
    font-weight: var(--fw-bold);
    background: var(--surface-2);
  }

  .row-empty td {
    text-align: center;
    color: var(--text-muted);
    padding: var(--sp-4);
    cursor: default;
  }

  .col-utc  { color: var(--text-muted); min-width: 5.5rem; }
  .col-snr  { text-align: right; min-width: 3rem; }
  .col-dt   { text-align: right; min-width: 3.5rem; }
  .col-freq { text-align: right; min-width: 3.5rem; }
  .col-msg  { width: 100%; }
</style>
