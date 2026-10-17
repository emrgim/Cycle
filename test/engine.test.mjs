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

function eq(actual, expected, msg) { assert.deepEqual(actual, expected, msg); }
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

await E.seed();
var settings = await E.settings();
assert.equal(settings.enabled, false);
assert.equal(settings.location.name, "Dubai");
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
assert.match(toml, /^mode = "(light|dark)"/);
assert.match(toml, /accent = "#[0-9a-f]{6}"/);

console.log("cycle engine tests passed");
