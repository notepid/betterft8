<script lang="ts">
  import { onMount } from 'svelte'
  import { waterfallLine } from '../lib/stores'
  import type { WaterfallMessage } from '../lib/messages'

  let canvas: HTMLCanvasElement
  let imageData: ImageData | null = null

  // Precompute 256-entry RGBA color lookup table (classic ham radio waterfall palette)
  const colorLut = buildColorLut()

  function buildColorLut(): Uint8ClampedArray {
    const lut = new Uint8ClampedArray(256 * 4)
    for (let i = 0; i < 256; i++) {
      const [r, g, b] = intensityToRgb(i)
      lut[i * 4 + 0] = r
      lut[i * 4 + 1] = g
      lut[i * 4 + 2] = b
      lut[i * 4 + 3] = 255
    }
    return lut
  }

  function intensityToRgb(v: number): [number, number, number] {
    if (v < 50) {
      // black → dark blue
      return [0, 0, Math.round((v / 50) * 128)]
    } else if (v < 100) {
      // dark blue → blue
      const t = (v - 50) / 50
      return [0, 0, Math.round(128 + t * 127)]
    } else if (v < 150) {
      // blue → cyan
      const t = (v - 100) / 50
      return [0, Math.round(t * 255), 255]
    } else if (v < 200) {
      // cyan → yellow (through green)
      const t = (v - 150) / 50
      return [Math.round(t * 255), 255, Math.round(255 * (1 - t))]
    } else if (v < 230) {
      // yellow → red
      const t = (v - 200) / 30
      return [255, Math.round(255 * (1 - t)), 0]
    } else {
      // red → white
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

    // (Re)allocate ImageData if canvas size changed
    if (!imageData || imageData.width !== w || imageData.height !== h) {
      imageData = ctx.createImageData(w, h)
      // Fill with black, full alpha
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
      const value = srcArr[srcIdx]
      const offset = x * 4
      imageData.data[offset + 0] = colorLut[value * 4 + 0]
      imageData.data[offset + 1] = colorLut[value * 4 + 1]
      imageData.data[offset + 2] = colorLut[value * 4 + 2]
      imageData.data[offset + 3] = 255
    }

    ctx.putImageData(imageData, 0, 0)
    drawFreqAxis(ctx, msg.freq_max, w)
  }

  function drawFreqAxis(ctx: CanvasRenderingContext2D, freqMax: number, w: number) {
    ctx.save()
    ctx.font = '10px monospace'
    for (let freq = 0; freq <= freqMax; freq += 1000) {
      const x = Math.round((freq / freqMax) * w)
      // Tick line
      ctx.strokeStyle = 'rgba(255,255,255,0.35)'
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(x + 0.5, 0)
      ctx.lineTo(x + 0.5, 14)
      ctx.stroke()
      // Label
      ctx.fillStyle = 'rgba(255,255,255,0.8)'
      const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`
      ctx.fillText(label, x + 2, 12)
    }
    ctx.restore()
  }

  onMount(() => {
    // Set canvas pixel width to match its CSS display width
    const updateSize = () => {
      const newW = canvas.clientWidth
      if (newW > 0 && newW !== canvas.width) {
        canvas.width = newW
        imageData = null // force re-alloc on next line
      }
    }

    updateSize()

    const ro = new ResizeObserver(updateSize)
    ro.observe(canvas)

    const unsub = waterfallLine.subscribe((line) => {
      if (line) processLine(line)
    })

    return () => {
      unsub()
      ro.disconnect()
    }
  })
</script>

<div class="waterfall-wrap">
  <canvas bind:this={canvas} height="300"></canvas>
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
