import QtQuick
import Quickshell
import Quickshell.Io

// Process and file bridge for the Cycle engine. Paths and file bodies travel as
// arguments or environment values. The shell text itself is fixed.
Item {
  id: root
  property var api: null
  property var onMinute: null
  property var queue: []
  property var longQueue: []
  property bool busy: false
  property bool longBusy: false
  property var current: null
  property var longCurrent: null

  function pluginRootPath() {
    var fromEnv = Quickshell.env("CYCLE_ROOT")
    if (fromEnv && fromEnv.length) return fromEnv
    return Quickshell.env("HOME") + "/.config/omarchy/plugins/emrgim.cycle"
  }

  function b64decode(input, maxBytes) {
    var chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    var str = String(input || "").replace(/\s/g, "")
    var cap = (typeof maxBytes === "number" && maxBytes > 0) ? maxBytes : (48 * 1024 * 1024)
    if (str.length > (Math.floor(cap / 3) + 1) * 4 + 4) return null
    var room = Math.floor(str.length * 3 / 4)
    if (room > cap) room = cap
    var out = (typeof Uint8Array === "function") ? new Uint8Array(room) : new Array(room)
    var n = 0
    var buffer = 0
    var bits = 0
    for (var i = 0; i < str.length; i++) {
      if (str.charAt(i) === "=") break
      var val = chars.indexOf(str.charAt(i))
      if (val < 0) continue
      buffer = (buffer << 6) | val
      bits += 6
      if (bits >= 8) {
        bits -= 8
        if (n >= cap || n >= out.length) return null
        out[n++] = (buffer >> bits) & 255
      }
    }
    if (n === out.length) return out
    if (out.subarray) return out.subarray(0, n)
    out.length = n
    return out
  }

  function startTimer() { bootTimer.start() }

  function httpsUrl(url) {
    var u = String(url || "")
    if (u.length < 8 || u.slice(0, 8).toLowerCase() !== "https://") return false
    for (var i = 0; i < u.length; i++) {
      var c = u.charCodeAt(i)
      if (c <= 32 || c === 127) return false
    }
    return true
  }

  // Fallback only when curl is missing. xhr.timeout does not fire in this shell; a Timer calls abort. readyState 3 and progress abort once the body passes the cap, and Content-Length is refused before the body is read.
  function cappedXhr(url, timeoutSec, maxBytes) {
    return new Promise(function(resolve) {
      if (!root.httpsUrl(url)) {
        resolve({ ok: false, text: "", error: "https required" })
        return
      }
      var cap = maxBytes > 0 ? maxBytes : 8192
      var xhr = new XMLHttpRequest()
      var settled = false
      var timer = Qt.createQmlObject("import QtQuick; Timer {}", root)
      function finish(result) {
        if (settled) return
        settled = true
        timer.stop()
        timer.destroy()
        resolve(result)
      }
      timer.interval = Math.max(1, timeoutSec || 8) * 1000
      timer.repeat = false
      timer.triggered.connect(function() {
        try { xhr.abort() } catch (e) {}
        finish({ ok: false, text: "", error: "timeout" })
      })
      xhr.open("GET", url)
      xhr.setRequestHeader("User-Agent", "Cycle/2.0.4")
      xhr.onreadystatechange = function() {
        if (xhr.readyState === 2) {
          var len = parseInt(xhr.getResponseHeader("Content-Length") || "", 10)
          if (!isNaN(len) && len > cap) {
            xhr.abort()
            finish({ ok: false, text: "", error: "too large" })
            return
          }
        }
        if (xhr.readyState === 3 || xhr.readyState === 4) {
          var body = xhr.responseText || ""
          if (body.length > cap) {
            xhr.abort()
            finish({ ok: false, text: "", error: "too large" })
            return
          }
        }
        if (xhr.readyState === 4 && !settled) {
          if (xhr.status >= 200 && xhr.status < 300) finish({ ok: true, text: xhr.responseText || "", error: "" })
          else finish({ ok: false, text: "", error: "HTTP " + xhr.status })
        }
      }
      xhr.onprogress = function() {
        var got = xhr.responseText || ""
        if (got.length > cap) {
          xhr.abort()
          finish({ ok: false, text: "", error: "too large" })
        }
      }
      xhr.onerror = function() { finish({ ok: false, text: "", error: "network" }) }
      timer.start()
      xhr.send()
    })
  }

  function fetchCapped(url, timeoutSec, maxBytes) {
    if (!root.httpsUrl(url)) return Promise.resolve({ ok: false, text: "", error: "https required" })
    var cap = maxBytes > 0 ? maxBytes : 8192
    var timeout = timeoutSec > 0 ? timeoutSec : 8
    var script = "set -o pipefail; curl --proto =https --proto-redir =https --max-time \"$1\" --max-filesize \"$2\" --fail --silent --show-error --location --max-redirs 2 --user-agent Cycle/2.0.4 --output - -- \"$3\" | head -c \"$2\""
    return root.enqueue(["bash", "-c", script, "bash", String(timeout), String(cap), String(url)], timeout + 2, null, false).then(function(r) {
      if (r.code === 127) return root.cappedXhr(url, timeout, cap)
      var text = r.out || ""
      if (text.length > cap) text = text.slice(0, cap)
      if (text.length >= cap) return { ok: false, text: "", error: "too large" }
      if (r.code !== 0) return { ok: false, text: "", error: String(r.err || "").trim() || ("curl " + r.code) }
      return { ok: true, text: text, error: "" }
    })
  }

  function enqueue(args, timeout, env, longJob) {
    return new Promise(function(resolve) {
      var job = { args: args, timeout: timeout || 10, env: env, resolve: resolve }
      if (longJob) { root.longQueue.push(job); root.pumpLong() }
      else { root.queue.push(job); root.pump() }
    })
  }

  function pump() {
    if (root.busy || !root.queue.length) return
    root.busy = true
    root.current = root.queue.shift()
    launch(proc, procOut, procErr, procTimer, root.current)
  }
  function pumpLong() {
    if (root.longBusy || !root.longQueue.length) return
    root.longBusy = true
    root.longCurrent = root.longQueue.shift()
    launch(longProc, longOut, longErr, longTimer, root.longCurrent)
  }
  function launch(target, out, err, timer, job) {
    if (out.clear) out.clear()
    if (err.clear) err.clear()
    target.command = job.args
    target.environment = job.env || ({})
    timer.interval = Math.max(1, job.timeout) * 1000
    timer.start()
    target.running = true
  }
  function finishShort(code) {
    procTimer.stop()
    var job = root.current
    root.current = null
    root.busy = false
    if (job) job.resolve({ code: code, out: procOut.text || "", err: procErr.text || "" })
    root.pump()
  }
  function finishLong(code) {
    longTimer.stop()
    var job = root.longCurrent
    root.longCurrent = null
    root.longBusy = false
    if (job) job.resolve({ code: code, out: longOut.text || "", err: longErr.text || "" })
    root.pumpLong()
  }

  Timer { id: bootTimer; interval: 5000; onTriggered: root.align() }
  Timer { id: alignTimer; onTriggered: { if (root.onMinute) root.onMinute(); minuteTimer.start() } }
  Timer { id: minuteTimer; interval: 60000; repeat: true; onTriggered: if (root.onMinute) root.onMinute() }
  function align() {
    if (root.onMinute) root.onMinute()
    var n = new Date()
    var ms = (60 - n.getSeconds()) * 1000 - n.getMilliseconds()
    if (ms < 200) ms += 60000
    alignTimer.interval = ms
    alignTimer.start()
  }

  Process {
    id: proc
    stdout: StdioCollector { id: procOut }
    stderr: StdioCollector { id: procErr }
    onExited: function(code) { root.finishShort(code) }
  }
  Process {
    id: longProc
    stdout: StdioCollector { id: longOut }
    stderr: StdioCollector { id: longErr }
    onExited: function(code) { root.finishLong(code) }
  }
  Timer {
    id: procTimer
    onTriggered: { if (proc.running) proc.signal(15); procKill.start() }
  }
  Timer {
    id: procKill
    interval: 2000
    onTriggered: if (proc.running) proc.signal(9)
  }
  Timer {
    id: longTimer
    onTriggered: { if (longProc.running) longProc.signal(15); longKill.start() }
  }
  Timer {
    id: longKill
    interval: 2000
    onTriggered: if (longProc.running) longProc.signal(9)
  }

  Component.onCompleted: {
    root.api = {
      home: Quickshell.env("HOME") || "/root",
      pluginRoot: root.pluginRootPath(),
      linkCli: true,
      pid: 0,
      env: function(name) { return Quickshell.env(name) || "" },
      setEnv: function(name, value) { root.api._env = root.api._env || ({}); root.api._env[name] = value },
      now: function() {
        var d = new Date()
        return {
          y: d.getFullYear(), mo: d.getMonth() + 1, d: d.getDate(),
          h: d.getHours(), mi: d.getMinutes(), s: d.getSeconds(), ms: d.getMilliseconds(),
          tzHours: -d.getTimezoneOffset() / 60,
          weekday: (d.getDay() + 6) % 7
        }
      },
      selfPid: async function() {
        var r = await root.enqueue(["sh", "-c", "printf %s \"$PPID\""], 5, null, false)
        var n = parseInt(String(r.out || "").trim(), 10)
        if (n) root.api.pid = n
        return root.api.pid
      },
      readFile: async function(p) {
        var r = await root.enqueue(["sh", "-c", "cat -- \"$1\"", "sh", p], 20, null, false)
        return r.code === 0 ? r.out : null
      },
      writeFile: async function(p, body) {
        var r = await root.enqueue(["sh", "-c", "mkdir -p \"$(dirname \"$CYCLE_PATH\")\" && printf '%s' \"$CYCLE_BODY\" > \"$CYCLE_PATH.tmp\" && mv -f \"$CYCLE_PATH.tmp\" \"$CYCLE_PATH\"", "sh"], 20, { CYCLE_PATH: p, CYCLE_BODY: String(body) }, false)
        return r.code === 0
      },
      removeFile: async function(p) {
        await root.enqueue(["rm", "-f", "--", p], 10, null, false)
        return true
      },
      exists: async function(p) {
        var r = await root.enqueue(["sh", "-c", "test -e \"$1\"", "sh", p], 5, null, false)
        return r.code === 0
      },
      listDir: async function(p) {
        var r = await root.enqueue(["find", p, "-mindepth", "1", "-maxdepth", "1", "-printf", "%y\\t%f\\n"], 10, null, false)
        if (r.code !== 0) return []
        var lines = String(r.out || "").split("\n")
        var out = []
        for (var i = 0; i < lines.length; i++) {
          if (!lines[i]) continue
          var tab = lines[i].indexOf("\t")
          if (tab < 0) continue
          out.push({ name: lines[i].slice(tab + 1), isFile: lines[i].charAt(0) === "f" })
        }
        return out
      },
      mkdirp: async function(p) {
        var r = await root.enqueue(["mkdir", "-p", "--", p], 10, null, false)
        return r.code === 0
      },
      copyFile: async function(src, dst) {
        var r = await root.enqueue(["cp", "-f", "--", src, dst], 30, null, false)
        return r.code === 0
      },
      rename: async function(src, dst) {
        var r = await root.enqueue(["mv", "-f", "--", src, dst], 10, null, false)
        return r.code === 0
      },
      readlink: async function(p) {
        var r = await root.enqueue(["readlink", "-n", "--", p], 5, null, false)
        return r.code === 0 ? String(r.out) : null
      },
      symlink: async function(src, dst) {
        var r = await root.enqueue(["ln", "-s", "--", src, dst], 5, null, false)
        return r.code === 0
      },
      createNew: async function(p, body) {
        var r = await root.enqueue(["sh", "-c", "set -C; printf %s \"$2\" > \"$1\"", "sh", p, String(body)], 5, null, false)
        return r.code === 0
      },
      pidAlive: async function(pid) {
        var r = await root.enqueue(["sh", "-c", "test -d \"/proc/$1\"", "sh", String(pid)], 5, null, false)
        return r.code === 0
      },
      mtime: async function(p) {
        var r = await root.enqueue(["stat", "-c", "%Y", "--", p], 5, null, false)
        if (r.code !== 0) return null
        var sec = parseInt(String(r.out).trim(), 10)
        return isNaN(sec) ? null : sec * 1000
      },
      readBytes: async function(p, maxBytes) {
        var cap = (typeof maxBytes === "number" && maxBytes > 0) ? maxBytes : (48 * 1024 * 1024)
        var st = await root.enqueue(["stat", "-c", "%s", "--", p], 5, null, false)
        if (st.code !== 0) return null
        var n = parseInt(String(st.out || "").trim(), 10)
        if (!(n > 0) || n > cap) return null
        var r = await root.enqueue(["base64", "-w", "0", "--", p], 60, null, false)
        if (r.code !== 0) return null
        var bytes = root.b64decode(r.out || "", cap)
        if (!bytes || bytes.length > cap) return null
        return bytes
      },
      exec: function(args, timeout, env) { return root.enqueue(args, timeout, env, false) },
      execLong: function(args, timeout, env) { return root.enqueue(args, timeout, env, true) },
      spawnDetached: function(args) {
        try { Quickshell.execDetached(args); return Promise.resolve(true) }
        catch (e) { return Promise.resolve(false) }
      },
      fetchText: function(url, timeoutSec, maxBytes) { return root.fetchCapped(url, timeoutSec, maxBytes) },
      sleep: function(ms) {
        return new Promise(function(resolve) {
          var t = Qt.createQmlObject("import QtQuick; Timer {}", root)
          t.interval = Math.max(1, ms)
          t.repeat = false
          t.triggered.connect(function() { resolve(); t.destroy() })
          t.start()
        })
      },
      log: function(line) { console.log(String(line)) },
      logErr: function(line) { console.log(String(line)) },
      startTimer: function() { root.startTimer() }
    }
    var baseEnv = root.api.env
    root.api.env = function(name) {
      if (root.api._env && Object.prototype.hasOwnProperty.call(root.api._env, name)) return root.api._env[name] || ""
      return baseEnv(name)
    }
  }
}
