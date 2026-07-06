<script lang="ts">
  import { waterfallScheme, waterfallFloor, waterfallCeiling, waterfallAutoLevel } from '../lib/stores'

  function resetDefaults() {
    waterfallFloor.set(-120)
    waterfallCeiling.set(0)
    waterfallAutoLevel.set(false)
  }
</script>

<div class="wf-controls">
  <div class="ctrl-group">
    <label for="wf-scheme">Color</label>
    <select id="wf-scheme" bind:value={$waterfallScheme}>
      <option value="classic">Classic</option>
      <option value="greyscale">Grey</option>
      <option value="heat">Heat</option>
    </select>
  </div>

  <div class="ctrl-group slider-group">
    <label for="wf-floor">Floor</label>
    <input id="wf-floor" type="range" min="-120" max="-1" step="1"
      bind:value={$waterfallFloor} disabled={$waterfallAutoLevel} />
    <span class="val">{$waterfallFloor}</span>
  </div>

  <div class="ctrl-group slider-group">
    <label for="wf-ceil">Ceil</label>
    <input id="wf-ceil" type="range" min="-119" max="0" step="1"
      bind:value={$waterfallCeiling} disabled={$waterfallAutoLevel} />
    <span class="val">{$waterfallCeiling}</span>
  </div>

  <div class="ctrl-group">
    <label class="auto-label">
      <input type="checkbox" bind:checked={$waterfallAutoLevel} />
      Auto
    </label>
  </div>

  <button class="btn" on:click={resetDefaults} title="Reset to defaults">Reset</button>
</div>

<style>
  .wf-controls {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-1) var(--sp-2);
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-top: none;
    border-radius: 0 0 var(--radius-sm) var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-100);
    flex-wrap: wrap;
  }

  .ctrl-group {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .slider-group {
    flex: 1;
    min-width: 120px;
  }

  label {
    color: var(--text-muted);
    white-space: nowrap;
    font-size: var(--fs-100);
  }

  select {
    background: var(--bg-sunken);
    border: 1px solid var(--border-strong);
    color: var(--text-primary);
    padding: 0.1rem 0.2rem;
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-100);
  }

  input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
    min-width: 0;
    height: 14px;
  }

  input[type='range']:disabled {
    opacity: 0.4;
  }

  .val {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    min-width: 30px;
    text-align: right;
    font-size: var(--fs-100);
  }

  .auto-label {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    color: var(--text-secondary);
    cursor: pointer;
  }

  .auto-label input[type='checkbox'] {
    accent-color: var(--accent);
  }
</style>
