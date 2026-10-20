import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { loadEngine } from "../js/load-engine.mjs";
import { createNodeHost } from "../js/node-host.mjs";

var loaded = loadEngine();
var E = loaded.engine;
var tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-"));
var host = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: tmp });
host.home = tmp;
E.setHost(host);

function plain(v) { return JSON.parse(JSON.stringify(v)); }
function eq(actual, expected, msg) { assert.deepEqual(plain(actual), expected, msg); }
function dt(h, mi, s) { return { y: 2026, mo: 10, d: 15, h: h, mi: mi, s: s || 0 }; }

var calls = E.parseCalls("light: /sun and /weather/city/Genova/tomorrow, then /news.");
eq(calls.map(function(c) { return c.name; }), ["sun", "weather", "news"]);
eq(calls[1].args, ["city", "Genova", "tomorrow"]);
eq("light: /sun and /weather/city/Genova/tomorrow, then /news.".slice(calls[1].start, calls[1].end), "/weather/city/Genova/tomorrow");
eq(E.parseCalls("a 16:9 image at ~/Pictures/x.png or http://a/b"), []);
eq(E.parseCalls("{{weather}} x")[0].name, "weather");
eq(E.parseCalls("{{weather}} x")[0].args, []);

function val(t) { return E.validateCall(E.parseCalls(t)[0], 1); }
eq(val("/weather/city/New_York/tomorrow").opts, { city: "New York", query: null, day: 1 });
eq(val("/sunrise/yesterday").opts.day, -1);
assert.equal(val("/weather/city").ok, false);
assert.equal(val("/weather/banana").ok, false);
assert.equal(val("/phase/tomorrow").ok, false);
assert.equal(val("/bogus").ok, false);
assert.equal(val("/image1").ok, true);
assert.equal(E.validateCall(E.parseCalls("/image2")[0], 1).ok, false);
eq(val("/news/query/expo_city").opts.query, "expo city");

assert.ok(E.completeCall("/wea", []).indexOf("weather") >= 0);
eq(E.completeCall("/weather/ci", []), ["city"]);
eq(E.completeCall("/weather/city/Gen", []), []);
var next = E.completeCall("/weather/city/Genova/", []);
assert.ok(next.indexOf("tomorrow") >= 0 && next.indexOf("city") < 0);
eq(E.completeCall("/phase/", []), []);
eq(E.completeCall("/news/", []), ["city", "query"]);
eq(E.promptErrors("/sun /nope /weather/x", 0).length, 2);

eq(E.median([3, 1, 2]), 2);
eq(E.median([1, 2, 3, 4]), 3);

var all = function() { return true; };
eq(E.nextMark(dt(14, 7, 30), 15, null, all), dt(14, 15, 0));
eq(E.nextMark(dt(14, 31, 0), 30, null, all), dt(15, 0, 0));
eq(E.nextMark(dt(14, 5, 0), 60, null, all), dt(15, 0, 0));
eq(E.nextMark(dt(13, 1, 0), 120, null, all), dt(14, 0, 0));
eq(E.nextMark(dt(13, 1, 0), 180, null, all), dt(15, 0, 0));
eq(E.nextMark(dt(14, 5, 0), 45, null, all), dt(15, 0, 0));
eq(E.nextMark(dt(14, 0, 0), 60, null, all), dt(14, 0, 0));
eq(E.nextMark(dt(14, 0, 20), 60, dt(14, 0, 0), all), dt(15, 0, 0));
eq(E.nextMark(dt(14, 20, 0), 60, dt(14, 0, 0), all), dt(15, 0, 0));
eq(E.leadSecsFrom([], 60), 210);
eq(E.leadSecsFrom([100, 200, 300, 400, 700], 60), 430);
eq(E.leadSecsFrom([900, 900, 900], 15), 840);
var win = function(x) { var m = x.h * 60 + x.mi; return m >= 360 && m < 1096; };
eq(E.nextMark(dt(18, 10, 0), 15, null, win), dt(18, 15, 0));
eq(E.nextMark(dt(18, 16, 0), 15, null, win), { y: 2026, mo: 10, d: 16, h: 6, mi: 0, s: 0 });
eq(E.nextMark(dt(17, 50, 0), 15, null, win), dt(18, 0, 0));

eq(E.buildGenCommand("x {prompt_q} {prompt} {out} {other}"), "x \"$CYCLE_PROMPT\" \"$CYCLE_PROMPT\" \"$CYCLE_OUT\" {other}");
var cleaned = E.cleanFull("Breaking'$(touch /tmp/pwned)'{prompt}`id`\u0007\nnews");
assert.equal(cleaned.indexOf("$"), -1);
assert.equal(cleaned.indexOf("{"), -1);
assert.equal(cleaned.indexOf("`"), -1);
assert.ok(!Array.from(cleaned).some(function(ch) { return ch.charCodeAt(0) < 32; }));

var sun = E.sunEventsAt({ y: 2026, mo: 10, d: 15 }, 25.2048, 55.2708, 4);
eq(sun, {
  civil_dawn: "05:54",
  sunrise: "06:17",
  solar_noon: "12:04",
  sunset: "17:53",
  civil_dusk: "18:16",
  nautical_dusk: "18:43",
  astronomical_dusk: "19:09",
});

var pwn = path.join(tmp, "pwned");
var evil = "'$(touch " + pwn + ")'{prompt}`touch " + pwn + "`;touch " + pwn;
for (var tpl of ["printf %s {prompt_q} > {out}", "printf %s {prompt} > {out}"]) {
  var out = path.join(tmp, "out-" + Math.random().toString(16).slice(2) + ".txt");
  var cmd = E.buildGenCommand(tpl);
  assert.equal(cmd.indexOf(evil), -1);
  var r = await host.exec(["bash", "-c", cmd], 10, { CYCLE_PROMPT: evil, CYCLE_OUT: out });
  assert.equal(r.code, 0, r.err);
  assert.equal(fs.readFileSync(out, "utf8"), evil);
}
assert.equal(fs.existsSync(pwn), false);

process.env.CYCLE_STATE_DIR = path.join(tmp, "state");
process.env.CYCLE_BRIGHTNESS_FILE = path.join(tmp, "bright");
fs.mkdirSync(process.env.CYCLE_STATE_DIR, { recursive: true });
fs.writeFileSync(process.env.CYCLE_BRIGHTNESS_FILE, "50\n");
var msg = await E.brightnessStep("morning", 70, dt(9, 0, 0));
assert.match(msg, /set 70%/);
fs.writeFileSync(process.env.CYCLE_BRIGHTNESS_FILE, "90\n");
var msg2 = await E.brightnessStep("morning", 70, dt(9, 5, 0));
assert.match(msg2, /manual change 70% → 90%/);
var overrides = JSON.parse(fs.readFileSync(path.join(process.env.CYCLE_STATE_DIR, "overrides.json"), "utf8"));
assert.equal(overrides[0].user, 90);
assert.equal(overrides[0].slot, "morning");

var earlyFetches = [];
host.fetchText = async function(url, timeoutSec, maxBytes) {
  earlyFetches.push({ url: url, timeoutSec: timeoutSec, maxBytes: maxBytes });
  return { ok: false, text: "", error: "timeout" };
};
await E.seed();
var settings = await E.settings();
assert.equal(settings.enabled, false);
assert.equal(settings.location.source, "unknown");
assert.equal(settings.location.name, "");
assert.equal(settings.location.lat, null);
assert.equal(E.locationText(settings), "location unknown, set it in Settings");
assert.equal(earlyFetches.length, 1);
assert.equal(earlyFetches[0].url, "https://get.geojs.io/v1/ip/geo.json");
assert.equal(earlyFetches[0].timeoutSec, 4);
assert.equal(earlyFetches[0].maxBytes, 8192);
assert.equal(JSON.stringify(settings).indexOf("Dubai"), -1);
await E.settings();
assert.equal(earlyFetches.length, 1);
assert.equal(fs.existsSync(path.join(tmp, ".config/omarchy/cycle/presets/day.json")), true);
assert.equal(fs.existsSync(path.join(tmp, ".config/omarchy/cycle/styles")), true);
assert.equal(fs.readdirSync(path.join(tmp, ".config/omarchy/cycle/styles")).length, 0);

function chunk(type, data) {
  var body = Buffer.from(data);
  var len = Buffer.alloc(4); len.writeUInt32BE(body.length);
  var kind = Buffer.from(type);
  var crc = zlib.crc32(Buffer.concat([kind, body]));
  var cbuf = Buffer.alloc(4); cbuf.writeUInt32BE(crc >>> 0);
  return Buffer.concat([len, kind, body, cbuf]);
}
var w = 2, h = 2, raw = [];
for (var y = 0; y < h; y++) {
  raw.push(0);
  raw.push(255, 0, 0, 0, 0, 255);
}
var ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
ihdr[8] = 8; ihdr[9] = 2;
var png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(Buffer.from(raw))),
  chunk("IEND", Buffer.alloc(0)),
]);
var image = E.decodePngBytes(png);
eq(image.width, 2);
eq(image.rgb[0], [255, 0, 0]);
eq(image.rgb[1], [0, 0, 255]);
var toml = E.paletteFromBytes(png);
assert.equal(toml, [
  "mode = \"dark\"",
  "",
  "accent = \"#7070ea\"",
  "selection = \"#272767\"",
  "muted = \"#897575\"",
  "",
  "background = \"#1c1111\"",
  "dark_background = \"#160d0d\"",
  "darker_background = \"#0f0909\"",
  "lighter_background = \"#331e1e\"",
  "",
  "foreground = \"#e8e2e2\"",
  "dark_foreground = \"#a38e8e\"",
  "light_foreground = \"#e8e2e2\"",
  "bright_foreground = \"#ffffff\"",
  "",
  "red = \"#ea7070\"",
  "yellow = \"#eacf70\"",
  "green = \"#72ea70\"",
  "cyan = \"#70eaea\"",
  "blue = \"#7099ea\"",
  "magenta = \"#e870ea\"",
  "",
  "bright_red = \"#f09c9c\"",
  "bright_yellow = \"#f0de9c\"",
  "bright_green = \"#9ef09c\"",
  "bright_cyan = \"#9cf0f0\"",
  "bright_blue = \"#9cb9f0\"",
  "bright_magenta = \"#ef9cf0\"",
  ""
].join("\n"));

function geoPayload() {
  return JSON.stringify({
    city: "Genova",
    country: "Italy",
    country_code: "IT",
    latitude: "44.4056",
    longitude: "8.9463",
    timezone: "Europe/Rome",
    ip: "203.0.113.8"
  });
}
async function openHost(fetchImpl) {
  var dir = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-loc-"));
  var h = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: dir });
  h.home = dir;
  var calls = [];
  h.fetchText = async function(url, timeoutSec, maxBytes) {
    calls.push({ url: url, timeoutSec: timeoutSec, maxBytes: maxBytes });
    return fetchImpl(url, timeoutSec, maxBytes);
  };
  E.setHost(h);
  return { dir: dir, host: h, calls: calls };
}
function writeSaved(dir, location, extra) {
  var base = path.join(dir, ".config/omarchy/cycle");
  fs.mkdirSync(base, { recursive: true });
  var body = Object.assign({
    enabled: false,
    location: location,
    feeds: [{ name: "Custom", url: "https://example.com/rss", enabled: true }],
    prompt_here: ["night-here"]
  }, extra || {});
  fs.writeFileSync(path.join(base, "settings.json"), JSON.stringify(body));
}

var fresh = await openHost(async function(url) {
  if (String(url).indexOf("geojs.io") >= 0) return { ok: true, text: geoPayload(), error: "" };
  return { ok: true, text: "{\"current\":{}}", error: "" };
});
await E.seed();
var detected = await E.settings();
assert.equal(fresh.calls.length, 1);
assert.equal(fresh.calls[0].url, "https://get.geojs.io/v1/ip/geo.json");
assert.equal(fresh.calls[0].timeoutSec, 4);
assert.equal(fresh.calls[0].maxBytes, 8192);
assert.equal(detected.location.source, "auto");
assert.equal(detected.location.name, "Genova, Italy");
assert.equal(detected.location.city, "Genova");
assert.equal(detected.location.cc, "IT");
assert.equal(detected.location.lat, 44.4056);
assert.equal(detected.location.lon, 8.9463);
assert.equal(detected.location.tz, "Europe/Rome");
assert.equal(JSON.stringify(detected.location).indexOf("203.0.113.8"), -1);
assert.equal(detected.feeds[0].name, "Google News \u00b7 Genova");
assert.ok(detected.feeds[0].url.indexOf("Genova") >= 0);
assert.equal(JSON.stringify(detected).indexOf("Dubai"), -1);
await E.settings();
await E.seed();
assert.equal(fresh.calls.length, 1, "auto location is not detected again");
process.env.CYCLE_HERE = "1";
await E.settings();
delete process.env.CYCLE_HERE;
assert.equal(fresh.calls.length, 1, "CYCLE_HERE does not refetch");
var live = await E.fetchLive(detected);
eq(live.errors, []);
assert.equal(fresh.calls.some(function(c) { return String(c.url).indexOf("Asia") >= 0 || String(c.url).indexOf("Dubai") >= 0; }), false);
assert.ok(fresh.calls.some(function(c) { return String(c.url).indexOf("timezone=Europe%2FRome") >= 0; }));

var again = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: fresh.dir });
again.home = fresh.dir;
var againCalls = [];
again.fetchText = async function() { againCalls.push(1); return { ok: true, text: geoPayload(), error: "" }; };
E.setHost(again);
await E.seed();
await E.settings();
assert.equal(againCalls.length, 0, "restart keeps the auto location");
assert.equal((await E.settings()).location.name, "Genova, Italy");

async function assertKept(source, name) {
  var box = await openHost(async function() { throw new Error("geolocation must not run"); });
  writeSaved(box.dir, { name: name, city: name, cc: "IT", lat: 41.9, lon: 12.5, tz: "Europe/Rome", source: source });
  process.env.CYCLE_HERE = "1";
  await E.seed();
  var s1 = await E.settings();
  await E.seed();
  var s2 = await E.settings();
  delete process.env.CYCLE_HERE;
  assert.equal(box.calls.length, 0, source + " location was fetched");
  assert.equal(s1.location.name, name);
  assert.equal(s1.location.source, source);
  assert.equal(s1.location.lat, 41.9);
  assert.equal(s2.location.name, name);
  assert.equal(s2.feeds[0].name, "Custom");
  var restarted = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: box.dir });
  restarted.home = box.dir;
  var n = 0;
  restarted.fetchText = async function() { n += 1; return { ok: true, text: geoPayload(), error: "" }; };
  E.setHost(restarted);
  await E.seed();
  assert.equal(n, 0, source + " location changed on restart");
  assert.equal((await E.settings()).location.name, name);
}
await assertKept("manual", "Roma");
await assertKept("fixed", "Roma");

var failed = await openHost(async function() { return { ok: false, text: "", error: "timeout" }; });
await E.seed();
var unknown = await E.settings();
assert.equal(failed.calls.length, 1);
assert.equal(unknown.location.source, "unknown");
assert.equal(unknown.location.lat, null);
assert.equal(unknown.location.lon, null);
assert.equal(E.locationStatus(unknown), "location unknown, set it in Settings");
assert.equal(JSON.stringify(unknown).indexOf("Dubai"), -1);
assert.equal(unknown.feeds[0].url.indexOf("Dubai"), -1);
var cb = E.circadian({ y: 2026, mo: 10, d: 15, h: 12, mi: 0, s: 0 }, unknown, null);
assert.equal(cb[2], null);
var missed = await E.fetchLive(unknown);
eq(missed.errors, ["location unknown, set it in Settings"]);
await E.seed();
await E.settings();
assert.equal(failed.calls.length, 1, "a failed setup is not retried on the next launch");
var failedRestart = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: failed.dir });
failedRestart.home = failed.dir;
var failedN = 0;
failedRestart.fetchText = async function() { failedN += 1; return { ok: false, text: "", error: "timeout" }; };
E.setHost(failedRestart);
await E.seed();
assert.equal(failedN, 0);
assert.equal((await E.settings()).location.source, "unknown");

var broken = await openHost(async function() { return { ok: true, text: "{", error: "" }; });
await E.seed();
assert.equal((await E.settings()).location.source, "unknown");
assert.equal(E.locationText(await E.settings()), "location unknown, set it in Settings");

var blocked = await openHost(async function() { return { ok: true, text: geoPayload(), error: "" }; });
await assert.rejects(function() { return E.getText("http://ip-api.com/json/", 1, 2, 100); }, /https required/);
assert.equal(blocked.calls.length, 0);

console.log("cycle engine tests passed");
