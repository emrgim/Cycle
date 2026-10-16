//! Config, phase resolution, conditions, circadian model, apply, generation, Cycle theme.
use chrono::{Datelike, Local, NaiveDateTime, Timelike};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const PHASES: [&str; 4] = ["morning", "afternoon", "evening", "night"];

pub fn home() -> PathBuf { PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into())) }
pub fn base() -> PathBuf { home().join(".config/omarchy/cycle") }
pub fn presets_dir() -> PathBuf { base().join("presets") }
pub fn cycles_dir() -> PathBuf { base().join("cycles") }
pub fn styles_dir() -> PathBuf { base().join("styles") }
pub fn settings_path() -> PathBuf { base().join("settings.json") }
pub fn weather_now_path() -> PathBuf { base().join("weather-now.json") }
pub fn state_dir() -> PathBuf { let p = home().join(".local/state/cycle"); let _ = fs::create_dir_all(&p); p }
pub fn gen_dir() -> PathBuf { home().join(".local/share/cycle/generated") }
pub fn theme_dir() -> PathBuf { home().join(".config/omarchy/themes/cycle") }
fn disp() -> String { home().join(".local/bin/omarchy-display").to_string_lossy().into_owned() }
pub fn now_ts() -> f64 { Local::now().timestamp_millis() as f64 / 1000.0 }

pub const DEFAULT_FEED: &str = "https://news.google.com/rss/search?q=Dubai+when:1d&hl=en-AE&gl=AE&ceid=AE:en";

pub fn default_settings() -> Value {
    json!({
        "enabled": false,
        "hold": null,
        "circadian": {"k_night": 2700, "k_noon": 6500, "b_min": 30, "b_max": 100, "twilight": -6},
        "location": {"name": "Dubai", "lat": 25.2048, "lon": 55.2708, "source": "fixed"},
        "times": {"morning": "sunrise", "afternoon": "solar_noon", "evening": "sunset", "night": "civil_dusk"},
        "weekend_days": [5, 6],
        "cycles": {"weekday": "weekday", "weekend": "weekend"},
        "wifi": {},
        "weather": {"enabled": true, "rules": [
            {"name": "hot", "when": "temperature >= 40", "preset": "hot", "generate": false},
            {"name": "rain", "when": "rain", "preset": "rain", "generate": false},
            {"name": "sandstorm", "when": "dust >= 150", "preset": "sandstorm", "generate": false}]},
        "generator": {
            "template": "A wide 16:9 desktop wallpaper of {{location}} in the {{phase}}, {{condition}}, {{temperature}}°C, Dubai skyline, cinematic, minimal, no text",
            "command": "grok -p {prompt_q} --always-approve --output-format json",
            "timeout": 1200,
            "note": "Command must write a PNG to {out}. {prompt} / {prompt_q} = filled prompt, {out} = target path."},
        "feeds": [{"name": "Google News · Dubai", "url": DEFAULT_FEED, "enabled": true}],
        "schedule": {"from": 6, "to": 18}
    })
}

fn default_presets() -> Vec<(&'static str, Value)> {
    let p = |label: &str, b: Value, ns: bool, w: i64| json!({"label": label, "theme": null, "wallpaper": null, "brightness": b, "night_shift": ns, "warmth": w, "dark": null});
    vec![("day", p("Day", Value::Null, false, 4500)), ("evening", p("Evening", Value::Null, true, 4500)),
         ("night", p("Night", json!(40), true, 3400)), ("hot", p("Hot day", Value::Null, false, 4500)),
         ("rain", p("Rain", Value::Null, false, 4500)), ("sandstorm", p("Sandstorm", Value::Null, true, 5000))]
}
pub fn default_preset() -> Value { default_presets()[0].1.clone() }
pub fn default_cycle(label: &str) -> Value {
    json!({"label": label, "phases": {"morning": "day", "afternoon": "day", "evening": "evening", "night": "night"}})
}

pub fn rd(p: &Path) -> Option<Value> { fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()) }
pub fn wr(p: &Path, v: &Value) {
    if let Some(d) = p.parent() { let _ = fs::create_dir_all(d); }
    let tmp = p.with_extension("tmp");
    if fs::write(&tmp, serde_json::to_string_pretty(v).unwrap() + "\n").is_ok() { let _ = fs::rename(&tmp, p); }
}

pub fn seed() {
    let _ = fs::create_dir_all(presets_dir()); let _ = fs::create_dir_all(cycles_dir()); let _ = fs::create_dir_all(styles_dir());
    if !settings_path().exists() { wr(&settings_path(), &default_settings()); }
    for (n, v) in default_presets() { let f = presets_dir().join(format!("{n}.json")); if !f.exists() { wr(&f, &v); } }
    for (n, l) in [("weekday", "Weekday"), ("weekend", "Weekend"), ("home", "Home")] {
        let f = cycles_dir().join(format!("{n}.json")); if !f.exists() { wr(&f, &default_cycle(l)); }
    }
}

/// Top-level merge of defaults with the file (same as the original implementation).
pub fn settings() -> Value {
    let mut s = default_settings();
    if let Some(Value::Object(f)) = rd(&settings_path()) { for (k, v) in f { s[k] = v; } }
    if std::env::var("CYCLE_HERE").is_ok() {
        let l = here_location(&s);
        let (city, cc) = (l["city"].as_str().unwrap_or("Dubai").to_string(), l["cc"].as_str().unwrap_or("AE").to_string());
        s["news_gl"] = json!(cc.clone());
        s["feeds"] = json!([{"name": format!("Google News · {city}"), "enabled": true,
            "url": format!("https://news.google.com/rss/search?q={}+when:1d&hl=en-{cc}&gl={cc}&ceid={cc}:en", city.replace(' ', "+"))}]);
        s["location"] = l;
    }
    s
}
/// "Here": manual override (settings.here_override = city name) → IP geolocation (ip-api.com, cached 30 min) → Dubai.
pub fn here_location(s: &Value) -> Value {
    static MEMO: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    MEMO.get_or_init(|| here_location_uncached(s)).clone()
}
fn here_location_uncached(s: &Value) -> Value {
    let dubai = json!({"name": "Dubai, United Arab Emirates", "city": "Dubai", "cc": "AE", "lat": 25.2048, "lon": 55.2708, "source": "fallback"});
    if let Some(c) = s["here_override"].as_str().filter(|c| !c.trim().is_empty()) {
        let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language=en&format=json", c.trim().replace(' ', "+"));
        if let Ok(g) = crate::live::get_json(&url, 2) {
            if let Some(r) = g["results"].as_array().and_then(|a| a.iter().max_by_key(|r| r["population"].as_i64().unwrap_or(0))) {
                return json!({"name": format!("{}, {}", r["name"].as_str().unwrap_or(c), r["country"].as_str().unwrap_or("")), "city": r["name"], "cc": r["country_code"],
                              "lat": r["latitude"], "lon": r["longitude"], "source": "override"});
            }
        }
    }
    let cp = state_dir().join("here.json");
    if let Some(c) = rd(&cp) { if now_ts() - c["at"].as_f64().unwrap_or(0.0) < 1800.0 { return c["loc"].clone(); } }
    match crate::live::get_json("http://ip-api.com/json/?fields=status,country,countryCode,city,lat,lon,timezone", 2) {
        Ok(j) if j["status"] == "success" => {
            let l = json!({"name": format!("{}, {}", j["city"].as_str().unwrap_or(""), j["country"].as_str().unwrap_or("")), "city": j["city"], "cc": j["countryCode"],
                           "lat": j["lat"], "lon": j["lon"], "tz": j["timezone"], "source": "ip"});
            wr(&cp, &json!({"at": now_ts(), "loc": l})); l
        }
        _ => dubai,
    }
}
pub fn save_settings(s: &Value) { wr(&settings_path(), s) }
pub fn update_settings(f: impl FnOnce(&mut Value)) { let mut s = settings(); f(&mut s); save_settings(&s); }

pub fn list_names(dir: &Path, ext: &str) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir).map(|r| r.filter_map(|e| e.ok()).filter_map(|e| {
        let p = e.path(); if p.extension().map(|x| x == ext).unwrap_or(false) { p.file_stem().map(|s| s.to_string_lossy().into_owned()) } else { None }
    }).collect()).unwrap_or_default();
    v.sort(); v
}
pub fn preset(n: &str) -> Option<Value> { rd(&presets_dir().join(format!("{n}.json"))) }
pub fn save_preset(n: &str, v: &Value) { wr(&presets_dir().join(format!("{n}.json")), v) }
pub fn cycle(n: &str) -> Option<Value> { rd(&cycles_dir().join(format!("{n}.json"))) }
pub fn save_cycle(n: &str, v: &Value) { wr(&cycles_dir().join(format!("{n}.json")), v) }

// ---------- prompt (style) presets: styles/<name>.txt + styles/active ----------
pub fn prompt_names() -> Vec<String> { list_names(&styles_dir(), "txt").into_iter().filter(|n| !n.contains(".bak")).collect() }
pub fn prompt_text(n: &str) -> String { fs::read_to_string(styles_dir().join(format!("{n}.txt"))).unwrap_or_default().trim().to_string() }
pub fn save_prompt(n: &str, t: &str) { let _ = fs::write(styles_dir().join(format!("{n}.txt")), t.trim().to_string() + "\n"); }
pub fn delete_prompt(n: &str) { let _ = fs::remove_file(styles_dir().join(format!("{n}.txt"))); }
pub fn active_prompt() -> String {
    let a = fs::read_to_string(styles_dir().join("active")).unwrap_or_default().trim().to_string();
    if !a.is_empty() && styles_dir().join(format!("{a}.txt")).exists() { a } else { "gits-classic".into() }
}
pub fn set_active_prompt(n: &str) { let _ = fs::write(styles_dir().join("active"), format!("{n}\n")); }

// ---------- processes ----------
pub struct Out { pub code: i32, pub out: String, pub err: String }
impl Out { pub fn ok(&self) -> bool { self.code == 0 } }

pub fn run(mut c: Command, timeout: u64) -> Out {
    use std::os::unix::process::CommandExt;
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let mut ch = match c.spawn() { Ok(c) => c, Err(e) => return Out { code: 127, out: String::new(), err: e.to_string() } };
    let mut so = ch.stdout.take().unwrap(); let mut se = ch.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || { let mut s = String::new(); let _ = so.read_to_string(&mut s); s });
    let t2 = std::thread::spawn(move || { let mut s = String::new(); let _ = se.read_to_string(&mut s); s });
    let dl = Instant::now() + Duration::from_secs(timeout);
    let code = loop {
        match ch.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(1),
            Ok(None) if Instant::now() < dl => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = Command::new("kill").args(["-TERM", "--", &format!("-{}", ch.id())]).status();
                std::thread::sleep(Duration::from_secs(2)); let _ = ch.kill(); let _ = ch.wait();
                let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", ch.id())]).stderr(Stdio::null()).status();
                return Out { code: 124, out: String::new(), err: format!("timed out after {timeout}s") };
            }
        }
    };
    Out { code, out: t1.join().unwrap_or_default(), err: t2.join().unwrap_or_default() }
}
pub fn sh(cmd: &str, timeout: u64) -> Out { let mut c = Command::new("bash"); c.arg("-c").arg(cmd); run(c, timeout) }
pub fn shv(args: &[&str], timeout: u64) -> Out { let mut c = Command::new(args[0]); c.args(&args[1..]); run(c, timeout) }
pub fn quote(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }

// ---------- sun / phase ----------
/// Today's sun events for the configured location, computed from the NOAA model (minute resolution):
/// sunrise/sunset at -0.833°, solar_noon = meridian transit (max elevation), civil (-6°), nautical (-12°), astronomical (-18°) dawn/dusk.
pub fn sun_events(day: chrono::NaiveDate, s: &Value) -> Value {
    let (la, lo) = loc(s); sun_events_at(day, la, lo, Local::now().offset().local_minus_utc() as f64 / 3600.0)
}
pub fn sun_events_at(day: chrono::NaiveDate, la: f64, lo: f64, tz: f64) -> Value {
    let el: Vec<f64> = (0..=1440).map(|m| sun_pos_tz(day.and_hms_opt(0, 0, 0).unwrap() + chrono::Duration::minutes(m), la, lo, tz).0).collect();
    let noon = (0..=1440).max_by(|&a, &b| el[a].partial_cmp(&el[b]).unwrap()).unwrap();
    let hm = |m: usize| format!("{:02}:{:02}", (m / 60) % 24, m % 60);
    // linear interpolation to the nearest minute of the crossing
    let up = |th: f64| (1..=noon).find(|&m| el[m - 1] < th && el[m] >= th).map(hm);
    let down = |th: f64| (noon + 1..=1440).find(|&m| el[m - 1] >= th && el[m] < th).map(hm);
    json!({"sunrise": up(-0.833), "solar_noon": hm(noon), "sunset": down(-0.833),
           "civil_dawn": up(-6.0), "civil_dusk": down(-6.0), "nautical_dusk": down(-12.0), "astronomical_dusk": down(-18.0)})
}
fn tmin(s: &str) -> i32 { let mut it = s.split(':').map(|x| x.trim().parse::<i32>().unwrap_or(0)); it.next().unwrap_or(0) * 60 + it.next().unwrap_or(0) }

/// Phase boundaries come from sun events (times.* = event name; an explicit HH:MM override is still accepted).
/// Legacy keys wake/bedtime are read as morning/night.
pub fn phase_at(now: NaiveDateTime, s: &Value, _sun: &(String, String)) -> (String, Value) {
    let ev = sun_events(now.date(), s); let t = &s["times"];
    let get = |k: &str, legacy: &str, def: &str| -> String {
        let v = t[k].as_str().or(t[legacy].as_str()).unwrap_or(def);
        let v = if Regex::new(r"^\d{1,2}:\d{2}$").unwrap().is_match(v) && legacy.is_empty() { v } else if ev[v].is_string() { v } else { def };
        ev[v].as_str().map(|x| x.to_string()).unwrap_or_else(|| v.to_string())
    };
    let (mo, af, evn, ni) = (get("morning", "", "sunrise"), get("afternoon", "", "solar_noon"), get("evening", "", "sunset"), get("night", "", "civil_dusk"));
    let m = (now.hour() * 60 + now.minute()) as i32;
    let (wake, aft, eve, bed) = (tmin(&mo), tmin(&af), tmin(&evn), tmin(&ni));
    let between = |a: i32, b: i32| if a <= b { a <= m && m < b } else { m >= a || m < b };
    let ph = if between(bed, wake) { "night" } else if between(wake, aft) { "morning" } else if between(aft, eve) { "afternoon" } else { "evening" };
    let mut info = ev.clone();
    info["morning_start"] = json!(mo); info["afternoon_start"] = json!(af); info["evening_start"] = json!(evn); info["night_start"] = json!(ni);
    (ph.into(), info)
}

pub fn wifi_ssid() -> Option<String> {
    let r = sh("nmcli -t -f active,ssid dev wifi 2>/dev/null | awk -F: '$1==\"yes\"{print $2; exit}'", 10);
    if !r.out.trim().is_empty() { return Some(r.out.trim().into()); }
    let r = sh("iwgetid -r 2>/dev/null", 5); let t = r.out.trim(); if t.is_empty() { None } else { Some(t.into()) }
}
pub fn location(s: &Value) -> Value {
    let mut l = s["location"].clone();
    if l["source"] == "gps" { l["note"] = json!("GPS not implemented yet (stub) – using fixed coordinates"); }
    l
}

fn is_rain_code(c: i64) -> bool { (51..68).contains(&c) || (80..83).contains(&c) || [95, 96, 99].contains(&c) }

/// Engine weather (cached 30 min): Open-Meteo current + dust/pm10, with derived condition.
pub fn weather(s: &Value) -> Value {
    let cache = state_dir().join("weather.json"); let c = rd(&cache);
    if let Some(c) = &c { if now_ts() - c["ts"].as_f64().unwrap_or(0.0) < 1800.0 { return c.clone(); } }
    let (lat, lon) = (s["location"]["lat"].as_f64().unwrap_or(25.2048), s["location"]["lon"].as_f64().unwrap_or(55.2708));
    let u = format!("https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code,precipitation,wind_speed_10m");
    match crate::live::get_json(&u, 1) {
        Ok(w) => {
            let w = &w["current"];
            let aq = crate::live::get_json(&format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=dust,pm10"), 1)
                .map(|a| a["current"].clone()).unwrap_or(json!({}));
            let code = w["weather_code"].as_i64().unwrap_or(0);
            let temp = w["temperature_2m"].as_f64();
            let rain = is_rain_code(code) || w["precipitation"].as_f64().unwrap_or(0.0) > 0.0;
            let dust = aq["dust"].as_f64().unwrap_or(0.0);
            let cond = if dust >= 150.0 { "sandstorm" } else if rain { "rain" } else if temp.unwrap_or(0.0) >= 40.0 { "scorching heat" } else if code <= 1 { "clear sky" } else { "cloudy" };
            let d = json!({"ts": now_ts(), "temperature": temp, "code": code, "rain": rain, "wind": w["wind_speed_10m"],
                           "dust": dust, "pm10": aq["pm10"].as_f64().unwrap_or(0.0), "condition": cond});
            wr(&cache, &d); d
        }
        Err(e) => c.unwrap_or(json!({"error": e})),
    }
}

/// expr: 'rain' | 'dust >= 150' | 'temperature >= 40'
pub fn rule_match(expr: &str, w: &Value) -> bool {
    let e = expr.trim();
    if Regex::new(r"^\w+$").unwrap().is_match(e) {
        return match &w[e] { Value::Bool(b) => *b, Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0, Value::String(s) => !s.is_empty(), _ => false };
    }
    let re = Regex::new(r"^(\w+)\s*(>=|<=|>|<|==)\s*(-?[\d.]+)$").unwrap();
    let Some(m) = re.captures(e) else { return false };
    let Some(a) = w[&m[1]].as_f64() else { return false };
    let b: f64 = m[3].parse().unwrap_or(0.0);
    match &m[2] { ">=" => a >= b, "<=" => a <= b, ">" => a > b, "<" => a < b, _ => a == b }
}

pub fn day_type(s: &Value, now: NaiveDateTime) -> String {
    let wd = now.weekday().num_days_from_monday() as i64;
    let we: Vec<i64> = s["weekend_days"].as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or(vec![5, 6]);
    if we.contains(&wd) { "weekend".into() } else { "weekday".into() }
}

pub fn resolve(use_weather: bool) -> Value {
    let s = settings(); let now = Local::now().naive_local();
    let (ph, sun) = phase_at(now, &s, &(String::new(), String::new())); let dtp = day_type(&s, now); let ssid = wifi_ssid();
    let mut cyc = s["cycles"][&dtp].as_str().unwrap_or(&dtp).to_string(); let mut why = format!("day type {dtp}");
    if let Some(id) = &ssid { if let Some(c) = s["wifi"][id].as_str() { cyc = c.into(); why = format!("Wi-Fi '{id}'"); } }
    let c = cycle(&cyc).or_else(|| cycle("weekday")).unwrap_or(json!({"phases": {}}));
    let mut pname = c["phases"][&ph].clone(); let mut src = format!("cycle {cyc} ({why}) → {ph}");
    let mut w = json!({}); let mut rule = Value::Null;
    if use_weather && s["weather"]["enabled"].as_bool().unwrap_or(false) {
        w = weather(&s);
        if w.get("error").is_none() {
            for r in s["weather"]["rules"].as_array().cloned().unwrap_or_default() {
                let pn = r["preset"].as_str().unwrap_or("");
                if rule_match(r["when"].as_str().unwrap_or(""), &w) && preset(pn).is_some() {
                    pname = json!(pn); src += &format!(" · weather rule '{}'", r["name"].as_str().unwrap_or("")); rule = r; break;
                }
            }
        }
    }
    json!({"phase": ph, "sun": sun, "day_type": dtp, "ssid": ssid, "cycle": cyc, "preset": pname, "source": src,
           "weather": w, "rule": rule, "location": location(&s)})
}

// ---------- state / apply ----------
pub fn theme_current() -> String { shv(&["omarchy-theme-current"], 20).out.trim().to_string() }
pub fn current_state() -> Value {
    let wp = fs::canonicalize(home().join(".local/state/omarchy/current/background")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let b: Vec<String> = shv(&["brightnessctl", "-m"], 10).out.split(',').map(|x| x.to_string()).collect();
    let br = b.get(3).and_then(|x| x.trim().trim_end_matches('%').parse::<i64>().ok());
    let warmth = fs::read_to_string(home().join(".config/omarchy/display-menu/night-temp")).ok().and_then(|x| x.trim().parse::<i64>().ok()).unwrap_or(4000);
    let d = disp(); let t = theme_current();
    json!({"theme": if t.is_empty() { Value::Null } else { json!(t) }, "wallpaper": wp, "brightness": br,
           "night_shift": shv(&[&d, "night-is-on"], 10).ok(), "warmth": warmth,
           "night_auto": shv(&[&d, "night-auto-is-on"], 10).ok(), "dark": shv(&[&d, "dark-is-on"], 10).ok()})
}

pub fn apply_preset(p: &Value) -> Vec<String> {
    let cur = current_state(); let mut done = vec![]; let d = disp();
    let theme = p["theme"].as_str().filter(|t| !t.is_empty());
    if let Some(t) = theme {
        if Some(t) != cur["theme"].as_str() { shv(&["omarchy-theme-set", t], 120); done.push(format!("theme→{t}")); }
    } else if let Some(dk) = p["dark"].as_bool() {
        if Some(dk) != cur["dark"].as_bool() { shv(&[&d, if dk { "dark-on" } else { "light-on" }], 120); done.push(format!("dark→{dk}")); }
    }
    if let Some(wp) = p["wallpaper"].as_str().filter(|x| !x.is_empty()) {
        let wp = if let Some(r) = wp.strip_prefix("~/") { home().join(r) } else { PathBuf::from(wp) };
        if wp.is_file() && fs::canonicalize(&wp).map(|x| x.to_string_lossy().into_owned()).unwrap_or_default() != cur["wallpaper"].as_str().unwrap_or("") {
            shv(&["omarchy-theme-bg-set", &wp.to_string_lossy()], 60); done.push("wallpaper".into());
        }
    }
    if let Some(b) = p["brightness"].as_i64() {
        if Some(b) != cur["brightness"].as_i64() { shv(&[&d, "bright", &format!("{b}%")], 20); done.push(format!("brightness→{b}%")); }
    }
    if let Some(ns) = p["night_shift"].as_bool() {
        if let Some(w) = p["warmth"].as_i64() { if Some(w) != cur["warmth"].as_i64() { shv(&[&d, "night-warmth", &w.to_string()], 20); done.push(format!("warmth→{w}K")); } }
        if Some(ns) != cur["night_shift"].as_bool() { shv(&[&d, if ns { "night-on" } else { "night-off" }], 20); done.push(format!("night→{ns}")); }
    }
    println!("applied: {}", if done.is_empty() { "nothing to change".into() } else { done.join(", ") });
    done
}

// ---------- generation ----------
pub fn fill(tpl: &str, ctx: &Value) -> String {
    Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap().replace_all(tpl, |c: &regex::Captures| match &ctx[&c[1]] {
        Value::String(s) => s.clone(), Value::Null => c[0].to_string(), v => v.to_string() }).into_owned()
}
fn hash10(s: &str) -> String {
    use std::hash::{Hash, Hasher}; let mut h = std::collections::hash_map::DefaultHasher::new(); s.hash(&mut h); format!("{:016x}", h.finish())[..10].into()
}

pub fn generate(name: &str, ctx: &Value, s: &Value) -> Option<String> {
    let g = &s["generator"];
    let prompt = fill(g["template"].as_str().unwrap_or(""), ctx);
    let prompt = crate::live::expand_placeholders(&prompt, s);
    // unique per run: a cached file from an earlier run must never be re-applied or archived as "new"
    let key = hash10(&format!("{prompt}{}", Local::now().format("%Y-%m-%d %H:%M:%S")));
    let _ = fs::create_dir_all(gen_dir()); let out = gen_dir().join(format!("{name}-{key}.png"));
    if out.exists() { return Some(out.to_string_lossy().into()); }
    let outs = out.to_string_lossy().to_string();
    let full = format!("{prompt}. Generate this image in ONE single text-to-image generation from this prompt alone (scene, style and any text rendered by the generator itself): do not use image_edit or any existing/reference image, and never overlay, composite or draw text with code. Save it as a PNG file at {outs}. Reply only with the path.");
    let cmd = g["command"].as_str().unwrap_or("").replace("{prompt_q}", &quote(&full)).replace("{out}", &quote(&outs)).replace("{prompt}", &full);
    let timeout = g["timeout"].as_u64().unwrap_or(1200).max(1200);
    println!("generating (timeout {timeout}s): {prompt}");
    let r = sh(&cmd, timeout);
    if out.exists() { return Some(outs); }
    let msg = if r.err.trim().is_empty() { r.out } else { r.err };
    let tail: String = msg.chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect();
    println!("generation failed (rc={}): {tail}", r.code);
    None
}

// ---------- engine ----------
pub fn tick(force: bool) -> Value {
    crate::sched::run_due();
    let s = settings();
    if !s["enabled"].as_bool().unwrap_or(false) && !force { println!("engine disabled"); return Value::Null; }
    let last = rd(&state_dir().join("last.json")).unwrap_or(json!({}));
    if let Some(hold) = s["hold"].as_str() {
        if let Some(p) = preset(hold) {
            let sig = format!("hold|{hold}|{p}");
            if !force && last["sig"].as_str() == Some(&sig) { return Value::Null; }
            println!("holding preset '{hold}' (Automatic off)"); apply_preset(&p);
            wr(&state_dir().join("last.json"), &json!({"sig": sig, "at": iso_now(), "preset": hold, "source": "hold"}));
            return json!({"preset": hold, "source": "hold"});
        }
    }
    let res = resolve(true);
    let mut p = res["preset"].as_str().and_then(preset).unwrap_or(json!({}));
    if res["rule"]["generate"].as_bool().unwrap_or(false) {
        let ctx = theme_context(&res);
        if let Some(img) = generate(res["rule"]["name"].as_str().unwrap_or("rule"), &ctx, &s) { p["wallpaper"] = json!(img); }
    }
    let (k, b, _) = circadian(Local::now().naive_local(), &s, None);
    println!("{}", crate::bright::step(res["phase"].as_str().unwrap_or("day"), b, crate::bright::now())); apply_circadian(k, None);
    if theme_current().to_lowercase() == "cycle" && s["theme_generation"].as_bool().unwrap_or(true) {
        let _ = generate_cycle_theme(Some(theme_context(&res)), None, false); // failure → phase preset below still applies
    }
    let p = json!({"dark": p["dark"], "theme": p["theme"], "wallpaper": p["wallpaper"]});
    let sig = format!("{}{}", res["preset"].as_str().unwrap_or(""), p);
    if !force && last["sig"].as_str() == Some(&sig) { return res; }
    println!("{}", res["source"].as_str().unwrap_or("")); apply_preset(&p);
    wr(&state_dir().join("last.json"), &json!({"sig": sig, "at": iso_now(), "phase": res["phase"], "cycle": res["cycle"], "preset": res["preset"], "source": res["source"]}));
    res
}
pub fn iso_now() -> String { Local::now().format("%Y-%m-%dT%H:%M:%S").to_string() }

pub fn set_engine(on: bool) {
    update_settings(|s| s["enabled"] = json!(on));
    // the minute timer stays on: it also drives scheduled prompts; `enabled` only gates the display engine
    shv(&["systemctl", "--user", "enable", "--now", "cycle-engine.timer"], 30);
}
pub fn set_hold(name: Option<&str>) {
    let h = match name { Some(n) if !n.is_empty() && n.to_lowercase() != "automatic" => json!(n), _ => Value::Null };
    update_settings(|s| s["hold"] = h); tick(true);
}

// ---------- circadian (NOAA solar position) ----------
pub fn solar_elevation(when: NaiveDateTime, lat: f64, lon: f64) -> f64 { sun_pos(when, lat, lon).0 }
/// NOAA general solar position → (elevation°, azimuth° from north clockwise)
pub fn sun_pos(when: NaiveDateTime, lat: f64, lon: f64) -> (f64, f64) { sun_pos_tz(when, lat, lon, Local::now().offset().local_minus_utc() as f64 / 3600.0) }
/// `when` is local clock time at UTC offset `tz` hours.
pub fn sun_pos_tz(when: NaiveDateTime, lat: f64, lon: f64, tz: f64) -> (f64, f64) {
    use std::f64::consts::PI;
    let doy = when.ordinal() as f64; let hr = when.hour() as f64 + when.minute() as f64 / 60.0 + when.second() as f64 / 3600.0;
    let g = 2.0 * PI / 365.0 * (doy - 1.0 + (hr - 12.0) / 24.0);
    let eqt = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin()
        - 0.002697 * (3.0 * g).cos() + 0.00148 * (3.0 * g).sin();
    let tst = hr * 60.0 + eqt + 4.0 * lon - 60.0 * tz;
    let had = tst / 4.0 - 180.0; let ha = had.to_radians(); let la = lat.to_radians();
    let cz = (la.sin() * decl.sin() + la.cos() * decl.cos() * ha.cos()).clamp(-1.0, 1.0);
    let zen = cz.acos();
    let th = ((la.sin() * cz - decl.sin()) / (la.cos() * zen.sin().max(1e-9))).clamp(-1.0, 1.0).acos().to_degrees();
    let az = if had > 0.0 { (th + 180.0) % 360.0 } else { (540.0 - th) % 360.0 };
    (90.0 - zen.to_degrees(), az)
}
fn loc(s: &Value) -> (f64, f64) { (s["location"]["lat"].as_f64().unwrap_or(25.2048), s["location"]["lon"].as_f64().unwrap_or(55.2708)) }
pub fn noon_elevation(day: NaiveDateTime, s: &Value) -> f64 {
    let (la, lo) = loc(s); let d = day.date(); let mut m = -90.0f64;
    for h in 10..15 { for mi in (0..60).step_by(2) { m = m.max(solar_elevation(d.and_hms_opt(h, mi, 0).unwrap(), la, lo)); } }
    m
}
/// Continuous K and brightness from solar elevation → (K, brightness %, elevation)
pub fn circadian(when: NaiveDateTime, s: &Value, peak: Option<f64>) -> (i64, i64, f64) {
    let mut c = default_settings()["circadian"].clone();
    if let Some(o) = s["circadian"].as_object() { for (k, v) in o { c[k] = v.clone(); } }
    let f = |k: &str| c[k].as_f64().unwrap_or(0.0);
    let (la, lo) = loc(s); let e = solar_elevation(when, la, lo); let peak = peak.unwrap_or_else(|| noon_elevation(when, s)); let tw = f("twilight");
    let x = ((e - tw) / (peak - tw)).clamp(0.0, 1.0); let ease = (std::f64::consts::FRAC_PI_2 * x).sin();
    let k = f("k_night") + (f("k_noon") - f("k_night")) * ease.powf(1.3);
    let b = f("b_min") + (f("b_max") - f("b_min")) * ease;
    (((k / 10.0).round() * 10.0) as i64, b.round() as i64, (e * 100.0).round() / 100.0)
}
pub fn apply_circadian(k: i64, b: Option<i64>) -> Vec<String> {
    let mut done = vec![];
    if let Some(b) = b {
        let cur_b = current_state()["brightness"].as_i64();
        if cur_b.map(|c| (c - b).abs() >= 1).unwrap_or(true) {
            shv(&["brightnessctl", "-q", "set", &format!("{b}%")], 10); done.push(format!("brightness {b}%"));
        }
    }
    if !shv(&["pgrep", "-x", "hyprsunset"], 5).ok() { sh("setsid uwsm-app -- hyprsunset >/dev/null 2>&1 &", 10); std::thread::sleep(Duration::from_secs(1)); }
    let r = shv(&["hyprctl", "hyprsunset", "temperature"], 10);
    let cur_k = Regex::new(r"\d+").unwrap().find(&r.out).and_then(|m| m.as_str().parse::<i64>().ok());
    if cur_k.map(|c| (c - k).abs() >= 10).unwrap_or(true) {
        if k >= 6500 { shv(&["hyprctl", "hyprsunset", "identity"], 10); } else { shv(&["hyprctl", "hyprsunset", "temperature", &k.to_string()], 10); }
        done.push(format!("{k}K"));
    }
    println!("circadian: {}", if done.is_empty() { "on target".into() } else { done.join(", ") });
    done
}
pub fn curve(step: u32) -> Value {
    let s = settings(); let now = Local::now().naive_local(); let peak = noon_elevation(now, &s); let res = resolve(true);
    let day0 = now.date().and_hms_opt(0, 0, 0).unwrap();
    let pts: Vec<Value> = (0..=1440).step_by(step as usize).map(|m| {
        let (k, b, e) = circadian(day0 + chrono::Duration::minutes(m as i64), &s, Some(peak)); json!([m, k, b, e]) }).collect();
    let (k, b, e) = circadian(now, &s, Some(peak));
    json!({"points": pts, "now": now.hour() as f64 * 60.0 + now.minute() as f64 + now.second() as f64 / 60.0, "target_k": k, "target_b": b,
           "elev": e, "peak": (peak * 10.0).round() / 10.0, "phase": res["phase"], "preset": res["preset"], "rule": res["rule"]["preset"], "sun": res["sun"],
           "resolve": res, "next_tick": next_tick(), "hold": s["hold"], "enabled": s["enabled"]})
}
pub fn next_tick() -> Option<String> {
    let r = shv(&["systemctl", "--user", "show", "cycle-engine.timer", "-p", "NextElapseUSecRealtime", "--value"], 10);
    let t = r.out.trim(); if t.is_empty() { None } else { Some(t.into()) }
}

// ---------- Cycle theme: generated background + palette ----------
pub fn theme_context(res: &Value) -> Value {
    let w = &res["weather"];
    json!({"location": res["location"]["name"], "temperature": w["temperature"].as_f64().unwrap_or(0.0).round() as i64,
           "condition": w["condition"].as_str().unwrap_or("clear sky"), "phase": res["phase"], "day_type": res["day_type"]})
}
fn theme_sig(ctx: &Value) -> String {
    format!("{}|{}|{}|{}", Local::now().date_naive(), ctx["phase"].as_str().unwrap_or(""), ctx["condition"].as_str().unwrap_or(""), ctx["day_type"].as_str().unwrap_or(""))
}

/// Generate background + palette into themes/cycle. apply=None → apply only if Cycle is the active theme.
/// Seconds the last image generation took (for the schedule's lead time).
pub static LAST_GEN_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn generate_cycle_theme(ctx: Option<Value>, apply: Option<bool>, force: bool) -> Option<String> { generate_cycle_theme_at(ctx, apply, force, None) }
/// `hold`: keep the finished image until this local time, then install/apply it (immediately if already past).
pub fn generate_cycle_theme_at(ctx: Option<Value>, apply: Option<bool>, force: bool, hold: Option<NaiveDateTime>) -> Option<String> {
    let ctx = ctx.unwrap_or_else(|| theme_context(&resolve(true))); let sig = theme_sig(&ctx);
    let stp = state_dir().join("theme.json"); let st = rd(&stp).unwrap_or(json!({}));
    if !force && st["sig"].as_str() == Some(&sig) { return st["background"].as_str().map(|x| x.to_string()); }
    if !force && st["failed_sig"].as_str() == Some(&sig) && now_ts() - st["failed_at"].as_f64().unwrap_or(0.0) < 1800.0 { return None; }
    let name = format!("{}-{}", ctx["phase"].as_str().unwrap_or("now"), ctx["condition"].as_str().unwrap_or("").replace(' ', "_"));
    let t0 = Instant::now();
    let made = generate(&name, &ctx, &settings());
    LAST_GEN_SECS.store(t0.elapsed().as_secs(), std::sync::atomic::Ordering::Relaxed);
    let Some(img) = made else {
        let mut st2 = st.clone(); st2["failed_sig"] = json!(sig); st2["failed_at"] = json!(now_ts()); wr(&stp, &st2);
        println!("cycle theme: generation failed → falling back to phase preset"); return None;
    };
    if let Some(at) = hold {
        let wait = (at - Local::now().naive_local()).num_milliseconds();
        if wait > 0 { println!("holding image until {} ({}s)", at.format("%H:%M:%S"), wait / 1000); std::thread::sleep(Duration::from_millis(wait as u64)); }
        else { println!("finished {}s after the mark – applying now", -wait / 1000); }
    }
    let td = theme_dir(); let bgd = td.join("backgrounds"); let _ = fs::create_dir_all(&bgd);
    let seedf = td.join("colors.toml.seed");
    if !seedf.exists() && td.join("colors.toml").exists() { let _ = fs::copy(td.join("colors.toml"), &seedf); }
    for e in fs::read_dir(&bgd).into_iter().flatten().flatten() { if e.file_name().to_string_lossy().starts_with("0-cycle-") { let _ = fs::remove_file(e.path()); } }
    let ext = Path::new(&img).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or("png".into());
    let dst = bgd.join(format!("0-cycle-{name}.{ext}")); let _ = fs::copy(&img, &dst);
    match crate::theme::palette_from_image(&dst) {
        Ok(toml) => { let _ = fs::write(td.join("colors.toml"), format!("# generated by Cycle {sig}\n{toml}")); }
        Err(e) => println!("palette failed: {e}"),
    }
    if let Ok(im) = image::open(&dst) { let _ = im.resize_exact(640, 360, image::imageops::FilterType::Triangle).to_rgb8().save(td.join("preview.png")); }
    let dsts = dst.to_string_lossy().to_string();
    wr(&stp, &json!({"sig": sig, "background": dsts, "at": iso_now()}));
    let active = theme_current().to_lowercase() == "cycle";
    if apply.unwrap_or(active) {
        shv(&["omarchy-theme-set", "Cycle"], 180); shv(&["omarchy-theme-bg-set", &dsts], 60); println!("cycle theme: applied");
    } else { println!("cycle theme: generated (not applied; active theme is not Cycle) → {dsts}"); }
    Some(dsts)
}
