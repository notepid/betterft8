<script lang="ts">
  import { onMount } from 'svelte'
  import { get } from 'svelte/store'
  import { decodes, selectedDecode, waterfallLine, waterfallScheme, waterfallFloor, waterfallCeiling } from '../lib/stores'
  import type { Decode } from '../lib/stores'
  import type { WaterfallMessage } from '../lib/messages'

  let canvas: HTMLCanvasElement
  let imageData: ImageData | null = null
  let currentFreqMax = 5000

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
    // Classic (default)
    if (v < 50) {
      return [0, 0, Math.round((v / 50) * 128)]
    } else if (v < 100) {
      const t = (v - 50) / 50
      return [0, 0, Math.round(128 + t * 127)]
    } else if (v < 150) {
      const t = (v - 100) / 50
      return [0, Math.round(t * 255), 255]
    } else if (v < 200) {
      const t = (v - 150) / 50
      return [Math.round(t * 255), 255, Math.round(255 * (1 - t))]
    } else if (v < 230) {
      const t = (v - 200) / 30
      return [255, Math.round(255 * (1 - t)), 0]
    } else {
      const t = Math.min((v - 230) / 25, 1)
      return [255, Math.round(t * 255), Math.round(t * 255)]
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

    // Decode base64 → Uint8Array
    const binaryStr = atob(msg.data)
    const srcArr = new Uint8Array(binaryStr.length)
    for (let i = 0; i < binaryStr.length; i++) {
      srcArr[i] = binaryStr.charCodeAt(i)
    }
    const numBins = srcArr.length

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
      const color = isCq ? `rgba(0,255,136,${alpha})` : `rgba(255,255,200,${alpha * 0.9})`

      // Vertical tick mark below the freq axis
      ctx.strokeStyle = color
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(x + 0.5, 14)
      ctx.lineTo(x + 0.5, 22)
      ctx.stroke()

      // Callsign label
      const parts = d.message.split(' ')
      const label = isCq ? (parts[1] ?? d.message) : (parts[1] ?? parts[0] ?? d.message)
      ctx.fillStyle = color
      ctx.fillText(label, x + 2, 32)
    }
    ctx.restore()
  }

  function handleCanvasClick(event: MouseEvent) {
    if (!canvas || overlayDecodes.length === 0) return
    const rect = canvas.getBoundingClientRect()
    const x = (event.clientX - rect.left) * (canvas.width / rect.width)
    const clickedFreq = (x / canvas.width) * currentFreqMax

    // Find nearest decode within 60 Hz tolerance
    let nearest: (typeof overlayDecodes)[0] | null = null
    let minDist = 60
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
    background: #000;
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    overflow: hidden;
  }

  canvas {
    display: block;
    width: 100%;
    height: 300px;
    cursor: crosshair;
  }
</style>
