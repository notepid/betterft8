<script lang="ts">
  import { notifications, dismissNotification } from '../lib/stores'
</script>

<!-- Transient notification stack. Announced to assistive tech via aria-live. -->
<div class="toast-stack" role="region" aria-label="Notifications" aria-live="polite">
  {#each $notifications as n (n.id)}
    <div class="toast toast--{n.severity}" role="status">
      <span class="toast-msg">{n.message}</span>
      <button
        class="btn btn--icon toast-dismiss"
        aria-label="Dismiss notification"
        onclick={() => dismissNotification(n.id)}
      >✕</button>
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    top: var(--sp-3);
    right: var(--sp-3);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    max-width: min(90vw, 22rem);
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius-md);
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    box-shadow: var(--shadow-overlay);
    font-size: var(--fs-200);
    animation: toast-in 0.15s ease;
  }

  .toast--success {
    border-color: var(--success-border);
    background: var(--success-bg);
    color: var(--success-text);
  }
  .toast--error {
    border-color: var(--danger-border);
    background: var(--danger-bg);
    color: var(--danger-text);
  }
  .toast--info {
    color: var(--text-secondary);
  }

  .toast-msg {
    flex: 1;
  }

  .toast-dismiss {
    flex: 0 0 auto;
  }

  @keyframes toast-in {
    from { opacity: 0; transform: translateY(-0.25rem); }
    to   { opacity: 1; transform: translateY(0); }
  }
</style>
