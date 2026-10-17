import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

var root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

var TEXT_TYPES = new Set(["Text", "Label", "TextArea", "TextEdit"]);
var MODEL_CONTROLS = new Set(["ComboBox"]);
var TEXT_CONTROLS = new Set([
  "Button", "Switch", "TabButton", "ItemDelegate", "ToolButton", "MenuItem",
  "CheckBox", "RadioButton", "RoundButton"
]);

function isIdent(c) { return !!c && /[A-Za-z0-9_]/.test(c); }

function skipString(src, i) {
  var q = src[i];
  i++;
  while (i < src.length) {
    if (src[i] === "\\") { i += 2; continue; }
    if (src[i] === q) return i + 1;
    if (src[i] === "\n") return i;
    i++;
  }
  return i;
}

function skipLine(src, i) {
  while (i < src.length && src[i] !== "\n") i++;
  return i;
}

function skipBlockComment(src, i) {
  i += 2;
  while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i++;
  return Math.min(src.length, i + 2);
}

function readExpr(src, i) {
  var start = i;
  var depth = 0;
  while (i < src.length) {
    var c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i); continue; }
    if (c === "/" && src[i + 1] === "/") { i = skipLine(src, i); continue; }
    if (c === "/" && src[i + 1] === "*") { i = skipBlockComment(src, i); continue; }
    if (c === "{" || c === "(" || c === "[") { depth++; i++; continue; }
    if (c === "}" || c === ")" || c === "]") {
      if (depth === 0) break;
      depth--;
      i++;
      continue;
    }
    if (depth === 0 && (c === ";" || c === "\n")) break;
    i++;
  }
  return { text: src.slice(start, i).trim(), end: i };
}

function isStringLiteral(expr) {
  if (!expr || (expr[0] !== '"' && expr[0] !== "'")) return false;
  return skipString(expr, 0) === expr.length;
}

function isLiteralStringArray(expr) {
  var s = expr.trim();
  if (!s.length || s[0] !== "[" || s[s.length - 1] !== "]") return false;
  var inner = s.slice(1, -1).trim();
  if (!inner) return true;
  var i = 0;
  while (i < inner.length) {
    while (i < inner.length && /\s/.test(inner[i])) i++;
    if (i >= inner.length) return true;
    if (inner[i] !== '"' && inner[i] !== "'") return false;
    i = skipString(inner, i);
    while (i < inner.length && /\s/.test(inner[i])) i++;
    if (i >= inner.length) return true;
    if (inner[i] !== ",") return false;
    i++;
  }
  return true;
}

function matchingBrace(src, open) {
  var depth = 0;
  var i = open;
  while (i < src.length) {
    var c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i); continue; }
    if (c === "/" && src[i + 1] === "/") { i = skipLine(src, i); continue; }
    if (c === "/" && src[i + 1] === "*") { i = skipBlockComment(src, i); continue; }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return i;
    }
    i++;
  }
  return src.length - 1;
}

function lineAt(src, index) {
  var line = 1;
  for (var i = 0; i < index && i < src.length; i++) if (src[i] === "\n") line++;
  return line;
}

function parseElementBody(body) {
  var info = { id: "", textKind: "absent", textExpr: "", modelKind: "absent", modelExpr: "", plain: false };
  var i = 0;
  var depth = 0;
  while (i < body.length) {
    var c = body[i];
    if (c === '"' || c === "'") { i = skipString(body, i); continue; }
    if (c === "/" && body[i + 1] === "/") { i = skipLine(body, i); continue; }
    if (c === "/" && body[i + 1] === "*") { i = skipBlockComment(body, i); continue; }
    if (c === "{") { depth++; i++; continue; }
    if (c === "}") { if (depth > 0) depth--; i++; continue; }
    if (depth === 0 && /[A-Za-z_]/.test(c)) {
      var start = i;
      i++;
      while (i < body.length && /[A-Za-z0-9_.]/.test(body[i])) i++;
      var prop = body.slice(start, i);
      var j = i;
      while (j < body.length && /[ \t]/.test(body[j])) j++;
      if (body[j] !== ":") continue;
      j++;
      while (j < body.length && /[ \t]/.test(body[j])) j++;
      var expr = readExpr(body, j);
      if (prop === "id" && /^[A-Za-z_][A-Za-z0-9_]*$/.test(expr.text)) info.id = expr.text;
      if (prop === "text") {
        info.textExpr = expr.text;
        info.textKind = isStringLiteral(expr.text) ? "literal" : "expr";
      }
      if (prop === "model") {
        info.modelExpr = expr.text;
        info.modelKind = isLiteralStringArray(expr.text) ? "literal" : "expr";
      }
      if (prop === "textFormat") {
        var compact = expr.text.replace(/\s+/g, "");
        info.plain = compact === "Text.PlainText" || compact === "TextEdit.PlainText";
      }
      i = expr.end;
      continue;
    }
    i++;
  }
  return info;
}

function findAssignments(src) {
  var text = new Set();
  var model = new Set();
  var i = 0;
  while (i < src.length) {
    var c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i); continue; }
    if (c === "/" && src[i + 1] === "/") { i = skipLine(src, i); continue; }
    if (c === "/" && src[i + 1] === "*") { i = skipBlockComment(src, i); continue; }
    if (!isIdent(c) || isIdent(src[i - 1])) { i++; continue; }
    var start = i;
    i++;
    while (i < src.length && isIdent(src[i])) i++;
    var word = src.slice(start, i);
    if (src[i] !== ".") continue;
    var j = i + 1;
    while (j < src.length && /[ \t]/.test(src[j])) j++;
    var prop = "";
    if (src.slice(j, j + 4) === "text" && !isIdent(src[j + 4])) prop = "text";
    else if (src.slice(j, j + 5) === "model" && !isIdent(src[j + 5])) prop = "model";
    if (!prop) continue;
    j += prop.length;
    while (j < src.length && /[ \t]/.test(src[j])) j++;
    if (src[j] === "=" || (src[j] === "+" && src[j + 1] === "=")) {
      if (prop === "text") text.add(word);
      else model.add(word);
    }
  }
  return { text: text, model: model };
}

function auditQml(src, filename) {
  var elements = [];
  var components = {};
  var pending = null;
  var i = 0;
  while (i < src.length) {
    var c = src[i];
    if (c === '"' || c === "'") { i = skipString(src, i); continue; }
    if (c === "/" && src[i + 1] === "/") { i = skipLine(src, i); continue; }
    if (c === "/" && src[i + 1] === "*") { i = skipBlockComment(src, i); continue; }
    if (c === "c" && src.slice(i, i + 9) === "component" && !isIdent(src[i - 1]) && !isIdent(src[i + 9])) {
      var j = i + 9;
      while (j < src.length && /\s/.test(src[j])) j++;
      var ns = j;
      while (j < src.length && isIdent(src[j])) j++;
      var name = src.slice(ns, j);
      while (j < src.length && /\s/.test(src[j])) j++;
      if (name && src[j] === ":") pending = name;
      i = j;
      continue;
    }
    if (!isIdent(c) || isIdent(src[i - 1]) || src[i - 1] === ".") { i++; continue; }
    var start = i;
    i++;
    while (i < src.length && isIdent(src[i])) i++;
    var word = src.slice(start, i);
    var j2 = i;
    while (j2 < src.length && /[ \t\r\n]/.test(src[j2])) j2++;
    if (src[j2] !== "{") {
      if (pending && word) pending = pending;
      continue;
    }
    var close = matchingBrace(src, j2);
    var body = src.slice(j2 + 1, close);
    var info = parseElementBody(body);
    var el = {
      type: word,
      id: info.id,
      textKind: info.textKind,
      textExpr: info.textExpr,
      modelKind: info.modelKind,
      modelExpr: info.modelExpr,
      plain: info.plain,
      start: start,
      end: close,
      line: lineAt(src, start),
      file: filename
    };
    elements.push(el);
    if (pending) {
      components[pending] = { base: word, element: el };
      pending = null;
    }
    i = j2 + 1;
  }

  function baseOf(type) {
    var seen = {};
    while (components[type] && !seen[type]) {
      seen[type] = true;
      type = components[type].base;
    }
    return type;
  }
  function hasPlainDynamicText(el) {
    for (var n = 0; n < elements.length; n++) {
      var child = elements[n];
      if (child.start <= el.start || child.end >= el.end) continue;
      if (!TEXT_TYPES.has(child.type)) continue;
      if (child.plain && child.textKind === "expr") return true;
    }
    return false;
  }
  function plainDisplay(el) {
    if (hasPlainDynamicText(el)) return true;
    var comp = components[el.type];
    return !!(comp && hasPlainDynamicText(comp.element));
  }

  var assigned = findAssignments(src);
  var violations = [];
  for (var n = 0; n < elements.length; n++) {
    var el = elements[n];
    var base = baseOf(el.type);
    if (TEXT_TYPES.has(el.type) || TEXT_TYPES.has(base)) {
      var dynamic = el.textKind === "expr" || (el.id && assigned.text.has(el.id));
      if (dynamic && !el.plain) {
        violations.push({
          file: filename,
          line: el.line,
          id: el.id || "",
          type: el.type,
          reason: "non-literal text without textFormat: Text.PlainText"
        });
      }
    }
    if (MODEL_CONTROLS.has(base) && !TEXT_TYPES.has(base)) {
      var dynamicModel = el.modelKind === "expr" || (el.id && assigned.model.has(el.id));
      if (dynamicModel && !plainDisplay(el)) {
        violations.push({
          file: filename,
          line: el.line,
          id: el.id || "",
          type: el.type,
          reason: "non-literal model without a plain-text Text"
        });
      }
    }
    if (TEXT_CONTROLS.has(base)) {
      var dynamicText = el.textKind === "expr" || (el.id && assigned.text.has(el.id));
      if (dynamicText && !plainDisplay(el)) {
        violations.push({
          file: filename,
          line: el.line,
          id: el.id || "",
          type: el.type,
          reason: "non-literal control text without a plain-text Text"
        });
      }
    }
  }
  return { elements: elements, violations: violations, components: components };
}

function walk(dir, out) {
  var ents = fs.readdirSync(dir, { withFileTypes: true });
  for (var i = 0; i < ents.length; i++) {
    var name = ents[i].name;
    if (name === ".git" || name === "node_modules") continue;
    var p = path.join(dir, name);
    if (ents[i].isDirectory()) walk(p, out);
    else if (name.endsWith(".qml")) out.push(p);
  }
}

function violationsOf(src, filename) {
  return auditQml(src, filename).violations;
}

var ssid = [
  "Label { id: wifiLabel; color: \"#a8a39a\" }",
  "function paint() { wifiLabel.text = ssid ? (\"Current Wi-Fi: \" + ssid) : \"Current Wi-Fi: none\" }"
].join("\n");
var ssidHits = violationsOf(ssid, "ssid.qml");
assert.equal(ssidHits.length, 1);
assert.equal(ssidHits[0].id, "wifiLabel");

var ssidFixed = [
  "Label { id: wifiLabel; textFormat: Text.PlainText; color: \"#a8a39a\" }",
  "function paint() { wifiLabel.text = ssid ? (\"Current Wi-Fi: \" + ssid) : \"Current Wi-Fi: none\" }"
].join("\n");
assert.equal(violationsOf(ssidFixed, "ssid-fixed.qml").length, 0);

assert.equal(violationsOf('Label { text: "Circadian curve"; wrapMode: Text.WordWrap }', "lit.qml").length, 0);
assert.equal(violationsOf('Label { text: root.noteText }', "note.qml").length, 1);
assert.equal(violationsOf('Label { text: root.noteText; textFormat: Text.PlainText }', "note-ok.qml").length, 0);
assert.equal(violationsOf('Label { id: liveText; textFormat: Text.AutoText }\nliveText.text = weather', "auto.qml").length, 1);
assert.equal(violationsOf('Label { id: liveText; textFormat: Text.RichText }\nliveText.text = weather', "rich.qml").length, 1);
assert.equal(violationsOf('TextArea { id: promptEdit }\npromptEdit.text = fileBody', "area.qml").length, 1);
assert.equal(violationsOf('TextArea { id: promptEdit; textFormat: TextEdit.PlainText }\npromptEdit.text = fileBody', "area-ok.qml").length, 0);

var comboBad = 'ComboBox { id: presetBox; model: [] }\npresetBox.model = names';
assert.equal(violationsOf(comboBad, "combo.qml").length, 1);
var comboLit = 'ComboBox { id: modeBox; model: ["15 min", "30 min"] }';
assert.equal(violationsOf(comboLit, "combo-lit.qml").length, 0);
var comboOk = [
  "component PlainCombo: ComboBox {",
  "  id: combo",
  "  contentItem: Text { text: combo.displayText; textFormat: Text.PlainText }",
  "  delegate: ItemDelegate {",
  "    text: modelData",
  "    contentItem: Text { text: parent.text; textFormat: Text.PlainText }",
  "  }",
  "}",
  "PlainCombo { id: presetBox; model: [\"Automatic\"] }",
  "presetBox.model = names"
].join("\n");
assert.equal(violationsOf(comboOk, "combo-ok.qml").length, 0);

var buttonBad = 'Button { text: modelData }';
assert.equal(violationsOf(buttonBad, "button.qml").length, 1);
var buttonOk = 'Button { text: modelData; contentItem: Text { text: parent.text; textFormat: Text.PlainText } }';
assert.equal(violationsOf(buttonOk, "button-ok.qml").length, 0);
assert.equal(violationsOf('Button { text: "Apply now" }', "button-lit.qml").length, 0);

var files = [];
walk(root, files);
assert.ok(files.some(function(p) { return p.endsWith(path.join("Panel.qml")); }));
var all = [];
var panel = null;
for (var f = 0; f < files.length; f++) {
  var rel = path.relative(root, files[f]);
  var report = auditQml(fs.readFileSync(files[f], "utf8"), rel);
  if (rel === "Panel.qml") panel = report;
  for (var v = 0; v < report.violations.length; v++) all.push(report.violations[v]);
}
if (all.length) {
  for (var k = 0; k < all.length; k++) {
    var hit = all[k];
    console.error(hit.file + ":" + hit.line + " " + hit.type + (hit.id ? " id=" + hit.id : "") + " " + hit.reason);
  }
}
assert.equal(all.length, 0, "QML binds non-literal text without Text.PlainText");

assert.ok(panel);
var wifi = panel.elements.find(function(el) { return el.id === "wifiLabel"; });
assert.ok(wifi, "wifiLabel missing");
assert.equal(wifi.type, "Label");
assert.equal(wifi.plain, true);
var required = ["nowLine", "nextLine", "resolveText", "liveText", "feedList", "errorLabel", "previewLabel", "schedInfo", "sunToday", "wifiLabel", "rulesText", "promptEdit"];
for (var r = 0; r < required.length; r++) {
  var el = panel.elements.find(function(item) { return item.id === required[r]; });
  assert.ok(el, required[r] + " missing");
  assert.equal(el.plain, true, required[r] + " must be plain text");
}
var note = panel.elements.find(function(el) { return el.textExpr.indexOf("noteText") >= 0; });
assert.ok(note && note.plain, "status line must be plain text");

console.log("qml plaintext checks passed");
