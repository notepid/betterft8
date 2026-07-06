<script lang="ts">
  import { onMount } from 'svelte'
  import { client } from './lib/websocket'
  import { connected, settingsOpen } from './lib/stores'
  import Waterfall from './components/Waterfall.svelte'
  import DecodeList from './components/DecodeList.svelte'
  import RadioStatus from './components/RadioStatus.svelte'
  import Controls from './components/Controls.svelte'
  import QsoPanel from './components/QsoPanel.svelte'
  import Login from './components/Login.svelte'
  import Settings from './components/Settings.svelte'
  import SetupWizard from './components/SetupWizard.svelte'
  import WaterfallControls from './components/WaterfallControls.svelte'

  onMount(() => {
    client.connect()
    const interval = setInterval(() => {
      client.send({ type: 'ping' })
    }, 5000)
    return () => clearInterval(interval)
  })
</script>

<main>
  <header>
    <h1>BetterFT8</h1>
    <span
      class="badge"
      class:badge--success={$connected}
      class:badge--danger={!$connected}
      title={$connected ? 'Connected' : 'Disconnected'}
    >
      {$connected ? 'Connected' : 'Disconnected'}
    </span>
    <Login />
    <button
      class="btn btn--icon settings-btn"
      title="Settings"
      on:click={() => settingsOpen.update((v) => !v)}
    >
      ⚙
    </button>
  </header>

  <section class="radio-section">
    <RadioStatus />
  </section>

  <section class="controls-section">
    <Controls />
  </section>

  <section class="qso-section">
    <QsoPanel />
  </section>

  <section class="waterfall-section">
    <Waterfall />
    <WaterfallControls />
  </section>

  <section class="decode-section">
    <h2>Decoded Messages</h2>
    <DecodeList />
  </section>
</main>

<!-- Settings slide-out panel (portal-style fixed overlay) -->
<Settings />

<!-- Setup wizard overlay (shown on first run or manually triggered) -->
<SetupWizard />

<style>
  main {
    max-width: 1200px;
    margin: 0 auto;
    padding: var(--sp-4) var(--sp-5);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    margin-bottom: var(--sp-4);
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-size: var(--fs-600);
    color: var(--accent);
  }

  .settings-btn {
    margin-left: auto;
    font-size: var(--fs-500);
  }

  .radio-section {
    margin-bottom: var(--sp-2);
  }

  .controls-section {
    margin-bottom: var(--sp-2);
  }

  .qso-section {
    margin-bottom: var(--sp-3);
  }

  .waterfall-section {
    margin-bottom: var(--sp-4);
  }

  .decode-section {
    margin-top: var(--sp-4);
  }

  .decode-section h2 {
    font-size: var(--fs-300);
    color: var(--text-muted);
    margin: 0 0 var(--sp-2);
  }
</style>
