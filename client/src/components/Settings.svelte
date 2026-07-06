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
    logEntries,
    wizardOpen,
    theme,
    THEMES,
  } from '../lib/stores'
  import { client } from '../lib/websocket'
  import { trapFocus } from '../lib/actions'

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
    <div
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
      use:trapFocus={{ onEscape: close }}
    >
      <header class="panel-header">
        <h2 id="settings-title">Settings</h2>
        <button class="btn btn--icon" on:click={close}>✕</button>
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
            class="input mono"
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
            class="input mono"
          />
        </div>
        {#if $myRole === 'operator'}
          <div class="btn-row">
            <button class="btn btn--primary" on:click={saveStation}>Save station</button>
          </div>
        {/if}
      </section>

      <!-- Audio -->
      <section>
        <h3>Audio</h3>
        <div class="field-row">
          <label for="s-input-dev">Input device</label>
          <select id="s-input-dev" class="input" bind:value={editInputDevice} disabled={$myRole !== 'operator'}>
            <option value="">(default)</option>
            {#each $deviceList.inputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </div>
        <div class="field-row">
          <label for="s-output-dev">Output device</label>
          <select id="s-output-dev" class="input" bind:value={editOutputDevice} disabled={$myRole !== 'operator'}>
            <option value="">(default)</option>
            {#each $deviceList.outputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </div>
        {#if $myRole === 'operator'}
          <div class="btn-row">
            <button class="btn btn--primary" on:click={saveAudio}>Save audio</button>
          </div>
        {/if}
        <p class="note">Audio device changes require server restart.</p>
      </section>

      <!-- Radio -->
      <section>
        <h3>Radio</h3>
        <div class="field-row">
          <label for="s-rig-host">rigctld host</label>
          <input id="s-rig-host" type="text" bind:value={editRigHost} disabled={$myRole !== 'operator'} class="input mono" />
        </div>
        <div class="field-row">
          <label for="s-rig-port">rigctld port</label>
          <input id="s-rig-port" type="number" bind:value={editRigPort} min="1" max="65535" disabled={$myRole !== 'operator'} class="input" />
        </div>
        {#if $myRole === 'operator'}
          <div class="btn-row">
            <button class="btn btn--primary" on:click={saveRadio}>Save radio</button>
            <button class="btn" on:click={testRigctld} disabled={rigTestPending}>
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

      <!-- Setup Wizard shortcut -->
      {#if $myRole === 'operator'}
        <section>
          <h3>Setup Wizard</h3>
          <p class="note">Re-run the setup wizard to reconfigure your station, audio, and radio settings.</p>
          <div class="btn-row">
            <button class="btn" on:click={() => { wizardOpen.set(true); settingsOpen.set(false) }}>
              Open Setup Wizard
            </button>
          </div>
        </section>
      {/if}

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

      <!-- Appearance -->
      <section>
        <h3>Appearance</h3>
        <div class="field-row">
          <label for="s-theme">Theme</label>
          <select id="s-theme" class="input" bind:value={$theme}>
            {#each THEMES as t}
              <option value={t.value}>{t.label}</option>
            {/each}
          </select>
        </div>
      </section>

      <!-- Notifications -->
      <section>
        <h3>Notifications</h3>
        <label class="checkbox-row">
          <input type="checkbox" bind:checked={$alertEnabled} />
          Alert when callsign is heard
        </label>
      </section>

      <!-- Log -->
      <section>
        <h3>Log</h3>
        <div class="field-row">
          <span class="field-label">Log file</span>
          <span class="display-val">{$logFile}</span>
        </div>
        <div class="btn-row">
          <a href="/api/log" download="ft8.adi" class="dl-link">Download ADIF log</a>
        </div>
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
    </div>
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
    background: var(--surface-1);
    border-left: 1px solid var(--border);
    box-shadow: var(--shadow-overlay);
    width: 380px;
    max-width: 100vw;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    font-size: var(--fs-200);
    /* Slide the drawer in from the right edge instead of popping in. */
    transform: translateX(0);
    transition: transform 0.2s ease;
    animation: drawer-slide-in 0.2s ease;
  }

  @keyframes drawer-slide-in {
    from { transform: translateX(100%); }
    to   { transform: translateX(0); }
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  h2 {
    margin: 0;
    font-size: var(--fs-400);
    color: var(--accent);
  }

  section {
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
  }

  h3 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-100);
    color: var(--accent);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .field-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-bottom: var(--sp-1);
  }

  label,
  .field-label {
    color: var(--text-muted);
    min-width: 100px;
    flex-shrink: 0;
  }

  /* Form fields use the global .input primitive; only layout is local. */
  .input {
    flex: 1;
    min-width: 0;
  }
  .input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .mono { text-transform: uppercase; }

  .display-val {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-row {
    display: flex;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
    flex-wrap: wrap;
  }

  .dl-link {
    color: var(--accent);
    text-decoration: none;
    font-size: var(--fs-200);
  }
  .dl-link:hover { text-decoration: underline; }

  .note {
    margin: var(--sp-1) 0 0;
    color: var(--text-muted);
    font-size: var(--fs-100);
  }

  .result-msg {
    margin-top: var(--sp-1);
    padding: var(--sp-1) var(--sp-2);
    border-radius: var(--radius-sm);
    font-size: var(--fs-100);
  }
  .result-msg.ok { background: var(--success-bg); color: var(--success-text); border: 1px solid var(--success-border); }
  .result-msg.err { background: var(--danger-bg); color: var(--danger-text); border: 1px solid var(--danger-border); }

  .banner {
    margin: var(--sp-2) var(--sp-4);
    padding: var(--sp-1) var(--sp-3);
    border-radius: var(--radius-sm);
    font-size: var(--fs-200);
  }
  .banner.ok { background: var(--success-bg); color: var(--success-text); border: 1px solid var(--success-border); }
  .banner.err { background: var(--danger-bg); color: var(--danger-text); border: 1px solid var(--danger-border); }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    color: var(--text-secondary);
    cursor: pointer;
    margin-top: var(--sp-1);
  }

  .log-table-wrap {
    overflow-x: auto;
    max-height: 200px;
    overflow-y: auto;
  }

  .log-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-100);
    font-family: var(--font-mono);
  }

  .log-table th {
    padding: var(--sp-1) var(--sp-2);
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    text-align: left;
    font-weight: var(--fw-regular);
    font-family: var(--font-ui);
    white-space: nowrap;
    position: sticky;
    top: 0;
    background: var(--surface-1);
  }

  .log-table td {
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
    color: var(--text-secondary);
    white-space: nowrap;
  }
</style>
