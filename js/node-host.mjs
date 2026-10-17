import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";

function run(args, timeoutSec, env) {
  return new Promise(function(resolve) {
    var child;
    var out = "";
    var err = "";
    var done = false;
    function finish(code, extra) {
      if (done) return;
      done = true;
      clearTimeout(timer);
      clearTimeout(killTimer);
      resolve({ code: code == null ? 1 : code, out: out, err: extra ? err + extra : err });
    }
    try {
      child = spawn(args[0], args.slice(1), {
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
        env: Object.assign({}, process.env, env || {}),
      });
    } catch (e) {
      resolve({ code: 127, out: "", err: String(e && e.message || e) });
      return;
    }
    var killTimer = null;
    var timer = setTimeout(function() {
      try { if (child.pid) process.kill(-child.pid, "SIGTERM"); } catch (e) {}
      killTimer = setTimeout(function() {
        try { if (child.pid) process.kill(-child.pid, "SIGKILL"); } catch (e2) {}
      }, 2000);
      finish(124, "timed out after " + (timeoutSec || 10) + "s");
    }, Math.max(1, timeoutSec || 10) * 1000);
    child.stdout.on("data", function(d) { out += d.toString(); });
    child.stderr.on("data", function(d) { err += d.toString(); });
    child.on("error", function(e) { finish(127, String(e && e.message || e)); });
    child.on("close", function(code) { finish(code == null ? 1 : code); });
  });
}

export function createNodeHost(opts) {
  opts = opts || {};
  var host = {
    home: opts.home || process.env.HOME || "/root",
    pluginRoot: opts.pluginRoot,
    linkCli: !!opts.linkCli,
    pid: process.pid,
    env: function(name) { return process.env[name] || ""; },
    setEnv: function(name, value) { process.env[name] = value; },
    now: function() {
      var d = new Date();
      return {
        y: d.getFullYear(), mo: d.getMonth() + 1, d: d.getDate(),
        h: d.getHours(), mi: d.getMinutes(), s: d.getSeconds(), ms: d.getMilliseconds(),
        tzHours: -d.getTimezoneOffset() / 60,
        weekday: (d.getDay() + 6) % 7,
      };
    },
    readFile: async function(p) {
      try { return fs.readFileSync(p, "utf8"); } catch (e) { return null; }
    },
    writeFile: async function(p, body) {
      fs.mkdirSync(path.dirname(p), { recursive: true });
      var tmp = p + ".tmp";
      fs.writeFileSync(tmp, body);
      fs.renameSync(tmp, p);
      return true;
    },
    removeFile: async function(p) { try { fs.rmSync(p, { force: true }); } catch (e) {} return true; },
    exists: async function(p) { try { return fs.existsSync(p); } catch (e) { return false; } },
    listDir: async function(p) {
      try {
        return fs.readdirSync(p, { withFileTypes: true }).map(function(d) { return { name: d.name, isFile: d.isFile() }; });
      } catch (e) { return []; }
    },
    mkdirp: async function(p) { fs.mkdirSync(p, { recursive: true }); return true; },
    copyFile: async function(src, dst) { fs.mkdirSync(path.dirname(dst), { recursive: true }); fs.copyFileSync(src, dst); return true; },
    rename: async function(src, dst) { fs.renameSync(src, dst); return true; },
    readlink: async function(p) { try { return fs.readlinkSync(p); } catch (e) { return null; } },
    symlink: async function(src, dst) { fs.symlinkSync(src, dst); return true; },
    createNew: async function(p, body) {
      try {
        var fd = fs.openSync(p, "wx");
        fs.writeSync(fd, String(body));
        fs.closeSync(fd);
        return true;
      } catch (e) { return false; }
    },
    pidAlive: async function(pid) { return fs.existsSync("/proc/" + pid); },
    mtime: async function(p) { try { return fs.statSync(p).mtimeMs; } catch (e) { return null; } },
    readBytes: async function(p) { try { return fs.readFileSync(p); } catch (e) { return null; } },
    exec: function(args, timeout, env) { return run(args, timeout, env); },
    execLong: function(args, timeout, env) { return run(args, timeout, env); },
    spawnDetached: async function(args) {
      try {
        var child = spawn(args[0], args.slice(1), { detached: true, stdio: "ignore" });
        child.unref();
        return true;
      } catch (e) { return false; }
    },
    fetchText: async function(url, timeoutSec) {
      try {
        var res = await fetch(url, {
          signal: AbortSignal.timeout(Math.max(1, timeoutSec || 30) * 1000),
          headers: { "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) Cycle/2" },
        });
        var text = await res.text();
        if (!res.ok) return { ok: false, text: "", error: "HTTP " + res.status };
        return { ok: true, text: text, error: "" };
      } catch (e) {
        return { ok: false, text: "", error: String(e && e.message || e) };
      }
    },
    sleep: function(ms) { return new Promise(function(resolve) { setTimeout(resolve, ms); }); },
    log: function(line) { console.log(line); },
    logErr: function(line) { console.error(line); },
    startTimer: function() {},
  };
  return host;
}
