<script lang="ts">
  import { onMount } from 'svelte'
  import { client } from './lib/websocket'
  import { connected, lastMessage } from './lib/stores'

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

  <section>
    <h2>Last message</h2>
    {#if $lastMessage}
      <pre>{JSON.stringify($lastMessage, null, 2)}</pre>
    {:else}
      <p class="muted">Waiting for server…</p>
    {/if}
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    background: #1a1a2e;
    color: #e0e0e0;
    font-family: monospace;
  }

  main {
    max-width: 800px;
    margin: 0 auto;
    padding: 1.5rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 2rem;
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

  pre {
    background: #0f0f1e;
    padding: 1rem;
    border-radius: 4px;
    overflow: auto;
    color: #7ec8e3;
  }

  .muted {
    color: #666;
  }
</style>
