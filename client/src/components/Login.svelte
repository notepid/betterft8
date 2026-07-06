<script lang="ts">
  import { myRole, needsAuth, operatorStatus, authError } from '../lib/stores'
  import { client } from '../lib/websocket'
  import { focusOnMount, trapFocus } from '../lib/actions'

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
    <div
      class="auth-box"
      role="dialog"
      aria-modal="true"
      aria-labelledby="viewer-auth-title"
      use:trapFocus
    >
      <h2 id="viewer-auth-title">BetterFT8</h2>
      <p class="auth-hint">This server requires a viewer password.</p>
      <form onsubmit={(e) => { e.preventDefault(); submitViewer() }}>
        <input
          class="input auth-input"
          type="password"
          placeholder="Viewer password"
          bind:value={viewerPassword}
          use:focusOnMount
        />
        <button class="btn btn--primary" type="submit">Connect</button>
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
      <span class="badge role-badge operator">Operating</span>
    {:else}
      <span class="badge role-badge viewer">Viewing</span>
    {/if}

    <!-- Client count -->
    <span class="client-count" title="Connected clients">
      {clientCount} client{clientCount === 1 ? '' : 's'}
    </span>

    <!-- Operator controls -->
    {#if $myRole === 'operator'}
      <button class="btn btn--danger" onclick={releaseOperator} title="Release operator lock">
        Release Operator
      </button>
    {:else}
      {#if claimOpen}
        <form
          class="claim-form"
          onsubmit={(e) => { e.preventDefault(); claimOperator() }}
        >
          <input
            class="input claim-input"
            type="password"
            placeholder="Operator password"
            bind:value={operatorPassword}
            use:focusOnMount
          />
          <button class="btn btn--primary" type="submit">Claim</button>
          <button class="btn btn--icon" type="button" onclick={() => { claimOpen = false; operatorPassword = '' }}>
            ✕
          </button>
        </form>
      {:else}
        <button
          class="btn btn--primary"
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
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    padding: var(--sp-6) var(--sp-6);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 280px;
    text-align: center;
  }

  .auth-box h2 {
    margin: 0;
    color: var(--accent);
    font-size: var(--fs-500);
  }

  .auth-hint {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--fs-200);
  }

  .auth-box form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .auth-error {
    margin: 0;
    color: var(--danger-text);
    font-size: var(--fs-200);
  }

  /* ---- Inline header session bar ------------------------------------------ */
  .session-bar {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .role-badge.operator {
    background: var(--success-bg);
    color: var(--role-operator);
    border: 1px solid var(--success-border);
  }

  .role-badge.viewer {
    background: var(--surface-2);
    color: var(--role-viewer);
    border: 1px solid var(--border-strong);
  }

  .client-count {
    font-size: var(--fs-100);
    color: var(--text-muted);
  }

  .claim-form {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .claim-input {
    width: 10rem;
  }

  .op-error {
    font-size: var(--fs-100);
    color: var(--danger-text);
  }
</style>
