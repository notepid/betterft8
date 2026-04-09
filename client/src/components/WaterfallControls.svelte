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

  <button class="reset-btn" on:click={resetDefaults} title="Reset to defaults">Reset</button>
</div>

<style>
  .wf-controls {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.3rem 0.6rem;
    background: #12122a;
    border: 1px solid #2a2a4a;
    border-top: none;
    border-radius: 0 0 4px 4px;
    font-family: monospace;
    font-size: 0.75rem;
    flex-wrap: wrap;
  }

  .ctrl-group {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .slider-group {
    flex: 1;
    min-width: 120px;
  }

  label {
    color: #8888aa;
    white-space: nowrap;
    font-size: 0.72rem;
  }

  select {
    background: #0d0d1a;
    border: 1px solid #2a2a4a;
    color: #e0e0e0;
    padding: 0.1rem 0.2rem;
    border-radius: 3px;
    font-family: monospace;
    font-size: 0.72rem;
  }

  input[type='range'] {
    flex: 1;
    accent-color: #7ec8e3;
    min-width: 0;
    height: 14px;
  }

  input[type='range']:disabled {
    opacity: 0.4;
  }

  .val {
    color: #aaa;
    font-family: monospace;
    min-width: 30px;
    text-align: right;
    font-size: 0.72rem;
  }

  .auto-label {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    color: #c0c0d8;
    cursor: pointer;
  }

  .auto-label input[type='checkbox'] {
    accent-color: #7ec8e3;
  }

  .reset-btn {
    background: #2a2a4a;
    border: 1px solid #3a3a6a;
    color: #8888aa;
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    cursor: pointer;
    font-family: monospace;
    font-size: 0.7rem;
  }
  .reset-btn:hover { background: #3a3a6a; color: #e0e0e0; }
</style>
