<script lang="ts">
  import { radioStatus, qsoUpdate, myRole, txFreq, connected, commandError } from '../lib/stores'
  import { client } from '../lib/websocket'

  $: txEnabled = $qsoUpdate?.tx_enabled ?? false
  $: transmitting = $radioStatus?.ptt ?? false
  $: txQueued = $qsoUpdate?.tx_queued ?? false
  $: isOperator = $myRole === 'operator'
  // TX-affecting controls require an operator AND a live connection; a command
  // sent over a dead socket is silently dropped, so disable rather than mislead.
  $: canControl = isOperator && $connected

  // Parity: false = even (0,30s), true = odd (15,45s)
  let parityOdd = false

  function toggleTx() {
    const next = !txEnabled
    client.send({ type: 'enable_tx', enabled: next })
  }

  function callCq() {
    client.send({ type: 'enable_tx', enabled: true })
    client.send({ type: 'call_cq', freq: $txFreq })
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
  <label class="toggle-label" title={isOperator ? 'Enable/disable automatic TX' : 'Claim operator to control TX'}>
    <input type="checkbox" checked={txEnabled} onchange={toggleTx} disabled={!canControl} />
    <span class="toggle-text">TX {txEnabled ? 'ON' : 'OFF'}</span>
  </label>

  <!-- TX Frequency -->
  <label class="freq-label">
    <span class="label-text">TX Hz</span>
    <input
      class="input tx-freq-input u-mono"
      type="number"
      min="200"
      max="3000"
      step="10"
      bind:value={$txFreq}
      disabled={!isOperator}
    />
  </label>

  <!-- Period selector -->
  <div class="parity-group" title="Select TX period (even=0,30s / odd=15,45s past minute)">
    <button
      class="btn btn--ghost parity-btn"
      class:active={!parityOdd}
      onclick={() => setParity(false)}
      disabled={!canControl}
    >Even</button>
    <button
      class="btn btn--ghost parity-btn"
      class:active={parityOdd}
      onclick={() => setParity(true)}
      disabled={!canControl}
    >Odd</button>
  </div>

  <!-- Call CQ -->
  <button
    class="btn btn--primary"
    onclick={callCq}
    disabled={transmitting || !canControl}
    title={!$connected ? 'Disconnected — cannot transmit' : isOperator ? 'Call CQ on TX frequency' : 'Claim operator to control TX'}
  >Call CQ</button>

  <!-- Halt TX -->
  <button
    class="btn btn--danger"
    onclick={haltTx}
    disabled={!canControl}
    title={!$connected ? 'Disconnected — cannot transmit' : isOperator ? 'Emergency stop TX' : 'Claim operator to control TX'}
  >Halt TX</button>

  <!-- Reset QSO -->
  <button
    class="btn btn--ghost"
    onclick={resetQso}
    disabled={!canControl}
    title={!$connected ? 'Disconnected — cannot transmit' : isOperator ? 'Clear QSO state and stop TX' : 'Claim operator to control TX'}
  >Reset</button>

  <!-- TX status badge -->
  {#if transmitting}
    <span class="badge badge--tx tx-badge">TX</span>
  {:else if txQueued}
    <span class="badge badge--warn">QUEUED</span>
  {/if}

  <!-- Command send failure (e.g. socket down) -->
  {#if $commandError}
    <span class="cmd-error">{$commandError}</span>
  {/if}
</div>

<style>
  .controls {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-2);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: var(--sp-2) var(--sp-3);
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    cursor: pointer;
    user-select: none;
  }

  .toggle-label input[type="checkbox"] {
    accent-color: var(--success);
    width: 1rem;
    height: 1rem;
    cursor: pointer;
  }

  .toggle-text {
    font-size: var(--fs-200);
    font-weight: var(--fw-bold);
    color: var(--success-text);
    min-width: 4.5rem;
  }

  .freq-label {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .label-text {
    font-size: var(--fs-100);
    color: var(--text-muted);
  }

  .tx-freq-input {
    width: 5.5rem;
  }

  .parity-group {
    display: flex;
    gap: var(--sp-1);
  }

  .parity-btn {
    font-size: var(--fs-100);
  }

  .parity-btn.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .tx-badge {
    animation: blink 0.8s step-end infinite;
  }

  .cmd-error {
    color: var(--danger-text);
    font-size: var(--fs-100);
    font-weight: var(--fw-bold);
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }
</style>
