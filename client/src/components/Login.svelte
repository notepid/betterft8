<script lang="ts">
  import { myRole, needsAuth, operatorStatus, authError } from '../lib/stores'
  import { client } from '../lib/websocket'

  let viewerPassword = ''
  let operatorPassword = ''
  let claimOpen = false

  function submitViewer() {
    authError.set(null)
    client.send({ type: 'auth', password: viewerPassword })
    viewerPassword = ''
  }

  function claimOperator() {
    authError.set(null)
    client.send({ type: 'claim_operator', password: operatorPassword })
    operatorPassword = ''
    claimOpen = false
  }

  function releaseOperator() {
    client.send({ type: 'release_operator' })
  }

  $: clientCount = $operatorStatus?.client_count ?? 0
  $: hasOperator = $operatorStatus?.operator_client_id !== null
</script>

<!-- Viewer auth overlay — shown when server requires a viewer password -->
{#if $needsAuth}
  <div class="auth-overlay">
    <div class="auth-box">
      <h2>BetterFT8</h2>
      <p class="auth-hint">This server requires a viewer password.</p>
      <form onsubmit={(e) => { e.preventDefault(); submitViewer() }}>
        <input
          class="auth-input"
          type="password"
          placeholder="Viewer password"
          bind:value={viewerPassword}
          autofocus
        />
        <button class="btn-connect" type="submit">Connect</button>
      </form>
      {#if $authError}
        <p class="auth-error">{$authError}</p>
      {/if}
    </div>
  </div>
{/if}

<!-- Inline header widget — shown after authentication -->
{#if $myRole !== 'unauthenticated'}
  <div class="session-bar">
    <!-- Role badge -->
    {#if $myRole === 'operator'}
      <span class="role-badge operator">Operating</span>
    {:else}
      <span class="role-badge viewer">Viewing</span>
    {/if}

    <!-- Client count -->
    <span class="client-count" title="Connected clients">
      {clientCount} client{clientCount === 1 ? '' : 's'}
    </span>

    <!-- Operator controls -->
    {#if $myRole === 'operator'}
      <button class="btn-release" onclick={releaseOperator} title="Release operator lock">
        Release Operator
      </button>
    {:else}
      {#if claimOpen}
        <form
          class="claim-form"
          onsubmit={(e) => { e.preventDefault(); claimOperator() }}
        >
          <input
            class="claim-input"
            type="password"
            placeholder="Operator password"
            bind:value={operatorPassword}
            autofocus
          />
          <button class="btn-claim-ok" type="submit">Claim</button>
          <button class="btn-cancel" type="button" onclick={() => { claimOpen = false; operatorPassword = '' }}>
            ✕
          </button>
        </form>
      {:else}
        <button
          class="btn-claim"
          onclick={() => { claimOpen = true }}
          title={hasOperator ? 'Another client is operating' : 'Claim operator control'}
        >
          {hasOperator ? 'Operator taken' : 'Claim Operator'}
        </button>
      {/if}
      {#if $authError && !claimOpen}
        <span class="op-error">{$authError}</span>
      {/if}
    {/if}
  </div>
{/if}

<style>
  /* ---- Full-screen viewer auth overlay ------------------------------------ */
  .auth-overlay {
    position: fixed;
    inset: 0;
    background: rgba(10, 10, 30, 0.92);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .auth-box {
    background: #16213e;
    border: 1px solid #3a3a7a;
    border-radius: 8px;
    padding: 2rem 2.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-width: 280px;
    text-align: center;
  }

  .auth-box h2 {
    margin: 0;
    color: #7ec8e3;
    font-size: 1.4rem;
  }

  .auth-hint {
    margin: 0;
    color: #888;
    font-size: 0.85rem;
  }

  .auth-box form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .auth-input {
    background: #0d0d2b;
    border: 1px solid #3a3a7a;
    border-radius: 4px;
    color: #c8d8f0;
    font-family: monospace;
    font-size: 0.9rem;
    padding: 0.4rem 0.6rem;
  }

  .btn-connect {
    background: #1a5a3a;
    border: 1px solid #2a8a5a;
    border-radius: 4px;
    color: #80e8b0;
    font-family: monospace;
    font-size: 0.9rem;
    padding: 0.4rem;
    cursor: pointer;
  }

  .btn-connect:hover {
    background: #246a46;
  }

  .auth-error {
    margin: 0;
    color: #e07070;
    font-size: 0.8rem;
  }

  /* ---- Inline header session bar ------------------------------------------ */
  .session-bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .role-badge {
    font-size: 0.75rem;
    font-weight: bold;
    border-radius: 999px;
    padding: 0.2rem 0.65rem;
  }

  .role-badge.operator {
    background: #1a5a1a;
    color: #80e880;
    border: 1px solid #2a8a2a;
  }

  .role-badge.viewer {
    background: #1e2a4a;
    color: #7888aa;
    border: 1px solid #3a4a7a;
  }

  .client-count {
    font-size: 0.75rem;
    color: #666;
  }

  .btn-release {
    background: #5a1a1a;
    border: 1px solid #8a2a2a;
    border-radius: 3px;
    color: #f08080;
    font-family: monospace;
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    cursor: pointer;
  }

  .btn-release:hover {
    background: #6a2020;
  }

  .btn-claim {
    background: #1e2a4a;
    border: 1px solid #3a5a8a;
    border-radius: 3px;
    color: #7ea8d8;
    font-family: monospace;
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    cursor: pointer;
  }

  .btn-claim:hover {
    background: #253a6a;
  }

  .claim-form {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .claim-input {
    background: #0d0d2b;
    border: 1px solid #3a5a8a;
    border-radius: 3px;
    color: #c8d8f0;
    font-family: monospace;
    font-size: 0.8rem;
    padding: 0.15rem 0.35rem;
    width: 10rem;
  }

  .btn-claim-ok {
    background: #1a3a5a;
    border: 1px solid #2a5a8a;
    border-radius: 3px;
    color: #80b8f0;
    font-family: monospace;
    font-size: 0.75rem;
    padding: 0.2rem 0.45rem;
    cursor: pointer;
  }

  .btn-claim-ok:hover {
    background: #1e4a6a;
  }

  .btn-cancel {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0.1rem 0.2rem;
  }

  .btn-cancel:hover {
    color: #aaa;
  }

  .op-error {
    font-size: 0.75rem;
    color: #e07070;
  }
</style>
