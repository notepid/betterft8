<script lang="ts">
  import { qsoUpdate, selectedDecode, myRole, txFreq } from '../lib/stores'
  import { client } from '../lib/websocket'
  import type { QsoStateValue } from '../lib/messages'

  $: update = $qsoUpdate
  $: qsoState = update?.state ?? ({ state: 'idle' } as QsoStateValue)
  $: nextTx = update?.next_tx ?? null
  $: txEnabled = update?.tx_enabled ?? false

  // Editable next-TX message (user can modify before it fires)
  let editedNextTx: string | null = null
  $: if (nextTx !== null && editedNextTx === null) {
    editedNextTx = nextTx
  }
  $: if (nextTx === null) {
    editedNextTx = null
  }

  function stateLabel(s: QsoStateValue): string {
    switch (s.state) {
      case 'idle':       return 'Idle'
      case 'calling_cq': return `Calling CQ`
      case 'in_qso':     return `In QSO — ${s.their_call}`
      case 'complete':   return `Complete — ${s.their_call}`
    }
  }

  function stepLabel(s: QsoStateValue): string {
    if (s.state !== 'in_qso') return ''
    switch (s.step) {
      case 'sent_grid':         return 'Sent grid → waiting for report'
      case 'sent_report':       return 'Sent report → waiting for R-report'
      case 'sent_roger_report': return 'Sent R-report → waiting for RR73'
      case 'sent_rr73':         return 'Sent RR73 → waiting for 73'
      case 'sent_73':           return 'Sent 73 — finishing'
    }
  }

  // Step progress indicators
  function stepDone(s: QsoStateValue, check: string): boolean {
    if (s.state !== 'in_qso' && s.state !== 'complete') return false
    const order = ['sent_grid', 'sent_report', 'sent_roger_report', 'sent_rr73', 'sent_73']
    if (s.state === 'complete') return true
    const current = order.indexOf(s.step)
    const target  = order.indexOf(check)
    return current > target
  }

  function stepActive(s: QsoStateValue, check: string): boolean {
    return s.state === 'in_qso' && s.step === check
  }

  // Respond button: visible when selectedDecode is a CQ
  $: isOperator = $myRole === 'operator'
  $: selected = $selectedDecode
  $: canRespond = isOperator
    && selected !== null
    && (selected.message.toUpperCase().startsWith('CQ '))
    && qsoState.state === 'idle'

  function respond() {
    if (!selected) return
    const words = selected.message.split(' ')
    const theirCall = words[1] ?? ''
    client.send({
      type:       'respond_to',
      their_call: theirCall,
      their_freq: selected.freq,
      tx_freq:    $txFreq,
    })
    selectedDecode.set(null)
  }

  function queueEdited() {
    if (editedNextTx) {
      client.send({ type: 'queue_tx', message: editedNextTx, freq: $txFreq })
    }
  }
</script>

<div class="qso-panel">
  <!-- State header -->
  <div class="state-row">
    <span class="state-label" class:active={qsoState.state !== 'idle'}>
      {stateLabel(qsoState)}
    </span>
    {#if qsoState.state === 'in_qso'}
      <span class="step-label">{stepLabel(qsoState)}</span>
    {/if}
    {#if qsoState.state === 'complete'}
      <span class="complete-badge">QSO COMPLETE</span>
    {/if}
  </div>

  <!-- QSO details (visible during InQso / Complete) -->
  {#if qsoState.state === 'in_qso' || qsoState.state === 'complete'}
    <div class="details-row">
      <span class="detail-item">
        <span class="detail-key">Their call:</span>
        <span class="detail-val">{qsoState.their_call}</span>
      </span>
      {#if qsoState.state === 'in_qso' && qsoState.their_grid}
        <span class="detail-item">
          <span class="detail-key">Grid:</span>
          <span class="detail-val">{qsoState.their_grid}</span>
        </span>
      {/if}
      {#if qsoState.state === 'in_qso' && qsoState.their_report !== null}
        <span class="detail-item">
          <span class="detail-key">Their SNR:</span>
          <span class="detail-val">{qsoState.their_report > 0 ? '+' : ''}{qsoState.their_report}</span>
        </span>
      {/if}
      {#if qsoState.state === 'in_qso' && qsoState.my_report !== null}
        <span class="detail-item">
          <span class="detail-key">My SNR:</span>
          <span class="detail-val">{qsoState.my_report > 0 ? '+' : ''}{qsoState.my_report}</span>
        </span>
      {/if}
      {#if qsoState.state === 'complete'}
        {#if qsoState.their_report !== null}
          <span class="detail-item">
            <span class="detail-key">Their SNR:</span>
            <span class="detail-val">{qsoState.their_report > 0 ? '+' : ''}{qsoState.their_report}</span>
          </span>
        {/if}
        {#if qsoState.my_report !== null}
          <span class="detail-item">
            <span class="detail-key">My SNR:</span>
            <span class="detail-val">{qsoState.my_report > 0 ? '+' : ''}{qsoState.my_report}</span>
          </span>
        {/if}
      {/if}
    </div>
  {/if}

  <!-- Progress steps (only while in QSO) -->
  {#if qsoState.state === 'in_qso'}
    <div class="steps-row">
      {#each [
        ['sent_grid',         'Grid'],
        ['sent_report',       'Report'],
        ['sent_roger_report', 'R-Report'],
        ['sent_rr73',         'RR73'],
        ['sent_73',           '73'],
      ] as [key, label]}
        <span
          class="step"
          class:done={stepDone(qsoState, key)}
          class:current={stepActive(qsoState, key)}
        >{label}</span>
      {/each}
    </div>
  {/if}

  <!-- Next TX message (editable by operator only) -->
  {#if nextTx !== null && txEnabled}
    <div class="next-tx-row">
      <span class="next-tx-label">Next TX:</span>
      <input
        class="next-tx-input"
        bind:value={editedNextTx}
        onblur={queueEdited}
        onkeydown={(e) => { if (e.key === 'Enter') queueEdited() }}
        disabled={!isOperator}
        title={isOperator ? 'Edit message before it fires (press Enter to confirm)' : 'Claim operator to edit'}
      />
    </div>
  {/if}

  <!-- Respond button: appears when a CQ is selected in the decode list -->
  {#if canRespond && selected}
    <div class="respond-row">
      <span class="respond-info">
        Respond to <strong>{selected.message.split(' ')[1] ?? ''}</strong>
        @ {Math.round(selected.freq)} Hz?
      </span>
      <button class="btn-respond" onclick={respond}>Respond</button>
      <button class="btn-dismiss" onclick={() => selectedDecode.set(null)}>✕</button>
    </div>
  {/if}
</div>

<style>
  .qso-panel {
    background: #111828;
    border: 1px solid #2a2a5a;
    border-radius: 4px;
    padding: 0.4rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-family: monospace;
    font-size: 0.8rem;
  }

  .state-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .state-label {
    color: #666;
    font-weight: bold;
  }

  .state-label.active {
    color: #7ec8e3;
  }

  .step-label {
    color: #888;
    font-size: 0.75rem;
  }

  .complete-badge {
    background: #27ae60;
    color: #fff;
    font-size: 0.7rem;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    font-weight: bold;
  }

  .details-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    color: #c0c0d0;
  }

  .detail-item {
    display: flex;
    gap: 0.3rem;
  }

  .detail-key {
    color: #666;
  }

  .detail-val {
    color: #a8e0f0;
  }

  .steps-row {
    display: flex;
    gap: 0;
    flex-wrap: nowrap;
  }

  .step {
    padding: 0.15rem 0.5rem;
    border: 1px solid #2a2a5a;
    color: #444;
    font-size: 0.7rem;
    border-radius: 2px;
    margin-right: 0.2rem;
  }

  .step.done {
    background: #1a3a2a;
    color: #60c080;
    border-color: #2a5a3a;
  }

  .step.current {
    background: #1a2a5a;
    color: #80b0ff;
    border-color: #3a5aaa;
    font-weight: bold;
  }

  .next-tx-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .next-tx-label {
    color: #888;
    white-space: nowrap;
  }

  .next-tx-input {
    flex: 1;
    background: #0d0d2b;
    border: 1px solid #f0c040;
    border-radius: 3px;
    color: #f0e080;
    font-family: monospace;
    font-size: 0.8rem;
    padding: 0.15rem 0.35rem;
  }

  .respond-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: #1a2a1a;
    border: 1px solid #2a5a2a;
    border-radius: 3px;
    padding: 0.25rem 0.5rem;
  }

  .respond-info {
    flex: 1;
    color: #90d890;
  }

  .respond-info strong {
    color: #b0f8b0;
  }

  .btn-respond {
    background: #1a5a1a;
    border: 1px solid #2a8a2a;
    color: #80e880;
    font-family: monospace;
    font-size: 0.78rem;
    padding: 0.2rem 0.6rem;
    border-radius: 3px;
    cursor: pointer;
  }

  .btn-respond:hover {
    background: #246a24;
  }

  .btn-dismiss {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0.1rem 0.2rem;
  }

  .btn-dismiss:hover {
    color: #aaa;
  }
</style>
