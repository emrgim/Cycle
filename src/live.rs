//! Live data (Open-Meteo weather / air quality / marine, RSS news), prompt building and `cycle generate`.
use crate::core::{self, home, iso_now, rd, settings, wr};
use chrono::{Local, Timelike};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into()
}
pub fn get_text(url: &str, tries: u32) -> Result<String, String> {
    let mut err = String::new();
    for i in 0..tries.max(1) {
        match agent().get(url).header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) Cycle/2").call() {
            Ok(mut r) => match r.body_mut().read_to_string() { Ok(s) => return Ok(s), Err(e) => err = e.to_string() },
            Err(e) => err = e.to_string(),
        }
        if i + 1 < tries { std::thread::sleep(Duration::from_secs(10 * (i as u64 + 1))); }
    }
    Err(err)
}
pub fn get_json(url: &str, tries: u32) -> Result<Value, String> {
    get_text(url, tries).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
}

/// Full live dataset for the configured location.
pub fn fetch(s: &Value) -> Value {
    let (lat, lon) = (s["location"]["lat"].as_f64().unwrap_or(25.2048), s["location"]["lon"].as_f64().unwrap_or(55.2708));
    let tz = "Asia%2FDubai";
    let w = get_json(&format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&timezone={tz}&current=temperature_2m,apparent_temperature,relative_humidity_2m,dew_point_2m,precipitation,rain,cloud_cover,cloud_cover_low,cloud_cover_mid,cloud_cover_high,visibility,wind_speed_10m,wind_direction_10m,wind_gusts_10m,pressure_msl,uv_index,is_day,weather_code"), 4);
    let a = get_json(&format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&timezone={tz}&current=us_aqi,pm10,pm2_5,dust,aerosol_optical_depth,ozone"), 4);
    let m = get_json(&format!("https://marine-api.open-meteo.com/v1/marine?latitude={:.2}&longitude={:.2}&timezone={tz}&current=wave_height,sea_surface_temperature", (lat * 4.0).round() / 4.0, (lon * 4.0).round() / 4.0), 2);
    let now = Local::now().naive_local(); let (el, az) = core::sun_pos(now, lat, lon);
    let mut errs = vec![];
    let pick = |r: Result<Value, String>, errs: &mut Vec<String>, n: &str| match r { Ok(v) => v["current"].clone(), Err(e) => { errs.push(format!("{n}: {e}")); json!({}) } };
    json!({"weather": pick(w, &mut errs, "forecast"), "air": pick(a, &mut errs, "air"), "marine": pick(m, &mut errs, "marine"),
           "sun": {"elevation": (el * 10.0).round() / 10.0, "azimuth": az.round()}, "fetched_at": iso_now(), "errors": errs})
}

fn used_path() -> std::path::PathBuf { core::base().join("headlines-used.txt") }
fn used() -> Vec<String> { fs::read_to_string(used_path()).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect() }
fn mark_used(full: &str) { let mut u = used(); if !u.iter().any(|x| x == full) { u.push(full.to_string()); let n = u.len(); let keep = &u[n.saturating_sub(300)..]; let _ = fs::write(used_path(), keep.join("\n") + "\n"); } }
fn feed_titles(url: &str) -> Vec<String> {
    let Ok(xml) = get_text(url, 2) else { return vec![] };
    let Ok(doc) = roxmltree::Document::parse(&xml) else { return vec![] };
    doc.descendants().filter(|n| n.has_tag_name("item") || n.tag_name().name() == "entry")
        .filter_map(|i| i.children().find(|c| c.tag_name().name() == "title").and_then(|c| c.text()).map(clean_full))
        .filter(|h| !h.is_empty() && is_english(h)).collect()
}
/// First English headline from the enabled feeds that has not been shown before (headlines-used.txt),
/// falling back to the newest one when all were used → (display headline ≤60 chars, feed name). Marks it used.
pub fn headline(s: &Value) -> (String, String) { headline_ex(s, false) }
pub fn headline_ex(s: &Value, mark: bool) -> (String, String) {
    let feeds = s["feeds"].as_array().cloned().unwrap_or_default();
    let u = used(); let mut first: Option<(String, String)> = None;
    for f in feeds.iter().filter(|f| f["enabled"].as_bool().unwrap_or(true)) {
        let Some(url) = f["url"].as_str() else { continue };
        let name = f["name"].as_str().unwrap_or(url).to_string();
        for t in feed_titles(url) {
            if first.is_none() { first = Some((t.clone(), name.clone())); }
            if !u.contains(&t) { if mark { mark_used(&t); } return (trim60(&t), name); }
        }
    }
    first.map(|(t, n)| (trim60(&t), n)).unwrap_or_default()
}
pub const TOP_FEED: &str = "https://news.google.com/rss?hl=en-AE&gl=AE&ceid=AE:en";
pub fn breaking(s: &Value) -> String {
    let top = s["news_gl"].as_str().map(|cc| format!("https://news.google.com/rss?hl=en-{cc}&gl={cc}&ceid={cc}:en")).unwrap_or(TOP_FEED.to_string());
    match feed_titles(&top).into_iter().next() { Some(t) => trim60(&t), None => headline(s).0 }
}
pub fn clean_full(t: &str) -> String {
    // untrusted feed text: drop control chars and placeholder/shell metacharacters
    let t: String = t.chars().filter(|c| !c.is_control() && !matches!(c, '{' | '}' | '`' | '$' | '\\')).collect();
    let t = Regex::new(r"\s+-\s+[^-]+$").unwrap().replace(t.trim(), "").replace('"', "'");
    Regex::new(r"^[A-Z]{2,6}\s*:\s*").unwrap().replace(t.trim(), "").trim().to_string()
}
pub fn trim60(t: &str) -> String {
    let t = t.to_string();
    if t.chars().count() <= 60 { return t; }
    let cut: String = t.chars().take(60).collect();
    match cut.rfind(' ') { Some(i) => format!("{}…", cut[..i].trim_end_matches([',', ':', ';', '-', ' '])), None => format!("{cut}…") }
}
fn is_english(t: &str) -> bool {
    let letters: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    !letters.is_empty() && letters.iter().filter(|c| c.is_ascii()).count() * 10 >= letters.len() * 9
}

fn f(v: &Value) -> f64 { v.as_f64().unwrap_or(0.0) }
fn compass(d: f64) -> &'static str { ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][(((d + 22.5) % 360.0) / 45.0) as usize % 8] }
pub fn condition(code: i64) -> &'static str {
    match code { 0 => "clear sky", 1 => "mainly clear", 2 => "partly cloudy", 3 => "overcast", 45 | 48 => "fog", 51..=57 => "drizzle",
                 61 | 63 => "rain", 65 => "heavy rain", 80..=82 => "showers", 95..=99 => "thunderstorm", _ => "cloudy" }
}
pub fn weather_text(d: &Value) -> String {
    let w = &d["weather"]; let a = &d["air"];
    if w.get("temperature_2m").is_none() { return "weather unavailable".into(); }
    format!("{:.0}°C (feels {:.0}°C), {}% humidity, {}, {}% cloud, visibility {:.0} km, wind {:.0} km/h {} (gusts {:.0}), UV {:.1}, US AQI {}, dust {:.0} µg/m³",
        f(&w["temperature_2m"]), f(&w["apparent_temperature"]), w["relative_humidity_2m"], condition(w["weather_code"].as_i64().unwrap_or(0)),
        w["cloud_cover"], f(&w["visibility"]) / 1000.0, f(&w["wind_speed_10m"]), compass(f(&w["wind_direction_10m"])), f(&w["wind_gusts_10m"]),
        f(&w["uv_index"]), a["us_aqi"], f(&a["dust"]))
}
/// Sun-driven lighting description from elevation/azimuth.
pub fn sun_text(d: &Value) -> String {
    let (el, az) = (f(&d["sun"]["elevation"]), f(&d["sun"]["azimuth"]));
    let shadow = compass((az + 180.0) % 360.0);
    let (light, sky) = if el < -6.0 { ("no direct sunlight, city lights and LED glow as the main light", "deep blue-black night sky") }
        else if el < 0.0 { ("civil twilight, soft indirect light, no direct sun", "deep blue to violet twilight sky with a pale glow at the horizon") }
        else if el < 10.0 { ("very low golden sun, long raking light, long shadows", "warm amber and pink sky near the horizon fading to soft blue") }
        else if el < 30.0 { ("low warm sun, clear directional light, long shadows", "warm pale gold sky fading to blue") }
        else if el < 50.0 { ("bright sun, crisp directional light, medium shadows", "clear bright blue sky") }
        else { ("high near-white sun, strong overhead light, short shadows", "bright pale white-blue midday sky") };
    let phase = if el < -6.0 { "night" } else if el < -0.833 { "twilight" } else if az < 180.0 { if el < 10.0 { "sunrise / early morning" } else { "morning" } } else if el < 10.0 { "sunset / late afternoon" } else { "afternoon" };
    format!("{phase}, sun at {el:.0}° elevation from the {} (azimuth {az:.0}°), {light}, shadows fall toward the {shadow}, {sky}", compass(az))
}
pub fn air_text(d: &Value) -> String {
    let a = &d["air"];
    if a.get("us_aqi").is_none() { return "air quality unavailable".into(); }
    let aqi = f(&a["us_aqi"]);
    let look = if f(&a["dust"]) > 80.0 || aqi > 150.0 { "visibly dusty air, desert haze softening distant towers" } else if aqi > 100.0 { "light dusty haze in the distance" } else { "clear clean air" };
    format!("US AQI {aqi:.0}, PM2.5 {:.0} µg/m³, PM10 {:.0} µg/m³, dust {:.0} µg/m³, aerosol optical depth {:.2} ({look})", f(&a["pm2_5"]), f(&a["pm10"]), f(&a["dust"]), f(&a["aerosol_optical_depth"]))
}
pub fn location_text(s: &Value, _d: &Value) -> String {
    format!("{} ({:.4}°N, {:.4}°E)", s["location"]["name"].as_str().unwrap_or("Dubai"), f(&s["location"]["lat"]), f(&s["location"]["lon"]))
}

// ---------- prompt composition language ----------
// A variable is `/name` (or `{{name}}`). Names: lowercase letters, digits, `_`.
// A `/` inside a word (paths, URLs, 16:9) is not a variable: it must start the text or follow a space or ( [ , ; : " '.
pub const VARS: [(&str, &str); 19] = [
    ("breaking", "top/breaking headline (Google News top stories, falls back to /news)"), ("now", "date and time now"), ("today", "today's date"), ("tomorrow", "tomorrow's date"), ("yesterday", "yesterday's date"),
    ("weather", "live weather summary"), ("location", "place name and coordinates"), ("sun", "sun phase, elevation, azimuth, shadows, sky colour"),
    ("air", "air quality (AQI, PM, dust)"), ("news", "LED billboard headline from the enabled feeds"), ("phase", "day phase: morning / afternoon / evening / night"),
    ("dawn", "civil dawn (sun at −6°)"), ("sunrise", "sunrise (−0.833°)"), ("solar_noon", "solar noon (meridian transit)"), ("sunset", "sunset (−0.833°)"),
    ("dusk", "civil dusk (−6°)"), ("time", "current local time"), ("date", "today's date"), ("style", "text of the style preset in settings.prompt_style"),
];
fn var_re() -> Regex { Regex::new(r#"(^|[\s(\[,;:"'])/([a-z][a-z0-9_]*)|\{\{\s*([a-z][a-z0-9_]*)\s*\}\}"#).unwrap() }

/// Every variable with its current value. /image1… come from settings.images (reference image paths).
pub fn variables(s: &Value, d: &Value, news: &str, breaking: &str) -> Vec<(String, String)> {
    let now = Local::now().naive_local();
    let ev = core::sun_events(now.date(), s);
    let (phase, _) = core::phase_at(now, s, &(String::new(), String::new()));
    let t = |k: &str| ev[k].as_str().unwrap_or("—").to_string();
    let style_name = s["prompt_style"].as_str().unwrap_or("gits-clean");
    let mut v = vec![
        ("weather".to_string(), weather_text(d)), ("location".into(), location_text(s, d)), ("sun".into(), sun_text(d)), ("air".into(), air_text(d)),
        ("news".into(), if news.is_empty() { "abstract glowing patterns with no readable text".into() } else { format!("\"{news}\"") }),
        ("breaking".into(), if breaking.is_empty() { if news.is_empty() { "abstract glowing patterns with no readable text".into() } else { format!("\"{news}\"") } } else { format!("\"{breaking}\"") }),
        ("phase".into(), phase), ("dawn".into(), t("civil_dawn")), ("sunrise".into(), t("sunrise")), ("solar_noon".into(), t("solar_noon")),
        ("sunset".into(), t("sunset")), ("dusk".into(), t("civil_dusk")), ("time".into(), now.format("%H:%M").to_string()),
        ("date".into(), now.format("%A %-d %B %Y").to_string()), ("style".into(), core::prompt_text(style_name)),
    ];
    for (i, img) in s["images"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        if let Some(p) = img.as_str() { v.push((format!("image{}", i + 1), format!("the reference image at {p}"))); }
    }
    v.push(("location_full".into(), location_text(s, d))); // legacy {{location_full}}
    v
}
pub fn expand(text: &str, s: &Value, d: &Value, news: &str) -> String { crate::lang::expand(text, s, &variables(s, d, news, d["breaking"].as_str().unwrap_or(""))) }
pub fn has_placeholders(t: &str) -> bool { var_re().is_match(t) }
/// Variables from cached live data (never blocks on the network) – for the editor.
pub fn cached_vars(s: &Value) -> Vec<(String, String)> {
    let d = rd(&core::weather_now_path()).unwrap_or(json!({}));
    let mut d2 = d.clone();
    { let (la, lo) = (f(&s["location"]["lat"]), f(&s["location"]["lon"])); let (el, az) = core::sun_pos(Local::now().naive_local(), la, lo); d2["sun"] = json!({"elevation": (el*10.0).round()/10.0, "azimuth": az.round()}); }
    variables(s, &d2, d["headline"].as_str().unwrap_or(""), d["breaking"].as_str().unwrap_or(""))
}
/// Expand placeholders at generation time using cached live data (fetches only if needed).
pub fn expand_placeholders(t: &str, s: &Value) -> String {
    if !has_placeholders(t) { return t.to_string(); }
    let d = rd(&core::weather_now_path()).filter(|d| d["weather"].get("temperature_2m").is_some()).unwrap_or_else(|| fetch(s));
    let base = variables(s, &d, d["headline"].as_str().unwrap_or(""), d["breaking"].as_str().unwrap_or(""));
    crate::lang::expand(t, s, &base)
}

pub fn build_prompt(s: &Value, d: &Value, hl: &str) -> String { build_prompt_named(s, d, hl, core::active_prompt()) }
pub fn build_prompt_named(s: &Value, d: &Value, hl: &str, sn: String) -> String {
    let (w, a) = (&d["weather"], &d["air"]);
    let h = Local::now().hour();
    let tod = if h < 5 || h >= 20 { "deep night" } else if h < 7 { "dawn" } else if h < 11 { "morning" } else if h < 15 { "midday" } else if h < 17 { "afternoon" } else { "dusk" };
    let cc = f(&w["cloud_cover"]);
    let sky = if cc < 20.0 { "clear sky" } else if cc < 60.0 { "scattered clouds" } else { "overcast sky" };
    let haze = if sn.contains("clean") { "faint distant atmospheric haze only" }
        else if f(&a["dust"]) > 80.0 || f(&a["us_aqi"]) > 100.0 { "thick dusty haze" }
        else if f(&w["visibility"]) < 20000.0 || f(&w["relative_humidity_2m"]) > 60.0 { "soft atmospheric haze" } else { "light haze" };
    let ws = f(&w["wind_speed_10m"]);
    let wind = if ws > 30.0 { "strong wind blowing dust" } else if ws < 8.0 { "still air" } else { "light breeze" };
    let raining = f(&w["rain"]) > 0.0 || f(&w["precipitation"]) > 0.0 || matches!(w["weather_code"].as_i64().unwrap_or(0), 51..=67 | 80..=82 | 95..=99);
    let wet = if raining { "light rain falling as the live data reports, wet reflective streets" } else { "completely dry, no rain, no wet surfaces, no puddles" };
    let sun = sun_text(d);
    let raw = core::prompt_text(&sn);
    if raw.contains("/news") || raw.contains("/breaking") || raw.contains("/sun") || raw.contains("/weather") {
        // full prompt with tags: tags carry all live data
        let mut p = expand(&raw, s, d, hl);
        if !raining { p += " Completely dry: no rain, no wet surfaces, no puddles."; }
        return p;
    }
    let style = expand(&raw, s, d, hl);
    let board = if hl.is_empty() { ", the LED billboard shows abstract glowing patterns with no readable text".to_string() }
        else { format!(", the LED billboard displays the headline \"{hl}\" in bright LED lettering, this headline is the only text in the image") };
    format!("A wide 16:9 desktop wallpaper: {} high-rise skyline at {tod} ({h:02}:00), {sky} ({cc:.0}% cloud), {sun}, {wet}, {haze}, {wind}, {:.0}°C and {}% humidity, {style}  One large rooftop/facade LED billboard wall (the LED text fills the entire screen surface edge to edge, huge letters, no empty margins) mounted on a tower, a real architectural element of the scene like Dubai's Sheikh Zayed Road screens, glowing and casting light, drawn in the same technical linework{board}. No characters, no other text, no logos",
        s["location"]["name"].as_str().unwrap_or("Dubai"), f(&w["temperature_2m"]), w["relative_humidity_2m"])
}

fn notify(msg: &str) { core::shv(&["omarchy-notification-send", "-u", "low", msg], 10); }

/// `cycle generate`: live data → prompt → image → Cycle theme applied → archived. Returns exit code.
pub fn generate(scheduled: bool, prompt: Option<&str>) -> i32 { generate_at(scheduled, prompt, None) }
pub fn generate_at(scheduled: bool, prompt: Option<&str>, apply_at: Option<chrono::NaiveDateTime>) -> i32 {
    let Some(_lock) = crate::sched::Lock::acquire() else { println!("another generation is running – skipped"); return 0 };
    if let Some(p) = prompt { if settings()["prompt_here"].as_array().map(|a| a.iter().any(|x| x == p)).unwrap_or(false) { unsafe { std::env::set_var("CYCLE_HERE", "1"); } } }
    let s = settings();
    if std::env::var("CYCLE_HERE").is_ok() { println!("here: {} ({}, {}) via {}", s["location"]["name"], s["location"]["lat"], s["location"]["lon"], s["location"]["source"]); }
    let no_apply = std::env::var("CYCLE_NO_APPLY").is_ok();
    if let Some(p) = prompt { crate::sched::mark(p, "running", None); }
    let h = Local::now().hour() as i64;
    let (from, to) = (s["schedule"]["from"].as_i64().unwrap_or(6), s["schedule"]["to"].as_i64().unwrap_or(18));
    if scheduled && !(from..=to).contains(&h) { println!("night pause ({h:02}:00 outside {from:02}–{to:02}), skipping"); return 0; }
    let mut d = fetch(&s);
    if d["weather"].get("temperature_2m").is_none() {
        if let Some(prev) = rd(&core::weather_now_path()) { println!("live weather failed ({}), using last data", d["errors"]); d["weather"] = prev["weather"].clone(); }
    }
    let (hl, src) = match std::env::var("CYCLE_HEADLINE") { Ok(h) if !h.trim().is_empty() => (h, "override".to_string()), _ => headline_ex(&s, true) }; let brk = breaking(&s);
    d["breaking"] = json!(brk);
    println!("headline: {}", if hl.is_empty() { "(none – abstract billboard)" } else { &hl });
    // CYCLE_PROMPT_TEXT: a one-off prompt text (same slash language), still ONE prompt to the generator – nothing is composited
    let p = match std::env::var("CYCLE_PROMPT_TEXT") { Ok(t) if !t.trim().is_empty() => expand(&t, &s, &d, &hl),
        _ => build_prompt_named(&s, &d, &hl, prompt.map(|x| x.to_string()).unwrap_or_else(core::active_prompt)) };
    core::update_settings(|s| s["generator"]["template"] = json!(p));
    d["prompt"] = json!(p); d["headline"] = json!(hl); d["headline_source"] = json!(src);
    wr(&core::weather_now_path(), &d);
    let res = core::resolve(false);
    let mut ctx = core::theme_context(&res); ctx["condition"] = json!(condition(d["weather"]["weather_code"].as_i64().unwrap_or(0)));
    if no_apply {
        // archive only: the wallpaper and the Cycle theme stay untouched
        let t0 = std::time::Instant::now();
        let img = core::generate(prompt.unwrap_or("cycle"), &ctx, &settings());
        let secs = t0.elapsed().as_secs();
        if let Some(p) = prompt { crate::sched::record_run(p, None, secs, img.is_some()); }
        return match img {
            Some(img) => {
                let dir = home().join("Pictures/desktop").join(Local::now().format("%Y-%m-%d").to_string()); let _ = fs::create_dir_all(&dir);
                let slug = prompt.unwrap_or("cycle").trim_end_matches("-here-now").to_string();
                let dst = dir.join(format!("{}-{slug}.png", Local::now().format("%H%M"))); let _ = fs::copy(&img, &dst);
                println!("archived (not applied) → {} in {secs}s", dst.display());
                if let Some(p) = prompt { crate::sched::mark(p, "ok (archived, not applied)", Some(&dst.to_string_lossy())); } 0
            }
            None => { if let Some(p) = prompt { crate::sched::mark(p, "failed (generator timed out or errored)", None); } println!("generation failed after {secs}s"); 1 }
        };
    }
    let res = core::generate_cycle_theme_at(Some(ctx), Some(true), true, apply_at);
    let secs = core::LAST_GEN_SECS.load(std::sync::atomic::Ordering::Relaxed);
    if let Some(p) = prompt { crate::sched::record_run(p, apply_at, secs, res.is_some()); }
    match res {
        Some(bg) if fs::metadata(&bg).and_then(|m| m.modified()).map(|t| t.elapsed().map(|e| e.as_secs() < 3600).unwrap_or(false)).unwrap_or(false) => {
            let dir = home().join("Pictures/desktop").join(Local::now().format("%Y-%m-%d").to_string());
            let _ = fs::create_dir_all(&dir);
            let dst = dir.join(format!("{}.png", Local::now().format("%H%M")));
            let _ = fs::copy(&bg, &dst);
            println!("archived → {}", dst.display()); notify("Cycle: new wallpaper applied");
            if let Some(p) = prompt { crate::sched::mark(p, "ok", Some(&dst.to_string_lossy())); } 0
        }
        _ => { notify("Cycle: image generation failed – kept current wallpaper"); if let Some(p) = prompt { crate::sched::mark(p, "failed (generator timed out or errored)", None); } 1 }
    }
}

#[cfg(test)]
mod headline_tests {
    #[test]
    fn clean_full_strips_dangerous_chars() {
        let c = super::clean_full("Breaking'$(touch /tmp/pwned)'{prompt}`id`\u{7}\nnews");
        assert!(!c.contains('$') && !c.contains('{') && !c.contains('`') && !c.chars().any(|x| x.is_control()), "{c}");
    }
}
