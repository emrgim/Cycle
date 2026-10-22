import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { loadEngine } from "../js/load-engine.mjs";
import { createNodeHost } from "../js/node-host.mjs";

var loaded = loadEngine();
var E = loaded.engine;
var SAFE = { y: 1970, mo: 1, d: 1, h: 0, mi: 0, s: 0 };

function stamp(y, mo, d, h, mi, s) {
  return { y: y, mo: mo, d: d, h: h, mi: mi, s: s };
}
function add(dt, mins) { return E.dateAddMinutes(dt, mins); }
function eq(actual, expected, msg) { assert.deepEqual(actual, expected, msg); }
function assertCivil(dt, msg) {
  assert.equal(typeof dt.y, "number", msg);
  assert.ok(isFinite(dt.y), msg);
  assert.ok(dt.mo >= 1 && dt.mo <= 12, msg + " mo");
  assert.ok(dt.d >= 1 && dt.d <= 31, msg + " d");
  assert.ok(dt.h >= 0 && dt.h <= 23, msg + " h");
  assert.ok(dt.mi >= 0 && dt.mi <= 59, msg + " mi");
  assert.ok(isFinite(dt.s), msg + " s");
}
function within(ms, limit, msg) {
  assert.ok(ms < limit, (msg || "slow") + " took " + ms + "ms");
}

var noon = stamp(2026, 10, 15, 12, 0, 4);
eq(add(stamp(2024, 2, 28, 23, 30, 7), 90), stamp(2024, 2, 29, 1, 0, 7));
eq(add(stamp(2024, 2, 29, 23, 30, 7), 90), stamp(2024, 3, 1, 1, 0, 7));
eq(add(stamp(2026, 2, 28, 23, 30, 7), 90), stamp(2026, 3, 1, 1, 0, 7));
eq(add(stamp(2024, 3, 1, 0, 0, 0), -1), stamp(2024, 2, 29, 23, 59, 0));
eq(add(stamp(2026, 1, 1, 0, 0, 0), -1), stamp(2025, 12, 31, 23, 59, 0));
eq(add(stamp(2026, 12, 31, 23, 59, 0), 1), stamp(2027, 1, 1, 0, 0, 0));
eq(add(stamp(9999, 12, 31, 23, 59, 0), 1), stamp(10000, 1, 1, 0, 0, 0));
eq(add(stamp(1, 1, 1, 0, 0, 0), -1), stamp(0, 12, 31, 23, 59, 0));
eq(add(stamp(2026, 10, 15, 14, 7, 0), 8), stamp(2026, 10, 15, 14, 15, 0));
eq(add(stamp(1900, 2, 29, 0, 0, 0), 0), stamp(1900, 3, 1, 0, 0, 0));
eq(add(stamp(2026, 10, 0, 12, 0, 1), 0), stamp(2026, 9, 30, 12, 0, 1));
eq(add(stamp(2026, 0, 15, 12, 30, 1), 0), stamp(2025, 12, 15, 12, 30, 1));
eq(add(stamp(2026, 13, 15, 12, 30, 1), 0), stamp(2027, 1, 15, 12, 30, 1));

eq(add(noon, 0), noon);
eq(add(noon, NaN), noon);
eq(add(noon, Infinity), noon);
eq(add(noon, -Infinity), noon);
eq(add(stamp(2026, 10, 15, Infinity, 0, 4), 30), stamp(2026, 10, 15, 0, 30, 4));
eq(add(stamp(2026, 10, 15, 12, NaN, 4), 0), noon);
eq(add(stamp(2026, 10, 15, 12, 0, Infinity), 0), stamp(2026, 10, 15, 12, 0, 0));

var hugeStarted = Date.now();
var up = add(noon, 1e300);
var down = add(noon, -1e300);
within(Date.now() - hugeStarted, 1000, "1e300 minutes");
eq(up, stamp(12047, 7, 12, 0, 0, 4));
eq(down, stamp(-7994, 1, 18, 0, 0, 4));
assert.ok(up.y > 2026);
assert.ok(down.y < 2026);
assertCivil(up, "+1e300");
assertCivil(down, "-1e300");

var reviewerShift = Math.round((1e300 / 3600) * 60);
var shiftStarted = Date.now();
var shifted = add(noon, reviewerShift);
within(Date.now() - shiftStarted, 1000, "reviewer shift");
assertCivil(shifted, "reviewer shift");

eq(add(stamp(1e300, 10, 15, 12, 0, 0), 0), SAFE);
eq(add(stamp(2026, 1e300, 15, 12, 0, 0), 0), SAFE);
eq(add(stamp(2026, 10, 1e300, 12, 0, 0), 0), SAFE);
eq(add(null, 30), SAFE);
var badClock = Date.now();
assertCivil(add(stamp(2026, 10, 15, 1e300, 0, 4), 0), "huge hour");
assertCivil(add(stamp(2026, 10, 15, 12, 1e300, 4), 0), "huge minute");
within(Date.now() - badClock, 1000, "huge clock parts");

var samples = [
  stamp(2024, 2, 29, 12, 0, 3),
  stamp(2024, 3, 1, 0, 0, 0),
  stamp(2026, 1, 1, 0, 0, 0),
  stamp(2026, 10, 15, 14, 7, 4),
  stamp(2026, 12, 31, 23, 59, 0)
];
var deltas = [-4000, -1441, -1, 0, 1, 90, 1439, 50000];
for (var i = 0; i < samples.length; i++) {
  for (var j = 0; j < deltas.length; j++) {
    var forward = add(samples[i], deltas[j]);
    eq(add(forward, -deltas[j]), samples[i], "round trip " + deltas[j]);
  }
}

var ordinalStarted = Date.now();
assert.equal(E.ordinal(2026, 1e300, 15), 15);
assert.equal(E.ordinal(2024, 3, 1), 61);
assert.equal(E.ordinal(2026, 3, 1), 60);
within(Date.now() - ordinalStarted, 1000, "ordinal");

var sunStarted = Date.now();
var pos = E.sunPosTz(stamp(2026, 1e300, 15, 12, 0, 0), 44.4, 8.9, 0);
var events = E.sunEventsAt(stamp(1e300, 1e300, 1e300, 0, 0, 0), 1e300, 1e300, 1e300);
within(Date.now() - sunStarted, 1000, "hostile sun");
assert.equal(pos.length, 2);
assert.ok(Object.prototype.hasOwnProperty.call(events, "sunrise"));

assert.equal(E.pad(7, 1e300), "00000007");
assert.equal(E.pad(7, Infinity), "7");
assert.equal(E.pad(7, 2), "07");
assert.equal(E.pad(7, 4), "0007");

assert.equal(E.utcOffsetHours(1e300), null);
assert.equal(E.utcOffsetHours(1e309), null);
assert.equal(E.utcOffsetHours(-1e309), null);
assert.equal(E.utcOffsetHours(NaN), null);
assert.equal(E.utcOffsetHours("3600"), null);
assert.equal(E.utcOffsetHours(null), null);
assert.equal(E.utcOffsetHours(50400), 14);
assert.equal(E.utcOffsetHours(50401), null);
assert.equal(E.utcOffsetHours(-50400), -14);
assert.equal(E.utcOffsetHours(-50401), null);
assert.equal(E.utcOffsetHours(0), 0);
assert.equal(E.utcOffsetHours(20700), 5.75);
assert.equal(E.utcOffsetHours(19800), 5.5);
assert.equal(E.utcOffsetHours(45900), 12.75);
assert.equal(E.civilTz(14), 14);
assert.equal(E.civilTz(14.0001), null);
assert.equal(E.civilTz(-14), -14);
assert.equal(E.civilTz(5.75), 5.75);

var tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-date-"));
var host = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: tmp });
host.home = tmp;
var clock = { y: 2026, mo: 10, d: 15, h: 12, mi: 0, s: 0, ms: 0, tzHours: 0, weekday: 3 };
var urls = [];
var geoBody = "";
var forecastBody = "";
var dailyBody = "{}";
host.now = function() { return clock; };
host.fetchText = function(url) {
  urls.push(String(url));
  var u = String(url);
  var text = "{}";
  if (u.indexOf("geocoding-api") >= 0) text = geoBody;
  else if (u.indexOf("daily=") >= 0) text = dailyBody;
  else if (u.indexOf("forecast") >= 0) text = forecastBody;
  return Promise.resolve({ ok: true, text: text, error: "" });
};
host.exec = function() { return Promise.resolve({ code: 1, out: "", err: "" }); };
host.execLong = host.exec;
host.log = function() {};
host.logErr = function() {};

var genova = {
  name: "Genova",
  latitude: 44.4056,
  longitude: 8.9463,
  country: "Italy",
  population: 580000
};
function geo(results) { return JSON.stringify({ results: results }); }
geoBody = geo([genova]);

function forecastOf(valueLiteral) {
  if (valueLiteral == null) return "{}";
  return "{\"utc_offset_seconds\":" + valueLiteral + "}";
}
async function expandCity(pathText, tzHours, forecastLiteral, results) {
  clock.tzHours = tzHours;
  forecastBody = forecastOf(forecastLiteral);
  geoBody = geo(results || [genova]);
  urls = [];
  E.setHost(host);
  var started = Date.now();
  var text = await E.expandCalls(pathText, {}, []);
  return { text: text, ms: Date.now() - started, urls: urls.slice() };
}
function forecastUrls(box) {
  var out = [];
  for (var i = 0; i < box.urls.length; i++) if (box.urls[i].indexOf("forecast") >= 0) out.push(box.urls[i]);
  return out;
}
function assertLocalNoon(box, msg) {
  within(box.ms, 1000, msg);
  assert.ok(box.text.indexOf("12:00") >= 0, msg + " " + box.text);
  assert.ok(box.text.indexOf("15 Oct") >= 0, msg + " " + box.text);
  assert.ok(forecastUrls(box)[0].indexOf("latitude=44.4056") >= 0, msg);
}

var plus4 = await expandCity("/sun/city/Genova", 0, "14400");
within(plus4.ms, 1000, "+4h");
assert.ok(plus4.text.indexOf("16:00") >= 0, plus4.text);
assert.ok(plus4.text.indexOf("15 Oct") >= 0, plus4.text);

var plus14 = await expandCity("/sun/city/Genova", 0, "50400");
assert.ok(plus14.text.indexOf("02:00") >= 0, plus14.text);
assert.ok(plus14.text.indexOf("16 Oct") >= 0, plus14.text);

var minus14 = await expandCity("/sun/city/Genova", 0, "-50400");
assert.ok(minus14.text.indexOf("22:00") >= 0, minus14.text);
assert.ok(minus14.text.indexOf("14 Oct") >= 0, minus14.text);

var nepal = await expandCity("/sun/city/Genova", 0, "20700");
assert.ok(nepal.text.indexOf("17:45") >= 0, nepal.text);

var chatham = await expandCity("/sun/city/Genova", 0, "45900");
assert.ok(chatham.text.indexOf("00:45") >= 0, chatham.text);
assert.ok(chatham.text.indexOf("16 Oct") >= 0, chatham.text);

var hostile = ["1e300", "1e309", "-1e309", "999999", "50401", "-50401", "null", "\"14400\"", "true", null];
for (var h = 0; h < hostile.length; h++) {
  assertLocalNoon(await expandCity("/sun/city/Genova", 0, hostile[h]), "offset " + hostile[h]);
}
clock.tzHours = 0;
forecastBody = "{";
geoBody = geo([genova]);
urls = [];
E.setHost(host);
var brokenStarted = Date.now();
var broken = await E.expandCalls("/sun/city/Genova", {}, []);
within(Date.now() - brokenStarted, 1000, "broken forecast");
assert.ok(broken.indexOf("12:00") >= 0, broken);
assert.ok(broken.indexOf("15 Oct") >= 0, broken);
assert.ok(urls.some(function(u) { return u.indexOf("latitude=44.4056") >= 0; }));

var utcKept = await expandCity("/sun/city/Genova", 4, "0");
assert.ok(utcKept.text.indexOf("08:00") >= 0, utcKept.text);
assert.ok(utcKept.text.indexOf("15 Oct") >= 0, utcKept.text);

var hostileLocal = await expandCity("/sun/city/Genova", 4, "1e300");
assert.ok(hostileLocal.text.indexOf("12:00") >= 0, hostileLocal.text);
assert.ok(hostileLocal.text.indexOf("15 Oct") >= 0, hostileLocal.text);
var missingLocal = await expandCity("/sun/city/Genova", 4, null);
assert.ok(missingLocal.text.indexOf("12:00") >= 0, missingLocal.text);

clock.tzHours = NaN;
var badHost = await expandCity("/sun/city/Genova", NaN, "3600");
assert.ok(badHost.text.indexOf("13:00") >= 0, badHost.text);
var hugeHost = await expandCity("/sun/city/Genova", 1e300, "3600");
assert.ok(hugeHost.text.indexOf("13:00") >= 0, hugeHost.text);

var day = stamp(2026, 10, 15, 12, 0, 0);
var dawnUtc = E.sunEventsAt(day, 44.4056, 8.9463, 0).civil_dawn;
var dawnOne = E.sunEventsAt(day, 44.4056, 8.9463, 1).civil_dawn;
assert.notEqual(dawnUtc, dawnOne);
var dawnHostile = await expandCity("/dawn/city/Genova", 0, "1e300");
within(dawnHostile.ms, 1000, "hostile dawn");
assert.equal(dawnHostile.text.indexOf(dawnUtc), 0, dawnHostile.text);
var dawnHour = await expandCity("/dawn/city/Genova", 0, "3600");
assert.equal(dawnHour.text.indexOf(dawnOne), 0, dawnHour.text);
assert.notEqual(dawnHour.text, dawnHostile.text);

var fake = await expandCity("/sun/city/Genova", 0, "14400", { length: 1e300 });
within(fake.ms, 1000, "fake results length");
assert.ok(fake.text.indexOf("not found") >= 0, fake.text);
assert.equal(forecastUrls(fake).length, 0);

var onlyBad = await expandCity("/sun/city/Genova", 0, "14400", [{ latitude: 1e300, longitude: 1e300, population: 1e300 }]);
within(onlyBad.ms, 1000, "bad coordinates");
assert.ok(onlyBad.text.indexOf("not found") >= 0, onlyBad.text);
assert.equal(forecastUrls(onlyBad).length, 0);

var mixed = await expandCity("/sun/city/Genova", 0, "14400", [
  { latitude: 1e300, longitude: 1e300, population: 1e300, name: "Nowhere" },
  genova
]);
within(mixed.ms, 1000, "mixed geocode");
assert.ok(mixed.text.indexOf("16:00") >= 0, mixed.text);
var mixedForecast = forecastUrls(mixed).join(" ");
assert.ok(mixedForecast.indexOf("latitude=44.4056") >= 0, mixedForecast);
assert.equal(mixedForecast.indexOf("1e300"), -1, mixedForecast);

dailyBody = "{\"daily\":{\"temperature_2m_max\":[0,0,1e300],\"temperature_2m_min\":[0,0,-3],\"weather_code\":[0,0,1e309],\"precipitation_sum\":[0,0,1e309],\"wind_speed_10m_max\":[0,0,12],\"uv_index_max\":[0,0,5],\"cloud_cover_mean\":[0,0,40]}}";
var weather = await expandCity("/weather/city/Genova/tomorrow", 0, "0");
within(weather.ms, 1000, "hostile daily");
assert.ok(weather.text.length < 400, weather.text);
assert.equal(weather.text.indexOf("Infinity"), -1, weather.text);
assert.ok(weather.text.indexOf("-3") >= 0, weather.text);
assert.ok(weather.text.indexOf("\u20130") >= 0, weather.text);
assert.ok(weather.text.indexOf("0.0") >= 0, weather.text);
assert.ok(weather.text.indexOf("12") >= 0, weather.text);

var curveHome = fs.mkdtempSync(path.join(os.tmpdir(), "cycle-curve-"));
var curveHost = createNodeHost({ pluginRoot: loaded.root, linkCli: false, home: curveHome });
curveHost.home = curveHome;
curveHost.now = function() { return clock; };
curveHost.fetchText = function() { throw new Error("curve must not fetch"); };
curveHost.exec = function() { return Promise.resolve({ code: 1, out: "", err: "" }); };
curveHost.execLong = curveHost.exec;
curveHost.log = function() {};
curveHost.logErr = function() {};
var settingsDir = path.join(curveHome, ".config/omarchy/cycle");
fs.mkdirSync(settingsDir, { recursive: true });
fs.writeFileSync(path.join(settingsDir, "settings.json"), JSON.stringify({
  enabled: false,
  location: { name: "Genova, Italy", city: "Genova", cc: "IT", lat: 44.4056, lon: 8.9463, tz: "Europe/Rome", source: "manual" },
  weather: { enabled: false }
}));
clock.tzHours = 0;
E.setHost(curveHost);
var curveStarted = Date.now();
var neg = await E.curve(-1);
var tiny = await E.curve(1e-9);
var giant = await E.curve(1e300);
within(Date.now() - curveStarted, 2000, "curve");
assert.equal(neg.points.length, 49);
assert.equal(tiny.points.length, 49);
assert.equal(giant.points.length, 1);

console.log("date bounds tests passed");
