<script lang="ts">
  import { onDestroy } from 'svelte'
  import { client } from '../lib/websocket'
  import { trapFocus } from '../lib/actions'
  import {
    configUpdateResult,
    connected,
    deviceList,
    hamlibAvailable,
    myCall,
    myGrid,
    needsSetup,
    osType,
    rigHost,
    rigPort,
    serialPorts,
    wizardOpen,
  } from '../lib/stores'

  // ---- Step management --------------------------------------------------------

  let step = 1
  const TOTAL_STEPS = 5

  function next() { if (step < TOTAL_STEPS) step++ }
  function back() { if (step > 1) step-- }

  // ---- Step 2: Station identity -----------------------------------------------

  let callsign = $myCall || ''
  let grid     = $myGrid || ''
  let password = ''
  let passwordConfirm = ''

  let callsignError = ''
  let gridError     = ''
  let passwordError = ''

  function validateStation(): boolean {
    callsignError = ''
    gridError     = ''
    passwordError = ''
    const cs = callsign.trim().toUpperCase()
    if (cs.length < 3 || cs.length > 13 || !/^[A-Z0-9/\-]+$/.test(cs)) {
      callsignError = 'Enter a valid callsign (3–13 alphanumeric characters)'
      return false
    }
    const g = grid.trim().toUpperCase()
    if (!/^[A-Z]{2}[0-9]{2}([A-Z]{2})?$/.test(g)) {
      gridError = 'Enter a valid Maidenhead grid (e.g. FN31 or FN31pr)'
      return false
    }
    if (password.length < 4) {
      passwordError = 'Password must be at least 4 characters'
      return false
    }
    if (password !== passwordConfirm) {
      passwordError = 'Passwords do not match'
      return false
    }
    return true
  }

  function nextFromStation() {
    if (validateStation()) next()
  }

  // ---- Step 3: Audio ----------------------------------------------------------

  let inputDevice  = ''
  let outputDevice = ''

  // ---- Step 4: Radio ----------------------------------------------------------

  type Backend = 'rigctld' | 'hamlib'
  let radioBackend: Backend = 'rigctld'
  let rigctldHost = $rigHost || 'localhost'
  let rigctldPort = $rigPort || 4532
  let showRigctldGuide = false

  // Hamlib direct fields
  let rigModel   = 1035  // IC-7300 default
  let serialPort = ''
  let baudRate   = 19200

  const RIG_MODELS = [
    { label: 'Hamlib Dummy (testing)',      model: 1     },
    { label: 'Icom IC-7300',                model: 1035  },
    { label: 'Icom IC-7610',                model: 1037  },
    { label: 'Icom IC-7100',                model: 1032  },
    { label: 'Icom IC-705',                 model: 3072  },
    { label: 'Icom IC-9700',                model: 3070  },
    { label: 'Yaesu FT-891',                model: 3085  },
    { label: 'Yaesu FT-991A',               model: 3086  },
    { label: 'Yaesu FT-817 / FT-818',       model: 120   },
    { label: 'Yaesu FT-DX10',               model: 3089  },
    { label: 'Kenwood TS-590S/G',           model: 2021  },
    { label: 'Kenwood TS-890S',             model: 2035  },
    { label: 'Kenwood TS-2000',             model: 2014  },
    { label: 'Elecraft K3 / K3S',           model: 2032  },
    { label: 'Elecraft KX3',                model: 2045  },
  ]

  const BAUD_RATES = [1200, 4800, 9600, 19200, 38400, 57600, 115200]

  let serialPortsLoaded = false

  function enterRadioStep() {
    if (!serialPortsLoaded) {
      serialPortsLoaded = true
      client.getSerialPorts()
    }
  }

  $: if (step === 4) enterRadioStep()

  function osLabel(os: string): string {
    switch (os) {
      case 'windows':      return 'Windows'
      case 'raspberry_pi': return 'Raspberry Pi'
      case 'linux':        return 'Linux'
      case 'macos':        return 'macOS'
      default:             return os || 'Unknown'
    }
  }

  function rigctldInstallCmd(os: string): string {
    switch (os) {
      case 'windows':
        return 'Download Hamlib from https://github.com/Hamlib/Hamlib/releases\nExtract the zip and add the bin/ folder to your PATH.\nThen run:  rigctld -m <model_number> -r COM3'
      case 'raspberry_pi':
      case 'linux':
        return 'sudo apt install hamlib-utils\nrigctld -m <model_number> -r /dev/ttyUSB0 -s 19200'
      case 'macos':
        return 'brew install hamlib\nrigctld -m <model_number> -r /dev/cu.usbserial-XXXX -s 19200'
      default:
        return 'Install Hamlib for your platform, then run: rigctld -m <model_number> -r <port>'
    }
  }

  // ---- Step 5 / Save ----------------------------------------------------------

  let saving = false
  let saveError = ''

  // Guard against the "Saving…" state hanging forever: a client-side timeout
  // (and a lost-connection watcher below) will bail out if the server never
  // confirms — e.g. if the socket drops mid-save on first run.
  const SAVE_TIMEOUT_MS = 10000
  let saveTimeout: ReturnType<typeof setTimeout> | null = null

  function clearSaveTimeout() {
    if (saveTimeout !== null) {
      clearTimeout(saveTimeout)
      saveTimeout = null
    }
  }

  $: if ($configUpdateResult && saving) {
    saving = false
    clearSaveTimeout()
    if ($configUpdateResult.success) {
      saveError = ''
    } else {
      saveError = $configUpdateResult.message || 'Unknown error'
    }
  }

  // Bail out if the connection drops while a save is in flight.
  $: if (saving && !$connected) {
    saving = false
    clearSaveTimeout()
    saveError = 'Lost connection to the server before the save completed. Please check your connection and try again.'
  }

  // Cancel any pending timeout when leaving the review step so it can't fire spuriously.
  $: step, clearSaveTimeout()

  onDestroy(clearSaveTimeout)

  function save() {
    saving = true
    saveError = ''
    configUpdateResult.set(null)
    clearSaveTimeout()
    saveTimeout = setTimeout(() => {
      saveTimeout = null
      if (saving) {
        saving = false
        saveError = 'Timed out waiting for the server to confirm the save. Please check your connection and try again.'
      }
    }, SAVE_TIMEOUT_MS)
    client.completeSetup({
      callsign:          callsign.trim().toUpperCase(),
      grid:              grid.trim().toUpperCase(),
      operator_password: password,
      input_device:      inputDevice || null,
      output_device:     outputDevice || null,
      radio_backend:     radioBackend,
      rigctld_host:      radioBackend === 'rigctld' ? rigctldHost : 'localhost',
      rigctld_port:      radioBackend === 'rigctld' ? rigctldPort : 4532,
      rig_model:         radioBackend === 'hamlib' ? rigModel : null,
      serial_port:       radioBackend === 'hamlib' ? (serialPort || null) : null,
      baud_rate:         radioBackend === 'hamlib' ? baudRate : null,
    })
  }

  function close() {
    clearSaveTimeout()
    saving = false
    wizardOpen.set(false)
    step = 1
    saveError = ''
    configUpdateResult.set(null)
  }
</script>

{#if $wizardOpen}
  <div class="overlay">
    <div
      class="wizard"
      role="dialog"
      aria-modal="true"
      aria-labelledby="wizard-title"
      use:trapFocus={{ onEscape: $needsSetup ? undefined : close }}
    >

      <!-- Close (✕) is only offered when the wizard was manually re-opened;
           during a forced first-run setup it stays non-dismissable. -->
      {#if !$needsSetup}
        <button class="wizard-close btn btn--icon" on:click={close} title="Close">✕</button>
      {/if}

      <!-- Step dots -->
      <div class="dots">
        {#each Array(TOTAL_STEPS) as _, i}
          <span class="dot" class:active={i + 1 === step} class:done={i + 1 < step}></span>
        {/each}
      </div>

      <!-- ── Step 1: Welcome ─────────────────────────────────────── -->
      {#if step === 1}
        <h2 id="wizard-title">Welcome to BetterFT8</h2>
        <p class="intro">
          This wizard will help you configure your station for the first time.
          You'll set your callsign, audio devices, and radio connection.
        </p>
        {#if $osType}
          <div class="info-row">
            <span class="label">Detected OS:</span>
            <span class="value">{osLabel($osType)}</span>
          </div>
        {/if}
        <div class="info-row">
          <span class="label">Hamlib direct backend:</span>
          <span class="value">{$hamlibAvailable ? 'Available' : 'Not compiled in'}</span>
        </div>
        <div class="nav">
          <span></span>
          <button class="btn btn--primary" on:click={next}>Get Started →</button>
        </div>

      <!-- ── Step 2: Station Identity ────────────────────────────── -->
      {:else if step === 2}
        <h2 id="wizard-title">Station Identity</h2>

        <label>
          Callsign
          <input
            class="input"
            bind:value={callsign}
            maxlength="13"
            placeholder="W1AW"
            on:input={() => callsign = callsign.toUpperCase()}
          />
          {#if callsignError}<span class="error">{callsignError}</span>{/if}
        </label>

        <label>
          Maidenhead Grid
          <input
            class="input"
            bind:value={grid}
            maxlength="6"
            placeholder="FN31"
            on:input={() => grid = grid.toUpperCase()}
          />
          {#if gridError}<span class="error">{gridError}</span>{/if}
        </label>

        <label>
          Operator Password
          <input class="input" type="password" bind:value={password} placeholder="Choose a password" />
        </label>

        <label>
          Confirm Password
          <input class="input" type="password" bind:value={passwordConfirm} placeholder="Repeat password" />
          {#if passwordError}<span class="error">{passwordError}</span>{/if}
        </label>

        <p class="hint">The operator password is required to control the radio and transmit.</p>

        <div class="nav">
          <button class="btn btn--ghost" on:click={back}>← Back</button>
          <button class="btn btn--primary" on:click={nextFromStation}>Next →</button>
        </div>

      <!-- ── Step 3: Audio Devices ────────────────────────────────── -->
      {:else if step === 3}
        <h2 id="wizard-title">Audio Devices</h2>

        <label>
          Audio Input (receive)
          <select class="input" bind:value={inputDevice}>
            <option value="">— System default —</option>
            {#each $deviceList.inputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </label>

        <label>
          Audio Output (transmit)
          <select class="input" bind:value={outputDevice}>
            <option value="">— System default —</option>
            {#each $deviceList.outputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </label>

        <p class="hint">Audio changes take effect after restarting the server.</p>

        <div class="nav">
          <button class="btn btn--ghost" on:click={back}>← Back</button>
          <button class="btn btn--primary" on:click={next}>Next →</button>
        </div>

      <!-- ── Step 4: Radio Setup ──────────────────────────────────── -->
      {:else if step === 4}
        <h2 id="wizard-title">Radio Connection</h2>

        <div class="backend-toggle">
          <button
            class="toggle-btn"
            class:selected={radioBackend === 'rigctld'}
            on:click={() => radioBackend = 'rigctld'}
          >
            rigctld <span class="tag">recommended</span>
          </button>
          <button
            class="toggle-btn"
            class:selected={radioBackend === 'hamlib'}
            disabled={!$hamlibAvailable}
            title={$hamlibAvailable ? '' : 'Hamlib direct support was not compiled into this build'}
            on:click={() => { if ($hamlibAvailable) radioBackend = 'hamlib' }}
          >
            Hamlib direct
            {#if !$hamlibAvailable}<span class="tag dim">unavailable</span>{/if}
          </button>
        </div>

        {#if radioBackend === 'rigctld'}
          <p class="hint">rigctld is a separate daemon that talks to your radio. BetterFT8 connects to it over TCP.</p>

          <label>
            rigctld Host
            <input class="input" bind:value={rigctldHost} placeholder="localhost" />
          </label>

          <label>
            rigctld Port
            <input class="input" type="number" bind:value={rigctldPort} min="1" max="65535" />
          </label>

          <button class="guide-toggle" on:click={() => showRigctldGuide = !showRigctldGuide}>
            {showRigctldGuide ? '▾' : '▸'} How to install &amp; start rigctld
          </button>

          {#if showRigctldGuide}
            <pre class="guide">{rigctldInstallCmd($osType)}</pre>
          {/if}

        {:else}
          <p class="hint">Hamlib direct controls the radio without a daemon. Choose your rig model and serial port.</p>

          <label>
            Rig Model
            <select class="input" bind:value={rigModel}>
              {#each RIG_MODELS as r}
                <option value={r.model}>{r.label} ({r.model})</option>
              {/each}
            </select>
          </label>

          <label>
            Serial Port
            <div class="port-row">
              <select class="input" bind:value={serialPort}>
                <option value="">— Select port —</option>
                {#each $serialPorts as p}
                  <option value={p}>{p}</option>
                {/each}
              </select>
              <button class="btn btn--icon" on:click={() => client.getSerialPorts()} title="Refresh port list">↻</button>
            </div>
          </label>

          <label>
            Baud Rate
            <select class="input" bind:value={baudRate}>
              {#each BAUD_RATES as b}
                <option value={b}>{b}</option>
              {/each}
            </select>
          </label>
        {/if}

        <div class="nav">
          <button class="btn btn--ghost" on:click={back}>← Back</button>
          <button class="btn btn--primary" on:click={next}>Next →</button>
        </div>

      <!-- ── Step 5: Review & Save ─────────────────────────────────── -->
      {:else if step === 5}
        <h2 id="wizard-title">Review &amp; Save</h2>

        <table class="summary">
          <tbody>
            <tr><td>Callsign</td><td>{callsign.toUpperCase()}</td></tr>
            <tr><td>Grid</td><td>{grid.toUpperCase()}</td></tr>
            <tr><td>Operator password</td><td>{'•'.repeat(password.length)}</td></tr>
            <tr><td>Audio input</td><td>{inputDevice || '(system default)'}</td></tr>
            <tr><td>Audio output</td><td>{outputDevice || '(system default)'}</td></tr>
            <tr><td>Radio backend</td><td>{radioBackend}</td></tr>
            {#if radioBackend === 'rigctld'}
              <tr><td>rigctld host</td><td>{rigctldHost}:{rigctldPort}</td></tr>
            {:else}
              <tr><td>Rig model</td><td>{rigModel}</td></tr>
              <tr><td>Serial port</td><td>{serialPort || '(none)'}</td></tr>
              <tr><td>Baud rate</td><td>{baudRate}</td></tr>
            {/if}
          </tbody>
        </table>

        {#if $configUpdateResult?.success}
          <div class="banner success">
            Setup saved. Restart the server to activate audio and radio settings.
            <button class="btn btn--primary" style="margin-top:0.75rem" on:click={close}>Close</button>
          </div>
        {:else}
          {#if saveError}
            <div class="banner error-banner">{saveError}</div>
          {/if}
          <div class="nav">
            <button class="btn btn--ghost" on:click={back}>← Back</button>
            <button class="btn btn--primary" disabled={saving} on:click={save}>
              {saving ? 'Saving…' : 'Save & Finish'}
            </button>
          </div>
        {/if}
      {/if}

    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    z-index: 500;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .wizard {
    position: relative;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    width: min(520px, 96vw);
    max-height: 90vh;
    overflow-y: auto;
    padding: var(--sp-6);
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    color: var(--text-primary);
  }

  .wizard-close {
    position: absolute;
    top: var(--sp-3);
    right: var(--sp-3);
    z-index: 1;
  }

  .dots {
    display: flex;
    gap: var(--sp-2);
    justify-content: center;
    margin-bottom: var(--sp-2);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
  }
  .dot.done   { background: var(--success); border-color: var(--success); }
  .dot.active { background: var(--accent); border-color: var(--accent); }

  h2 {
    margin: 0;
    font-size: var(--fs-500);
    color: var(--accent);
    text-align: center;
  }

  .intro {
    margin: 0;
    color: var(--text-secondary);
    line-height: var(--lh-normal);
    text-align: center;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    padding: var(--sp-1) 0;
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-300);
  }
  .info-row .label { color: var(--text-muted); }
  .info-row .value { color: var(--text-primary); font-family: var(--font-mono); }

  label {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    font-size: var(--fs-300);
    color: var(--text-secondary);
  }

  /* Form fields use the global .input primitive; only sizing is local. */
  .input {
    font-size: var(--fs-300);
    padding: var(--sp-2) var(--sp-3);
  }

  .error {
    color: var(--danger-text);
    font-size: var(--fs-200);
  }

  .hint {
    margin: 0;
    font-size: var(--fs-200);
    color: var(--text-muted);
  }

  .nav {
    display: flex;
    justify-content: space-between;
    margin-top: var(--sp-2);
  }

  /* Give the wizard buttons more presence than the compact global default. */
  .nav .btn,
  .banner .btn {
    padding: var(--sp-2) var(--sp-5);
    font-size: var(--fs-300);
  }

  .backend-toggle {
    display: flex;
    gap: var(--sp-3);
  }

  .toggle-btn {
    flex: 1;
    background: var(--bg-sunken);
    border: 1px solid var(--border-strong);
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    padding: var(--sp-3);
    cursor: pointer;
    font-size: var(--fs-200);
  }
  .toggle-btn.selected {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--surface-2);
  }
  .toggle-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .tag {
    font-size: var(--fs-100);
    background: var(--success);
    color: var(--accent-ink);
    border-radius: var(--radius-sm);
    padding: 0.1rem 0.3rem;
    margin-left: var(--sp-1);
    vertical-align: middle;
  }
  .tag.dim { background: var(--text-disabled); }

  .guide-toggle {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: var(--fs-200);
    padding: 0;
    text-align: left;
  }
  .guide-toggle:hover { text-decoration: underline; }

  .guide {
    background: var(--bg-sunken);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--fs-200);
    color: var(--text-secondary);
    white-space: pre-wrap;
    margin: 0;
  }

  .port-row {
    display: flex;
    gap: var(--sp-2);
  }
  .port-row select { flex: 1; }

  .summary {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-300);
    font-family: var(--font-mono);
  }
  .summary td {
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
  }
  .summary td:first-child {
    color: var(--text-muted);
    font-family: var(--font-ui);
    width: 45%;
  }

  .banner {
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--radius-sm);
    font-size: var(--fs-300);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }
  .banner.success     { background: var(--success-bg); border: 1px solid var(--success-border); color: var(--success-text); }
  .banner.error-banner { background: var(--danger-bg); border: 1px solid var(--danger-border); color: var(--danger-text); }
</style>
