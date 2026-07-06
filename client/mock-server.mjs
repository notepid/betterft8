// Mock BetterFT8 server for UI development / review — NO radio required.
//
// Serves the built client (client/dist) AND a /ws endpoint that speaks the
// BetterFT8 JSON protocol with canned data, so you can see the full operating
// UI (waterfall, decodes, in-QSO, TX lamp) without a rig or the Rust server.
//
//   cd client
//   npm install
//   npm run build     # produce client/dist
//   npm run mock      # serve on http://localhost:4180
//
// Then open http://localhost:4180 in a browser. Set PORT to override 4180.

import http from 'http'
import fs from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'
import { WebSocketServer } from 'ws'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const DIST = path.join(__dirname, 'dist')
const PORT = Number(process.env.PORT) || 4180

const MIME = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.json': 'application/json',
}

if (!fs.existsSync(path.join(DIST, 'index.html'))) {
  console.error(`\n  client/dist not found — run "npm run build" first.\n`)
  process.exit(1)
}

const server = http.createServer((req, res) => {
  let url = req.url.split('?')[0]
  if (url === '/') url = '/index.html'
  const file = path.join(DIST, url)
  fs.readFile(file, (err, buf) => {
    if (err) {
      // SPA fallback to index.html
      fs.readFile(path.join(DIST, 'index.html'), (e2, idx) => {
        if (e2) {
          res.writeHead(404)
          res.end('not found')
        } else {
          res.writeHead(200, { 'content-type': 'text/html' })
          res.end(idx)
        }
      })
      return
    }
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] || 'application/octet-stream' })
    res.end(buf)
  })
})

// A fake waterfall line: 512 bins of noise with a few bright "signals".
function waterfallData() {
  const n = 512
  const a = new Uint8Array(n)
  for (let i = 0; i < n; i++) a[i] = 20 + Math.floor(Math.random() * 30)
  for (const c of [90, 180, 300, 410]) {
    for (let k = -2; k <= 2; k++) a[c + k] = 180 + Math.floor(Math.random() * 60)
  }
  return Buffer.from(a).toString('base64')
}

const wss = new WebSocketServer({ server, path: '/ws' })
wss.on('connection', (ws) => {
  const send = (m) => ws.readyState === 1 && ws.send(JSON.stringify(m))

  // Open viewing (no viewer password) so the client streams immediately, then
  // make this client the operator so the full operator UI is visible.
  send({ type: 'hello', needs_viewer_auth: false, callsign: 'W1AW', grid: 'FN31', log_file: 'ft8.adi', rig_host: 'localhost', rig_port: 4532, needs_setup: false, os_type: 'linux', hamlib_available: true })
  send({ type: 'operator_status', operator_client_id: 'me', you_are_operator: true, client_count: 3 })
  send({ type: 'radio_status', connected: true, freq: 14074000, mode: 'USB', ptt: false })

  const now = Math.floor(Date.now() / 1000)
  send({
    type: 'decode',
    period: now,
    messages: [
      { snr: -2, dt: 0.1, freq: 1240, message: 'CQ DX K1ABC FN42' },
      { snr: -11, dt: 0.2, freq: 800, message: 'CQ JA1XYZ PM95' },
      { snr: 5, dt: -0.1, freq: 1580, message: 'W1AW K9QQ EM69' },
      { snr: -18, dt: 0.4, freq: 2100, message: 'K1ABC W1AW R-12' },
      { snr: -7, dt: 0.0, freq: 1000, message: 'CQ POTA W5XYZ EL29' },
      { snr: 12, dt: 0.1, freq: 600, message: 'VE3ABC F4XYZ -05' },
      { snr: -21, dt: 0.5, freq: 2400, message: 'DL1XX EA5YY JN01' },
    ],
  })
  send({
    type: 'qso_update',
    state: { state: 'in_qso', their_call: 'K1ABC', their_grid: 'FN42', their_report: -12, my_report: -7, step: 'sent_roger_report', tx_freq: 1240 },
    next_tx: 'K1ABC W1AW RR73',
    tx_enabled: true,
    tx_queued: true,
  })

  const wf = setInterval(() => send({ type: 'waterfall', timestamp: Date.now(), freq_min: 0, freq_max: 3000, data: waterfallData() }), 100)
  // Toggle PTT every few seconds so the TX / QUEUED / RX lamp cycles.
  let ptt = false
  const pttT = setInterval(() => {
    ptt = !ptt
    send({ type: 'radio_status', connected: true, freq: 14074000, mode: 'USB', ptt })
  }, 4000)

  ws.on('close', () => {
    clearInterval(wf)
    clearInterval(pttT)
  })
})

server.listen(PORT, () => {
  console.log(`\n  Mock BetterFT8 server → http://localhost:${PORT}\n  (serving client/dist + a fake /ws feed; Ctrl-C to stop)\n`)
})
