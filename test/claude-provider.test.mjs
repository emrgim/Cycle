import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { execFileSync } from "node:child_process";
import { loadEngine } from "../js/load-engine.mjs";
import { createNodeHost } from "../js/node-host.mjs";

var loaded = loadEngine();
var E = loaded.engine;

function plain(v) { return JSON.parse(JSON.stringify(v)); }
function eq(actual, expected, msg) { assert.deepEqual(plain(actual), expected, msg); }

// Provider table and old settings that only have a command.
eq(E.providerNames(), ["grok", "claude", "custom"]);
assert.equal(E.providerCommand("grok"), "grok -p {prompt_q} --always-approve --output-format json");
assert.equal(E.providerCommand("claude"), "claude -p {prompt_q} --tools \"\" --strict-mcp-config --output-format json --no-session-persistence < /dev/null");
assert.equal(E.providerCommand("custom"), "");
assert.equal(E.providerCommand("__proto__"), "");
assert.equal(E.genProvider({ provider: "claude", command: "grok -p {prompt_q}" }), "claude");
assert.equal(E.genProvider({ provider: "Custom", command: "claude -p {prompt_q}" }), "custom");
assert.equal(E.genProvider({ provider: "bogus", command: "claude -p {prompt_q}" }), "claude");
assert.equal(E.genProvider({ command: "grok -p {prompt_q} --always-approve --output-format json" }), "grok");
assert.equal(E.genProvider({ command: "/usr/local/bin/claude -p {prompt_q}" }), "claude");
assert.equal(E.genProvider({ command: "cp /tmp/x.png {out}" }), "custom");
assert.equal(E.genProvider({ command: "" }), "grok");
assert.equal(E.genProvider(null), "grok");
var defaults = E.defaultSettings().generator;
assert.equal(defaults.provider, "grok");
assert.equal(defaults.command, E.providerCommand("grok"));

// The Claude prompt asks for SVG text; the Grok prompt still names the PNG path.
var claudeAsk = E.genInstructions("claude", "/x/out.png");
assert.ok(claudeAsk.indexOf("SVG") >= 0);
assert.equal(claudeAsk.indexOf("/x/out.png"), -1);
assert.equal(claudeAsk.indexOf("http://"), -1);
assert.ok(E.genInstructions("grok", "/x/out.png").indexOf("Save it as a PNG file at /x/out.png") >= 0);
assert.equal(E.genInstructions("custom", "/x/out.png"), E.genInstructions("grok", "/x/out.png"));

// Pulling the SVG out of a reply.
var SVG = "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 1920 1080\">" +
  "<defs><linearGradient id=\"sky\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" stop-color=\"#1d3b6a\"/><stop offset=\"1\" stop-color=\"#f2a65a\"/></linearGradient>" +
  "<rect id=\"tower\" width=\"60\" height=\"400\" fill=\"#111\"/></defs>" +
  "<style><![CDATA[ .glow { fill: url('#sky'); } ]]></style>" +
  "<!-- skyline --><rect width=\"1920\" height=\"1080\" fill=\"url(#sky)\"/>" +
  "<use href=\"#tower\" x=\"300\" y=\"680\"/><use xlink:href=\"#tower\" x=\"500\" y=\"600\"/>" +
  "<text x=\"960\" y=\"200\" fill=\"#fff\">Genova · 18°C</text></svg>";
eq(E.svgFromReply(JSON.stringify({ type: "result", is_error: false, result: "```svg\n" + SVG + "\n```" })), { svg: SVG, error: "" });
eq(E.svgFromReply("Here it is:\n<?xml version=\"1.0\"?>\n" + SVG + "\nEnjoy."), { svg: SVG, error: "" });
eq(E.svgFromReply(JSON.stringify({ type: "result", is_error: true, result: "Not logged in" })), { svg: null, error: "provider error: Not logged in" });
assert.equal(E.svgFromReply("I can't draw that.").svg, null);
assert.equal(E.svgFromReply("</svg> <svg>").svg, null);
assert.equal(E.svgFromReply("").svg, null);

// Only references to ids inside the document get through.
assert.equal(E.svgProblem(SVG), "");
var bad = [
  "<svg><image href=\"file:///etc/passwd\"/></svg>",
  "<svg><use href=\"/home/user/.ssh/id_rsa#x\"/></svg>",
  "<svg><use xlink:href=\"https://example.com/a.svg#x\"/></svg>",
  "<svg><use href=x.svg/></svg>",
  "<svg><use href=\"&#x66;ile:///etc/passwd\"/></svg>",
  "<svg><rect fill=\"url(file:///etc/passwd)\"/></svg>",
  "<svg><rect style=\"fill:url( 'text:/etc/passwd' )\"/></svg>",
  "<svg><style>@import 'evil.css';</style></svg>",
  "<!DOCTYPE svg [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><svg>&x;</svg>",
  "<svg><!ENTITY x \"y\"></svg>",
  "<?xml-stylesheet href=\"evil.css\"?><svg></svg>",
  "<svg><script>alert(1)</script></svg>",
  "<svg><foreignObject><div/></foreignObject></svg>",
  "<svg><feImage/></svg>",
  "<svg>\u0000</svg>",
  ""
];
for (var b = 0; b < bad.length; b++) assert.notEqual(E.svgProblem(bad[b]), "", bad[b]);
var padded = "<svg>" + "<!--" + "x".repeat(120 * 1024) + "--></svg>";
assert.match(E.svgProblem(padded), /larger than/);
var wide = "<svg>" + "é".repeat(62 * 1024) + "</svg>";
assert.match(E.svgProblem(wide), /larger than/, "the cap counts UTF-8 bytes");

// End to end with stand-ins for claude and rsvg-convert on PATH. The default
// command runs unchanged, so its argument vector is checked as the CLI sees it.
function chunk(type, data) {
  var body = Buffer.from(data);
  var len = Buffer.alloc(4); len.writeUInt32BE(body.length);
  var kind = Buffer.from(type);
  var cbuf = Buffer.alloc(4); cbuf.writeUInt32BE(zlib.crc32(Buffer.concat([kind, body])) >>> 0);
  return Buffer.concat([len, kind, body, cbuf]);
}
var head = Buffer.alloc(13);
head.writeUInt32BE(2, 0); head.writeUInt32BE(1, 4); head[8] = 8; head[9] = 2;
var tiny = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", head),
  chunk("IDAT", zlib.deflateSync(Buffer.from([0, 255, 0, 0, 0, 0, 255]))),
  chunk("IEND", Buffer.alloc(0))
]);

var home = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-claude-"));
var fake = path.join(home, "fake");
var claudeBin = path.join(fake, "claude-bin");
var rsvgBin = path.join(fake, "rsvg-bin");
fs.mkdirSync(claudeBin, { recursive: true });
fs.mkdirSync(rsvgBin, { recursive: true });
fs.writeFileSync(path.join(fake, "tiny.png"), tiny);
fs.writeFileSync(path.join(claudeBin, "claude"), [
  "#!/bin/sh",
  "printf '%s\\0' \"$@\" > \"$FAKE_DIR/claude-args\"",
  "readlink /proc/$$/fd/0 > \"$FAKE_DIR/claude-stdin\"",
  "cat \"$FAKE_DIR/reply\""
].join("\n") + "\n", { mode: 0o755 });
fs.writeFileSync(path.join(rsvgBin, "rsvg-convert"), [
  "#!/bin/sh",
  "printf '%s\\0' \"$@\" > \"$FAKE_DIR/rsvg-args\"",
  "cat > \"$FAKE_DIR/rsvg-stdin\"",
  "while [ $# -gt 0 ]; do if [ \"$1\" = -o ]; then cp \"$FAKE_DIR/tiny.png\" \"$2\"; fi; shift; done"
].join("\n") + "\n", { mode: 0o755 });

var realPath = process.env.PATH || "/usr/bin:/bin";
process.env.FAKE_DIR = fake;
process.env.PATH = claudeBin + ":" + rsvgBin + ":" + realPath;

var h = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: home });
h.home = home;
h.fetchText = async function() { return { ok: false, text: "", error: "offline" }; };
var notes = [];
h.log = function(line) { notes.push(String(line)); };
h.logErr = h.log;
E.setHost(h);
await E.seed();
await E.updateSettings(function(s) {
  s.generator.provider = "claude";
  s.generator.command = E.providerCommand("claude");
  s.generator.template = "A quiet harbour at {{phase}}, {{condition}}";
});
var ctx = { phase: "evening", condition: "clear sky", location: "Genova", temperature: 18 };
function reply(text) {
  fs.writeFileSync(path.join(fake, "reply"), text);
  for (var f of ["claude-args", "rsvg-args", "rsvg-stdin"]) fs.rmSync(path.join(fake, f), { force: true });
}

reply(JSON.stringify({ type: "result", subtype: "success", is_error: false, result: "```svg\n" + SVG + "\n```" }));
var made = await E.generateImage("claude-ok", ctx, await E.settings());
assert.ok(made, notes.join("\n"));
assert.ok(made.indexOf(path.join(home, ".local/share/cycle/generated/claude-ok-")) === 0);
assert.ok(fs.readFileSync(made).equals(tiny));
assert.equal(fs.existsSync(made + ".part"), false);
var argv = fs.readFileSync(path.join(fake, "claude-args"), "utf8").split("\0").slice(0, -1);
assert.equal(argv.length, 8);
assert.equal(argv[0], "-p");
assert.ok(argv[1].indexOf("A quiet harbour at evening, clear sky. Paint this as one standalone SVG wallpaper") === 0, argv[1]);
eq(argv.slice(2), ["--tools", "", "--strict-mcp-config", "--output-format", "json", "--no-session-persistence"]);
assert.equal(fs.readFileSync(path.join(fake, "claude-stdin"), "utf8").trim(), "/dev/null");
assert.equal(fs.readFileSync(path.join(fake, "rsvg-stdin"), "utf8"), SVG);
var rargv = fs.readFileSync(path.join(fake, "rsvg-args"), "utf8").split("\0").slice(0, -1);
eq(rargv, ["-w", "3840", "-h", "2160", "-f", "png", "-o", made + ".part"]);
assert.ok(notes.some(function(line) { return line.indexOf("generating with Claude") === 0; }));

// A reply that points outside the document is never rendered.
reply(JSON.stringify({ type: "result", is_error: false, result: "<svg><image href=\"file:///etc/passwd\"/></svg>" }));
notes.length = 0;
assert.equal(await E.generateImage("claude-evil", ctx, await E.settings()), null);
assert.equal(fs.existsSync(path.join(fake, "rsvg-args")), false, "renderer ran on a rejected SVG");
assert.ok(notes.some(function(line) { return line.indexOf("SVG has a script, image or foreign element") >= 0; }), notes.join("\n"));

reply(JSON.stringify({ type: "result", is_error: true, result: "Invalid API key" }));
notes.length = 0;
assert.equal(await E.generateImage("claude-err", ctx, await E.settings()), null);
assert.ok(notes.some(function(line) { return line.indexOf("provider error: Invalid API key") >= 0; }), notes.join("\n"));

// A renderer that fails leaves no PNG and no partial file behind.
fs.writeFileSync(path.join(rsvgBin, "rsvg-convert"), "#!/bin/sh\ncat >/dev/null\necho broken >&2\nexit 3\n", { mode: 0o755 });
reply(JSON.stringify({ type: "result", is_error: false, result: SVG }));
notes.length = 0;
assert.equal(await E.generateImage("claude-broken", ctx, await E.settings()), null);
assert.ok(notes.some(function(line) { return line.indexOf("SVG render failed (rc=3): broken") >= 0; }), notes.join("\n"));
var leftovers = fs.readdirSync(path.join(home, ".local/share/cycle/generated")).filter(function(n) { return n.indexOf("claude-broken") === 0; });
eq(leftovers, []);

// Without rsvg-convert the run fails with a message that names the package.
var bare = path.join(fake, "bare-bin");
fs.mkdirSync(bare);
for (var tool of ["bash", "rm", "mv", "cat", "readlink"]) {
  fs.symlinkSync(execFileSync("sh", ["-c", "command -v " + tool], { env: { PATH: realPath }, encoding: "utf8" }).trim(), path.join(bare, tool));
}
process.env.PATH = claudeBin + ":" + bare;
reply(JSON.stringify({ type: "result", is_error: false, result: SVG }));
notes.length = 0;
assert.equal(await E.generateImage("claude-norsvg", ctx, await E.settings()), null);
assert.ok(notes.some(function(line) { return line.indexOf("SVG render failed (rc=127): rendering SVG needs rsvg-convert (Arch package librsvg)") >= 0; }), notes.join("\n"));
process.env.PATH = claudeBin + ":" + rsvgBin + ":" + realPath;

// Settings saved before providers existed keep working for Grok-style commands.
await E.updateSettings(function(s) { delete s.generator.provider; s.generator.command = "cp " + path.join(fake, "tiny.png") + " {out}"; });
assert.ok(await E.generateImage("legacy", ctx, await E.settings()));

// With a real rsvg-convert, the sample SVG becomes a 3840x2160 PNG.
var realRsvg = null;
try { realRsvg = execFileSync("sh", ["-c", "command -v rsvg-convert"], { env: { PATH: realPath }, encoding: "utf8" }).trim(); } catch (e) {}
if (realRsvg) {
  process.env.PATH = claudeBin + ":" + realPath;
  await E.updateSettings(function(s) { s.generator.provider = "claude"; s.generator.command = E.providerCommand("claude"); });
  reply(JSON.stringify({ type: "result", is_error: false, result: SVG }));
  var real = await E.generateImage("claude-real", ctx, await E.settings());
  assert.ok(real, notes.join("\n"));
  var png = fs.readFileSync(real);
  eq([png.readUInt32BE(16), png.readUInt32BE(20)], [3840, 2160]);
  assert.match(E.paletteFromBytes(png), /^mode = /);
} else {
  console.log("rsvg-convert not found: skipped the real render");
}

process.env.PATH = realPath;
console.log("claude provider tests passed");
