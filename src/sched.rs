//! Scheduled prompts: settings.schedules.<prompt> = {"enabled", "every_min", "window": "dawn-dusk" | "always"}.
//! `cycle tick` (cycle-engine.timer, every minute) starts due runs as transient units; a lock file prevents overlap.
use crate::core::{self, rd, settings, state_dir, wr};
use chrono::{Local, NaiveDateTime, Timelike};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn lock_path() -> PathBuf { state_dir().join("generate.lock") }
fn state_path() -> PathBuf { state_dir().join("schedules.json") }

pub struct Lock;
impl Lock {
    /// Exclusive lock (pid file, stale if the pid is gone).
    pub fn acquire() -> Option<Lock> {
        if running().is_some() { return None; }
        let tmp = state_dir().join(format!("generate.lock.{}", std::process::id()));
        fs::write(&tmp, std::process::id().to_string()).ok()?;
        let ok = fs::hard_link(&tmp, lock_path()).is_ok(); let _ = fs::remove_file(&tmp);
        if !ok { if running().is_none() { let _ = fs::remove_file(lock_path()); return Lock::acquire(); } return None; }
        Some(Lock)
    }
}
impl Drop for Lock { fn drop(&mut self) { let _ = fs::remove_file(lock_path()); } }
/// pid of the running generation, if any
pub fn running() -> Option<u32> {
    let pid: u32 = fs::read_to_string(lock_path()).ok()?.trim().parse().ok()?;
    if PathBuf::from(format!("/proc/{pid}")).exists() { Some(pid) } else { None }
}

pub fn mark(name: &str, status: &str, image: Option<&str>) {
    let mut st = rd(&state_path()).unwrap_or(json!({}));
    let e = &mut st[name];
    e["status"] = json!(status);
    if status == "running" { e["last_run"] = json!(core::iso_now()); }
    if let Some(i) = image { e["last_image"] = json!(i); }
    e["updated"] = json!(core::iso_now());
    wr(&state_path(), &st);
}

fn window_ok(sc: &Value, now: NaiveDateTime, s: &Value) -> bool {
    if sc["window"].as_str() == Some("always") { return true; }
    let ev = core::sun_events(now.date(), s);
    let m = |k: &str| ev[k].as_str().map(|t| { let v: Vec<u32> = t.split(':').filter_map(|x| x.parse().ok()).collect(); v[0] * 60 + v[1] });
    let cur = now.hour() * 60 + now.minute();
    match (m("civil_dawn"), m("civil_dusk")) { (Some(a), Some(b)) => a <= cur && cur < b, _ => true }
}
fn parse_t(s: &str) -> Option<NaiveDateTime> { NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok() }

/// Allowed slot sizes (minutes). Marks are aligned to the clock: minute-of-day % every == 0
/// (15 → :00 :15 :30 :45, 30 → :00 :30, 60 → :00, 120 → even hours, 180 → 00/03/06…).
pub const SLOTS: [i64; 5] = [15, 30, 60, 120, 180];
pub fn slot(every: i64) -> i64 { if SLOTS.contains(&every) { every } else { 60 } }
pub fn is_mark(t: NaiveDateTime, every: i64) -> bool { t.second() == 0 && ((t.hour() * 60 + t.minute()) as i64) % slot(every) == 0 }
/// First aligned mark ≥ `now` (minute precision) that is not `done` (the last mark already fired or skipped)
/// and lies inside the window. Manual runs never touch `done`, so they don't shift the slots.
pub fn next_mark(now: NaiveDateTime, every: i64, done: Option<NaiveDateTime>, in_window: impl Fn(NaiveDateTime) -> bool) -> Option<NaiveDateTime> {
    let e = slot(every);
    let mut t = now.with_second(0).unwrap().with_nanosecond(0).unwrap();
    let m = (t.hour() * 60 + t.minute()) as i64;
    if m % e != 0 { t += chrono::Duration::minutes(e - m % e); }
    for _ in 0..(3 * 1440 / e + 2) {
        if Some(t) != done && in_window(t) { return Some(t); }
        t += chrono::Duration::minutes(e);
    }
    None
}
/// Durations (s) of the last ≤20 generations of a prompt.
pub fn durations(name: &str) -> Vec<u64> { rd(&state_path()).unwrap_or(json!({}))[name]["durations"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default() }
/// Robust lead: p75 of the measured durations (180 s default) + 30 s, clamped to slot − 60 s.
pub fn lead_secs_from(d: &[u64], every: i64) -> i64 {
    let base = if d.is_empty() { 180 } else { let mut v = d.to_vec(); v.sort(); v[((v.len() - 1) as f64 * 0.75).round() as usize] as i64 };
    (base + 30).min(slot(every) * 60 - 60).max(0)
}
pub fn lead_secs(name: &str, sc: &Value) -> i64 {
    if !sc["anticipate"].as_bool().unwrap_or(false) { return 0; }
    lead_secs_from(&durations(name), sc["every_min"].as_i64().unwrap_or(60))
}
pub fn record_run(name: &str, mark_t: Option<NaiveDateTime>, secs: u64, ok: bool) {
    let mut st = rd(&state_path()).unwrap_or(json!({}));
    let e = &mut st[name];
    if ok && secs > 0 {
        let mut d: Vec<Value> = e["durations"].as_array().cloned().unwrap_or_default(); d.push(json!(secs));
        let n = d.len(); e["durations"] = json!(d[n.saturating_sub(20)..].to_vec());
    }
    let mut runs: Vec<Value> = e["runs"].as_array().cloned().unwrap_or_default();
    runs.push(json!({"at": core::iso_now(), "mark": mark_t.map(|m| m.format("%H:%M").to_string()), "secs": secs, "ok": ok,
                     "late": mark_t.map(|m| Local::now().naive_local() > m + chrono::Duration::seconds(5))}));
    let n = runs.len(); e["runs"] = json!(runs[n.saturating_sub(10)..].to_vec());
    wr(&state_path(), &st);
}
fn done_mark(name: &str) -> Option<NaiveDateTime> { rd(&state_path()).unwrap_or(json!({}))[name]["last_slot"].as_str().and_then(parse_t) }

/// Next scheduled run (None when disabled).
pub fn next_run(name: &str, sc: &Value, s: &Value) -> Option<NaiveDateTime> {
    if !sc["enabled"].as_bool().unwrap_or(false) { return None; }
    next_mark(Local::now().naive_local(), sc["every_min"].as_i64().unwrap_or(60), done_mark(name), |t| window_ok(sc, t, s))
}

pub fn status_all() -> Value {
    let s = settings(); let st = rd(&state_path()).unwrap_or(json!({}));
    let mut out = json!({});
    if let Some(m) = s["schedules"].as_object() {
        for (n, sc) in m {
            out[n] = json!({"schedule": sc, "last_run": st[n]["last_run"], "status": st[n]["status"], "last_image": st[n]["last_image"],
                            "next_run": next_run(n, sc, &s).map(|t| t.format("%Y-%m-%d %H:%M").to_string()), "running_pid": running(),
                            "lead_secs": lead_secs(n, sc), "durations": st[n]["durations"], "runs": st[n]["runs"]});
        }
    }
    out
}

/// Called every minute by `cycle tick`.
pub fn run_due() {
    let s = settings();
    let Some(m) = s["schedules"].as_object() else { return };
    let now = Local::now().naive_local();
    for (n, sc) in m {
        if !sc["enabled"].as_bool().unwrap_or(false) || !core::styles_dir().join(format!("{n}.txt")).exists() { continue; }
        let every = slot(sc["every_min"].as_i64().unwrap_or(60));
        let lead = lead_secs(n, sc);
        // the earliest not-yet-done mark that is at most 4 min old; start it once now ≥ mark − lead
        let Some(mark_t) = next_mark(now - chrono::Duration::minutes(4), every, done_mark(n), |t| window_ok(sc, t, &s)) else { continue };
        if now < mark_t - chrono::Duration::seconds(lead) { continue; }
        // previous run still going: wait for it while the mark is ahead; skip the mark only once it has passed
        if running().is_some() && now < mark_t { println!("schedule '{n}': waiting for the running generation before the {} mark", mark_t.format("%H:%M")); continue; }
        let mut st = rd(&state_path()).unwrap_or(json!({})); st[n]["last_slot"] = json!(mark_t.format("%Y-%m-%dT%H:%M:%S").to_string()); wr(&state_path(), &st);
        if running().is_some() { println!("schedule '{n}': {} mark skipped – a generation is still running", mark_t.format("%H:%M")); mark(n, &format!("skipped {} (previous run still going)", mark_t.format("%H:%M")), None); continue; }
        println!("schedule '{n}': starting generation for the {} mark (lead {lead}s)", mark_t.format("%H:%M"));
        mark(n, "running", None);
        let unit = format!("cycle-gen-{}", Local::now().format("%H%M%S"));
        let exe = core::home().join(".local/bin/cycle");
        let at = mark_t.format("%Y-%m-%dT%H:%M:%S").to_string();
        let mut args: Vec<String> = ["systemd-run", "--user", "--collect", "--quiet", &format!("--unit={unit}"), "-p", "TimeoutStartSec=2400",
                    &exe.to_string_lossy(), "generate", "--prompt", n].iter().map(|x| x.to_string()).collect();
        if lead > 0 { args.push("--apply-at".into()); args.push(at); }
        let r: Vec<&str> = args.iter().map(|x| x.as_str()).collect(); core::shv(&r, 30);
        return; // one at a time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(h: u32, m: u32) -> NaiveDateTime { chrono::NaiveDate::from_ymd_opt(2026, 10, 15).unwrap().and_hms_opt(h, m, 0).unwrap() }
    fn ts(h: u32, m: u32, s: u32) -> NaiveDateTime { t(h, m) + chrono::Duration::seconds(s as i64) }
    #[test] fn aligned_marks() {
        let all = |_| true;
        assert_eq!(next_mark(ts(14, 7, 30), 15, None, all), Some(t(14, 15)));
        assert_eq!(next_mark(ts(14, 31, 0), 30, None, all), Some(t(15, 0)));
        assert_eq!(next_mark(ts(14, 5, 0), 60, None, all), Some(t(15, 0)));
        assert_eq!(next_mark(ts(13, 1, 0), 120, None, all), Some(t(14, 0)));
        assert_eq!(next_mark(ts(13, 1, 0), 180, None, all), Some(t(15, 0)));
        assert_eq!(next_mark(ts(14, 5, 0), 45, None, all), Some(t(15, 0))); // unsupported → hourly
    }
    #[test] fn exact_mark_and_done() {
        let all = |_| true;
        assert_eq!(next_mark(t(14, 0), 60, None, all), Some(t(14, 0)));
        assert_eq!(next_mark(ts(14, 0, 20), 60, Some(t(14, 0)), all), Some(t(15, 0)));
    }
    #[test] fn manual_run_does_not_shift() {
        // done only changes when a mark fires; a manual run at 14:20 leaves the next hourly mark at 15:00
        assert_eq!(next_mark(t(14, 20), 60, Some(t(14, 0)), |_| true), Some(t(15, 0)));
    }
    #[test] fn lead_time() {
        assert_eq!(lead_secs_from(&[], 60), 210);
        assert_eq!(lead_secs_from(&[100, 200, 300, 400, 700], 60), 430);
        assert_eq!(lead_secs_from(&[900, 900, 900], 15), 840); // clamped to slot − 1 min
    }
    #[test] fn window_respected() {
        let w = |x: NaiveDateTime| { let m = x.hour() * 60 + x.minute(); (360..1096).contains(&m) };
        assert_eq!(next_mark(t(18, 10), 15, None, w), Some(t(18, 15)));
        assert_eq!(next_mark(t(18, 16), 15, None, w), Some(chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap().and_hms_opt(6, 0, 0).unwrap()));
        assert_eq!(next_mark(t(17, 50), 15, None, w), Some(t(18, 0)));
    }
}
