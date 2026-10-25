import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { loadEngine } from "../js/load-engine.mjs";
import { createNodeHost } from "../js/node-host.mjs";

var loaded = loadEngine();
var E = loaded.engine;

function plain(v) { return JSON.parse(JSON.stringify(v)); }
function eq(actual, expected, msg) { assert.deepEqual(plain(actual), expected, msg); }
function dt(h, mi) { return { y: 2026, mo: 10, d: 15, h: h, mi: mi, s: 0 }; }

var home = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-safety-"));
var host = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: home });
host.home = home;
host.fetchText = async function() { return { ok: false, text: "", error: "offline" }; };
var out = [];
var errs = [];
host.log = function(line) { out.push(String(line)); };
host.logErr = function(line) { errs.push(String(line)); };
E.setHost(host);
await E.seed();
var base = path.join(home, ".config/omarchy/cycle");

// Names that become file names.
for (var good of ["day", "night-here", "gits_clean", "a.b", "my prompt", "x".repeat(128)]) assert.equal(E.validName(good), true, good);
var bad = ["", ".", "..", "../x", "a/b", "a\\b", ".hidden", " x", "x ", "a\nb", "a\u0000b", "__proto__", "constructor", "prototype", "x".repeat(129), null, 5, ["day"]];
for (var b of bad) assert.equal(E.validName(b), false, JSON.stringify(b));

// Reading, writing and deleting through a bad name touches nothing.
var outside = path.join(home, ".config/omarchy/x.txt");
assert.equal(await E.preset("../../x"), null);
assert.equal(await E.cycleOf("../../x"), null);
assert.equal(await E.savePreset("../x", { label: "x" }), false);
assert.equal(await E.saveCycle("../x", { label: "x" }), false);
assert.equal(await E.promptText("../x"), "");
assert.equal(await E.savePrompt("../x", "text"), false);
assert.equal(await E.deletePrompt("../x"), false);
assert.equal(await E.setActivePrompt("../x"), false);
assert.equal(fs.existsSync(outside), false);
assert.equal(fs.existsSync(path.join(home, ".config/omarchy/x.json")), false);
assert.equal(fs.existsSync(path.join(base, "styles/active")), false);
assert.equal(await E.savePrompt("ok", "A quiet harbour"), true);
assert.equal(await E.promptText("ok"), "A quiet harbour");
assert.equal(await E.setActivePrompt("ok"), true);
assert.equal(await E.activePrompt(), "ok");
fs.writeFileSync(path.join(base, "styles/active"), "../x\n");
assert.equal(await E.activePrompt(), "gits-classic");
fs.writeFileSync(path.join(base, "styles/.hidden.txt"), "x");
fs.writeFileSync(path.join(base, "styles/old.bak.txt"), "x");
eq(await E.promptNames(), ["ok"]);
await E.saveSchedule("../x", { enabled: true, every_min: 60 });
await E.saveSchedule("ok", { enabled: true, every_min: 60 });
eq(Object.keys((await E.settings()).schedules), ["ok"]);
await E.setTitle("__proto__", "Nope");
await E.setTitle("ok", "Harbour");
eq((await E.settings()).prompt_titles, { ok: "Harbour" });

// Settings keys and ids that name an object's own machinery are ignored.
var settingsFile = path.join(base, "settings.json");
var saved = JSON.parse(fs.readFileSync(settingsFile, "utf8"));
fs.writeFileSync(settingsFile, JSON.stringify(Object.assign({ __proto__: { hold: "evil" }, constructor: { x: 1 }, prototype: 1 }, saved, { enabled: true })));
var s = await E.settings();
assert.equal(s.enabled, true);
assert.equal(s.hold, null);
assert.equal(Object.prototype.hasOwnProperty.call(s, "hold"), true, "defaults stay own properties");
assert.equal(typeof s.constructor, "function");
assert.equal(Object.prototype.hasOwnProperty.call(s, "prototype"), false);
assert.equal(E.titleFrom("constructor", { prompt_titles: {} }), "Constructor");
assert.equal(E.titleFrom("ok", { prompt_titles: { ok: "Nice" } }), "Nice");
assert.equal(E.titleFrom("night-here", {}), "Night Here");

// A generated file name stays inside the generated folder.
fs.writeFileSync(path.join(home, "src.bin"), "not really a png");
await E.updateSettings(function(st) { st.generator.provider = "custom"; st.generator.command = "cp " + path.join(home, "src.bin") + " {out}"; });
var made = await E.generateImage("../../evil", { phase: "day", condition: "clear sky", location: "X", temperature: 1 }, await E.settings());
assert.ok(made && made.indexOf(path.join(home, ".local/share/cycle/generated/") + "-") === 0, made);
assert.equal(fs.readdirSync(path.join(home, ".local/share")).filter(function(n) { return n.indexOf("evil") >= 0; }).length, 0);

// The override log is bounded; the shell host cannot write an unbounded one.
process.env.CYCLE_STATE_DIR = path.join(home, "state");
process.env.CYCLE_BRIGHTNESS_FILE = path.join(home, "bright");
fs.mkdirSync(process.env.CYCLE_STATE_DIR, { recursive: true });
var old = [];
for (var i = 0; i < 1000; i++) old.push({ at: "2020-01-01T00:00:00", slot: "night", base: 40, user: 50 });
fs.writeFileSync(path.join(process.env.CYCLE_STATE_DIR, "overrides.json"), JSON.stringify(old));
fs.writeFileSync(process.env.CYCLE_BRIGHTNESS_FILE, "50\n");
assert.match(await E.brightnessStep("morning", 70, dt(9, 0)), /set 70%/);
fs.writeFileSync(process.env.CYCLE_BRIGHTNESS_FILE, "90\n");
assert.match(await E.brightnessStep("morning", 70, dt(9, 5)), /manual change/);
var log = JSON.parse(fs.readFileSync(path.join(process.env.CYCLE_STATE_DIR, "overrides.json"), "utf8"));
assert.equal(log.length, 1000);
eq(log[999], { at: "2026-10-15T09:05:00", slot: "morning", base: 70, user: 90 });
assert.equal(log[0].at, "2020-01-01T00:00:00");

// A write that fails is reported, and a failed settings write stops the caller.
var realWrite = host.writeFile;
host.writeFile = async function() { return false; };
errs.length = 0;
assert.equal(await E.savePreset("day", { label: "Day" }), false);
assert.equal(await E.savePrompt("ok", "changed"), false);
assert.ok(errs.some(function(l) { return l.indexOf("write failed: ") === 0 && l.indexOf("presets/day.json") > 0; }), errs.join("\n"));
await assert.rejects(function() { return E.updateSettings(function(st) { st.enabled = false; }); }, /could not write/);
host.writeFile = realWrite;
assert.equal(await E.promptText("ok"), "A quiet harbour", "the failed save changed nothing");
assert.equal((await E.settings()).enabled, true, "the failed settings write changed nothing");

// A second generation says so instead of reporting success.
fs.writeFileSync(path.join(process.env.CYCLE_STATE_DIR, "generate.lock"), String(process.pid));
assert.equal(await E.generateAt(false, null, null), 3);
fs.rmSync(path.join(process.env.CYCLE_STATE_DIR, "generate.lock"));

// The command line refuses what it cannot act on, and says why.
async function cli(args) { errs.length = 0; out.length = 0; return E.main(args); }
assert.equal(await cli(["help"]), 0);
assert.ok(out.join("\n").indexOf("usage: cycle COMMAND") === 0, out.join("\n"));
assert.equal(await cli(["--help"]), 0);
assert.equal(await cli(["engine", "maybe"]), 2);
assert.match(errs.join("\n"), /engine takes 'on' or 'off'/);
assert.match(errs.join("\n"), /cycle help/);
assert.equal((await E.settings()).enabled, true, "a refused command changes nothing");
assert.equal(await cli(["engine", "off"]), 0);
assert.equal((await E.settings()).enabled, false);
assert.equal(await cli(["hold"]), 2);
assert.equal(await cli(["hold", "nope"]), 2);
assert.match(errs.join("\n"), /unknown preset 'nope'/);
assert.equal((await E.settings()).hold, null);
assert.equal(await cli(["hold", "day"]), 0);
assert.equal((await E.settings()).hold, "day");
assert.equal(await cli(["automatic"]), 0);
assert.equal((await E.settings()).hold, null);
assert.equal(await cli(["apply-preset", "../x"]), 2);
assert.equal(await cli(["generate", "--prompt", "nope"]), 2);
assert.match(errs.join("\n"), /unknown prompt 'nope'/);
assert.equal(await cli(["generate", "--prompt", "ok", "--apply-at", "soon"]), 2);
assert.equal(await cli(["open", "--prompt", "../x"]), 2);
assert.equal(fs.existsSync(path.join(process.env.CYCLE_STATE_DIR, "open-prompt")), false);
assert.equal(await cli(["reset-profile", "noon"]), 2);
assert.equal(await cli(["brightness-step", "--slot", "morning", "--base", "500"]), 2);
assert.equal(await cli(["brightness-step", "--slot", "lunch"]), 2);
assert.equal(await cli(["expand"]), 2);
assert.equal(await cli(["bogus"]), 2);
assert.match(errs.join("\n"), /unknown command 'bogus'/);
assert.equal(await cli(["engine-is-on"]), 1);

// Provider hints exist for every provider the window can pick.
for (var name of E.providerNames()) assert.ok(E.providerHint(name).length > 20, name);
assert.equal(E.providerHint("nope"), "");

// A detached program that does not exist is reported, not fatal.
assert.equal(await host.spawnDetached(["cycle-no-such-program-xyz"]), false);
assert.equal(await host.spawnDetached(["true"]), true);

console.log("safety tests passed");
