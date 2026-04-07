<script lang="ts">
  import { radioStatus, qsoUpdate } from '../lib/stores'
  import { client } from '../lib/websocket'

  // TX frequency (audio Hz within passband)
  let txFreq = 1000

  $: txEnabled = $qsoUpdate?.tx_enabled ?? false
  $: transmitting = $radioStatus?.ptt ?? false
  $: txQueued = $qsoUpdate?.tx_queued ?? false

  // Parity: false = even (0,30s), true = odd (15,45s)
  let parityOdd = false

  function toggleTx() {
    const next = !txEnabled
    client.send({ type: 'enable_tx', enabled: next })
  }

  function callCq() {
    client.send({ type: 'enable_tx', enabled: true })
    client.send({ type: 'call_cq', freq: txFreq })
  }

  function haltTx() {
    client.send({ type: 'halt_tx' })
  }

  function resetQso() {
    client.send({ type: 'reset_qso' })
  }

  function setParity(odd: boolean) {
    parityOdd = odd
    client.send({ type: 'set_tx_parity', parity: odd ? 1 : 0 })
  }
</script>

<div class="controls">
  <!-- TX Enable toggle -->
  <label class="toggle-label" title="Enable/disable automatic TX">
    <input type="checkbox" checked={txEnabled} onchange={toggleTx} />
    <span class="toggle-text">TX {txEnabled ? 'ON' : 'OFF'}</span>
  </label>

  <!-- TX Frequency -->
  <label class="freq-label">
    <span class="label-text">TX Hz</span>
    <input
      class="tx-freq-input"
      type="number"
      min="200"
      max="2700"
      step="10"
      bind:value={txFreq}
    />
  </label>

  <!-- Period selector -->
  <div class="parity-group" title="Select TX period (even=0,30s / odd=15,45s past minute)">
    <button
      class="parity-btn"
      class:active={!parityOdd}
      onclick={() => setParity(false)}
    >Even</button>
    <button
      class="parity-btn"
      class:active={parityOdd}
      onclick={() => setParity(true)}
    >Odd</button>
  </div>

  <!-- Call CQ -->
  <button
    class="btn btn-cq"
    onclick={callCq}
    disabled={transmitting}
    title="Call CQ on TX frequency"
  >Call CQ</button>

  <!-- Halt TX — always available -->
  <button
    class="btn btn-halt"
    onclick={haltTx}
    title="Emergency stop TX"
  >Halt TX</button>

  <!-- Reset QSO -->
  <button
    class="btn btn-reset"
    onclick={resetQso}
    title="Clear QSO state and stop TX"
  >Reset</button>

  <!-- TX status badge -->
  {#if transmitting}
    <span class="tx-badge">TX</span>
  {:else if txQueued}
    <span class="queued-badge">QUEUED</span>
  {/if}
</div>

<style>
  .controls {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem;
    background: #16213e;
    border: 1px solid #2a2a5a;
    border-radius: 4px;
    padding: 0.4rem 0.75rem;
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    cursor: pointer;
    user-select: none;
  }

  .toggle-label input[type="checkbox"] {
    accent-color: #27ae60;
    width: 1rem;
    height: 1rem;
    cursor: pointer;
  }

  .toggle-text {
    font-size: 0.8rem;
    font-weight: bold;
    color: #a0d8c0;
    min-width: 4.5rem;
  }

  .freq-label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .label-text {
    font-size: 0.75rem;
    color: #888;
  }

  .tx-freq-input {
    background: #0d0d2b;
    border: 1px solid #3a3a6a;
    border-radius: 3px;
    color: #c8d8f0;
    font-family: monospace;
    font-size: 0.85rem;
    padding: 0.15rem 0.35rem;
    width: 5.5rem;
  }

  .parity-group {
    display: flex;
    gap: 0;
    border: 1px solid #3a3a6a;
    border-radius: 3px;
    overflow: hidden;
  }

  .parity-btn {
    background: #1e2a4a;
    border: none;
    color: #7888aa;
    font-family: monospace;
    font-size: 0.75rem;
    padding: 0.2rem 0.55rem;
    cursor: pointer;
    transition: background 0.1s;
  }

  .parity-btn:first-child {
    border-right: 1px solid #3a3a6a;
  }

  .parity-btn.active {
    background: #2a4a8a;
    color: #b0d0ff;
  }

  .parity-btn:hover:not(.active) {
    background: #253a6a;
  }

  .btn {
    border: none;
    border-radius: 3px;
    font-family: monospace;
    font-size: 0.8rem;
    padding: 0.25rem 0.65rem;
    cursor: pointer;
    transition: background 0.1s, opacity 0.1s;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .btn-cq {
    background: #1a5a3a;
    color: #80e8b0;
    border: 1px solid #2a8a5a;
  }

  .btn-cq:hover:not(:disabled) {
    background: #246a46;
  }

  .btn-halt {
    background: #5a1a1a;
    color: #f08080;
    border: 1px solid #8a2a2a;
    font-weight: bold;
  }

  .btn-halt:hover {
    background: #6a2020;
  }

  .btn-reset {
    background: #2a2a4a;
    color: #8888aa;
    border: 1px solid #3a3a6a;
  }

  .btn-reset:hover {
    background: #3a3a5a;
  }

  .tx-badge {
    background: #c0392b;
    color: #fff;
    font-size: 0.7rem;
    font-weight: bold;
    border-radius: 3px;
    padding: 0.1rem 0.4rem;
    animation: blink 0.8s step-end infinite;
  }

  .queued-badge {
    background: #7d6608;
    color: #ffe;
    font-size: 0.7rem;
    border-radius: 3px;
    padding: 0.1rem 0.4rem;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }
</style>
