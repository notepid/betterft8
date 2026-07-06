<script lang="ts">
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { decodes, selectedDecode, waterfallLine, waterfallScheme, waterfallFloor, waterfallCeiling, waterfallAutoLevel, txFreq } from '../lib/stores'
  import type { Decode } from '../lib/stores'
  import { callerCall } from '../lib/callsign'
  import type { WaterfallMessage } from '../lib/messages'

  let canvas: HTMLCanvasElement
  let imageData: ImageData | null = null
  let currentFreqMax = 5000

  // Canvas draw colours, kept in sync with the app.css domain palette.
  // These are JS string colours (not CSS), so they mirror the token values.
  const CQ_COLOR = '#37e39b' // matches --cq
  const TX_BAND_COLOR = '#ff3ad2' // matches --tx-active
  // The canvas background well uses --waterfall-bg (#000010) via CSS on .waterfall-wrap.

  // Apply an alpha to one of the hex domain colours for canvas fills/strokes.
  function withAlpha(hex: string, alpha: number): string {
    const r = parseInt(hex.slice(1, 3), 16)
    const g = parseInt(hex.slice(3, 5), 16)
    const b = parseInt(hex.slice(5, 7), 16)
    return `rgba(${r}, ${g}, ${b}, ${alpha})`
  }

  // Overlay: decodes from the most-recent period(s), up to ~30 s old.
  let overlayDecodes: Array<{ freq: number; message: string; period: number }> = []

  // Rebuild LUT when scheme changes.
  let colorLut = buildColorLut($waterfallScheme)
  $: colorLut = buildColorLut($waterfallScheme)

  // Rebuild remap LUT when floor/ceiling changes.
  let remapLut = buildRemapLut($waterfallFloor, $waterfallCeiling)
  $: remapLut = buildRemapLut($waterfallFloor, $waterfallCeiling)

  function buildRemapLut(floorDb: number, ceilingDb: number): Uint8Array {
    const lut = new Uint8Array(256)
    const floorU8 = Math.round(Math.max(0, ((floorDb + 120) / 120) * 255))
    const ceilingU8 = Math.round(Math.min(255, ((ceilingDb + 120) / 120) * 255))
    const range = Math.max(1, ceilingU8 - floorU8)
    for (let i = 0; i < 256; i++) {
      lut[i] = i <= floorU8 ? 0 : i >= ceilingU8 ? 255 : Math.round((i - floorU8) / range * 255)
    }
    return lut
  }

  // Auto-level: estimate noise floor from incoming data and set floor/ceiling.
  // Uses exponential smoothing so the display adapts gradually.
  let smoothFloor = 0
  let smoothCeiling = 255
  let autoLevelInitialized = false

  // Change-guard: last rounded dB values actually pushed to the stores, so we
  // only write (and rebuild the remap LUT / re-render the sliders) on a change.
  let lastFloorDb = NaN
  let lastCeilingDb = NaN

  // Persistent scratch buffers reused across frames to avoid per-frame allocs.
  const histScratch = new Uint32Array(256)
  let scratchSrc = new Uint8Array(0)
  let frameCount = 0

  function autoLevel(srcArr: Uint8Array, numBins: number) {
    if (!$waterfallAutoLevel) return

    // Build a simple histogram (reuse the persistent buffer)
    const hist = histScratch
    hist.fill(0)
    for (let i = 0; i < numBins; i++) {
      hist[srcArr[i]]++
    }

    // Find the noise floor: the peak of the histogram (most common value)
    let peakBin = 0
    let peakCount = 0
    for (let i = 0; i < 256; i++) {
      if (hist[i] > peakCount) {
        peakCount = hist[i]
        peakBin = i
      }
    }

    // Find P95 of the distribution (signal level estimate)
    const total = numBins
    let cumulative = 0
    let p95Bin = 255
    for (let i = 0; i < 256; i++) {
      cumulative += hist[i]
      if (cumulative >= total * 0.97) {
        p95Bin = i
        break
      }
    }

    // Target: floor just below noise peak, ceiling a bit above the 97th percentile
    const targetFloor = Math.max(0, peakBin - 8)
    const targetCeiling = Math.min(255, Math.max(p95Bin + 15, targetFloor + 40))

    // Exponential smoothing (slow adaptation)
    const alpha = autoLevelInitialized ? 0.05 : 1.0
    smoothFloor = smoothFloor + alpha * (targetFloor - smoothFloor)
    smoothCeiling = smoothCeiling + alpha * (targetCeiling - smoothCeiling)
    autoLevelInitialized = true

    // Convert u8 back to dB and clamp to the slider ranges
    const floorDb = Math.max(-120, Math.min(-1, Math.round((smoothFloor / 255) * 120 - 120)))
    const ceilingDb = Math.max(-119, Math.min(0, Math.round((smoothCeiling / 255) * 120 - 120)))

    // Only write when the rounded value actually changed — avoids ~20 store
    // writes/sec that would otherwise rebuild the remap LUT and re-render the
    // bound sliders/labels every frame for no visible difference.
    if (floorDb !== lastFloorDb) {
      lastFloorDb = floorDb
      waterfallFloor.set(floorDb)
    }
    if (ceilingDb !== lastCeilingDb) {
      lastCeilingDb = ceilingDb
      waterfallCeiling.set(ceilingDb)
    }
  }

  // Reset smoothing (and the change-guard) when auto-level is toggled on so the
  // first frame re-initialises and force-writes the freshly computed levels.
  $: if ($waterfallAutoLevel) {
    autoLevelInitialized = false
    lastFloorDb = NaN
    lastCeilingDb = NaN
  }

  function buildColorLut(scheme: string): Uint8ClampedArray {
    const lut = new Uint8ClampedArray(256 * 4)
    for (let i = 0; i < 256; i++) {
      const [r, g, b] = intensityToRgb(i, scheme)
      lut[i * 4 + 0] = r
      lut[i * 4 + 1] = g
      lut[i * 4 + 2] = b
      lut[i * 4 + 3] = 255
    }
    return lut
  }

  function intensityToRgb(v: number, scheme: string): [number, number, number] {
    if (scheme === 'greyscale') {
      return [v, v, v]
    }
    if (scheme === 'heat') {
      if (v < 85)  return [Math.round((v / 85) * 255), 0, 0]
      if (v < 170) return [255, Math.round(((v - 85) / 85) * 255), 0]
      return [255, 255, Math.round(((v - 170) / 85) * 255)]
    }
    // Classic (default) — dark background with good signal contrast
    if (v < 20) {
      // Near-black for the noise floor
      const t = v / 20
      return [0, 0, Math.round(t * 40)]
    } else if (v < 60) {
      // Dark blue rising
      const t = (v - 20) / 40
      return [0, 0, Math.round(40 + t * 180)]
    } else if (v < 110) {
      // Blue to cyan
      const t = (v - 60) / 50
      return [0, Math.round(t * 255), Math.round(220 + t * 35)]
    } else if (v < 160) {
      // Cyan to green
      const t = (v - 110) / 50
      return [0, 255, Math.round(255 * (1 - t))]
    } else if (v < 210) {
      // Green to yellow
      const t = (v - 160) / 50
      return [Math.round(t * 255), 255, 0]
    } else if (v < 240) {
      // Yellow to red
      const t = (v - 210) / 30
      return [255, Math.round(255 * (1 - t)), 0]
    } else {
      // Red to white (very strong signals)
      const t = Math.min((v - 240) / 15, 1)
      return [255, Math.round(t * 200), Math.round(t * 200)]
    }
  }

  function processLine(msg: WaterfallMessage) {
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const w = canvas.width
    const h = canvas.height
    currentFreqMax = msg.freq_max

    // (Re)allocate ImageData if canvas size changed
    if (!imageData || imageData.width !== w || imageData.height !== h) {
      imageData = ctx.createImageData(w, h)
      for (let i = 3; i < imageData.data.length; i += 4) {
        imageData.data[i] = 255
      }
    }

    // Decode base64 → Uint8Array (reuse the scratch buffer, grow only if needed)
    const binaryStr = atob(msg.data)
    const numBins = binaryStr.length
    if (scratchSrc.length < numBins) {
      scratchSrc = new Uint8Array(numBins)
    }
    const srcArr = scratchSrc
    for (let i = 0; i < numBins; i++) {
      srcArr[i] = binaryStr.charCodeAt(i)
    }

    // Auto-level: adapt floor/ceiling from signal statistics. The recompute is
    // throttled to ~1 Hz (every 10th ~100ms frame); the smoothing alpha is small
    // so the slower adaptation is visually identical. The first frame after
    // enabling always runs so levels initialise immediately. Drawing below still
    // happens every frame.
    if ($waterfallAutoLevel && (!autoLevelInitialized || frameCount % 10 === 0)) {
      autoLevel(srcArr, numBins)
    }
    frameCount++

    // Scroll existing rows down by one row
    imageData.data.copyWithin(w * 4, 0)

    // Write new line at the top (row 0), stretching bins to canvas width
    for (let x = 0; x < w; x++) {
      const srcF = (x * (numBins - 1)) / Math.max(w - 1, 1)
      const srcIdx = Math.min(Math.floor(srcF), numBins - 1)
      const value = remapLut[srcArr[srcIdx]]
      const offset = x * 4
      imageData.data[offset + 0] = colorLut[value * 4 + 0]
      imageData.data[offset + 1] = colorLut[value * 4 + 1]
      imageData.data[offset + 2] = colorLut[value * 4 + 2]
      imageData.data[offset + 3] = 255
    }

    ctx.putImageData(imageData, 0, 0)
    drawFreqAxis(ctx, msg.freq_max, w)
    drawDecodeOverlay(ctx, w, msg.freq_max)
    drawTxFreqIndicator(ctx, w, h, msg.freq_max)
  }

  function drawFreqAxis(ctx: CanvasRenderingContext2D, freqMax: number, w: number) {
    ctx.save()
    ctx.font = '10px monospace'
    for (let freq = 0; freq <= freqMax; freq += 1000) {
      const x = Math.round((freq / freqMax) * w)
      ctx.strokeStyle = 'rgba(255,255,255,0.35)'
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(x + 0.5, 0)
      ctx.lineTo(x + 0.5, 14)
      ctx.stroke()
      ctx.fillStyle = 'rgba(255,255,255,0.8)'
      const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`
      ctx.fillText(label, x + 2, 12)
    }
    ctx.restore()
  }

  function drawDecodeOverlay(ctx: CanvasRenderingContext2D, w: number, freqMax: number) {
    if (overlayDecodes.length === 0) return
    const nowSec = Date.now() / 1000
    ctx.save()
    ctx.font = '9px monospace'
    for (const d of overlayDecodes) {
      const age = nowSec - d.period
      if (age > 30) continue
      const alpha = Math.max(0, 1 - age / 30)
      if (alpha <= 0) continue

      const x = Math.round((d.freq / freqMax) * w)
      const isCq = d.message.toUpperCase().startsWith('CQ ')
      const color = isCq ? withAlpha(CQ_COLOR, alpha) : `rgba(255,255,200,${alpha * 0.9})`

      // Vertical tick mark below the freq axis
      ctx.strokeStyle = color
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(x + 0.5, 14)
      ctx.lineTo(x + 0.5, 22)
      ctx.stroke()

      // Callsign label
      const parts = d.message.split(' ')
      const label = isCq ? callerCall(d.message) : (parts[1] ?? parts[0] ?? d.message)
      ctx.fillStyle = color
      ctx.fillText(label, x + 2, 32)
    }
    ctx.restore()
  }

  function drawTxFreqIndicator(ctx: CanvasRenderingContext2D, w: number, h: number, freqMax: number) {
    const freqLo = get(txFreq)
    if (freqLo <= 0 || freqLo > freqMax) return
    const freqHi = freqLo + 50 // FT8 signal bandwidth ~50 Hz
    const xLo = Math.round((freqLo / freqMax) * w)
    const xHi = Math.round((freqHi / freqMax) * w)
    const bandW = Math.max(2, xHi - xLo)

    ctx.save()

    // Semitransparent band fill — the "on the air" TX colour for visibility
    ctx.fillStyle = withAlpha(TX_BAND_COLOR, 0.18)
    ctx.fillRect(xLo, 0, bandW, h)

    // Solid edge lines (thick for visibility)
    ctx.strokeStyle = withAlpha(TX_BAND_COLOR, 0.9)
    ctx.lineWidth = 2
    ctx.beginPath()
    ctx.moveTo(xLo + 0.5, 0)
    ctx.lineTo(xLo + 0.5, h)
    ctx.stroke()
    ctx.beginPath()
    ctx.moveTo(xHi + 0.5, 0)
    ctx.lineTo(xHi + 0.5, h)
    ctx.stroke()

    // Labels at bottom showing lower and upper frequencies
    ctx.font = '10px monospace'
    ctx.fillStyle = withAlpha(TX_BAND_COLOR, 0.95)
    const loLabel = `${freqLo}`
    const hiLabel = `${freqHi}`
    const loTextW = ctx.measureText(loLabel).width
    const hiTextW = ctx.measureText(hiLabel).width
    // Lower freq label to the left of the lower edge
    ctx.fillText(loLabel, Math.max(2, xLo - loTextW - 2), h - 4)
    // Upper freq label to the right of the upper edge
    ctx.fillText(hiLabel, Math.min(xHi + 3, w - hiTextW - 2), h - 4)
    ctx.restore()
  }

  function handleCanvasClick(event: MouseEvent) {
    if (!canvas) return
    const rect = canvas.getBoundingClientRect()
    const x = (event.clientX - rect.left) * (canvas.width / rect.width)
    const clickedFreq = (x / canvas.width) * currentFreqMax

    // Find nearest decode within 25 Hz tolerance (FT8 signal is ~50 Hz wide)
    let nearest: (typeof overlayDecodes)[0] | null = null
    let minDist = 25
    for (const d of overlayDecodes) {
      const dist = Math.abs(d.freq - clickedFreq)
      if (dist < minDist) {
        minDist = dist
        nearest = d
      }
    }

    if (nearest) {
      const all = get(decodes)
      const found = all.find(
        (d: Decode) => d.period === nearest!.period && Math.abs(d.freq - nearest!.freq) < 1
      )
      if (found) selectedDecode.set(found)
    } else {
      // Set TX frequency to clicked position (rounded to 10 Hz, clamped to valid range)
      const snapped = Math.max(200, Math.min(3000, Math.round(clickedFreq / 10) * 10))
      txFreq.set(snapped)
    }
  }

  onMount(() => {
    const updateSize = () => {
      const newW = canvas.clientWidth
      if (newW > 0 && newW !== canvas.width) {
        canvas.width = newW
        imageData = null
      }
    }

    updateSize()

    const ro = new ResizeObserver(updateSize)
    ro.observe(canvas)

    const unsubWaterfall = waterfallLine.subscribe((line) => {
      if (line) processLine(line)
    })

    // Maintain overlay decode list from the decodes store
    const unsubDecodes = decodes.subscribe((ds) => {
      if (ds.length === 0) { overlayDecodes = []; return }
      const latestPeriod = ds[0].period
      // Keep last 2 periods (30 s window)
      overlayDecodes = ds
        .filter((d) => d.period >= latestPeriod - 15)
        .map((d) => ({ freq: d.freq, message: d.message, period: d.period }))
    })

    return () => {
      unsubWaterfall()
      unsubDecodes()
      ro.disconnect()
    }
  })
</script>

<div class="waterfall-wrap">
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <canvas bind:this={canvas} height="300" on:click={handleCanvasClick}></canvas>
</div>

<style>
  .waterfall-wrap {
    width: 100%;
    background: var(--waterfall-bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  canvas {
    display: block;
    width: 100%;
    height: 300px;
    cursor: crosshair;
  }
</style>
