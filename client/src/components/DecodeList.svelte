<script lang="ts">
  import { decodes, myCall, selectedDecode, type Decode } from '../lib/stores'

  function rowClass(d: Decode): string {
    const upper = d.message.toUpperCase()
    if ($myCall && upper.includes($myCall.toUpperCase())) return 'row-mycall'
    if (upper.startsWith('CQ ') || upper === 'CQ') return 'row-cq'
    return ''
  }

  function handleClick(d: Decode) {
    selectedDecode.set(d)
  }
</script>

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
      {#each $decodes as d (d.period + '-' + d.freq.toFixed(0) + '-' + d.message)}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <tr
          class={rowClass(d)}
          class:selected={$selectedDecode?.period === d.period && $selectedDecode?.freq === d.freq && $selectedDecode?.message === d.message}
          on:click={() => handleClick(d)}
        >
          <td class="col-utc">{d.utcTime}</td>
          <td class="col-snr">{d.snr > 0 ? '+' : ''}{d.snr}</td>
          <td class="col-dt">{d.dt >= 0 ? '+' : ''}{d.dt.toFixed(1)}</td>
          <td class="col-freq">{Math.round(d.freq)}</td>
          <td class="col-msg">{d.message}</td>
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
    overflow-y: auto;
    max-height: 500px;
    background: #0d0d1a;
    border: 1px solid #2a2a4a;
    border-radius: 4px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8rem;
    font-family: monospace;
  }

  thead tr {
    position: sticky;
    top: 0;
    background: #1a1a3a;
    z-index: 1;
  }

  th {
    padding: 0.3rem 0.5rem;
    text-align: left;
    color: #8888aa;
    font-weight: normal;
    border-bottom: 1px solid #2a2a4a;
    white-space: nowrap;
  }

  td {
    padding: 0.2rem 0.5rem;
    border-bottom: 1px solid #1a1a2e;
    white-space: nowrap;
  }

  tr {
    cursor: pointer;
    color: #c8c8e0;
  }

  tr:hover {
    background: #1e1e3a;
  }

  tr.selected {
    background: #1a2a3a;
    outline: 1px solid #3a5a8a;
  }

  .row-cq {
    color: #7ec8e3;
  }

  .row-mycall {
    color: #f0f040;
    font-weight: bold;
    background: #1a2a1a;
  }

  .row-empty td {
    text-align: center;
    color: #555577;
    padding: 1rem;
    cursor: default;
  }

  .col-utc  { color: #888899; min-width: 5.5rem; }
  .col-snr  { text-align: right; min-width: 3rem; }
  .col-dt   { text-align: right; min-width: 3.5rem; }
  .col-freq { text-align: right; min-width: 3.5rem; }
  .col-msg  { width: 100%; }
</style>
