//! GTK4 / libadwaita window: Now, Data, Prompt, Presets, Cycles, Rules.
use crate::{core, live, theme};
use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const KEEP: &str = "— keep current —";

struct Ui { stack: adw::ViewStack, toast: adw::ToastOverlay, graph: RefCell<Option<gtk::DrawingArea>>, cdata: Rc<RefCell<Option<Value>>>,
            live: RefCell<Option<adw::ActionRow>>, next: RefCell<Option<adw::ActionRow>>, resgroup: RefCell<Option<adw::PreferencesGroup>> }
type U = Rc<Ui>;
thread_local! {
    static UI: RefCell<Option<U>> = const { RefCell::new(None) };
    static PROMPT_CARDS: RefCell<Vec<(String, gtk::TextView)>> = const { RefCell::new(Vec::new()) };
}

fn notify(u: &U, m: &str) { u.toast.add_toast(adw::Toast::builder().title(m).timeout(3).build()); }
fn row(t: &str, s: &str) -> adw::ActionRow { adw::ActionRow::builder().title(glib::markup_escape_text(t)).subtitle(glib::markup_escape_text(s)).subtitle_selectable(true).build() }
fn btn(l: &str) -> gtk::Button { gtk::Button::builder().label(l).valign(gtk::Align::Center).build() }
fn icon_btn(i: &str) -> gtk::Button { gtk::Button::builder().icon_name(i).valign(gtk::Align::Center).css_classes(["flat"]).build() }
fn dropdown(opts: &[String], cur: &str) -> gtk::DropDown {
    let v: Vec<&str> = opts.iter().map(|s| s.as_str()).collect();
    let d = gtk::DropDown::from_strings(&v); d.set_valign(gtk::Align::Center);
    if let Some(i) = opts.iter().position(|o| o == cur) { d.set_selected(i as u32); } d
}
/// ActionRow + Entry + ✓ (Adw.EntryRow is avoided: crashed on this setup)
fn erow(title: &str, text: &str, cb: impl Fn(String) + 'static) -> (adw::ActionRow, gtk::Entry) {
    let r = adw::ActionRow::builder().title(title).build();
    let e = gtk::Entry::builder().text(text).hexpand(true).valign(gtk::Align::Center).width_chars(28).build();
    let b = icon_btn("object-select-symbolic"); r.add_suffix(&e); r.add_suffix(&b);
    let cb = Rc::new(cb);
    { let cb = cb.clone(); e.connect_activate(move |e| cb(e.text().to_string())); }
    { let e = e.clone(); b.connect_clicked(move |_| cb(e.text().to_string())); }
    (r, e)
}
fn page(u: &U, name: &str, title: &str, icon: &str) -> adw::PreferencesPage {
    if let Some(old) = u.stack.child_by_name(name) { u.stack.remove(&old); }
    let pg = adw::PreferencesPage::new();
    // keep tab order stable: re-add at the right position by rebuilding everything in order (see build)
    u.stack.add_titled_with_icon(&pg, Some(name), title, icon); pg
}
fn group(t: &str, d: &str) -> adw::PreferencesGroup { adw::PreferencesGroup::builder().title(glib::markup_escape_text(t)).description(glib::markup_escape_text(d)).build() }

/// Run blocking work off the main thread, then continue on the main loop.
fn bg<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    glib::spawn_future_local(async move { if let Ok(v) = gio::spawn_blocking(work).await { done(v); } });
}
fn spawn_self(u: &U, args: &[&str], msg: &'static str) {
    let exe = core::home().join(".local/bin/cycle");
    let mut argv: Vec<std::ffi::OsString> = vec![exe.into()]; argv.extend(args.iter().map(|a| a.into()));
    let refs: Vec<&std::ffi::OsStr> = argv.iter().map(|a| a.as_os_str()).collect();
    match gio::Subprocess::newv(&refs, gio::SubprocessFlags::NONE) {
        Ok(p) => { let u = u.clone(); p.wait_async(None::<&gio::Cancellable>, move |_| notify(&u, msg)); }
        Err(e) => notify(u, &format!("Error: {e}")),
    }
}

fn spawn_self_owned(u: &U, args: Vec<String>, msg: &'static str) { let r: Vec<&str> = args.iter().map(|x| x.as_str()).collect(); spawn_self(u, &r, msg); }
/// `cycle open --prompt ID`: show the Prompt tab and focus that card.
fn open_requested(u: &U) {
    if let Ok(t) = std::env::var("CYCLE_TAB") { u.stack.set_visible_child_name(&t); }
    let f = core::state_dir().join("open-prompt");
    let Ok(id) = std::fs::read_to_string(&f) else { return }; let _ = std::fs::remove_file(&f);
    u.stack.set_visible_child_name("prompt");
    if let Some(g) = PROMPT_CARDS.with(|c| c.borrow().iter().find(|(n, _)| *n == id).map(|x| x.1.clone())) {
        let g2 = g.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || { g2.grab_focus(); });
        let _ = g;
    }
}
fn build(u: &U) {
    let vis = u.stack.visible_child_name();
    for n in ["status", "data", "prompt", "schedule", "presets"] { if let Some(c) = u.stack.child_by_name(n) { u.stack.remove(&c); } }
    build_status(u); build_data(u); build_prompt(u); build_schedule(u); build_presets(u); build_cycles(u); build_rules(u);
    if let Some(v) = vis { u.stack.set_visible_child_name(&v); }
}
fn rebuild(u: &U) { let u = u.clone(); glib::idle_add_local_once(move || build(&u)); }

// ---------------- Now ----------------
fn build_status(u: &U) {
    let pg = page(u, "status", "Now", "weather-clear-symbolic");
    let cg = group("Circadian curve – today", "Continuous K and brightness from real solar elevation (NOAA), Dubai. Updates every minute.");
    cg.add(&hold_row(u));
    let live = row("Now", "…"); live.add_css_class("property"); cg.add(&live);
    let next = row("Next update", "…"); cg.add(&next);
    let graph = gtk::DrawingArea::builder().content_height(210).hexpand(true).build();
    { let cd = u.cdata.clone(); graph.set_draw_func(move |a, cr, w, h| draw_curve(a, cr, w, h, &cd.borrow())); }
    cg.add(&gtk::Frame::builder().child(&graph).margin_top(6).build()); pg.add(&cg);
    *u.graph.borrow_mut() = Some(graph); *u.live.borrow_mut() = Some(live); *u.next.borrow_mut() = Some(next);
    let g = group("Engine", "Background timer (every minute) applies the resolved preset.");
    let sw = adw::SwitchRow::builder().title("Automatic Cycle").subtitle("Off by default – nothing changes until you enable it").active(core::settings()["enabled"].as_bool().unwrap_or(false)).build();
    { let u = u.clone(); sw.connect_active_notify(move |r| { let on = r.is_active(); bg(move || core::set_engine(on), { let u = u.clone(); move |_| notify(&u, if on { "Engine on" } else { "Engine off" }) }); }); }
    g.add(&sw);
    let b = btn("Apply now"); b.add_css_class("suggested-action");
    { let u = u.clone(); b.connect_clicked(move |_| bg(|| { core::tick(true); }, { let u = u.clone(); move |_| notify(&u, "Applied") })); }
    let ar = adw::ActionRow::builder().title("Apply the resolved preset once").build(); ar.add_suffix(&b); g.add(&ar);
    let gb = btn("Generate"); 
    { let u = u.clone(); gb.connect_clicked(move |_| { notify(&u, "Cycle: generating wallpaper from live data"); spawn_self(&u, &["generate"], "Generation finished"); }); }
    let gr = adw::ActionRow::builder().title("Cycle Generate").subtitle("Live data → prompt → image → applied + archived (can take minutes)").build(); gr.add_suffix(&gb); g.add(&gr);
    pg.add(&g);
    let rg = group("Resolution", "Loading…"); pg.add(&rg); *u.resgroup.borrow_mut() = Some(rg);
    refresh_curve(u);
}
fn hold_row(u: &U) -> adw::ActionRow {
    let mut names = vec!["Automatic".to_string()]; names.extend(core::list_names(&core::presets_dir(), "json"));
    let cur = core::settings()["hold"].as_str().unwrap_or("Automatic").to_string();
    let d = dropdown(&names, &cur);
    let r = adw::ActionRow::builder().title("Preset").subtitle("Automatic = curve, phases, weather, day type, theme generation · a preset = fixed values, held").build();
    r.add_suffix(&d);
    let u = u.clone();
    d.connect_selected_notify(move |d| {
        let n = names[d.selected() as usize].clone();
        if core::settings()["hold"].as_str().unwrap_or("Automatic") == n { return; }
        let u2 = u.clone(); let n2 = n.clone();
        bg(move || core::set_hold(Some(&n2)), move |_| { notify(&u2, &format!("Preset: {n}")); rebuild(&u2); });
    });
    r
}
fn summary(p: &Value) -> String {
    format!("theme {} · bright {} · night {} · {}", p["theme"].as_str().unwrap_or("keep"),
        p["brightness"].as_i64().map(|b| b.to_string()).unwrap_or("keep".into()),
        if p["night_shift"].as_bool().unwrap_or(false) { format!("on {}K", p["warmth"]) } else { "off".into() },
        if p["wallpaper"].as_str().map(|w| !w.is_empty()).unwrap_or(false) { "wallpaper ✓" } else { "wallpaper keep" })
}
fn refresh_curve(u: &U) {
    let u = u.clone();
    bg(|| core::curve(5), move |d| {
        let (Some(live), Some(next)) = (u.live.borrow().clone(), u.next.borrow().clone()) else { return };
        if let Some(h) = d["hold"].as_str() {
            live.set_subtitle(&glib::markup_escape_text(&format!("HOLD '{h}': {} · curve shown for reference only", summary(&core::preset(h).unwrap_or(json!({}))))));
        } else {
            live.set_subtitle(&glib::markup_escape_text(&format!("{} K · brightness {}% · sun {:+.1}° (noon peak {}°) · {} → preset {}{}", d["target_k"], d["target_b"],
                d["elev"].as_f64().unwrap_or(0.0), d["peak"], d["phase"].as_str().unwrap_or(""), d["preset"].as_str().unwrap_or("—"),
                d["rule"].as_str().map(|r| format!(" (weather: {r})")).unwrap_or_default())));
        }
        let eng = d["enabled"].as_bool().unwrap_or(false);
        next.set_subtitle(&format!("sunrise {}, sunset {} · engine: {}", d["sun"]["sunrise"].as_str().unwrap_or(""), d["sun"]["sunset"].as_str().unwrap_or(""),
            if eng { d["next_tick"].as_str().map(|t| format!("next check {t}")).unwrap_or("on".into()) } else { "off (manual only)".into() }));
        if let Some(rg) = u.resgroup.borrow().clone() {
            if rg.description().map(|x| x == "Loading…").unwrap_or(false) {
                rg.set_description(None); let r = &d["resolve"]; let w = &r["weather"];
                let wt = w["error"].as_str().map(|e| e.to_string()).unwrap_or(format!("{}°C, {}, dust {} µg/m³", w["temperature"], w["condition"].as_str().unwrap_or(""), w["dust"]));
                for (t, s) in [("Phase", format!("{}  (morning {} · afternoon {} · evening {} · night {})", r["phase"].as_str().unwrap_or(""), r["sun"]["morning_start"].as_str().unwrap_or(""), r["sun"]["afternoon_start"].as_str().unwrap_or(""), r["sun"]["evening_start"].as_str().unwrap_or(""), r["sun"]["night_start"].as_str().unwrap_or(""))),
                               ("Day type", r["day_type"].as_str().unwrap_or("").into()), ("Wi-Fi", r["ssid"].as_str().unwrap_or("—").into()),
                               ("Location", format!("{} {}", r["location"]["name"].as_str().unwrap_or(""), r["location"]["note"].as_str().unwrap_or(""))),
                               ("Weather", wt), ("Cycle", r["cycle"].as_str().unwrap_or("").into()), ("Preset", r["preset"].as_str().unwrap_or("—").into()), ("Why", r["source"].as_str().unwrap_or("").into())] {
                    rg.add(&row(t, &s));
                }
            }
        }
        *u.cdata.borrow_mut() = Some(d);
        if let Some(g) = u.graph.borrow().as_ref() { g.queue_draw(); }
    });
}
fn draw_curve(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, w: i32, h: i32, d: &Option<Value>) {
    let Some(d) = d else { return };
    let (w, h) = (w as f64, h as f64); let (l, r, t, b) = (40.0, 36.0, 12.0, 22.0); let (ww, hh) = (w - l - r, h - t - b);
    let x = |m: f64| l + ww * m / 1440.0;
    let fg = area.color();
    let pts: Vec<[f64; 4]> = d["points"].as_array().map(|a| a.iter().map(|p| { let g = |i: usize| p[i].as_f64().unwrap_or(0.0); [g(0), g(1), g(2), g(3)] }).collect()).unwrap_or_default();
    for pr in pts.windows(2) {
        let e = (pr[0][3] + pr[1][3]) / 2.0;
        if e < 0.0 { cr.set_source_rgba(0.25, 0.3, 0.6, (0.06 + -e / 60.0).min(0.22)); cr.rectangle(x(pr[0][0]), t, x(pr[1][0]) - x(pr[0][0]) + 0.6, hh); let _ = cr.fill(); }
    }
    cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.55); cr.set_font_size(10.0);
    for hr in (0..25).step_by(3) { cr.move_to(x(hr as f64 * 60.0) - 7.0, h - 6.0); let _ = cr.show_text(&format!("{hr:02}")); }
    let smooth = |idx: usize, lo: f64, hi: f64, c: (f64, f64, f64)| {
        let p: Vec<(f64, f64)> = pts.iter().map(|q| (x(q[0]), t + hh * (1.0 - (q[idx] - lo) / (hi - lo)))).collect();
        if p.is_empty() { return; }
        cr.set_source_rgb(c.0, c.1, c.2); cr.set_line_width(2.2); cr.move_to(p[0].0, p[0].1);
        for i in 1..p.len() {
            let (p0, p1, p2, p3) = (p[i.saturating_sub(2)], p[i - 1], p[i], p[(i + 1).min(p.len() - 1)]);
            cr.curve_to(p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0, p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0, p2.0, p2.1);
        }
        let _ = cr.stroke();
    };
    smooth(1, 2500.0, 6600.0, (1.0, 0.55, 0.15)); smooth(2, 0.0, 100.0, (0.25, 0.6, 1.0));
    cr.set_source_rgb(1.0, 0.55, 0.15); cr.move_to(2.0, t + 9.0); let _ = cr.show_text("6600K"); cr.move_to(2.0, t + hh); let _ = cr.show_text("2500K");
    cr.set_source_rgb(0.25, 0.6, 1.0); cr.move_to(w - r + 4.0, t + 9.0); let _ = cr.show_text("100%"); cr.move_to(w - r + 4.0, t + hh); let _ = cr.show_text("0%");
    let nx = x(d["now"].as_f64().unwrap_or(0.0)); cr.set_source_rgb(0.9, 0.1, 0.2); cr.set_line_width(1.5); cr.move_to(nx, t); cr.line_to(nx, t + hh); let _ = cr.stroke();
    let (tk, tb) = (d["target_k"].as_f64().unwrap_or(0.0), d["target_b"].as_f64().unwrap_or(0.0));
    for (v, lo, hi) in [(tk, 2500.0, 6600.0), (tb, 0.0, 100.0)] { cr.arc(nx, t + hh * (1.0 - (v - lo) / (hi - lo)), 3.5, 0.0, 6.3); let _ = cr.fill(); }
    cr.move_to((nx + 4.0).min(w - r - 80.0), t + 10.0); let _ = cr.show_text(&format!("now {tk}K {tb}%"));
}

// ---------------- Data ----------------
fn build_data(u: &U) {
    let pg = page(u, "data", "Data", "network-transmit-receive-symbolic");
    let d = core::rd(&core::weather_now_path()).unwrap_or(json!({}));
    let g = group("Live data", &format!("Open-Meteo forecast, air quality and marine for {} · fetched {}", core::settings()["location"]["name"].as_str().unwrap_or("Dubai"), d["fetched_at"].as_str().unwrap_or("at the last Cycle Generate")));
    let rb = btn("Refresh");
    { let u = u.clone(); rb.connect_clicked(move |b| { b.set_sensitive(false); let u = u.clone();
        bg(|| { let s = core::settings(); let mut d = live::fetch(&s); let (h, src) = live::headline(&s);
                if let Some(old) = core::rd(&core::weather_now_path()) { d["prompt"] = old["prompt"].clone(); }
                d["headline"] = json!(h); d["headline_source"] = json!(src); core::wr(&core::weather_now_path(), &d); },
           move |_| { notify(&u, "Live data refreshed"); rebuild(&u); }); }); }
    g.set_header_suffix(Some(&rb));
    let (w, a, m) = (&d["weather"], &d["air"], &d["marine"]);
    let v = |x: &Value, unit: &str| if x.is_null() { "—".to_string() } else { format!("{x}{unit}") };
    let rows: Vec<(&str, String)> = vec![
        ("Summary", live::weather_text(&d)),
        ("Sun", live::sun_text(&d)),
        ("Temperature", format!("{} (feels {})", v(&w["temperature_2m"], " °C"), v(&w["apparent_temperature"], " °C"))),
        ("Humidity / dew point", format!("{} · {}", v(&w["relative_humidity_2m"], " %"), v(&w["dew_point_2m"], " °C"))),
        ("Condition", format!("{} (WMO {}) · {}", live::condition(w["weather_code"].as_i64().unwrap_or(0)), w["weather_code"], if w["is_day"] == json!(1) { "day" } else { "night" })),
        ("Precipitation / rain", format!("{} · {}", v(&w["precipitation"], " mm"), v(&w["rain"], " mm"))),
        ("Cloud cover", format!("{} total · low {} · mid {} · high {}", v(&w["cloud_cover"], "%"), v(&w["cloud_cover_low"], "%"), v(&w["cloud_cover_mid"], "%"), v(&w["cloud_cover_high"], "%"))),
        ("Visibility", w["visibility"].as_f64().map(|x| format!("{:.1} km", x / 1000.0)).unwrap_or("—".into())),
        ("Wind", format!("{} from {}° · gusts {}", v(&w["wind_speed_10m"], " km/h"), w["wind_direction_10m"], v(&w["wind_gusts_10m"], " km/h"))),
        ("Pressure / UV", format!("{} · UV {}", v(&w["pressure_msl"], " hPa"), w["uv_index"])),
        ("Air quality", format!("US AQI {} · PM2.5 {} · PM10 {} · dust {} · AOD {} · ozone {}", a["us_aqi"], a["pm2_5"], a["pm10"], a["dust"], a["aerosol_optical_depth"], a["ozone"])),
        ("Sea", format!("waves {} · water {}", v(&m["wave_height"], " m"), v(&m["sea_surface_temperature"], " °C"))),
        ("Headline", format!("{} {}", d["headline"].as_str().filter(|h| !h.is_empty()).unwrap_or("—"), d["headline_source"].as_str().map(|s| format!("({s})")).unwrap_or_default())),
    ];
    for (t, s) in rows { g.add(&row(t, &s)); }
    pg.add(&g);

    let s = core::settings();
    let fg = group("RSS feeds", "The LED billboard headline comes from the first enabled feed with a fresh English headline.");
    let editing: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let name_e = gtk::Entry::builder().placeholder_text("Name").hexpand(true).build();
    let url_e = gtk::Entry::builder().placeholder_text("https://…/rss").hexpand(true).build();
    for (i, f) in s["feeds"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
        let r = row(f["name"].as_str().unwrap_or("feed"), f["url"].as_str().unwrap_or(""));
        let sw = gtk::Switch::builder().active(f["enabled"].as_bool().unwrap_or(true)).valign(gtk::Align::Center).build();
        { let u = u.clone(); sw.connect_active_notify(move |w| { let on = w.is_active(); core::update_settings(|s| s["feeds"][i]["enabled"] = json!(on)); notify(&u, "Saved"); }); }
        let eb = icon_btn("document-edit-symbolic");
        { let (ed, ne, ue) = (editing.clone(), name_e.clone(), url_e.clone()); let f = f.clone();
          eb.connect_clicked(move |_| { ed.set(Some(i)); ne.set_text(f["name"].as_str().unwrap_or("")); ue.set_text(f["url"].as_str().unwrap_or("")); ne.grab_focus(); }); }
        let xb = icon_btn("user-trash-symbolic");
        { let u = u.clone(); xb.connect_clicked(move |_| { core::update_settings(|s| { if let Some(a) = s["feeds"].as_array_mut() { if i < a.len() { a.remove(i); } } }); notify(&u, "Feed removed"); rebuild(&u); }); }
        r.add_suffix(&sw); r.add_suffix(&eb); r.add_suffix(&xb); fg.add(&r);
    }
    pg.add(&fg);
    let ag = group("Add / edit feed", "Pencil on a feed loads it here; Save updates it, otherwise a new feed is added.");
    let r1 = adw::ActionRow::builder().title("Name").build(); r1.add_suffix(&name_e); ag.add(&r1);
    let r2 = adw::ActionRow::builder().title("URL").build(); r2.add_suffix(&url_e); ag.add(&r2);
    let sb = btn("Save feed"); sb.add_css_class("suggested-action");
    { let u = u.clone(); let (ed, ne, ue) = (editing.clone(), name_e.clone(), url_e.clone());
      sb.connect_clicked(move |_| {
        let (n, url) = (ne.text().trim().to_string(), ue.text().trim().to_string());
        if !url.starts_with("http") { notify(&u, "Enter an http(s) feed URL"); return; }
        let n = if n.is_empty() { url.clone() } else { n };
        let idx = ed.get();
        core::update_settings(|s| {
            if !s["feeds"].is_array() { s["feeds"] = json!([]); }
            let a = s["feeds"].as_array_mut().unwrap();
            match idx { Some(i) if i < a.len() => { a[i]["name"] = json!(n); a[i]["url"] = json!(url); } _ => a.push(json!({"name": n, "url": url, "enabled": true})) }
        });
        notify(&u, "Feed saved"); rebuild(&u);
      }); }
    let r3 = adw::ActionRow::builder().title("").build(); r3.add_suffix(&sb); ag.add(&r3);
    pg.add(&ag);
}

// ---------------- Prompt ----------------
fn text_of(tv: &gtk::TextView) -> String { let b = tv.buffer(); b.text(&b.start_iter(), &b.end_iter(), false).to_string() }
fn safe_name(n: &str) -> String { n.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect() }
fn after_prompt_change(u: &U) { crate::menu::sync(); rebuild(u); }

/// Slash autocomplete: `/` + letters (and further `/segments`) pops up the next valid segment.
fn attach_autocomplete(tv: &gtk::TextView, extra: Rc<Vec<String>>) {
    let pop = gtk::Popover::builder().autohide(false).has_arrow(false).position(gtk::PositionType::Bottom).build();
    pop.set_parent(tv);
    let lb = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build(); pop.set_child(Some(&lb));
    let tv2 = tv.clone();
    tv.buffer().connect_changed(move |buf| {
        let it = buf.iter_at_mark(&buf.get_insert()); let mut st = it.clone();
        let mut tok = String::new(); let start_ok;
        loop {
            if !st.backward_char() { start_ok = tok.starts_with('/'); break; }
            let c = st.char();
            if c.is_ascii_alphanumeric() || "_/+%-".contains(c) { tok.insert(0, c) } else { start_ok = tok.starts_with('/') && ("([,;:\"'".contains(c) || c.is_whitespace()); break; }
        }
        while let Some(c) = lb.first_child() { lb.remove(&c); }
        if !start_ok { pop.popdown(); return; }
        let cur = tok.rsplit('/').next().unwrap_or("").to_string();
        let hits = crate::lang::complete(&tok, &extra);
        if hits.is_empty() { pop.popdown(); return; }
        for h in hits.into_iter().take(8) {
            let b = gtk::Button::builder().label(h.clone()).css_classes(["flat"]).build();
            let (tv3, cur, pop2) = (tv2.clone(), cur.clone(), pop.clone());
            b.connect_clicked(move |_| { pop2.popdown(); tv3.buffer().insert_at_cursor(&h[cur.len().min(h.len())..]); tv3.grab_focus(); });
            lb.append(&b);
        }
        let r = tv2.iter_location(&it); let (x, y) = tv2.buffer_to_window_coords(gtk::TextWindowType::Widget, r.x(), r.y() + r.height());
        pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x, y, 1, 1))); pop.popup();
    });
}
/// Underline invalid calls in red; returns the error messages.
fn mark_errors(buf: &gtk::TextBuffer, images: usize) -> Vec<String> {
    let tag = buf.tag_table().lookup("unknown").unwrap_or_else(|| { let t = gtk::TextTag::builder().name("unknown").underline(gtk::pango::Underline::Error).foreground("#e06c75").build(); buf.tag_table().add(&t); t });
    buf.remove_tag(&tag, &buf.start_iter(), &buf.end_iter());
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
    let mut errs = vec![];
    for c in crate::lang::parse(&text) {
        if let Err(e) = crate::lang::validate(&c, images) {
            let a = text[..c.start].chars().count() as i32; let b = text[..c.end].chars().count() as i32;
            buf.apply_tag(&tag, &buf.iter_at_offset(a), &buf.iter_at_offset(b)); errs.push(e);
        }
    }
    errs.dedup(); errs
}

fn build_prompt(u: &U) {
    let pg = page(u, "prompt", "Prompt", "document-edit-symbolic");
    PROMPT_CARDS.with(|c| c.borrow_mut().clear());
    let names = core::prompt_names(); let active = core::active_prompt();
    let s = core::settings(); let images = s["images"].as_array().map(|a| a.len()).unwrap_or(0);
    let vars = Rc::new(live::cached_vars(&s));
    let extra: Rc<Vec<String>> = Rc::new((1..=images).map(|i| format!("image{i}")).collect());
    let hg = group("Prompts", "Each prompt is a card; ✓ = used by Cycle Generate. Syntax: /command/arg/arg (see Variables at the bottom). Invalid calls are underlined in red.");
    let nb = btn("New prompt");
    { let u = u.clone(); nb.connect_clicked(move |_| {
        let mut i = 1; while core::styles_dir().join(format!("prompt-{i}.txt")).exists() { i += 1; }
        core::save_prompt(&format!("prompt-{i}"), "A wide 16:9 desktop wallpaper of /location in the /phase. Light: /sun. Weather: /weather. Air: /air. In the style: /style. LED billboard shows: /breaking. No characters, no other text.");
        notify(&u, &format!("Created prompt-{i}")); after_prompt_change(&u); }); }
    hg.set_header_suffix(Some(&nb)); pg.add(&hg);
    for (i, n) in names.iter().enumerate() {
        let is_active = *n == active;
        let g = group(&format!("Prompt {} · {}{}", i + 1, crate::menu::title(n), if is_active { "  ✓ active" } else { "" }), &format!("{}/{n}.txt", core::styles_dir().display()));
        let hb = gtk::Box::builder().spacing(4).build();
        let use_ = gtk::Button::with_label("Use"); use_.set_sensitive(!is_active);
        let genb = icon_btn("media-playback-start-symbolic"); genb.set_tooltip_text(Some("Generate now"));
        let dup = icon_btn("edit-copy-symbolic"); dup.set_tooltip_text(Some("Duplicate"));
        let del = icon_btn("user-trash-symbolic"); del.set_tooltip_text(Some("Delete")); del.set_sensitive(!is_active);
        hb.append(&use_); hb.append(&genb); hb.append(&dup); hb.append(&del); g.set_header_suffix(Some(&hb));
        { let u = u.clone(); let n = n.clone(); use_.connect_clicked(move |_| { core::set_active_prompt(&n); notify(&u, &format!("Active prompt: {n}")); after_prompt_change(&u); }); }
        { let u = u.clone(); let n = n.clone(); genb.connect_clicked(move |_| { notify(&u, "Cycle: generating wallpaper from live data"); spawn_self_owned(&u, vec!["generate".into(), "--prompt".into(), n.clone()], "Generation finished"); }); }
        { let u = u.clone(); let n = n.clone(); dup.connect_clicked(move |_| { let mut c = format!("{n}-copy"); let mut k = 2;
            while core::styles_dir().join(format!("{c}.txt")).exists() { c = format!("{n}-copy{k}"); k += 1; }
            core::save_prompt(&c, &core::prompt_text(&n)); crate::menu::set_title(&c, &format!("{} (copy)", crate::menu::title(&n))); notify(&u, &format!("Duplicated as {c}")); after_prompt_change(&u); }); }
        { let u = u.clone(); let n = n.clone(); del.connect_clicked(move |_| { core::delete_prompt(&n); core::update_settings(|s| { if let Some(o) = s["schedules"].as_object_mut() { o.remove(&n); } }); notify(&u, &format!("Deleted {n}")); after_prompt_change(&u); }); }
        let title_e = gtk::Entry::builder().text(crate::menu::title(n)).hexpand(true).valign(gtk::Align::Center).build();
        let tr = adw::ActionRow::builder().title("Title").build(); tr.add_suffix(&title_e); g.add(&tr);
        let name_e = gtk::Entry::builder().text(n).hexpand(true).valign(gtk::Align::Center).build();
        let nr = adw::ActionRow::builder().title("Id").build(); nr.add_suffix(&name_e); g.add(&nr);
        // full text, never truncated: the TextView grows with its content
        let tv = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).top_margin(8).bottom_margin(8).left_margin(8).right_margin(8).build();
        tv.buffer().set_text(&core::prompt_text(n));
        g.add(&gtk::Frame::builder().child(&tv).margin_top(6).build());
        attach_autocomplete(&tv, extra.clone());
        PROMPT_CARDS.with(|c| c.borrow_mut().push((n.clone(), tv.clone())));
        let err = gtk::Label::builder().xalign(0.0).wrap(true).margin_top(4).css_classes(["error"]).visible(false).build(); g.add(&err);
        let save = gtk::Button::with_label("Save"); save.add_css_class("suggested-action");
        let bar = gtk::Box::builder().spacing(6).margin_top(6).halign(gtk::Align::End).build(); bar.append(&save); g.add(&bar);
        g.add(&gtk::Label::builder().label("Live preview").xalign(0.0).margin_top(8).css_classes(["heading"]).build());
        let preview = gtk::Label::builder().wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).selectable(true).xalign(0.0).margin_top(4).css_classes(["dim-label"]).build();
        g.add(&preview);
        let gen_id = Rc::new(Cell::new(0u32));
        let refresh = { let (pv, err, tv) = (preview.clone(), err.clone(), tv.clone()); let vars = vars.clone(); let gen_id = gen_id.clone();
            move || {
                let errs = mark_errors(&tv.buffer(), images);
                err.set_visible(!errs.is_empty()); err.set_text(&errs.join("\n"));
                // debounce, then resolve off the main thread (network results cached per run)
                let id = gen_id.get() + 1; gen_id.set(id);
                let (pv, tv, vars, gen_id) = (pv.clone(), tv.clone(), vars.clone(), gen_id.clone());
                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    if gen_id.get() != id { return; }
                    let t = text_of(&tv); let base: Vec<(String, String)> = (*vars).clone();
                    bg(move || crate::lang::expand(&t, &core::settings(), &base), move |x| if gen_id.get() == id { pv.set_text(&x) });
                });
            } };
        refresh();
        { let refresh = refresh.clone(); tv.buffer().connect_changed(move |_| refresh()); }
        { let u = u.clone(); let n = n.clone(); let tv = tv.clone(); save.connect_clicked(move |_| {
            let nn = safe_name(&name_e.text()); if nn.is_empty() { notify(&u, "Give the prompt an id"); return; }
            let t = text_of(&tv); let errs = crate::lang::errors(&t, images);
            core::save_prompt(&nn, &t);
            if nn != n { core::delete_prompt(&n); if is_active { core::set_active_prompt(&nn); }
                core::update_settings(|s| { if let Some(o) = s["schedules"].as_object_mut() { if let Some(v) = o.remove(&n) { o.insert(nn.clone(), v); } } }); }
            crate::menu::set_title(&nn, &title_e.text());
            notify(&u, &if errs.is_empty() { format!("Saved {nn}") } else { format!("Saved {nn} – {} error(s)", errs.len()) }); after_prompt_change(&u); }); }
        { let sl = gtk::Button::with_label("Schedule…"); sl.add_css_class("flat"); let u2 = u.clone(); sl.connect_clicked(move |_| u2.stack.set_visible_child_name("schedule")); bar.prepend(&sl); }
        pg.add(&g);
    }
    let vg = group("Variables", "Syntax: /command[/arg…] (or {{command}}). Args: /city/<Name> (_ for spaces, geocoded), /today /tomorrow /yesterday, /query/<words> for /news. Values below: today, current location.");
    for (k, v) in vars.iter().filter(|v| v.0 != "location_full") {
        let desc = live::VARS.iter().find(|x| x.0 == k).map(|x| x.1).unwrap_or("reference image");
        let args = crate::lang::spec(k).map(|a| if a.is_empty() { String::new() } else { format!(" · args: {}", a.join(", ")) }).unwrap_or_default();
        vg.add(&row(&format!("/{k}"), &format!("{desc}{args} → {}", if v.chars().count() > 160 { format!("{}…", v.chars().take(160).collect::<String>()) } else { v.clone() })));
    }
    pg.add(&vg);
}

// ---------------- Schedule ----------------
/// The scheduled prompt (first enabled entry, else the active prompt) and its config.
fn sched_target(s: &Value) -> (String, Value) {
    if let Some(m) = s["schedules"].as_object() { for (n, sc) in m { if sc["enabled"].as_bool().unwrap_or(false) { return (n.clone(), sc.clone()); } } }
    let a = core::active_prompt(); let sc = s["schedules"][&a].clone();
    (a, if sc.is_null() { json!({"enabled": false, "every_min": 60, "window": "dawn-dusk", "anticipate": false}) } else { sc })
}
/// Write the single schedule: `name` gets `sc`, every other prompt is disabled.
fn save_sched(name: &str, sc: Value) {
    core::update_settings(|s| {
        if !s["schedules"].is_object() { s["schedules"] = json!({}); }
        if let Some(m) = s["schedules"].as_object_mut() { for (k, v) in m.iter_mut() { if k != name { v["enabled"] = json!(false); } } }
        s["schedules"][name] = sc;
    });
}
fn fmt_secs(x: i64) -> String { if x >= 60 { format!("{}m{:02}s", x / 60, x % 60) } else { format!("{x}s") } }

#[allow(deprecated)]
fn build_schedule(u: &U) {
    let pg = page(u, "schedule", "Schedule", "alarm-symbolic");
    let s = core::settings(); let (name, sc) = sched_target(&s);
    let every = crate::sched::slot(sc["every_min"].as_i64().unwrap_or(60));
    let st = crate::sched::status_all(); let e = st[&name].clone();
    let lead = crate::sched::lead_secs_from(&crate::sched::durations(&name), every);
    let anticipate = sc["anticipate"].as_bool().unwrap_or(false);
    let set = { let u = u.clone(); let name = name.clone(); let sc = sc.clone();
        Rc::new(move |k: &str, v: Value| { let mut c = sc.clone(); c[k] = v; save_sched(&name, c); rebuild(&u); }) };

    // modes: big segmented control
    let g = group("Mode", "Clock-aligned slots. Manual runs never shift them.");
    let seg = gtk::Box::builder().css_classes(["linked"]).homogeneous(true).hexpand(true).margin_top(4).build();
    let mut first: Option<gtk::ToggleButton> = None;
    for (mins, label) in [(15i64, "15 min"), (30, "30 min"), (60, "Hourly")] {
        let b = gtk::ToggleButton::builder().label(label).active(every == mins).height_request(48).build();
        if let Some(f) = &first { b.set_group(Some(f)); } else { first = Some(b.clone()); }
        let set = set.clone(); b.connect_toggled(move |b| if b.is_active() && every != mins { set("every_min", json!(mins)) });
        seg.append(&b);
    }
    g.add(&seg);
    let on = adw::SwitchRow::builder().title("Scheduled").subtitle("Off = only manual runs (menu / Generate)").active(sc["enabled"].as_bool().unwrap_or(false)).build();
    { let set = set.clone(); on.connect_active_notify(move |r| set("enabled", json!(r.is_active()))); }
    g.add(&on); pg.add(&g);

    // timeline of the current hour + countdown
    let tg = group("This hour", "Bars: generation starts ahead of each mark (lead time); the image lands exactly on the mark.");
    let tl = gtk::DrawingArea::builder().content_height(84).hexpand(true).build();
    let lead_draw = if anticipate { lead } else { 0 };
    tl.set_draw_func(move |a, cr, w, h| {
        let (w, h) = (w as f64, h as f64); let (l, r) = (14.0, 14.0); let ww = w - l - r;
        let x = |min: f64| l + ww * min / 60.0;
        let fg = a.color(); let acc = a.style_context().lookup_color("accent_color").unwrap_or(fg);
        cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.35); cr.set_line_width(1.0);
        cr.move_to(l, h - 26.0); cr.line_to(w - r, h - 26.0); let _ = cr.stroke();
        cr.set_font_size(11.0);
        let mut m = 0i64;
        while m <= 60 {
            let xm = x(m as f64);
            // lead bar
            if lead_draw > 0 && m > 0 || (lead_draw > 0 && m == 0) {
                let x0 = x((m as f64 - lead_draw as f64 / 60.0).max(0.0));
                cr.set_source_rgba(acc.red() as f64, acc.green() as f64, acc.blue() as f64, 0.35);
                cr.rectangle(x0, h - 50.0, xm - x0, 14.0); let _ = cr.fill();
            }
            cr.set_source_rgba(acc.red() as f64, acc.green() as f64, acc.blue() as f64, 1.0);
            cr.rectangle(xm - 2.0, h - 54.0, 4.0, 28.0); let _ = cr.fill();
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.8);
            cr.move_to(xm - 12.0, h - 8.0); let _ = cr.show_text(&format!(":{:02}", m % 60));
            m += every.min(60);
        }
        use chrono::Timelike; let now = chrono::Local::now(); let nm = now.minute() as f64 + now.second() as f64 / 60.0;
        cr.set_source_rgb(0.9, 0.2, 0.25); cr.set_line_width(1.5); cr.move_to(x(nm), 6.0); cr.line_to(x(nm), h - 26.0); let _ = cr.stroke();
        cr.move_to((x(nm) + 4.0).min(w - 40.0), 16.0); let _ = cr.show_text("now");
    });
    tg.add(&gtk::Frame::builder().child(&tl).margin_top(4).build());
    let cd = gtk::Label::builder().css_classes(["title-1"]).margin_top(10).build();
    let cd2 = gtk::Label::builder().css_classes(["dim-label"]).build();
    tg.add(&cd); tg.add(&cd2); pg.add(&tg);
    {
        let (name, sc, s) = (name.clone(), sc.clone(), s.clone());
        let tick = move || {
            let next = crate::sched::next_run(&name, &sc, &s);
            match next {
                Some(t) => {
                    let left = (t - chrono::Local::now().naive_local()).num_seconds().max(0);
                    cd.set_text(&format!("{}  ·  in {:02}:{:02}:{:02}", t.format("%H:%M"), left / 3600, left % 3600 / 60, left % 60));
                    let l = if anticipate { lead } else { 0 };
                    cd2.set_text(&if l > 0 { format!("generation starts {} (lead {})", (t - chrono::Duration::seconds(l)).format("%H:%M:%S"), fmt_secs(l)) } else { "generation starts at the mark".into() });
                }
                None => { cd.set_text("—"); cd2.set_text("schedule off"); }
            }
            tl.queue_draw();
        };
        tick();
        glib::timeout_add_seconds_local(1, move || { tick(); glib::ControlFlow::Continue });
    }

    // anticipate
    let ag = group("Timing", "");
    let d = crate::sched::durations(&name);
    let avg = if d.is_empty() { None } else { Some(d.iter().sum::<u64>() as i64 / d.len() as i64) };
    let an = adw::SwitchRow::builder().title("Anticipate generation delay")
        .subtitle(format!("measured avg {} over {} runs · lead {} (p75 + 30 s, max slot − 1 min)", avg.map(fmt_secs).unwrap_or("—".into()), d.len(), fmt_secs(lead)))
        .active(anticipate).build();
    { let set = set.clone(); an.connect_active_notify(move |r| set("anticipate", json!(r.is_active()))); }
    ag.add(&an);
    // prompt picker
    let names = core::prompt_names(); let titles: Vec<String> = names.iter().map(|n| crate::menu::title(n)).collect();
    let pd = dropdown(&titles, &crate::menu::title(&name));
    { let u = u.clone(); let names = names.clone(); let sc = sc.clone();
      pd.connect_selected_notify(move |d| { let n = &names[d.selected() as usize]; save_sched(n, sc.clone()); notify(&u, &format!("Schedule runs {}", crate::menu::title(n))); rebuild(&u); }); }
    let pr = adw::ActionRow::builder().title("Prompt").build(); pr.add_suffix(&pd); ag.add(&pr);
    // window
    let ev = core::sun_events(chrono::Local::now().date_naive(), &s);
    let wl: Vec<String> = vec![format!("dawn {} → civil dusk {}", ev["civil_dawn"].as_str().unwrap_or("?"), ev["civil_dusk"].as_str().unwrap_or("?")), "always on".into()];
    let wd = dropdown(&wl, if sc["window"].as_str() == Some("always") { &wl[1] } else { &wl[0] });
    { let set = set.clone(); wd.connect_selected_notify(move |d| set("window", json!(if d.selected() == 1 { "always" } else { "dawn-dusk" }))); }
    let wr_ = adw::ActionRow::builder().title("Window").build(); wr_.add_suffix(&wd); ag.add(&wr_);
    pg.add(&ag);

    // last runs
    let rg = group("Last runs", &format!("status: {}{}", e["status"].as_str().unwrap_or("idle"), if crate::sched::running().is_some() { " · a generation is running" } else { "" }));
    let runs = e["runs"].as_array().cloned().unwrap_or_default();
    if runs.is_empty() { rg.add(&row("No scheduled runs yet", "")); }
    for r in runs.iter().rev().take(5) {
        rg.add(&row(&format!("{} {}", r["at"].as_str().unwrap_or("").replace('T', " "), r["mark"].as_str().map(|m| format!("· mark {m}")).unwrap_or_default()),
            &format!("{} · {}{}", if r["ok"].as_bool().unwrap_or(false) { "ok" } else { "failed" }, fmt_secs(r["secs"].as_i64().unwrap_or(0)),
                if r["late"].as_bool().unwrap_or(false) { " · late, applied on finish" } else { "" })));
    }
    pg.add(&rg);
}

// ---------------- Presets ----------------
fn themes() -> Vec<String> { core::shv(&["omarchy-theme-list"], 20).out.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect() }
fn build_presets(u: &U) {
    let pg = page(u, "presets", "Settings", "preferences-desktop-appearance-symbolic");
    let mut th = vec![KEEP.to_string()]; th.extend(themes());
    let held = core::settings()["hold"].as_str().map(|s| s.to_string());
    let ga = group("Automatic", "No fixed preset: the engine drives everything (default)");
    let ar = adw::ActionRow::builder().title(if held.is_none() { "Automatic  ✓ active" } else { "Automatic" }).subtitle("Circadian K/brightness, phases → cycle presets, weather rules, day type, Cycle theme generation").build();
    let ab = btn("Use"); ab.set_sensitive(held.is_some());
    { let u = u.clone(); ab.connect_clicked(move |_| { let u = u.clone(); bg(|| core::set_hold(None), move |_| rebuild(&u)); }); }
    ar.add_suffix(&ab); ga.add(&ar); pg.add(&ga);
    for n in core::list_names(&core::presets_dir(), "json") {
        let p = core::preset(&n).unwrap_or(json!({}));
        let g = group(&format!("{}{}", p["label"].as_str().unwrap_or(&n), if held.as_deref() == Some(&n) { "  ✓ held" } else { "" }), &format!("{}/{n}.json", core::presets_dir().display()));
        let hb = btn("Hold this"); hb.set_sensitive(held.as_deref() != Some(&n));
        { let u = u.clone(); let n = n.clone(); hb.connect_clicked(move |_| { let u = u.clone(); let n = n.clone(); bg(move || core::set_hold(Some(&n)), move |_| rebuild(&u)); }); }
        g.set_header_suffix(Some(&hb));
        let sr = row("Summary", &summary(&p)); sr.add_css_class("property"); g.add(&sr);
        let thd = dropdown(&th, p["theme"].as_str().unwrap_or(KEEP)); let r = adw::ActionRow::builder().title("Theme").build(); r.add_suffix(&thd); g.add(&r);
        let wr_ = adw::ActionRow::builder().title("Wallpaper path (empty = keep)").build();
        let wp = gtk::Entry::builder().text(p["wallpaper"].as_str().unwrap_or("")).hexpand(true).valign(gtk::Align::Center).build(); wr_.add_suffix(&wp); g.add(&wr_);
        let bk = adw::SwitchRow::builder().title("Set brightness").active(!p["brightness"].is_null()).build(); g.add(&bk);
        let br = adw::SpinRow::with_range(1.0, 100.0, 5.0); br.set_title("Brightness %"); br.set_value(p["brightness"].as_f64().unwrap_or(70.0)); g.add(&br);
        let ns = adw::SwitchRow::builder().title("Night Shift").active(p["night_shift"].as_bool().unwrap_or(false)).build(); g.add(&ns);
        let wm = adw::SpinRow::with_range(2500.0, 6500.0, 100.0); wm.set_title("Warmth (K)"); wm.set_value(p["warmth"].as_f64().unwrap_or(4000.0)); g.add(&wm);
        let dk_opts = vec![KEEP.to_string(), "Dark".into(), "Light".into()];
        let dk = dropdown(&dk_opts, match p["dark"].as_bool() { None => KEEP, Some(true) => "Dark", Some(false) => "Light" });
        let r = adw::ActionRow::builder().title("Appearance").subtitle("used only when Theme = keep").build(); r.add_suffix(&dk); g.add(&r);
        let bx = gtk::Box::builder().spacing(6).halign(gtk::Align::End).margin_top(6).build();
        let tr = gtk::Button::with_label("Try"); let sv = gtk::Button::with_label("Save"); sv.add_css_class("suggested-action");
        { let u = u.clone(); let n = n.clone(); tr.connect_clicked(move |_| { let n = n.clone(); let u2 = u.clone(); let msg = format!("Applied {n}");
            bg(move || { if let Some(p) = core::preset(&n) { core::apply_preset(&p); } }, move |_| notify(&u2, &msg)); }); }
        { let u = u.clone(); let n = n.clone(); let th = th.clone();
          sv.connect_clicked(move |_| {
            let mut p = core::preset(&n).unwrap_or(json!({}));
            let t = &th[thd.selected() as usize]; p["theme"] = if t == KEEP { Value::Null } else { json!(t) };
            let w = wp.text().trim().to_string(); p["wallpaper"] = if w.is_empty() { Value::Null } else { json!(w) };
            p["brightness"] = if bk.is_active() { json!(br.value() as i64) } else { Value::Null };
            p["night_shift"] = json!(ns.is_active()); p["warmth"] = json!(wm.value() as i64);
            p["dark"] = [Value::Null, json!(true), json!(false)][dk.selected() as usize].clone();
            core::save_preset(&n, &p); notify(&u, &format!("Saved {n}")); rebuild(&u);
          }); }
        bx.append(&tr); bx.append(&sv); g.add(&bx); pg.add(&g);
    }
    let g = group("New preset", "");
    let u2 = u.clone();
    let (r, _) = erow("name (e.g. work-night)", "", move |t| { let t = t.trim().to_string(); if t.is_empty() { return; }
        let mut p = core::default_preset(); p["label"] = json!(t); core::save_preset(&t, &p); rebuild(&u2); });
    g.add(&r); pg.add(&g);
}

// ---------------- Cycles ----------------
fn build_cycles(u: &U) {
    let Some(pg) = u.stack.child_by_name("presets").and_then(|w| w.downcast::<adw::PreferencesPage>().ok()) else { return };
    let pre = core::list_names(&core::presets_dir(), "json");
    for n in core::list_names(&core::cycles_dir(), "json") {
        let c = core::cycle(&n).unwrap_or(json!({"phases": {}}));
        let g = group(&format!("Cycle · {}", c["label"].as_str().unwrap_or(&n)), &format!("Phase → preset · {}/{n}.json", core::cycles_dir().display()));
        for ph in core::PHASES {
            let d = dropdown(&pre, c["phases"][ph].as_str().unwrap_or("")); let mut t = ph.to_string(); t[..1].make_ascii_uppercase();
            let r = adw::ActionRow::builder().title(t).build(); r.add_suffix(&d); g.add(&r);
            let (u, n, pre) = (u.clone(), n.clone(), pre.clone());
            d.connect_selected_notify(move |d| { let mut c = core::cycle(&n).unwrap_or(json!({"phases": {}})); let v = &pre[d.selected() as usize];
                c["phases"][ph] = json!(v); core::save_cycle(&n, &c); notify(&u, &format!("{n}: {ph} → {v}")); });
        }
        pg.add(&g);
    }
    let g = group("New cycle", ""); let u2 = u.clone();
    let (r, _) = erow("name (e.g. work)", "", move |t| { let t = t.trim().to_string(); if t.is_empty() { return; } core::save_cycle(&t, &core::default_cycle(&t)); rebuild(&u2); });
    g.add(&r); pg.add(&g);
}

// ---------------- Rules ----------------
fn upd(u: &U, f: impl FnOnce(&mut Value)) { core::update_settings(f); notify(u, "Saved"); }
fn build_rules(u: &U) {
    let Some(pg) = u.stack.child_by_name("presets").and_then(|w| w.downcast::<adw::PreferencesPage>().ok()) else { return };
    let s = core::settings(); let cyc = core::list_names(&core::cycles_dir(), "json");
    let ev = core::sun_events(chrono::Local::now().date_naive(), &s);
    let g = group("Phase boundaries", &format!("Computed daily from the sun over {}. Today: sunrise {} · solar noon {} · sunset {} · civil dusk {} (nautical {}, astronomical {}). Values: sunrise, solar_noon, sunset, civil_dusk, civil_dawn, nautical_dusk, astronomical_dusk.",
        s["location"]["name"].as_str().unwrap_or("Dubai"), ev["sunrise"].as_str().unwrap_or("?"), ev["solar_noon"].as_str().unwrap_or("?"), ev["sunset"].as_str().unwrap_or("?"), ev["civil_dusk"].as_str().unwrap_or("?"), ev["nautical_dusk"].as_str().unwrap_or("?"), ev["astronomical_dusk"].as_str().unwrap_or("?"))); pg.add(&g);
    let defs = [("morning", "sunrise"), ("afternoon", "solar_noon"), ("evening", "sunset"), ("night", "civil_dusk")];
    for (k, lab) in [("morning", "Morning starts at"), ("afternoon", "Afternoon starts at"), ("evening", "Evening starts at"), ("night", "Night starts at")] {
        let def = defs.iter().find(|d| d.0 == k).unwrap().1; let ev2 = ev.clone();
        let u = u.clone();
        let (r, _) = erow(lab, s["times"][k].as_str().filter(|v| ev2[*v].is_string()).unwrap_or(def), move |v| { let v = v.trim().to_string();
            if !ev2[&v].is_string() { notify(&u, "Use a sun event: sunrise, solar_noon, sunset, civil_dusk…"); return; }
            upd(&u, |s| s["times"][k] = json!(v)); });
        g.add(&r);
    }
    let g = group("Day type", "UAE weekend: Saturday–Sunday"); pg.add(&g);
    for k in ["weekday", "weekend"] {
        let d = dropdown(&cyc, s["cycles"][k].as_str().unwrap_or("")); let mut t = format!("{k} cycle"); t[..1].make_ascii_uppercase();
        let r = adw::ActionRow::builder().title(t).build(); r.add_suffix(&d); g.add(&r);
        let (u, cyc) = (u.clone(), cyc.clone()); d.connect_selected_notify(move |d| { let v = cyc[d.selected() as usize].clone(); upd(&u, |s| s["cycles"][k] = json!(v)); });
    }
    let g = group("Location and Wi-Fi", "A matching Wi-Fi network overrides the day-type cycle. GPS: not yet (stub)."); pg.add(&g);
    if let Some(m) = s["wifi"].as_object() { for (ssid, c) in m {
        let r = row(ssid, &format!("→ {}", c.as_str().unwrap_or(""))); let x = icon_btn("user-trash-symbolic");
        let (u, ssid) = (u.clone(), ssid.clone()); x.connect_clicked(move |_| { upd(&u, |s| { if let Some(o) = s["wifi"].as_object_mut() { o.remove(&ssid); } }); rebuild(&u); });
        r.add_suffix(&x); g.add(&r);
    } }
    let cur = core::wifi_ssid();
    let add = adw::ActionRow::builder().title(format!("Map current Wi-Fi ({})", cur.clone().unwrap_or("none".into()))).build();
    let d = dropdown(&cyc, "home"); add.add_suffix(&d); let bt = btn("Add"); bt.set_sensitive(cur.is_some()); add.add_suffix(&bt); g.add(&add);
    { let (u, cyc) = (u.clone(), cyc.clone()); bt.connect_clicked(move |_| { if let Some(id) = cur.clone() { let v = cyc[d.selected() as usize].clone();
        upd(&u, |s| { if !s["wifi"].is_object() { s["wifi"] = json!({}); } s["wifi"][&id] = json!(v); }); rebuild(&u); } }); }
    let g = group("Weather (Open-Meteo, no key)", "First matching rule wins. Fields: temperature, rain, dust, pm10, wind, code"); pg.add(&g);
    let sw = adw::SwitchRow::builder().title("Use weather rules").active(s["weather"]["enabled"].as_bool().unwrap_or(true)).build(); g.add(&sw);
    { let u = u.clone(); sw.connect_active_notify(move |r| { let a = r.is_active(); upd(&u, |s| s["weather"]["enabled"] = json!(a)); }); }
    let pre = core::list_names(&core::presets_dir(), "json");
    for (i, rl) in s["weather"]["rules"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
        let h = row(rl["name"].as_str().unwrap_or(""), &format!("if {} → {}{}", rl["when"].as_str().unwrap_or(""), rl["preset"].as_str().unwrap_or(""), if rl["generate"].as_bool().unwrap_or(false) { " + generated wallpaper" } else { "" }));
        h.add_css_class("property"); g.add(&h);
        let u1 = u.clone(); let (r, _) = erow("Condition", rl["when"].as_str().unwrap_or(""), move |v| upd(&u1, |s| s["weather"]["rules"][i]["when"] = json!(v))); g.add(&r);
        let d = dropdown(&pre, rl["preset"].as_str().unwrap_or("")); let r = adw::ActionRow::builder().title("Preset").build(); r.add_suffix(&d); g.add(&r);
        { let (u, pre) = (u.clone(), pre.clone()); d.connect_selected_notify(move |d| { let v = pre[d.selected() as usize].clone(); upd(&u, |s| s["weather"]["rules"][i]["preset"] = json!(v)); }); }
        let gs = adw::SwitchRow::builder().title("Generate wallpaper with agent").active(rl["generate"].as_bool().unwrap_or(false)).build(); g.add(&gs);
        { let u = u.clone(); gs.connect_active_notify(move |r| { let a = r.is_active(); upd(&u, |s| s["weather"]["rules"][i]["generate"] = json!(a)); }); }
    }
    let g = group("Generation", "Hourly Cycle Generate 06:00–18:00, paused at night. Template variables: {{location}} {{temperature}} {{condition}} {{phase}}; command placeholders: {prompt_q} {out}. The template is rewritten by every Cycle Generate."); pg.add(&g);
    let u1 = u.clone(); let (r, _) = erow("Prompt template", s["generator"]["template"].as_str().unwrap_or(""), move |v| upd(&u1, |s| s["generator"]["template"] = json!(v))); g.add(&r);
    let u1 = u.clone(); let (r, _) = erow("Generator command", s["generator"]["command"].as_str().unwrap_or(""), move |v| upd(&u1, |s| s["generator"]["command"] = json!(v))); g.add(&r);
    let to = adw::SpinRow::with_range(60.0, 3600.0, 60.0); to.set_title("Generator timeout (s)"); to.set_value(s["generator"]["timeout"].as_f64().unwrap_or(900.0)); g.add(&to);
    { let u = u.clone(); to.connect_value_notify(move |r| { let v = r.value() as i64; core::update_settings(|s| s["generator"]["timeout"] = json!(v)); let _ = &u; }); }
    let tb = btn("Test generation now"); let r = adw::ActionRow::builder().title("Uses current weather, doesn't apply").build(); r.add_suffix(&tb); g.add(&r);
    { let u = u.clone(); tb.connect_clicked(move |_| { notify(&u, "Generating… (can take minutes)"); spawn_self(&u, &["test-gen"], "Generation finished – see ~/.local/share/cycle/generated"); }); }
}

// ---------------- app ----------------
fn apply_css() {
    // real Omarchy themed CSS (current/theme/cycle.css rendered from themed/cycle.css.tpl)
    let p = gtk::CssProvider::new(); p.load_from_string(&(theme::css()));
    #[allow(deprecated)]
    gtk::style_context_add_provider_for_display(&gtk::gdk::Display::default().unwrap(), &p, gtk::STYLE_PROVIDER_PRIORITY_USER);
}
/// The configured icon theme (Yaru-grey) is not installed → GTK icon lookup crashes libadwaita widgets; use Adwaita then.
fn fix_icon_theme() {
    let Some(st) = gtk::Settings::default() else { return };
    let name = st.gtk_icon_theme_name().map(|s| s.to_string()).unwrap_or_default();
    let h = core::home();
    let found = [h.join(".local/share/icons"), h.join(".icons"), "/usr/share/icons".into()].iter().any(|d| d.join(&name).join("index.theme").is_file());
    if !found { st.set_gtk_icon_theme_name(Some("Adwaita")); }
}
pub fn run() {
    glib::set_prgname(Some("cycle")); glib::set_application_name("Cycle");
    let app = adw::Application::builder().application_id("dev.emiliano.Cycle").build();
    app.connect_startup(|_| fix_icon_theme());
    app.connect_activate(|app| {
        if let Some(w) = app.active_window() { if let Some(u) = UI.with(|x| x.borrow().clone()) { build(&u); open_requested(&u); } w.present(); return; }
        apply_css();
        let dark = theme::colors().get("mode").map(|m| m != "light").unwrap_or(true);
        adw::StyleManager::default().set_color_scheme(if dark { adw::ColorScheme::ForceDark } else { adw::ColorScheme::ForceLight });
        let win = adw::ApplicationWindow::builder().application(app).title("Cycle").default_width(620).default_height(660).build();
        let stack = adw::ViewStack::new();
        let sw = adw::InlineViewSwitcher::builder().stack(&stack).display_mode(adw::InlineViewSwitcherDisplayMode::Labels).build();
        let hb = adw::HeaderBar::new(); hb.set_title_widget(Some(&sw));
        let tv = adw::ToolbarView::new(); tv.add_top_bar(&hb);
        let toast = adw::ToastOverlay::new(); toast.set_child(Some(&stack)); tv.set_content(Some(&toast)); win.set_content(Some(&tv));
        let u: U = Rc::new(Ui { stack, toast, graph: RefCell::new(None), cdata: Rc::new(RefCell::new(None)), live: RefCell::new(None), next: RefCell::new(None), resgroup: RefCell::new(None) });
        build(&u);
        if let Ok(t) = std::env::var("CYCLE_TAB") { u.stack.set_visible_child_name(&t); }
        open_requested(&u); UI.with(|x| *x.borrow_mut() = Some(u.clone()));
        { let u = u.clone(); glib::timeout_add_seconds_local(60, move || { refresh_curve(&u); glib::ControlFlow::Continue }); }
        win.present();
    });
    app.run_with_args::<&str>(&[]);
}
