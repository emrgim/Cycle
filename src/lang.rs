//! Prompt composition language.
//!
//! ```text
//! prompt   := { text | call | legacy }
//! call     := "/" name { "/" arg }        -- must start the text or follow whitespace or one of ( [ , ; : " '
//! name     := [a-z] [a-z0-9_]*
//! arg      := [A-Za-z0-9_+%-]+            -- "_" and "+" in a city/query value mean a space
//! legacy   := "{{" name "}}"
//! args     := keyword | keyword value     -- per command, see SPECS
//! keyword  := "city" value | "query" value | "today" | "tomorrow" | "yesterday"
//! ```
use crate::core;
use crate::live;
use chrono::{Duration, Local, NaiveDate};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq)]
pub struct Call { pub name: String, pub args: Vec<String>, pub start: usize, pub end: usize }

const DAY: [&str; 3] = ["today", "tomorrow", "yesterday"];
/// command → accepted keywords (a keyword in VALUE_KW takes the next segment as its value)
pub fn spec(name: &str) -> Option<&'static [&'static str]> {
    const WX: &[&str] = &["city", "today", "tomorrow", "yesterday"];
    const LOC: &[&str] = &["city"];
    const NEWS: &[&str] = &["city", "query"];
    const NONE: &[&str] = &[];
    Some(match name {
        "weather" | "air" | "sun" | "sunrise" | "sunset" | "dawn" | "dusk" | "solar_noon" => WX,
        "location" => LOC, "news" => NEWS,
        "breaking" | "phase" | "time" | "date" | "now" | "today" | "tomorrow" | "yesterday" | "style" | "location_full" => NONE,
        n if n.starts_with("image") && n[5..].parse::<u32>().is_ok() => NONE,
        _ => return None,
    })
}
const VALUE_KW: [&str; 2] = ["city", "query"];
pub const COMMANDS: [&str; 19] = ["weather", "location", "sun", "air", "news", "breaking", "phase", "dawn", "sunrise", "solar_noon", "sunset", "dusk",
    "date", "now", "today", "tomorrow", "yesterday", "time", "style"];

fn call_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?:^|[\s(\[,;:"'])(/([a-z][a-z0-9_]*)((?:/[A-Za-z0-9_+%-]+)*))|\{\{\s*([a-z][a-z0-9_]*)\s*\}\}"#).unwrap())
}
pub fn parse(text: &str) -> Vec<Call> {
    call_re().captures_iter(text).map(|c| {
        if let Some(m) = c.get(1) {
            let args = c.get(3).map(|a| a.as_str().split('/').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect()).unwrap_or_default();
            Call { name: c[2].to_string(), args, start: m.start(), end: m.end() }
        } else { let m = c.get(0).unwrap(); Call { name: c[4].to_string(), args: vec![], start: m.start(), end: m.end() } }
    }).collect()
}
/// Parsed options of a call: (city, query, day offset)
#[derive(Debug, Default, PartialEq)]
pub struct Opts { pub city: Option<String>, pub query: Option<String>, pub day: i64 }
pub fn validate(c: &Call, images: usize) -> Result<Opts, String> {
    let Some(sp) = spec(&c.name) else { return Err(format!("unknown command /{}", c.name)) };
    if let Some(n) = c.name.strip_prefix("image").and_then(|n| n.parse::<usize>().ok()) { if n == 0 || n > images { return Err(format!("/{}: no reference image {n} in settings.images", c.name)); } }
    let mut o = Opts::default(); let mut i = 0;
    while i < c.args.len() {
        let a = c.args[i].as_str();
        if !sp.contains(&a) { return Err(format!("/{}: invalid argument '{a}' (accepted: {})", c.name, if sp.is_empty() { "none".into() } else { sp.join(", ") })); }
        if VALUE_KW.contains(&a) {
            let Some(v) = c.args.get(i + 1) else { return Err(format!("/{}: '{a}' needs a value, e.g. /{}/{a}/Genova", c.name, c.name)) };
            let v = v.replace(['_', '+'], " ").replace("%20", " ");
            if a == "city" { o.city = Some(v) } else { o.query = Some(v) }
            i += 2;
        } else { o.day = match a { "tomorrow" => 1, "yesterday" => -1, _ => 0 }; i += 1; }
    }
    Ok(o)
}
/// Suggestions for the segment being typed. `token` is the partial call, e.g. "/weather/ci".
pub fn complete(token: &str, extra: &[String]) -> Vec<String> {
    let segs: Vec<&str> = token.trim_start_matches('/').split('/').collect();
    if segs.len() <= 1 {
        let p = segs.first().copied().unwrap_or("");
        return COMMANDS.iter().map(|s| s.to_string()).chain(extra.iter().cloned()).filter(|c| c.starts_with(p) && c != p).collect();
    }
    let Some(sp) = spec(segs[0]) else { return vec![] };
    let done = &segs[1..segs.len() - 1]; let cur = segs[segs.len() - 1];
    if let Some(prev) = done.last() { if VALUE_KW.contains(prev) && !done[..done.len() - 1].last().map(|p| VALUE_KW.contains(p)).unwrap_or(false) { return vec![]; } }
    let used_day = done.iter().any(|d| DAY.contains(d));
    sp.iter().filter(|k| !done.contains(k) && !(used_day && DAY.contains(k)) && k.starts_with(cur) && **k != cur).map(|k| k.to_string()).collect()
}
pub fn errors(text: &str, images: usize) -> Vec<String> {
    let mut e: Vec<String> = parse(text).iter().filter_map(|c| validate(c, images).err()).collect(); e.dedup(); e
}

// ---------- resolution (network results cached for the process lifetime = one run) ----------
fn cache() -> &'static Mutex<HashMap<String, Value>> { static C: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new(); C.get_or_init(Default::default) }
fn cached_json(url: &str) -> Result<Value, String> {
    if let Some(v) = cache().lock().unwrap().get(url) { return Ok(v.clone()); }
    let v = live::get_json(url, 2)?; cache().lock().unwrap().insert(url.into(), v.clone()); Ok(v)
}
fn enc(s: &str) -> String { s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect() }
pub struct Place { pub name: String, pub lat: f64, pub lon: f64, pub tz: f64 }
fn place(s: &Value, city: &Option<String>) -> Result<Place, String> {
    let local_tz = Local::now().offset().local_minus_utc() as f64 / 3600.0;
    let Some(c) = city else { return Ok(Place { name: s["location"]["name"].as_str().unwrap_or("Dubai").into(), lat: s["location"]["lat"].as_f64().unwrap_or(25.2048), lon: s["location"]["lon"].as_f64().unwrap_or(55.2708), tz: local_tz }) };
    // local-language names (Genova, München…) are only indexed in that language: search a few, keep the most populous match
    let mut all: Vec<Value> = vec![];
    for lang in ["en", "it", "fr", "es", "de", "ar"] {
        if let Ok(g) = cached_json(&format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language={lang}&format=json", enc(c))) {
            all.extend(g["results"].as_array().cloned().unwrap_or_default());
        }
        if all.iter().any(|r| r["population"].as_i64().unwrap_or(0) > 100_000) { break; }
    }
    let r = all.iter().max_by_key(|r| r["population"].as_i64().unwrap_or(0)).ok_or(format!("city '{c}' not found"))?;
    let (lat, lon) = (r["latitude"].as_f64().unwrap_or(0.0), r["longitude"].as_f64().unwrap_or(0.0));
    let off = cached_json(&format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=is_day&timezone=auto")).ok()
        .and_then(|v| v["utc_offset_seconds"].as_f64()).map(|x| x / 3600.0).unwrap_or(local_tz);
    Ok(Place { name: format!("{}, {}", r["name"].as_str().unwrap_or(c), r["country"].as_str().unwrap_or("")), lat, lon, tz: off })
}
fn day(o: &Opts) -> NaiveDate { Local::now().date_naive() + Duration::days(o.day) }
fn f(v: &Value) -> f64 { v.as_f64().unwrap_or(0.0) }

fn weather_for(s: &Value, o: &Opts) -> Result<String, String> {
    let p = place(s, &o.city)?;
    if o.day == 0 {
        let d = json!({"weather": cached_json(&format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&timezone=auto&current=temperature_2m,apparent_temperature,relative_humidity_2m,cloud_cover,visibility,wind_speed_10m,wind_direction_10m,wind_gusts_10m,uv_index,weather_code", p.lat, p.lon))?["current"],
                       "air": cached_json(&format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=us_aqi,dust", p.lat, p.lon)).map(|a| a["current"].clone()).unwrap_or(json!({}))});
        return Ok(format!("{}: {}", p.name, live::weather_text(&d)));
    }
    let v = cached_json(&format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&timezone=auto&past_days=1&forecast_days=3&daily=temperature_2m_max,temperature_2m_min,weather_code,precipitation_sum,wind_speed_10m_max,uv_index_max,cloud_cover_mean", p.lat, p.lon))?;
    let i = (o.day + 1) as usize; let dd = &v["daily"];
    Ok(format!("{} on {}: {:.0}–{:.0}°C, {}, {:.0}% cloud, precipitation {:.1} mm, wind up to {:.0} km/h, UV {:.0}", p.name, day(o).format("%a %-d %b"),
        f(&dd["temperature_2m_min"][i]), f(&dd["temperature_2m_max"][i]), live::condition(dd["weather_code"][i].as_i64().unwrap_or(0)),
        f(&dd["cloud_cover_mean"][i]), f(&dd["precipitation_sum"][i]), f(&dd["wind_speed_10m_max"][i]), f(&dd["uv_index_max"][i])))
}
fn air_for(s: &Value, o: &Opts) -> Result<String, String> {
    let p = place(s, &o.city)?;
    let v = cached_json(&format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&timezone=auto&past_days=1&forecast_days=2&current=us_aqi,pm10,pm2_5,dust,aerosol_optical_depth&hourly=us_aqi,pm10,pm2_5,dust,aerosol_optical_depth", p.lat, p.lon))?;
    let a = if o.day == 0 { v["current"].clone() } else {
        let i = ((o.day + 1) * 24 + 12) as usize; let h = &v["hourly"];
        json!({"us_aqi": h["us_aqi"][i], "pm10": h["pm10"][i], "pm2_5": h["pm2_5"][i], "dust": h["dust"][i], "aerosol_optical_depth": h["aerosol_optical_depth"][i]}) };
    Ok(format!("{}{}: {}", p.name, if o.day == 0 { String::new() } else { format!(" ({} noon)", day(o).format("%a %-d %b")) }, live::air_text(&json!({"air": a}))))
}
fn sun_for(s: &Value, o: &Opts) -> Result<String, String> {
    let p = place(s, &o.city)?;
    let local_tz = Local::now().offset().local_minus_utc() as f64 / 3600.0;
    // same instant shifted to the place's local clock
    let now = Local::now().naive_local() + Duration::minutes(((p.tz - local_tz) * 60.0) as i64) + Duration::days(o.day);
    let (el, az) = core::sun_pos_tz(now, p.lat, p.lon, p.tz);
    let mut t = live::sun_text(&json!({"sun": {"elevation": el, "azimuth": az}}));
    if o.city.is_some() || o.day != 0 { t = format!("{} at {}: {t}", p.name, now.format("%H:%M %a %-d %b")); }
    Ok(t)
}
fn event_for(s: &Value, o: &Opts, key: &str) -> Result<String, String> {
    let p = place(s, &o.city)?;
    let ev = core::sun_events_at(day(o), p.lat, p.lon, p.tz);
    let t = ev[key].as_str().unwrap_or("—").to_string();
    Ok(if o.city.is_some() || o.day != 0 { format!("{t} ({}, {})", p.name, day(o).format("%a %-d %b")) } else { t })
}

/// Resolve one call. `base` = the no-argument values (from cached live data).
pub fn resolve(c: &Call, s: &Value, base: &[(String, String)]) -> Result<String, String> {
    let images = s["images"].as_array().map(|a| a.len()).unwrap_or(0);
    let o = validate(c, images)?;
    let plain = o == Opts::default();
    let base_val = || base.iter().find(|(k, _)| *k == c.name).map(|(_, v)| v.clone());
    let fmt_day = |d: i64| (Local::now().date_naive() + Duration::days(d)).format("%A %-d %B %Y").to_string();
    match c.name.as_str() {
        "today" | "date" => Ok(fmt_day(0)), "tomorrow" => Ok(fmt_day(1)), "yesterday" => Ok(fmt_day(-1)),
        "now" => Ok(Local::now().format("%A %-d %B %Y, %H:%M").to_string()),
        _ if plain => base_val().ok_or(format!("/{} has no value", c.name)),
        "weather" => weather_for(s, &o), "air" => air_for(s, &o), "sun" => sun_for(s, &o),
        "dawn" => event_for(s, &o, "civil_dawn"), "dusk" => event_for(s, &o, "civil_dusk"),
        "sunrise" | "sunset" | "solar_noon" => event_for(s, &o, &c.name),
        "location" => place(s, &o.city).map(|p| format!("{} ({:.4}, {:.4})", p.name, p.lat, p.lon)),
        "news" => {
            let q = o.query.clone().or(o.city.clone()).unwrap_or_default();
            let url = format!("https://news.google.com/rss/search?q={}+when:1d&hl=en-AE&gl=AE&ceid=AE:en", enc(&q));
            let h = live::headline(&json!({"feeds": [{"name": q, "url": url, "enabled": true}]})).0;
            Ok(if h.is_empty() { "abstract glowing patterns with no readable text".into() } else { format!("\"{h}\"") })
        }
        _ => base_val().ok_or(format!("/{} has no value", c.name)),
    }
}
/// Expand all calls in `text`; failures are left in place with a [!error] note.
pub fn expand(text: &str, s: &Value, base: &[(String, String)]) -> String {
    let mut out = String::new(); let mut last = 0;
    for c in parse(text) {
        out.push_str(&text[last..c.start]);
        match resolve(&c, s, base) { Ok(v) => out.push_str(&v), Err(e) => out.push_str(&format!("{} [!{e}]", &text[c.start..c.end])) }
        last = c.end;
    }
    out.push_str(&text[last..]); out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parses_plain_and_args() {
        let c = parse("light: /sun and /weather/city/Genova/tomorrow, then /news.");
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["sun", "weather", "news"]);
        assert_eq!(c[1].args, ["city", "Genova", "tomorrow"]);
        assert_eq!(&"light: /sun and /weather/city/Genova/tomorrow, then /news."[c[1].start..c[1].end], "/weather/city/Genova/tomorrow");
    }
    #[test] fn ignores_paths_and_ratios() { assert!(parse("a 16:9 image at ~/Pictures/x.png or http://a/b").is_empty()); }
    #[test] fn legacy_braces() { let c = parse("{{weather}} x"); assert_eq!(c[0].name, "weather"); assert!(c[0].args.is_empty()); }
    #[test] fn validates() {
        let v = |t: &str| validate(&parse(t)[0], 1);
        assert_eq!(v("/weather/city/New_York/tomorrow").unwrap(), Opts { city: Some("New York".into()), query: None, day: 1 });
        assert_eq!(v("/sunrise/yesterday").unwrap().day, -1);
        assert!(v("/weather/city").is_err());
        assert!(v("/weather/banana").is_err());
        assert!(v("/phase/tomorrow").is_err());
        assert!(v("/bogus").is_err());
        assert!(v("/image1").is_ok()); assert!(v("/image2").is_err());
        assert_eq!(v("/news/query/expo_city").unwrap().query.as_deref(), Some("expo city"));
    }
    #[test] fn completes() {
        assert!(complete("/wea", &[]).contains(&"weather".to_string()));
        assert_eq!(complete("/weather/ci", &[]), ["city"]);
        assert!(complete("/weather/city/Gen", &[]).is_empty());
        let n = complete("/weather/city/Genova/", &[]); assert!(n.contains(&"tomorrow".to_string()) && !n.contains(&"city".to_string()));
        assert!(complete("/phase/", &[]).is_empty());
        assert_eq!(complete("/news/", &[]), ["city", "query"]);
    }
    #[test] fn errors_listed() { assert_eq!(errors("/sun /nope /weather/x", 0).len(), 2); }
}
