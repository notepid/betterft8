<script lang="ts">
  import { radioStatus, myRole } from '../lib/stores'
  import { client } from '../lib/websocket'

  const FT8_BANDS = [
    { label: '160m', freq: 1840000 },
    { label: '80m',  freq: 3573000 },
    { label: '40m',  freq: 7074000 },
    { label: '30m',  freq: 10136000 },
    { label: '20m',  freq: 14074000 },
    { label: '17m',  freq: 18100000 },
    { label: '15m',  freq: 21074000 },
    { label: '12m',  freq: 24915000 },
    { label: '10m',  freq: 28074000 },
    { label: '6m',   freq: 50313000 },
  ]

  let editing = false
  let editValue = ''
  // Set when Escape is pressed so the ensuing onblur->commitEdit cancels
  // instead of committing the typed value.
  let cancelling = false

  // Widest FT8 band segment span (Hz) used to decide which band button is
  // "current" — bands are spaced far wider apart than this, so no overlap.
  const BAND_TOLERANCE_HZ = 500_000

  function formatFreq(hz: number): string {
    const mhz = hz / 1_000_000
    const whole = Math.floor(mhz)
    const frac = mhz - whole
    // Format as "14.074 000 MHz"
    const fracStr = frac.toFixed(6).slice(1) // ".074000"
    const [dec3, dec6] = [fracStr.slice(1, 4), fracStr.slice(4, 7)]
    return `${whole}.${dec3} ${dec6} MHz`
  }

  $: isOperator = $myRole === 'operator'

  function startEdit() {
    if (!$radioStatus?.connected || !isOperator) return
    editValue = ($radioStatus.freq / 1_000_000).toFixed(6)
    editing = true
  }

  function commitEdit() {
    // Enter commits then unmounts the input, whose onblur calls commitEdit
    // again; this guard makes the trailing call a no-op so we send only once.
    if (!editing) return
    editing = false
    // Escape flagged a cancel; swallow the commit without sending.
    if (cancelling) {
      cancelling = false
      return
    }
    const hz = Math.round(parseFloat(editValue) * 1_000_000)
    if (hz > 0) {
      client.send({ type: 'set_frequency', freq: hz })
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitEdit()
    else if (e.key === 'Escape') {
      cancelling = true
      commitEdit()
    }
  }

  function setBand(freq: number) {
    client.send({ type: 'set_frequency', freq })
  }

  // A band button is "current" when the radio's tuned frequency sits within a
  // band's FT8 segment tolerance.
  $: currentFreq = $radioStatus?.freq ?? 0
  function isCurrentBand(bandFreq: number): boolean {
    return !!$radioStatus?.connected && Math.abs(currentFreq - bandFreq) < BAND_TOLERANCE_HZ
  }
</script>

<div class="radio-panel">
  <h2 class="panel-title">Tuning</h2>

  <div class="top-row">
    <div class="freq-block">
      {#if $radioStatus?.connected}
        {#if editing}
          <input
            class="input freq-input u-mono"
            bind:value={editValue}
            onblur={commitEdit}
            onkeydown={onKeydown}
            autofocus
          />
          <span class="freq-unit">MHz</span>
        {:else}
          <button class="freq-display u-mono" onclick={startEdit} title={isOperator ? 'Click to edit frequency' : 'Claim operator to change frequency'} class:locked={!isOperator}>
            {formatFreq($radioStatus.freq)}
          </button>
        {/if}
      {:else}
        <span class="no-radio">No radio</span>
      {/if}
    </div>
  </div>

  <div class="band-buttons">
    {#each FT8_BANDS as band}
      <button
        class="btn btn--ghost band-btn"
        class:active={isCurrentBand(band.freq)}
        onclick={() => setBand(band.freq)}
        disabled={!$radioStatus?.connected || !isOperator}
        title={isOperator ? `${band.freq / 1_000_000} MHz` : 'Claim operator to change band'}
      >
        {band.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .radio-panel {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: var(--sp-2) var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .panel-title {
    font-size: var(--fs-100);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin: 0;
  }

  .top-row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .freq-block {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .freq-display {
    background: none;
    border: none;
    cursor: pointer;
    font-size: var(--fs-500);
    font-weight: var(--fw-bold);
    color: var(--accent);
    padding: 0;
    letter-spacing: 0.04em;
  }

  .freq-display:hover {
    color: var(--accent-hover);
  }

  .freq-display.locked {
    cursor: default;
    opacity: 0.7;
  }

  .freq-display.locked:hover {
    color: var(--accent);
  }

  .freq-input {
    color: var(--accent);
    border-color: var(--accent);
    font-size: var(--fs-500);
    width: 10rem;
  }

  .freq-unit {
    color: var(--text-muted);
    font-size: var(--fs-200);
  }

  .no-radio {
    color: var(--text-muted);
    font-size: var(--fs-300);
    font-style: italic;
  }

  .band-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }

  .band-btn {
    font-size: var(--fs-100);
  }

  .band-btn.active {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
