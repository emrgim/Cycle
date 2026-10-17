import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createNodeHost } from "../js/node-host.mjs";

var root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
var files = [
  "js/engine.js",
  "js/node-host.mjs",
  "js/cli.mjs",
  "js/load-engine.mjs",
  "EngineHost.qml",
  "Panel.qml",
  "Service.qml",
  "Widget.qml",
  "cli/main.qml"
];

var sources = {};
for (var i = 0; i < files.length; i++) {
  sources[files[i]] = fs.readFileSync(path.join(root, files[i]), "utf8");
  assert.equal(sources[files[i]].indexOf("http://"), -1, files[i] + " fetches or names a plain-HTTP URL");
}

var xhrFiles = files.filter(function(name) { return sources[name].indexOf("new XMLHttpRequest") >= 0; });
assert.deepEqual(xhrFiles, ["EngineHost.qml"]);
var hostSrc = sources["EngineHost.qml"];
assert.equal(hostSrc.split("new XMLHttpRequest").length - 1, 1);
var xhrAt = hostSrc.indexOf("new XMLHttpRequest");
var helperAt = hostSrc.indexOf("function cappedXhr");
var nextFn = hostSrc.indexOf("\n  function ", helperAt + 1);
assert.ok(helperAt >= 0 && nextFn > helperAt, "cappedXhr must be a real function");
var helperBody = hostSrc.slice(helperAt, nextFn);
assert.ok(xhrAt > helperAt && xhrAt < nextFn, "XMLHttpRequest must live inside cappedXhr");
for (var marker of ["xhr.abort()", "Content-Length", "readyState === 3", "onprogress", "import QtQuick; Timer", "timer.start()"]) {
  assert.ok(helperBody.indexOf(marker) >= 0, "cappedXhr missing " + marker);
}
assert.equal(hostSrc.indexOf("xhr.timeout ="), -1);
assert.ok(hostSrc.indexOf("xhr.timeout does not fire") >= 0);
for (var flag of ["--proto", "=https", "--proto-redir", "--max-time", "--max-filesize", "head -c"]) {
  assert.ok(hostSrc.indexOf(flag) >= 0, "EngineHost missing " + flag);
  assert.ok(sources["js/node-host.mjs"].indexOf(flag) >= 0, "node-host missing " + flag);
}
assert.equal(sources["js/node-host.mjs"].indexOf("fetch("), -1);
assert.equal(sources["js/engine.js"].indexOf("XMLHttpRequest"), -1);
assert.ok(sources["js/engine.js"].indexOf("https://get.geojs.io/v1/ip/geo.json") >= 0);
assert.equal(apiFetchCallsCapped(hostSrc), true);

function apiFetchCallsCapped(src) {
  var at = src.indexOf("fetchText: function");
  assert.ok(at >= 0);
  var end = src.indexOf("}", at);
  var body = src.slice(at, end);
  return body.indexOf("fetchCapped") >= 0 && body.indexOf("XMLHttpRequest") < 0;
}

var dir = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-fetch-"));
var host = createNodeHost({ home: dir, pluginRoot: root, linkCli: false });
var refused = await host.fetchText("http://ip-api.com/json/", 2, 128);
assert.equal(refused.ok, false);
assert.match(refused.error, /https required/);
var blank = await host.fetchText("https://example.com/a\nb", 2, 128);
assert.equal(blank.ok, false);

console.log("fetch bound checks passed");
