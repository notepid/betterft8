<script lang="ts">
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { client } from './lib/websocket'
  import {
    myRole,
    connectionState,
    settingsOpen,
    wizardOpen,
    needsAuth,
  } from './lib/stores'
  import Waterfall from './components/Waterfall.svelte'
  import DecodeList from './components/DecodeList.svelte'
  import RadioStatus from './components/RadioStatus.svelte'
  import Controls from './components/Controls.svelte'
  import QsoPanel from './components/QsoPanel.svelte'
  import Settings from './components/Settings.svelte'
  import SetupWizard from './components/SetupWizard.svelte'
  import WaterfallControls from './components/WaterfallControls.svelte'
  import StatusBar from './components/StatusBar.svelte'
  import Toast from './components/Toast.svelte'

  // Global Escape = emergency Halt TX. Only fires for a connected operator, and
  // only when the operator is NOT typing in a field and NO overlay is open —
  // overlays own Esc for close, so we must not collide with that.
  function onGlobalKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') return
    if (get(myRole) !== 'operator') return
    if (get(connectionState) !== 'connected') return
    if (get(settingsOpen) || get(wizardOpen) || get(needsAuth)) return

    const t = e.target as HTMLElement | null
    if (t) {
      const tag = t.tagName
      if (
        tag === 'INPUT' ||
        tag === 'TEXTAREA' ||
        tag === 'SELECT' ||
        t.isContentEditable
      ) {
        return
      }
    }

    client.send({ type: 'halt_tx' })
  }

  onMount(() => {
    client.connect()
    const interval = setInterval(() => {
      client.send({ type: 'ping' })
    }, 5000)
    window.addEventListener('keydown', onGlobalKeydown)
    return () => {
      clearInterval(interval)
      window.removeEventListener('keydown', onGlobalKeydown)
    }
  })
</script>

<main>
  <StatusBar />

  <section class="waterfall-section">
    <Waterfall />
    <WaterfallControls />
  </section>

  <section class="decode-section">
    <h2>Decoded Messages</h2>
    <DecodeList />
  </section>

  <section class="operate-section">
    <RadioStatus />
    <QsoPanel />
    <Controls />
  </section>
</main>

<!-- Settings slide-out panel (portal-style fixed overlay) -->
<Settings />

<!-- Setup wizard overlay (shown on first run or manually triggered) -->
<SetupWizard />

<!-- Transient notification stack (toast layer) -->
<Toast />

<style>
  /* Viewport-height operating console: nothing an operator watches scrolls
     off-screen. The page body never scrolls; each cell scrolls internally. */
  :global(html),
  :global(body) {
    height: 100%;
  }

  main {
    height: 100vh;
    max-width: none;
    margin: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 360px;
    grid-template-rows: auto minmax(240px, 42vh) minmax(0, 1fr);
    grid-template-areas:
      "statusbar statusbar"
      "waterfall waterfall"
      "decodes   operate";
    gap: var(--sp-2);
    padding: var(--sp-2);
    overflow: hidden;
  }

  .waterfall-section {
    grid-area: waterfall;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .decode-section {
    grid-area: decodes;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    overflow: hidden;
  }

  .decode-section h2 {
    font-size: var(--fs-300);
    color: var(--text-muted);
    margin: 0;
    flex: 0 0 auto;
  }

  /* Right operate column: tuning, QSO, and TX controls stacked. */
  .operate-section {
    grid-area: operate;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    overflow-y: auto;
  }

  /* ---- Tablet: single column, operate becomes a wrapping row above decodes ---- */
  @media (max-width: 1099px) {
    main {
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(240px, 40vh) auto minmax(0, 1fr);
      grid-template-areas:
        "statusbar"
        "waterfall"
        "operate"
        "decodes";
    }

    .operate-section {
      flex-direction: row;
      flex-wrap: wrap;
      align-items: flex-start;
      overflow: visible;
    }
  }

  /* ---- Phone: waterfall shrinks, decodes fill, operate panels stack below ---- */
  @media (max-width: 599px) {
    main {
      grid-template-rows: auto 28vh minmax(0, 1fr) auto;
      grid-template-areas:
        "statusbar"
        "waterfall"
        "decodes"
        "operate";
    }

    .operate-section {
      flex-direction: column;
      flex-wrap: nowrap;
    }
  }
</style>
