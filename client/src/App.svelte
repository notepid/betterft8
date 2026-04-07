<script lang="ts">
  import { onMount } from 'svelte'
  import { client } from './lib/websocket'
  import { connected, lastMessage } from './lib/stores'
  import Waterfall from './components/Waterfall.svelte'

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
  </header>

  <section class="waterfall-section">
    <Waterfall />
  </section>

  {#if $lastMessage}
    <section class="last-msg">
      <h2>Last message</h2>
      <pre>{JSON.stringify($lastMessage, null, 2)}</pre>
    </section>
  {/if}
</main>

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

  .waterfall-section {
    margin-bottom: 1rem;
  }

  .last-msg h2 {
    font-size: 0.9rem;
    color: #888;
    margin: 0.5rem 0;
  }

  pre {
    background: #0f0f1e;
    padding: 1rem;
    border-radius: 4px;
    overflow: auto;
    color: #7ec8e3;
    font-size: 0.8rem;
  }
</style>
