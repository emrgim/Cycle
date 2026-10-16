//! Cycle brightness: manual override detection, override log, learned per-slot profile.
//!
//! * A "slot" is the current day phase (morning / afternoon / evening / night).
//! * If the screen brightness differs from what Cycle last set, the user changed it by hand:
//!   that is an override. Cycle leaves brightness alone until the slot changes.
//! * Every override is appended to `~/.local/state/cycle/overrides.json`.
//! * When a slot has >= 3 overrides in the same direction on different days, the median of
//!   those user values becomes the slot's learned base in `~/.local/state/cycle/profile.json`.
//!   Cycle walks the effective base towards it by at most `STEP` points per day.
use chrono::{Local, NaiveDateTime};
use serde_json::{json, Value};
use std::path::PathBuf;

const TOLERANCE: i64 = 2; // brightnessctl rounding noise
const MIN_DAYS: usize = 3;
const RECENT_DAYS: usize = 7;
const STEP: i64 = 5;

pub fn dir() -> PathBuf {
    match std::env::var("CYCLE_STATE_DIR") {
        Ok(d) if !d.is_empty() => { let p = PathBuf::from(d); let _ = std::fs::create_dir_all(&p); p }
        _ => crate::core::state_dir(),
    }
}
fn state_path() -> PathBuf { dir().join("brightness.json") }
pub fn overrides_path() -> PathBuf { dir().join("overrides.json") }
pub fn profile_path() -> PathBuf { dir().join("profile.json") }
fn rd(p: &PathBuf, d: Value) -> Value { crate::core::rd(p).unwrap_or(d) }

/// Current brightness %. `CYCLE_BRIGHTNESS_FILE` replaces the real backlight (tests).
pub fn read() -> Option<i64> {
    if let Ok(f) = std::env::var("CYCLE_BRIGHTNESS_FILE") { return std::fs::read_to_string(f).ok()?.trim().parse().ok(); }
    crate::core::current_state()["brightness"].as_i64()
}
pub fn write(b: i64) {
    if let Ok(f) = std::env::var("CYCLE_BRIGHTNESS_FILE") { let _ = std::fs::write(f, format!("{b}\n")); return; }
    crate::core::shv(&["brightnessctl", "-q", "set", &format!("{b}%")], 10);
}

fn median(mut v: Vec<i64>) -> i64 {
    v.sort(); let n = v.len(); if n == 0 { return 0; }
    if n % 2 == 1 { v[n / 2] } else { ((v[n / 2 - 1] + v[n / 2]) as f64 / 2.0).round() as i64 }
}

/// Recompute the learned target for `slot` from the override log. Returns the profile entry if learned.
pub fn learn(slot: &str, now: NaiveDateTime) -> Option<Value> {
    let log = rd(&overrides_path(), json!([]));
    // last override of each day for this slot, newest day first
    let mut per_day: Vec<(String, i64, i64)> = vec![];
    for o in log.as_array().into_iter().flatten().rev() {
        if o["slot"].as_str() != Some(slot) { continue; }
        let day = o["at"].as_str().unwrap_or("").get(..10).unwrap_or("").to_string();
        if day.is_empty() || per_day.iter().any(|(d, _, _)| *d == day) { continue; }
        per_day.push((day, o["base"].as_i64().unwrap_or(0), o["user"].as_i64().unwrap_or(0)));
        if per_day.len() >= RECENT_DAYS { break; }
    }
    let up: Vec<_> = per_day.iter().filter(|(_, b, u)| u > b).collect();
    let down: Vec<_> = per_day.iter().filter(|(_, b, u)| u < b).collect();
    let (dir_name, pick) = if up.len() >= MIN_DAYS && up.len() >= down.len() { ("up", up) }
        else if down.len() >= MIN_DAYS { ("down", down) } else { return None };
    let target = median(pick.iter().map(|x| x.2).collect()).clamp(1, 100);
    let mut prof = rd(&profile_path(), json!({}));
    if !prof.is_object() { prof = json!({}); }
    let prev = prof["slots"][slot].clone();
    let current = prev["current"].as_i64().unwrap_or_else(|| median(pick.iter().map(|x| x.1).collect()));
    let entry = json!({
        "target": target, "current": current, "direction": dir_name,
        "days": pick.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
        "user_values": pick.iter().map(|x| x.2).collect::<Vec<_>>(),
        "last_step_day": prev["last_step_day"], "updated": now.format("%Y-%m-%dT%H:%M:%S").to_string()});
    prof["note"] = json!("Learned brightness base per Cycle slot. 'current' walks to 'target' by at most 5 points a day. Reset: cycle reset-profile [SLOT]");
    prof["slots"][slot] = entry.clone();
    crate::core::wr(&profile_path(), &prof);
    Some(entry)
}

/// Effective base for the slot: learned value (moved one step per day towards the target) or the circadian one.
fn effective_base(slot: &str, raw: i64, now: NaiveDateTime) -> (i64, bool) {
    let mut prof = rd(&profile_path(), json!({}));
    let e = &prof["slots"][slot];
    let (Some(target), Some(mut cur)) = (e["target"].as_i64(), e["current"].as_i64()) else { return (raw, false) };
    let today = now.format("%Y-%m-%d").to_string();
    if e["last_step_day"].as_str() != Some(today.as_str()) && cur != target {
        cur += (target - cur).clamp(-STEP, STEP);
        prof["slots"][slot]["current"] = json!(cur);
        prof["slots"][slot]["last_step_day"] = json!(today);
        crate::core::wr(&profile_path(), &prof);
    }
    (cur, true)
}

/// One engine step for brightness. Returns what it did (for the tick log).
pub fn step(slot: &str, raw_base: i64, now: NaiveDateTime) -> String {
    let mut st = rd(&state_path(), json!({}));
    let ts = now.format("%Y-%m-%dT%H:%M:%S").to_string();
    let cur = read();
    let same_slot = st["slot"].as_str() == Some(slot);
    if !same_slot {
        if !st["override"].is_null() { println!("brightness: slot {} → {slot}, override released", st["slot"].as_str().unwrap_or("?")); }
        st["override"] = Value::Null; st["slot"] = json!(slot); st["slot_since"] = json!(ts);
    }
    let (base, learned) = effective_base(slot, raw_base, now);
    let log_override = |base: i64, user: i64| {
        let mut log = rd(&overrides_path(), json!([]));
        if !log.is_array() { log = json!([]); }
        log.as_array_mut().unwrap().push(json!({"at": ts, "slot": slot, "base": base, "user": user}));
        crate::core::wr(&overrides_path(), &log);
        if let Some(p) = learn(slot, now) { println!("brightness: profile {slot} → target {} (current {})", p["target"], p["current"]); }
    };
    let msg;
    if let Some(o) = st["override"].as_object().cloned() {
        let user = o["user"].as_i64().unwrap_or(0);
        if let Some(c) = cur.filter(|c| (c - user).abs() > TOLERANCE) {
            log_override(o["base"].as_i64().unwrap_or(base), c);
            st["override"]["user"] = json!(c); st["override"]["at"] = json!(ts);
            msg = format!("override updated to {c}% (kept until next slot)");
        } else { msg = format!("override {user}% kept until next slot"); }
    } else if let (true, Some(last), Some(c)) = (same_slot, st["last_set"].as_i64(), cur) {
        if (c - last).abs() > TOLERANCE {
            log_override(last, c);
            st["override"] = json!({"slot": slot, "base": last, "user": c, "at": ts});
            msg = format!("manual change {last}% → {c}% detected: override kept until next slot");
        } else { msg = apply(&mut st, base, cur, learned); }
    } else { msg = apply(&mut st, base, cur, learned); }
    crate::core::wr(&state_path(), &st);
    format!("brightness: {msg}")
}
fn apply(st: &mut Value, base: i64, cur: Option<i64>, learned: bool) -> String {
    st["last_set"] = json!(base);
    let tag = if learned { " (learned)" } else { "" };
    if cur.map(|c| c != base).unwrap_or(true) { write(base); format!("set {base}%{tag}") } else { format!("on target {base}%{tag}") }
}

pub fn reset_profile(slot: Option<&str>) {
    match slot {
        Some(s) => { let mut p = rd(&profile_path(), json!({}));
            if let Some(m) = p["slots"].as_object_mut() { m.remove(s); }
            crate::core::wr(&profile_path(), &p); println!("profile reset for slot {s}"); }
        None => { let _ = std::fs::remove_file(profile_path()); println!("profile reset (overrides log kept: {})", overrides_path().display()); }
    }
}
pub fn now() -> NaiveDateTime { Local::now().naive_local() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn med() { assert_eq!(median(vec![3, 1, 2]), 2); assert_eq!(median(vec![1, 2, 3, 4]), 3); }
}
