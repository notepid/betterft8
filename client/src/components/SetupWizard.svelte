<script lang="ts">
  import { client } from '../lib/websocket'
  import {
    configUpdateResult,
    deviceList,
    hamlibAvailable,
    myCall,
    myGrid,
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
  let rictgldHost = $rigHost || 'localhost'
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

  $: if ($configUpdateResult && saving) {
    saving = false
    if ($configUpdateResult.success) {
      saveError = ''
    } else {
      saveError = $configUpdateResult.message || 'Unknown error'
    }
  }

  function save() {
    saving = true
    saveError = ''
    configUpdateResult.set(null)
    client.completeSetup({
      callsign:          callsign.trim().toUpperCase(),
      grid:              grid.trim().toUpperCase(),
      operator_password: password,
      input_device:      inputDevice || null,
      output_device:     outputDevice || null,
      radio_backend:     radioBackend,
      rigctld_host:      radioBackend === 'rigctld' ? rictgldHost : 'localhost',
      rigctld_port:      radioBackend === 'rigctld' ? rigctldPort : 4532,
      rig_model:         radioBackend === 'hamlib' ? rigModel : null,
      serial_port:       radioBackend === 'hamlib' ? (serialPort || null) : null,
      baud_rate:         radioBackend === 'hamlib' ? baudRate : null,
    })
  }

  function close() {
    wizardOpen.set(false)
    step = 1
    saveError = ''
    configUpdateResult.set(null)
  }
</script>

{#if $wizardOpen}
  <div class="overlay">
    <div class="wizard">

      <!-- Step dots -->
      <div class="dots">
        {#each Array(TOTAL_STEPS) as _, i}
          <span class="dot" class:active={i + 1 === step} class:done={i + 1 < step}></span>
        {/each}
      </div>

      <!-- ── Step 1: Welcome ─────────────────────────────────────── -->
      {#if step === 1}
        <h2>Welcome to BetterFT8</h2>
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
          <button class="btn-primary" on:click={next}>Get Started →</button>
        </div>

      <!-- ── Step 2: Station Identity ────────────────────────────── -->
      {:else if step === 2}
        <h2>Station Identity</h2>

        <label>
          Callsign
          <input
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
            bind:value={grid}
            maxlength="6"
            placeholder="FN31"
            on:input={() => grid = grid.toUpperCase()}
          />
          {#if gridError}<span class="error">{gridError}</span>{/if}
        </label>

        <label>
          Operator Password
          <input type="password" bind:value={password} placeholder="Choose a password" />
        </label>

        <label>
          Confirm Password
          <input type="password" bind:value={passwordConfirm} placeholder="Repeat password" />
          {#if passwordError}<span class="error">{passwordError}</span>{/if}
        </label>

        <p class="hint">The operator password is required to control the radio and transmit.</p>

        <div class="nav">
          <button class="btn-secondary" on:click={back}>← Back</button>
          <button class="btn-primary" on:click={nextFromStation}>Next →</button>
        </div>

      <!-- ── Step 3: Audio Devices ────────────────────────────────── -->
      {:else if step === 3}
        <h2>Audio Devices</h2>

        <label>
          Audio Input (receive)
          <select bind:value={inputDevice}>
            <option value="">— System default —</option>
            {#each $deviceList.inputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </label>

        <label>
          Audio Output (transmit)
          <select bind:value={outputDevice}>
            <option value="">— System default —</option>
            {#each $deviceList.outputs as dev}
              <option value={dev}>{dev}</option>
            {/each}
          </select>
        </label>

        <p class="hint">Audio changes take effect after restarting the server.</p>

        <div class="nav">
          <button class="btn-secondary" on:click={back}>← Back</button>
          <button class="btn-primary" on:click={next}>Next →</button>
        </div>

      <!-- ── Step 4: Radio Setup ──────────────────────────────────── -->
      {:else if step === 4}
        <h2>Radio Connection</h2>

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
            <input bind:value={rictgldHost} placeholder="localhost" />
          </label>

          <label>
            rigctld Port
            <input type="number" bind:value={rigctldPort} min="1" max="65535" />
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
            <select bind:value={rigModel}>
              {#each RIG_MODELS as r}
                <option value={r.model}>{r.label} ({r.model})</option>
              {/each}
            </select>
          </label>

          <label>
            Serial Port
            <div class="port-row">
              <select bind:value={serialPort}>
                <option value="">— Select port —</option>
                {#each $serialPorts as p}
                  <option value={p}>{p}</option>
                {/each}
              </select>
              <button class="btn-small" on:click={() => client.getSerialPorts()} title="Refresh port list">↻</button>
            </div>
          </label>

          <label>
            Baud Rate
            <select bind:value={baudRate}>
              {#each BAUD_RATES as b}
                <option value={b}>{b}</option>
              {/each}
            </select>
          </label>
        {/if}

        <div class="nav">
          <button class="btn-secondary" on:click={back}>← Back</button>
          <button class="btn-primary" on:click={next}>Next →</button>
        </div>

      <!-- ── Step 5: Review & Save ─────────────────────────────────── -->
      {:else if step === 5}
        <h2>Review &amp; Save</h2>

        <table class="summary">
          <tbody>
            <tr><td>Callsign</td><td>{callsign.toUpperCase()}</td></tr>
            <tr><td>Grid</td><td>{grid.toUpperCase()}</td></tr>
            <tr><td>Operator password</td><td>{'•'.repeat(password.length)}</td></tr>
            <tr><td>Audio input</td><td>{inputDevice || '(system default)'}</td></tr>
            <tr><td>Audio output</td><td>{outputDevice || '(system default)'}</td></tr>
            <tr><td>Radio backend</td><td>{radioBackend}</td></tr>
            {#if radioBackend === 'rigctld'}
              <tr><td>rigctld host</td><td>{rictgldHost}:{rigctldPort}</td></tr>
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
            <button class="btn-primary" style="margin-top:0.75rem" on:click={close}>Close</button>
          </div>
        {:else}
          {#if saveError}
            <div class="banner error-banner">{saveError}</div>
          {/if}
          <div class="nav">
            <button class="btn-secondary" on:click={back}>← Back</button>
            <button class="btn-primary" disabled={saving} on:click={save}>
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
    background: #16213e;
    border: 1px solid #2a2a5a;
    border-radius: 8px;
    width: min(520px, 96vw);
    max-height: 90vh;
    overflow-y: auto;
    padding: 2rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    color: #e0e0e0;
    font-family: monospace;
  }

  .dots {
    display: flex;
    gap: 0.5rem;
    justify-content: center;
    margin-bottom: 0.5rem;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #2a2a5a;
    border: 1px solid #4a4a8a;
  }
  .dot.done   { background: #27ae60; border-color: #27ae60; }
  .dot.active { background: #7ec8e3; border-color: #7ec8e3; }

  h2 {
    margin: 0;
    font-size: 1.2rem;
    color: #7ec8e3;
    text-align: center;
  }

  .intro {
    margin: 0;
    color: #aaa;
    line-height: 1.5;
    text-align: center;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    padding: 0.4rem 0;
    border-bottom: 1px solid #2a2a4a;
    font-size: 0.9rem;
  }
  .info-row .label { color: #888; }
  .info-row .value { color: #e0e0e0; }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.85rem;
    color: #aaa;
  }

  input, select {
    background: #0d0d1a;
    border: 1px solid #2a2a5a;
    border-radius: 4px;
    color: #e0e0e0;
    padding: 0.4rem 0.6rem;
    font-family: monospace;
    font-size: 0.9rem;
  }
  input:focus, select:focus {
    outline: none;
    border-color: #7ec8e3;
  }
  input:disabled, select:disabled { opacity: 0.5; cursor: not-allowed; }

  .error {
    color: #e74c3c;
    font-size: 0.8rem;
  }

  .hint {
    margin: 0;
    font-size: 0.8rem;
    color: #666;
  }

  .nav {
    display: flex;
    justify-content: space-between;
    margin-top: 0.5rem;
  }

  .btn-primary {
    background: #7ec8e3;
    color: #0d0d1a;
    border: none;
    border-radius: 4px;
    padding: 0.45rem 1.2rem;
    font-family: monospace;
    font-size: 0.9rem;
    cursor: pointer;
    font-weight: bold;
  }
  .btn-primary:hover:not(:disabled) { background: #a0d8ef; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn-secondary {
    background: transparent;
    color: #7ec8e3;
    border: 1px solid #7ec8e3;
    border-radius: 4px;
    padding: 0.45rem 1.2rem;
    font-family: monospace;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .btn-secondary:hover { background: rgba(126, 200, 227, 0.1); }

  .btn-small {
    background: #0d0d1a;
    border: 1px solid #2a2a5a;
    color: #7ec8e3;
    border-radius: 4px;
    padding: 0.4rem 0.6rem;
    cursor: pointer;
    font-size: 1rem;
    line-height: 1;
    flex-shrink: 0;
  }
  .btn-small:hover { border-color: #7ec8e3; }

  .backend-toggle {
    display: flex;
    gap: 0.75rem;
  }

  .toggle-btn {
    flex: 1;
    background: #0d0d1a;
    border: 1px solid #2a2a5a;
    color: #aaa;
    border-radius: 4px;
    padding: 0.6rem;
    font-family: monospace;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .toggle-btn.selected {
    border-color: #7ec8e3;
    color: #7ec8e3;
    background: rgba(126, 200, 227, 0.08);
  }
  .toggle-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .tag {
    font-size: 0.7rem;
    background: #27ae60;
    color: #fff;
    border-radius: 3px;
    padding: 0.1rem 0.3rem;
    margin-left: 0.3rem;
    vertical-align: middle;
  }
  .tag.dim { background: #555; }

  .guide-toggle {
    background: none;
    border: none;
    color: #7ec8e3;
    cursor: pointer;
    font-family: monospace;
    font-size: 0.85rem;
    padding: 0;
    text-align: left;
  }
  .guide-toggle:hover { text-decoration: underline; }

  .guide {
    background: #0d0d1a;
    border: 1px solid #2a2a5a;
    border-radius: 4px;
    padding: 0.75rem;
    font-size: 0.8rem;
    color: #aaa;
    white-space: pre-wrap;
    margin: 0;
  }

  .port-row {
    display: flex;
    gap: 0.5rem;
  }
  .port-row select { flex: 1; }

  .summary {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }
  .summary td {
    padding: 0.35rem 0.5rem;
    border-bottom: 1px solid #2a2a4a;
  }
  .summary td:first-child { color: #888; width: 45%; }

  .banner {
    padding: 0.75rem 1rem;
    border-radius: 4px;
    font-size: 0.9rem;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }
  .banner.success     { background: #1a3a1a; border: 1px solid #27ae60; color: #aaffaa; }
  .banner.error-banner { background: #3a1a1a; border: 1px solid #e74c3c; color: #ffaaaa; }
</style>
