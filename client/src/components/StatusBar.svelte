<script lang="ts">
  import { connected, radioStatus, qsoUpdate, settingsOpen } from '../lib/stores'
  import Login from './Login.svelte'

  function formatFreq(hz: number): string {
    const mhz = hz / 1_000_000
    const whole = Math.floor(mhz)
    const frac = mhz - whole
    // Format as "14.074 000"
    const fracStr = frac.toFixed(6).slice(1) // ".074000"
    const [dec3, dec6] = [fracStr.slice(1, 4), fracStr.slice(4, 7)]
    return `${whole}.${dec3} ${dec6}`
  }

  // Single source of truth for the transmit lamp. Order matters: an actual
  // PTT beats a queued frame, which beats the idle RX state.
  $: transmitting = $radioStatus?.ptt ?? false
  $: txQueued = $qsoUpdate?.tx_queued ?? false
  $: lampState = transmitting ? 'tx' : txQueued ? 'queued' : 'rx'
  $: lampText = transmitting ? 'TX' : txQueued ? 'QUEUED' : 'RX'
</script>

<div class="statusbar card">
  <h1>BetterFT8</h1>

  <span
    class="badge"
    class:badge--success={$connected}
    class:badge--danger={!$connected}
    title={$connected ? 'Connected' : 'Disconnected'}
  >
    {$connected ? 'Connected' : 'Disconnected'}
  </span>

  <Login />

  <!-- Frequency + mode readout promoted out of RadioStatus -->
  <div class="freq-readout">
    {#if $radioStatus?.connected}
      <span class="freq u-mono">{formatFreq($radioStatus.freq)}</span>
      <span class="freq-unit">MHz</span>
      <span class="mode">{$radioStatus.mode}</span>
    {:else}
      <span class="no-radio">No radio</span>
    {/if}
  </div>

  <!-- Single source of truth for transmit state -->
  <div
    class="tx-lamp"
    class:tx={lampState === 'tx'}
    class:queued={lampState === 'queued'}
    class:rx={lampState === 'rx'}
    title={lampState === 'tx' ? 'Transmitting' : lampState === 'queued' ? 'Transmission queued' : 'Receiving'}
  >
    {lampText}
  </div>

  <button
    class="btn btn--icon settings-btn"
    title="Settings"
    on:click={() => settingsOpen.update((v) => !v)}
  >
    ⚙
  </button>
</div>

<style>
  .statusbar {
    grid-area: statusbar;
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex-wrap: wrap;
    padding: var(--sp-2) var(--sp-3);
  }

  h1 {
    margin: 0;
    font-size: var(--fs-500);
    color: var(--accent);
    white-space: nowrap;
  }

  .freq-readout {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
  }

  .freq {
    font-size: var(--fs-500);
    font-weight: var(--fw-bold);
    color: var(--accent);
    letter-spacing: 0.04em;
  }

  .freq-unit {
    color: var(--text-muted);
    font-size: var(--fs-100);
  }

  .mode {
    font-size: var(--fs-200);
    color: var(--text-secondary);
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    padding: var(--sp-1) var(--sp-2);
  }

  .no-radio {
    color: var(--text-muted);
    font-size: var(--fs-300);
    font-style: italic;
  }

  /* One large, unmistakable TX/RX lamp — the single transmit indicator. */
  .tx-lamp {
    margin-left: auto;
    min-width: 5.5rem;
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    font-family: var(--font-ui);
    font-weight: var(--fw-bold);
    font-size: var(--fs-500);
    letter-spacing: 0.08em;
    text-align: center;
  }

  .tx-lamp.rx {
    background: var(--success-bg);
    color: var(--success-text);
    border-color: var(--success-border);
  }

  .tx-lamp.queued {
    background: var(--warn-bg);
    color: var(--warn-text);
    border-color: var(--warn-border);
  }

  .tx-lamp.tx {
    background: var(--tx-active-bg);
    color: var(--tx-active);
    border-color: var(--tx-active);
    animation: tx-pulse 0.8s step-end infinite;
  }

  @keyframes tx-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.45; }
  }

  .settings-btn {
    font-size: var(--fs-500);
  }
</style>
