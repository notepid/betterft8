<script lang="ts">
  import { radioStatus } from '../lib/stores'
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

  function formatFreq(hz: number): string {
    const mhz = hz / 1_000_000
    const whole = Math.floor(mhz)
    const frac = mhz - whole
    // Format as "14.074 000 MHz"
    const fracStr = frac.toFixed(6).slice(1) // ".074000"
    const [dec3, dec6] = [fracStr.slice(1, 4), fracStr.slice(4, 7)]
    return `${whole}.${dec3} ${dec6} MHz`
  }

  function startEdit() {
    if (!$radioStatus?.connected) return
    editValue = ($radioStatus.freq / 1_000_000).toFixed(6)
    editing = true
  }

  function commitEdit() {
    editing = false
    const hz = Math.round(parseFloat(editValue) * 1_000_000)
    if (hz > 0) {
      client.send({ type: 'set_frequency', freq: hz })
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitEdit()
    else if (e.key === 'Escape') editing = false
  }

  function setBand(freq: number) {
    client.send({ type: 'set_frequency', freq })
  }
</script>

<div class="radio-panel">
  <div class="top-row">
    <div class="freq-block">
      {#if $radioStatus?.connected}
        {#if editing}
          <input
            class="freq-input"
            bind:value={editValue}
            onblur={commitEdit}
            onkeydown={onKeydown}
            autofocus
          />
          <span class="freq-unit">MHz</span>
        {:else}
          <button class="freq-display" onclick={startEdit} title="Click to edit frequency">
            {formatFreq($radioStatus.freq)}
          </button>
        {/if}
        <span class="mode">{$radioStatus.mode}</span>
        {#if $radioStatus.ptt}
          <span class="ptt-indicator" title="Transmitting">TX</span>
        {/if}
      {:else}
        <span class="no-radio">No radio</span>
      {/if}
    </div>
  </div>

  <div class="band-buttons">
    {#each FT8_BANDS as band}
      <button
        class="band-btn"
        onclick={() => setBand(band.freq)}
        disabled={!$radioStatus?.connected}
        title="{band.freq / 1_000_000} MHz"
      >
        {band.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .radio-panel {
    background: #16213e;
    border: 1px solid #2a2a5a;
    border-radius: 4px;
    padding: 0.5rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .top-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .freq-block {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .freq-display {
    background: none;
    border: none;
    cursor: pointer;
    font-family: monospace;
    font-size: 1.4rem;
    font-weight: bold;
    color: #7ec8e3;
    padding: 0;
    letter-spacing: 0.04em;
  }

  .freq-display:hover {
    color: #a8e0f0;
  }

  .freq-input {
    background: #0d0d2b;
    border: 1px solid #7ec8e3;
    border-radius: 3px;
    color: #7ec8e3;
    font-family: monospace;
    font-size: 1.3rem;
    padding: 0.1rem 0.3rem;
    width: 10rem;
  }

  .freq-unit {
    color: #888;
    font-size: 0.85rem;
  }

  .mode {
    font-size: 0.9rem;
    color: #aaa;
    background: #1e1e4a;
    border-radius: 3px;
    padding: 0.1rem 0.4rem;
  }

  .ptt-indicator {
    background: #c0392b;
    color: #fff;
    font-size: 0.75rem;
    font-weight: bold;
    border-radius: 3px;
    padding: 0.15rem 0.4rem;
    animation: blink 0.8s step-end infinite;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .no-radio {
    color: #666;
    font-size: 0.95rem;
    font-style: italic;
  }

  .band-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .band-btn {
    background: #1e2a4a;
    border: 1px solid #2e4a7a;
    border-radius: 3px;
    color: #a0b8d8;
    font-family: monospace;
    font-size: 0.75rem;
    padding: 0.2rem 0.45rem;
    cursor: pointer;
    transition: background 0.1s;
  }

  .band-btn:hover:not(:disabled) {
    background: #2a3e6a;
    color: #c0d8f0;
  }

  .band-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
