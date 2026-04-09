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
    <span class="status" class:online={$connected} title={$connected ? 'Connected' : 'Disconnected'}>
      {$connected ? 'Connected' : 'Disconnected'}
    </span>
    <Login />
    <button
      class="settings-btn"
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
  :global(body) {
    margin: 0;
    background: #1a1a2e;
    color: #e0e0e0;
    font-family: monospace;
  }

  main {
    max-width: 1200px;
    margin: 0 auto;
    padding: 1rem 1.5rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1rem;
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-size: 1.5rem;
    color: #7ec8e3;
  }

  .status {
    padding: 0.25rem 0.75rem;
    border-radius: 999px;
    font-size: 0.8rem;
    background: #c0392b;
    color: #fff;
  }

  .status.online {
    background: #27ae60;
  }

  .settings-btn {
    margin-left: auto;
    background: none;
    border: 1px solid #2a2a4a;
    color: #8888aa;
    font-size: 1.1rem;
    cursor: pointer;
    border-radius: 4px;
    padding: 0.2rem 0.5rem;
    line-height: 1;
  }
  .settings-btn:hover {
    color: #e0e0e0;
    border-color: #4a4a8a;
  }

  .radio-section {
    margin-bottom: 0.5rem;
  }

  .controls-section {
    margin-bottom: 0.5rem;
  }

  .qso-section {
    margin-bottom: 0.75rem;
  }

  .waterfall-section {
    margin-bottom: 1rem;
  }

  .decode-section {
    margin-top: 1rem;
  }

  .decode-section h2 {
    font-size: 0.9rem;
    color: #888;
    margin: 0 0 0.4rem;
  }
</style>
