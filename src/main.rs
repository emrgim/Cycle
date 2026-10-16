//! Cycle — daily display/theme cycle for Omarchy (Rust). Emiliano Morgia.
mod bright;
mod core;
mod lang;
mod live;
mod menu;
mod sched;
mod theme;
mod ui;

fn main() {
    core::seed();
    let a: Vec<String> = std::env::args().skip(1).collect();
    let cmd = a.first().map(|s| s.as_str()).unwrap_or("app");
    let has = |f: &str| a.iter().any(|x| x == f);
    match cmd {
        "app" => ui::run(),
        "tick" => { core::tick(has("--force")); }
        "apply-now" => { core::tick(true); }
        "resolve" => println!("{}", serde_json::to_string_pretty(&core::resolve(true)).unwrap()),
        "state" => println!("{}", serde_json::to_string_pretty(&core::current_state()).unwrap()),
        "apply-preset" => match a.get(1).and_then(|n| core::preset(n)) {
            Some(p) => { core::apply_preset(&p); }
            None => { eprintln!("unknown preset"); std::process::exit(2) }
        },
        "engine" => { let on = a.get(1).map(|s| s == "on").unwrap_or(false); core::set_engine(on); println!("engine {}", if on { "on" } else { "off" }); }
        "engine-is-on" => std::process::exit(if core::settings()["enabled"].as_bool().unwrap_or(false) { 0 } else { 1 }),
        "hold" => { core::set_hold(a.get(1).map(|s| s.as_str())); println!("hold: {}", core::settings()["hold"].as_str().unwrap_or("Automatic")); }
        "theme-gen" => { core::generate_cycle_theme(None, if has("--apply") { Some(true) } else { None }, has("--force")); }
        "test-gen" => {
            let s = core::settings(); let ctx = core::theme_context(&core::resolve(true));
            println!("{:?}", core::generate("test", &ctx, &s));
            let _ = s;
        }
        "generate" => { let pr = a.iter().position(|x| x == "--prompt").and_then(|i| a.get(i + 1)).map(|x| x.as_str()); let at = a.iter().position(|x| x == "--apply-at").and_then(|i| a.get(i + 1)).and_then(|t| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S").ok());
            std::process::exit(live::generate_at(has("--scheduled"), pr, at)) }
        "prompts" => match a.get(1).map(|x| x.as_str()) {
            Some("sync-menu") => { menu::sync(); println!("menu synced"); }
            _ => for id in core::prompt_names() {
                if has("--titles") { println!("{}", menu::title(&id)); } else { println!("{id}\t{}{}", menu::title(&id), if id == core::active_prompt() { "\t(active)" } else { "" }); }
            },
        },
        "open" => { if let Some(p) = a.iter().position(|x| x == "--prompt").and_then(|i| a.get(i + 1)) { let _ = std::fs::write(core::state_dir().join("open-prompt"), p); } ui::run() }
        "expand" => { let s = core::settings(); let t = a[1..].join(" "); for e in lang::errors(&t, s["images"].as_array().map(|x| x.len()).unwrap_or(0)) { eprintln!("error: {e}"); } println!("{}", live::expand_placeholders(&t, &s)) }
        "reset-profile" => crate::bright::reset_profile(a.get(1).map(|x| x.as_str())),
        "profile" => println!("{}", std::fs::read_to_string(crate::bright::profile_path()).unwrap_or("{}".into())),
        "brightness-step" => { let g = |f: &str| a.iter().position(|x| x == f).and_then(|i| a.get(i + 1)).cloned();
            let at = g("--at").and_then(|t| chrono::NaiveDateTime::parse_from_str(&t, "%Y-%m-%dT%H:%M:%S").ok()).unwrap_or_else(crate::bright::now);
            println!("{}", crate::bright::step(&g("--slot").unwrap_or("morning".into()), g("--base").and_then(|x| x.parse().ok()).unwrap_or(70), at)); }
        "schedules" => println!("{}", serde_json::to_string_pretty(&sched::status_all()).unwrap()),
        "live" => println!("{}", serde_json::to_string_pretty(&live::fetch(&core::settings())).unwrap()),
        "prompt" => println!("{}", live::build_prompt(&core::settings(), &live::fetch(&core::settings()), &live::headline(&core::settings()).0)),
        _ => {
            eprintln!("usage: cycle [app|generate [--scheduled]|tick [--force]|apply-now|resolve|state|apply-preset NAME|hold NAME|automatic|engine on|off|engine-is-on|theme-gen [--force] [--apply]|test-gen|live|prompt|generate --prompt NAME|schedules|prompts [list [--titles]|sync-menu]|open --prompt ID|expand TEXT|reset-profile [SLOT]|profile|brightness-step --slot S --base N [--at ISO]]");
            std::process::exit(2)
        }
    }
}
