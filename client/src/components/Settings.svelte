<script lang="ts">
  import {
    alertEnabled,
    configUpdateResult,
    deviceList,
    logFile,
    myCall,
    myGrid,
    myRole,
    rigctldTestResult,
    rigHost,
    rigPort,
    settingsOpen,
    waterfallScheme,
    logEntries,
  } from '../lib/stores'
  import { client } from '../lib/websocket'

  // Local form state — initialised from stores when panel opens
  let editCallsign = ''
  let editGrid = ''
  let editRigHost = ''
  let editRigPort = 4532
  let editInputDevice = ''
  let editOutputDevice = ''
  let rigTestPending = false

  // Re-initialise fields whenever the panel opens
  $: if ($settingsOpen) {
    editCallsign = $myCall
    editGrid = $myGrid
    editRigHost = $rigHost
    editRigPort = $rigPort
    editInputDevice = ''
    editOutputDevice = ''
    rigctldTestResult.set(null)
    configUpdateResult.set(null)
  }

  function saveStation() {
    const call = editCallsign.trim().toUpperCase()
    const grid = editGrid.trim().toUpperCase()
    if (!call || !grid) return
    configUpdateResult.set(null)
    client.send({ type: 'config_update', section: 'station', values: { callsign: call, grid } })
    // Update local stores immediately (server will confirm)
    myCall.set(call)
    myGrid.set(grid)
  }

  function saveRadio() {
    configUpdateResult.set(null)
    client.send({
      type: 'config_update',
      section: 'radio',
      values: { rigctld_host: editRigHost, rigctld_port: editRigPort },
    })
  }

  function saveAudio() {
    configUpdateResult.set(null)
    client.send({
      type: 'config_update',
      section: 'audio',
      values: { input_device: editInputDevice, output_device: editOutputDevice },
    })
  }

  function testRigctld() {
    rigTestPending = true
    rigctldTestResult.set(null)
    client.send({ type: 'test_rigctld' })
  }

  $: if ($rigctldTestResult) {
    rigTestPending = false
  }

  function close() {
    settingsOpen.set(false)
  }
</script>

{#if $settingsOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="overlay" on:click|self={close}>
    <aside class="panel">
      <header class="panel-header">
        <h2>Settings</h2>
        <button class="close-btn" on:click={close}>✕</button>
      </header>

      <!-- Station -->
      <section>
        <h3>Station</h3>
        <div class="field-row">
          <label for="s-callsign">Callsign</label>
          <input
            id="s-callsign"
            type="text"
            bind:value={editCallsign}
            maxlength="13"
            disabled={$myRole !== 'operator'}
            class="mono"
          />
        </div>
        <div class="field-row">
          <label for="s-grid">Grid</label>
          <input
            id="s-grid"
            type="text"
            bind:value={editGrid}
            maxlength="6"
            disabled={$myRole !== 'operator'}
            class="mono"
          />
        </div>
        <div class="field-row">
          <span class="field-label">Log file</span>
          <span class="display-val">{$logFile}</span>
        </div>
        {#if $myRole === 'operator'}
          <div class="btn-row">
            <button on:click={saveStation}>Save station</button>
          </div>
        {/if}
        <div class="btn-row">
          <a href="/api/log" download="ft8.adi" class="dl-link">Download ADIF log</a>
        </div>
      </section>

      <!-- Audio -->
      <section>
        <h3>Audio</h3>
        <div class="field-row">
          <label for="s-input-dev">Input device</label>
          <select id="s-input-dev" bind:value={editInputDevice} disabled={$myRole !== 'operator'}>
            <option value="">(default)</option>
            {#each $deviceList.inputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </div>
        <div class="field-row">
          <label for="s-output-dev">Output device</label>
          <select id="s-output-dev" bind:value={editOutputDevice} disabled={$myRole !== 'operator'}>
            <option value="">(default)</option>
            {#each $deviceList.outputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </div>
        {#if $myRole === 'operator'}
          <div class="btn-row">
            <button on:click={saveAudio}>Save audio</button>
          </div>
        {/if}
        <p class="note">Audio device changes require server restart.</p>
      </section>

      <!-- Radio -->
      <section>
        <h3>Radio</h3>
        <div class="field-row">
          <label for="s-rig-host">rigctld host</label>
          <input id="s-rig-host" type="text" bind:value={editRigHost} disabled={$myRole !== 'operator'} class="mono" />
        </div>
        <div class="field-row">
          <label for="s-rig-port">rigctld port</label>
          <input id="s-rig-port" type="number" bind:value={editRigPort} min="1" max="65535" disabled={$myRole !== 'operator'} />
        </div>
        {#if $myRole === 'operator'}
          <div class="btn-row">
            <button on:click={saveRadio}>Save radio</button>
            <button on:click={testRigctld} disabled={rigTestPending}>
              {rigTestPending ? 'Testing…' : 'Test connection'}
            </button>
          </div>
          {#if $rigctldTestResult}
            <div class="result-msg" class:ok={$rigctldTestResult.success} class:err={!$rigctldTestResult.success}>
              {$rigctldTestResult.message}
            </div>
          {/if}
          <p class="note">Radio config changes require server restart.</p>
        {/if}
      </section>

      <!-- Config update result banner -->
      {#if $configUpdateResult}
        <div class="banner" class:ok={$configUpdateResult.success} class:err={!$configUpdateResult.success}>
          {$configUpdateResult.success ? '✓ Saved' : '✗ Error'}
          {#if $configUpdateResult.message} — {$configUpdateResult.message}{/if}
          {#if $configUpdateResult.requires_restart}
            <strong> (restart required)</strong>
          {/if}
        </div>
      {/if}

      <!-- Display -->
      <section>
        <h3>Display</h3>
        <div class="field-row">
          <label for="s-wf-scheme">Waterfall scheme</label>
          <select id="s-wf-scheme" bind:value={$waterfallScheme}>
            <option value="classic">Classic</option>
            <option value="greyscale">Greyscale</option>
            <option value="heat">Heat</option>
          </select>
        </div>
        <label class="checkbox-row">
          <input type="checkbox" bind:checked={$alertEnabled} />
          Alert when callsign is heard
        </label>
      </section>

      <!-- Recent QSOs -->
      {#if $logEntries.length > 0}
        <section>
          <h3>Recent QSOs</h3>
          <div class="log-table-wrap">
            <table class="log-table">
              <thead>
                <tr><th>Call</th><th>Grid</th><th>Sent</th><th>Rcvd</th><th>Band</th><th>UTC</th></tr>
              </thead>
              <tbody>
                {#each $logEntries as e}
                  <tr>
                    <td>{e.their_call}</td>
                    <td>{e.their_grid ?? '—'}</td>
                    <td>{e.rst_sent}</td>
                    <td>{e.rst_rcvd}</td>
                    <td>{e.band}</td>
                    <td>{e.time_on}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </section>
      {/if}
    </aside>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 200;
    display: flex;
    justify-content: flex-end;
  }

  .panel {
    background: #1a1a2e;
    border-left: 1px solid #2a2a4a;
    width: 380px;
    max-width: 100vw;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    font-family: monospace;
    font-size: 0.82rem;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #2a2a4a;
    background: #12122a;
    position: sticky;
    top: 0;
    z-index: 1;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
    color: #7ec8e3;
  }

  .close-btn {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    font-size: 1rem;
    padding: 0.2rem 0.4rem;
  }
  .close-btn:hover { color: #e0e0e0; }

  section {
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #1e1e3a;
  }

  h3 {
    margin: 0 0 0.5rem;
    font-size: 0.75rem;
    color: #7ec8e3;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .field-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.4rem;
  }

  label,
  .field-label {
    color: #8888aa;
    min-width: 100px;
    flex-shrink: 0;
  }

  input[type='text'],
  input[type='number'],
  select {
    background: #0d0d1a;
    border: 1px solid #2a2a4a;
    color: #e0e0e0;
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
    font-family: monospace;
    font-size: 0.82rem;
    flex: 1;
    min-width: 0;
  }

  input:disabled,
  select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .mono { font-family: monospace; text-transform: uppercase; }

  .display-val {
    color: #aaa;
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-row {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.4rem;
    flex-wrap: wrap;
  }

  button {
    background: #2a2a4a;
    border: 1px solid #3a3a6a;
    color: #e0e0e0;
    padding: 0.25rem 0.6rem;
    border-radius: 3px;
    cursor: pointer;
    font-family: monospace;
    font-size: 0.8rem;
  }
  button:hover:not(:disabled) { background: #3a3a6a; }
  button:disabled { opacity: 0.5; cursor: not-allowed; }

  .dl-link {
    color: #7ec8e3;
    text-decoration: none;
    font-size: 0.8rem;
  }
  .dl-link:hover { text-decoration: underline; }

  .note {
    margin: 0.3rem 0 0;
    color: #666688;
    font-size: 0.75rem;
  }

  .result-msg {
    margin-top: 0.35rem;
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
    font-size: 0.78rem;
  }
  .result-msg.ok { background: #0a2a0a; color: #66cc66; border: 1px solid #226622; }
  .result-msg.err { background: #2a0a0a; color: #cc6666; border: 1px solid #662222; }

  .banner {
    margin: 0.5rem 1rem;
    padding: 0.3rem 0.6rem;
    border-radius: 3px;
    font-size: 0.8rem;
  }
  .banner.ok { background: #0a2a0a; color: #66cc66; border: 1px solid #226622; }
  .banner.err { background: #2a0a0a; color: #cc6666; border: 1px solid #662222; }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: #c0c0d8;
    cursor: pointer;
    margin-top: 0.3rem;
  }

  .log-table-wrap {
    overflow-x: auto;
    max-height: 200px;
    overflow-y: auto;
  }

  .log-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.78rem;
  }

  .log-table th {
    padding: 0.2rem 0.4rem;
    color: #8888aa;
    border-bottom: 1px solid #2a2a4a;
    text-align: left;
    font-weight: normal;
    white-space: nowrap;
    position: sticky;
    top: 0;
    background: #1a1a2e;
  }

  .log-table td {
    padding: 0.15rem 0.4rem;
    border-bottom: 1px solid #14142a;
    color: #c8c8e0;
    white-space: nowrap;
  }
</style>
