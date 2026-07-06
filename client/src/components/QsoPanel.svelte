<script lang="ts">
  import { qsoUpdate, selectedDecode, myRole, txFreq } from '../lib/stores'
  import { client } from '../lib/websocket'
  import { callerCall } from '../lib/callsign'
  import type { QsoStateValue } from '../lib/messages'

  $: update = $qsoUpdate
  $: qsoState = update?.state ?? ({ state: 'idle' } as QsoStateValue)
  $: nextTx = update?.next_tx ?? null
  $: txEnabled = update?.tx_enabled ?? false

  // Editable next-TX message (user can modify before it fires).
  // Keep it in sync with the server's next_tx unless the operator is actively
  // editing the field, so a server-advanced message never leaves stale text.
  let editedNextTx: string | null = null
  let editingNextTx = false
  $: if (!editingNextTx) {
    editedNextTx = nextTx
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
    const theirCall = callerCall(selected.message)
    client.send({
      type:       'respond_to',
      their_call: theirCall,
      their_freq: selected.freq,
      tx_freq:    $txFreq,
    })
    selectedDecode.set(null)
  }

  function queueEdited() {
    editingNextTx = false
    const msg = editedNextTx?.trim()
    // Only queue if the operator actually changed the message; blurring the
    // field without edits must not re-send a message the server has superseded.
    if (msg && msg !== nextTx) {
      client.send({ type: 'queue_tx', message: msg, freq: $txFreq })
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
      <span class="badge badge--success">QSO COMPLETE</span>
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
        onfocus={() => { editingNextTx = true }}
        onblur={queueEdited}
        onkeydown={(e) => { if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur() }}
        disabled={!isOperator}
        title={isOperator ? 'Edit message before it fires (press Enter to confirm)' : 'Claim operator to edit'}
      />
    </div>
  {/if}

  <!-- Respond button: appears when a CQ is selected in the decode list -->
  {#if canRespond && selected}
    <div class="respond-row">
      <span class="respond-info">
        Respond to <strong>{callerCall(selected.message)}</strong>
        @ {Math.round(selected.freq)} Hz?
      </span>
      <button class="btn btn--primary" onclick={respond}>Respond</button>
      <button class="btn btn--icon" onclick={() => selectedDecode.set(null)}>✕</button>
    </div>
  {/if}
</div>

<style>
  .qso-panel {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: var(--sp-2) var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    font-size: var(--fs-200);
  }

  .state-row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .state-label {
    color: var(--text-muted);
    font-weight: var(--fw-bold);
  }

  .state-label.active {
    color: var(--accent);
  }

  .step-label {
    color: var(--text-muted);
    font-size: var(--fs-100);
  }

  .details-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
    color: var(--text-secondary);
  }

  .detail-item {
    display: flex;
    gap: var(--sp-1);
  }

  .detail-key {
    color: var(--text-muted);
  }

  .detail-val {
    color: var(--accent);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .steps-row {
    display: flex;
    gap: 0;
    flex-wrap: nowrap;
  }

  .step {
    padding: 0.15rem var(--sp-2);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-size: var(--fs-100);
    border-radius: var(--radius-sm);
    margin-right: var(--sp-1);
  }

  /* Non-colour cues so step progress survives colour-blindness. */
  .step.done::before {
    content: "\2713 ";
  }

  .step.current::before {
    content: "\25CF ";
  }

  .step.done {
    background: var(--success-bg);
    color: var(--success-text);
    border-color: var(--success-border);
  }

  .step.current {
    background: var(--surface-2);
    color: var(--accent);
    border-color: var(--accent);
    font-weight: var(--fw-bold);
  }

  .next-tx-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .next-tx-label {
    color: var(--text-muted);
    white-space: nowrap;
  }

  .next-tx-input {
    flex: 1;
    background: var(--bg-sunken);
    border: 1px solid var(--warn);
    border-radius: var(--radius-sm);
    color: var(--warn-text);
    font-family: var(--font-mono);
    font-size: var(--fs-200);
    padding: 0.15rem var(--sp-1);
  }

  .respond-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    background: var(--success-bg);
    border: 1px solid var(--success-border);
    border-radius: var(--radius-sm);
    padding: var(--sp-1) var(--sp-2);
  }

  .respond-info {
    flex: 1;
    color: var(--success-text);
  }

  .respond-info strong {
    color: var(--success-text);
    font-family: var(--font-mono);
  }
</style>
