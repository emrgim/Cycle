import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";
import { loadEngine } from "../js/load-engine.mjs";
import { createNodeHost } from "../js/node-host.mjs";

var root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
var loaded = loadEngine();
var E = loaded.engine;

function chunk(type, data, declared) {
  var body = Buffer.from(data);
  var use = declared == null ? body.length : declared >>> 0;
  var len = Buffer.alloc(4);
  len.writeUInt32BE(use);
  var kind = Buffer.from(type);
  var crc = zlib.crc32(Buffer.concat([kind, body]));
  var cbuf = Buffer.alloc(4);
  cbuf.writeUInt32BE(crc >>> 0);
  return Buffer.concat([len, kind, body, cbuf]);
}
function ihdr(w, h, colorType, bitDepth) {
  var body = Buffer.alloc(13);
  body.writeUInt32BE(w >>> 0, 0);
  body.writeUInt32BE(h >>> 0, 4);
  body[8] = bitDepth == null ? 8 : bitDepth;
  body[9] = colorType == null ? 2 : colorType;
  return body;
}
function wrap(parts) {
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])].concat(parts));
}
function pngFromRaw(w, h, raw, colorType) {
  return wrap([
    chunk("IHDR", ihdr(w, h, colorType == null ? 2 : colorType)),
    chunk("IDAT", zlib.deflateSync(Buffer.from(raw))),
    chunk("IEND", Buffer.alloc(0))
  ]);
}
function timedThrow(fn) {
  var t = Date.now();
  var err = null;
  try { fn(); } catch (e) { err = e; }
  return { err: err, ms: Date.now() - t };
}
function same(actual, expected) {
  assert.equal(JSON.stringify(actual), JSON.stringify(expected));
}
function mustThrow(label, fn, pattern) {
  var hit = timedThrow(fn);
  assert.ok(hit.err, label + " should fail");
  assert.match(String(hit.err.message || hit.err), pattern, label + " (" + (hit.err && hit.err.message) + ")");
  assert.ok(hit.ms < 1000, label + " took " + hit.ms + "ms");
}

var sig = [137, 80, 78, 71, 13, 10, 26, 10];
var tiny = pngFromRaw(2, 2, [0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0]);
var image = E.decodePngBytes(tiny);
same(image.rgb, [[255, 0, 0], [0, 0, 255], [0, 255, 255], [255, 0, 0]]);
var asArray = Array.from(tiny);
assert.equal(asArray.subarray, undefined);
same(E.decodePngBytes(asArray).rgb, image.rgb);
assert.equal(E.paletteFromBytes(tiny), E.paletteFromBytes(asArray));
assert.equal(E.paletteFromBytes(new Uint8Array(tiny)), E.paletteFromBytes(tiny));

var rgba = pngFromRaw(2, 1, [0, 10, 20, 30, 255, 40, 50, 60, 128], 6);
same(E.decodePngBytes(rgba).rgb, [[10, 20, 30], [40, 50, 60]]);
same(E.decodePngBytes(pngFromRaw(2, 1, [1, 10, 20, 30, 5, 5, 5])).rgb, [[10, 20, 30], [15, 25, 35]]);
same(E.decodePngBytes(pngFromRaw(1, 2, [0, 10, 20, 30, 2, 1, 2, 3])).rgb, [[10, 20, 30], [11, 22, 33]]);
same(E.decodePngBytes(pngFromRaw(1, 2, [0, 8, 9, 10, 4, 1, 1, 1])).rgb, [[8, 9, 10], [9, 10, 11]]);
same(E.decodePngBytes(pngFromRaw(1, 1, [3, 7, 8, 9])).rgb, [[7, 8, 9]]);

var body = zlib.deflateSync(Buffer.from([0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0]));
var split = wrap([
  chunk("IHDR", ihdr(2, 2)),
  chunk("tEXt", Buffer.from("Comment\0hi")),
  chunk("IDAT", body.subarray(0, 3)),
  chunk("IDAT", body.subarray(3)),
  chunk("IEND", Buffer.alloc(0))
]);
same(E.decodePngBytes(split).rgb, image.rgb);
var storedRaw = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0];
var stored = pngFromRaw(4, 2, storedRaw);
var storedPng = wrap([
  chunk("IHDR", ihdr(4, 2)),
  chunk("IDAT", zlib.deflateSync(Buffer.from(storedRaw), { level: 0 })),
  chunk("IEND", Buffer.alloc(0))
]);
same(E.decodePngBytes(storedPng).rgb, E.decodePngBytes(stored).rgb);
assert.equal(E.paletteFromBytes(storedPng), E.paletteFromBytes(stored));

var W = 300, H = 280, raw = [];
for (var y = 0; y < H; y++) {
  raw.push(1);
  var pr = 0, pg = 0, pb = 0;
  for (var x = 0; x < W; x++) {
    var r = (x * 5 + y) & 255, g = (y * 3) & 255, b = (x * 2) & 255;
    raw.push((r - pr) & 255, (g - pg) & 255, (b - pb) & 255);
    pr = r; pg = g; pb = b;
  }
}
var wide = pngFromRaw(W, H, raw);
var wideImage = E.decodePngBytes(wide);
same(wideImage.rgb[0], [0, 0, 0]);
same(wideImage.rgb[wideImage.rgb.length - 1], [238, 69, 86]);
same(wideImage.rgb[140 * 300 + 150], [122, 164, 44]);
assert.equal(E.paletteFromBytes(wide), [
  "mode = \"dark\"",
  "",
  "accent = \"#d98182\"",
  "selection = \"#5e3030\"",
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
  "red = \"#d98181\"",
  "yellow = \"#d9c681\"",
  "green = \"#83d981\"",
  "cyan = \"#81d9d9\"",
  "blue = \"#81a4d9\"",
  "magenta = \"#d781d9\"",
  "",
  "bright_red = \"#e5a8a8\"",
  "bright_yellow = \"#e5d7a8\"",
  "bright_green = \"#a9e5a8\"",
  "bright_cyan = \"#a8e5e5\"",
  "bright_blue = \"#a8c0e5\"",
  "bright_magenta = \"#e3a8e5\"",
  ""
].join("\n"));

mustThrow("oversized declared length", function() {
  var lied = chunk("IDAT", Buffer.from([1, 2, 3, 4]), 0x0fffffff);
  E.decodePngBytes(wrap([chunk("IHDR", ihdr(8, 8)), lied]));
}, /truncated/);

mustThrow("truncated chunk", function() {
  var partial = Buffer.concat([
    Buffer.from(sig),
    chunk("IHDR", ihdr(8, 8)).subarray(0, 20)
  ]);
  E.decodePngBytes(partial);
}, /truncated|not png|png size/);

mustThrow("oversized dimensions", function() {
  E.decodePngBytes(wrap([
    chunk("IHDR", ihdr(100000, 100000)),
    chunk("IDAT", zlib.deflateSync(Buffer.from([0, 0, 0, 0]))),
    chunk("IEND", Buffer.alloc(0))
  ]));
}, /dimensions/);

mustThrow("zero dimensions", function() {
  E.decodePngBytes(wrap([
    chunk("IHDR", ihdr(0, 1)),
    chunk("IDAT", zlib.deflateSync(Buffer.from([0]))),
    chunk("IEND", Buffer.alloc(0))
  ]));
}, /dimensions/);

mustThrow("file larger than the cap", function() {
  E.paletteFromBytes({ length: E.PNG_MAX_BYTES + 1 });
}, /png size/);

mustThrow("unsupported bit depth does not inflate", function() {
  E.decodePngBytes(wrap([
    chunk("IHDR", ihdr(8, 8, 2, 16)),
    chunk("IDAT", zlib.deflateSync(Buffer.alloc(64))),
    chunk("IEND", Buffer.alloc(0))
  ]));
}, /unsupported png/);

function BitWriter() {
  this.buf = 0;
  this.cnt = 0;
  this.bytes = [];
}
BitWriter.prototype.bit = function(b) {
  this.buf |= (b & 1) << this.cnt;
  this.cnt++;
  if (this.cnt === 8) {
    this.bytes.push(this.buf);
    this.buf = 0;
    this.cnt = 0;
  }
};
BitWriter.prototype.bits = function(value, n) {
  for (var i = 0; i < n; i++) this.bit((value >> i) & 1);
};
BitWriter.prototype.code = function(code, len) {
  for (var i = len - 1; i >= 0; i--) this.bit((code >> i) & 1);
};
BitWriter.prototype.finish = function() {
  if (this.cnt) this.bytes.push(this.buf);
  return Buffer.from(this.bytes);
};
function fixedLitCode(sym) {
  if (sym <= 143) return [48 + sym, 8];
  if (sym <= 255) return [400 + (sym - 144), 9];
  if (sym <= 279) return [sym - 256, 7];
  return [192 + (sym - 280), 8];
}
function deflateBomb(repeats) {
  var bw = new BitWriter();
  bw.bit(1);
  bw.bits(1, 2);
  var lit0 = fixedLitCode(0);
  bw.code(lit0[0], lit0[1]);
  var lenSym = fixedLitCode(285);
  for (var i = 0; i < repeats; i++) {
    bw.code(lenSym[0], lenSym[1]);
    bw.code(0, 5);
  }
  var eob = fixedLitCode(256);
  bw.code(eob[0], eob[1]);
  return bw.finish();
}
var bombRaw = deflateBomb(100000);
var bombZlib = Buffer.concat([Buffer.from([0x78, 0x01]), bombRaw, Buffer.alloc(4)]);
var bombPng = wrap([
  chunk("IHDR", ihdr(8, 8)),
  chunk("IDAT", bombZlib),
  chunk("IEND", Buffer.alloc(0))
]);
assert.ok(bombPng.length < 200000, "bomb fixture should stay small");
mustThrow("deflate bomb", function() { E.decodePngBytes(bombPng); }, /inflate limit/);
mustThrow("deflate bomb palette", function() { E.paletteFromBytes(bombPng); }, /inflate limit/);

var dir = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-png-"));
var host = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: dir });
host.home = dir;
var small = path.join(dir, "small.png");
fs.writeFileSync(small, tiny);
var read = await host.readBytes(small);
assert.equal(Buffer.compare(Buffer.from(read), tiny), 0);
assert.equal(await host.readBytes(small, 32), null);
assert.equal(await host.readBytes(path.join(dir, "missing.png")), null);
var huge = path.join(dir, "huge.bin");
fs.writeFileSync(huge, Buffer.alloc(64));
assert.equal(await host.readBytes(huge, 32), null);

var kept = "mode = \"dark\"\naccent = \"#112233\"\n";
async function themeHost(dirName) {
  var home = fs.mkdtempSync(path.join(os.tmpdir(), dirName));
  var h = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: home });
  h.home = home;
  h.fetchText = async function() { return { ok: false, text: "", error: "offline" }; };
  var notes = [];
  h.log = function(line) { notes.push(String(line)); };
  h.logErr = h.log;
  E.setHost(h);
  await E.seed();
  return { home: home, notes: notes };
}
var badHome = await themeHost("cycle-bad-");
var badPng = path.join(badHome.home, "bad.png");
fs.writeFileSync(badPng, bombPng);
await E.updateSettings(function(s) { s.generator.command = "cp " + badPng + " {out}"; });
var theme = path.join(badHome.home, ".config/omarchy/themes/cycle");
fs.mkdirSync(theme, { recursive: true });
fs.writeFileSync(path.join(theme, "colors.toml"), kept);
var started = Date.now();
await E.generateCycleTheme({ phase: "day", condition: "clear sky", day_type: "weekday", location: "Test", temperature: 20 }, false, true);
assert.ok(Date.now() - started < 5000, "bad wallpaper stalled palette extraction");
assert.equal(fs.readFileSync(path.join(theme, "colors.toml"), "utf8"), kept);
assert.ok(badHome.notes.some(function(line) { return line.indexOf("palette failed") >= 0; }));

var goodHome = await themeHost("cycle-good-");
var goodPng = path.join(goodHome.home, "good.png");
fs.writeFileSync(goodPng, tiny);
await E.updateSettings(function(s) { s.generator.command = "cp " + goodPng + " {out}"; });
var goodTheme = path.join(goodHome.home, ".config/omarchy/themes/cycle");
fs.mkdirSync(goodTheme, { recursive: true });
fs.writeFileSync(path.join(goodTheme, "colors.toml"), kept);
await E.generateCycleTheme({ phase: "day", condition: "clear sky", day_type: "weekday", location: "Test", temperature: 21 }, false, true);
var written = fs.readFileSync(path.join(goodTheme, "colors.toml"), "utf8");
assert.match(written, /^# generated by Cycle /);
assert.ok(written.indexOf("accent = \"#ea7070\"") >= 0);

var engineSrc = fs.readFileSync(path.join(root, "js/engine.js"), "utf8");
assert.equal(engineSrc.indexOf("idat.push"), -1);
assert.equal(JSON.parse(fs.readFileSync(path.join(root, "manifest.json"), "utf8")).version, "2.0.3");
for (var file of ["EngineHost.qml", "js/node-host.mjs", "js/engine.js", "manifest.json"]) {
  assert.equal(fs.readFileSync(path.join(root, file), "utf8").indexOf("2.0.2"), -1, file);
}

console.log("png bounds tests passed");
