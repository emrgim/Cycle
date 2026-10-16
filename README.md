# Cycle

**Cycle** is a native Rust (GTK4 / libadwaita) app that runs your display through the day on **Omarchy** (Arch Linux + Hyprland).
It was built for Omarchy and runs daily on *SuperOmi*, an Omarchy laptop.

## What it does
- **Time-of-day slots.** The day is split into four slots: **morning, afternoon, evening and night**. Their boundaries come from the real sun position for the configured location (sunrise, solar noon, sunset, civil dusk), recomputed every day, so there are no fixed clock hours. Each slot sets a screen brightness and colour temperature.
- **Your manual change wins.** If you change the brightness yourself, Cycle notices (a difference of more than 2 points from what it last set) and **keeps your value until the next slot** instead of overwriting it a minute later.
- **Every override is saved** to `~/.local/state/cycle/overrides.json` (time, slot, Cycle's base value, your value).
- **It learns.** When a slot gets **3 or more overrides in the same direction** on different days, Cycle takes the median of your values as that slot's learned brightness and moves towards it gradually (at most 5 points per day).
- **Generated theme and wallpaper.** Cycle also builds a "Cycle" Omarchy theme whose wallpaper is generated from live data (weather, air quality, sun position, a headline) using prompts you write in the app, on a schedule.

### Why it helps
- No fighting the automation: one manual adjustment holds for the whole slot.
- No config editing: repeated corrections become the new default by themselves.
- Slots follow the sun across the seasons.
- Everything runs in user space as one small binary plus a systemd user timer. No daemon and no sudo.

## Omarchy integration: agents and the bar
- The Omarchy bar on SuperOmi has an **agents icon** (the "My Agents" panel with usage and limits for the AI coding agents). Cycle sits next to it as a peer.
- Cycle has its own bar widget, `emiliano.cycle` (an Omarchy quickshell plugin, see `omarchy-plugin/`), placed in the **center of the bar**. It is an icon-only button drawn with the same `BarIconButton` style as the agents icon.
  - **Left click** opens the Cycle app (`cycle app`).
  - **Right click** triggers Cycle right away (`cycle apply-now`): it applies the current slot immediately.
- Because Cycle is a plain CLI (`cycle tick`, `cycle state`, `cycle profile`, `cycle apply-now`, …) with JSON output, agents running on the machine can read its state or trigger it from the shell.
- Cycle also adds entries to the Omarchy launcher menu (Super+Space → Cycle) and installs a desktop file.

### Install the bar widget
```sh
cp -r omarchy-plugin/emiliano.cycle ~/.config/omarchy/plugins/
# then add {"id": "emiliano.cycle"} to the "center" section of ~/.config/omarchy/shell.json and restart the shell
```

## How it works
- `cycle-engine.timer` (systemd user) runs `cycle tick` every minute.
- On each tick, `tick` resolves the current slot from today's sun events and loads that slot's preset (brightness and warmth).
- `bright.rs` then compares the real backlight with the value it last set:
  - if the gap is more than 2 points, it records an override and holds your value for the rest of the slot;
  - otherwise it applies the base value, which may be a learned one.
- State files live in `~/.local/state/cycle/`: `brightness.json` (engine state), `overrides.json` (log) and `profile.json` (learned values).


Daily display/theme cycle for Omarchy on SuperOmi: circadian colour temperature and brightness from the real
solar position over Dubai, presets, cycles and weather rules, and the generated **Cycle** Omarchy theme.
Its wallpaper is created hourly from live data: weather, air quality, sea, sun position and the top Dubai news headline on an LED billboard.

## Reference

### Where things live
| What | Path |
|---|---|
| Source (single crate) | `src/` — `main.rs` (CLI), `core.rs` (config, engine, NOAA sun, apply, theme generation), `live.rs` (Open-Meteo, RSS, prompt, `cycle generate`), `theme.rs` (palette from image, Omarchy CSS), `ui.rs` (GTK4/libadwaita window) |
| Binary | `~/.local/bin/cycle` |
| Settings | `~/.config/omarchy/cycle/settings.json` (+ `presets/`, `cycles/`, `styles/*.txt` prompt presets, `styles/active`, `weather-now.json`) |
| Systemd user units | `packaging/` → `~/.config/systemd/user/`: `cycle-engine.timer` (every minute), `cycle-generate-now.service` (manual run); scheduled prompts run as transient units started by `cycle tick` |
| Archive | `~/Pictures/desktop/YYYY-MM-DD/HHMM.png` |
| Window styling | real Omarchy theme: `~/.config/omarchy/themed/cycle.css.tpl` rendered with `~/.local/state/omarchy/current/theme/colors.toml` |
| Launcher menu | `~/.config/omarchy/extensions/omarchy-menu.jsonc` (`cycle.*`), `~/.local/share/applications/cycle.desktop` |
| Floating window rule | `~/.config/hypr/looknfeel.lua` (class `dev.emiliano.Cycle`) |

## Build / install / run
```sh
git clone https://github.com/emrgim/Cycle && cd Cycle
cargo build --release        # needs gtk4 ≥ 4.12 and libadwaita ≥ 1.7 (system), Rust via rustup
./install.sh                 # builds, installs ~/.local/bin/cycle, units and desktop file (no sudo)
cycle app                    # open the window (tabs: Now, Data, Prompt, Presets, Cycles, Rules)
cycle generate               # live data → prompt → image → apply Cycle theme → archive
```
Other subcommands: `tick [--force]`, `apply-now`, `resolve`, `state`, `apply-preset NAME`, `hold NAME|automatic`,
`engine on|off`, `engine-is-on`, `theme-gen [--force] [--apply]`, `test-gen`, `live` (print live data), `prompt` (print the prompt without generating).
`generate --prompt ID`, `schedules`, `prompts list|sync-menu`, `open --prompt ID`, `expand TEXT`.

## Prompts (Prompt tab)
Prompts are text files `~/.config/omarchy/cycle/styles/<id>.txt` (titles in `settings.prompt_titles`, the active one in `styles/active`).
Each prompt is a card in the Prompt tab with a title, id, full editor, slash autocomplete, inline errors (red underline),
a live preview with every variable resolved, Use / Generate / Duplicate / Delete, and a schedule.

### Composition language (`src/lang.rs`, unit-tested: `cargo test --release`)
```text
prompt   := { text | call | legacy }
call     := "/" name { "/" arg }        -- must start the text or follow whitespace or ( [ , ; : " '
name     := [a-z] [a-z0-9_]*
arg      := [A-Za-z0-9_+%-]+            -- "_" or "+" inside a city/query value = space
legacy   := "{{" name "}}"
args     := { keyword }   keyword := "city" value | "query" value | "today" | "tomorrow" | "yesterday"
```
| Command | Args | Value |
|---|---|---|
| `/weather` | city, today/tomorrow/yesterday | live weather (current) or daily forecast for that day |
| `/air` | city, day | AQI, PM2.5, PM10, dust, AOD |
| `/sun` | city, day | phase, elevation, azimuth, shadow direction, sky colour |
| `/dawn` `/sunrise` `/solar_noon` `/sunset` `/dusk` | city, day | civil dawn (−6°), sunrise (−0.833°), transit, sunset, civil dusk (−6°) |
| `/location` | city | name and coordinates (geocoded with the Open-Meteo geocoding API) |
| `/news` | city, query | LED billboard headline (enabled feeds; with args a Google News search) |
| `/breaking` | – | Google News top story, falls back to `/news` |
| `/phase` `/time` `/date` `/now` `/today` `/tomorrow` `/yesterday` | – | day phase / clock / dates |
| `/style` | – | text of the style preset `settings.prompt_style` (default gits-clean) |
| `/image1`… | – | reference image paths from `settings.images` |

Examples: `/weather/city/Genova/tomorrow`, `/sunrise/tomorrow`, `/sun/city/Tokyo`, `/air/city/Genova`, `/news/query/expo_city`.
Default location: Dubai. Network results are cached for the run. Unknown commands/invalid args are flagged in the editor and
left as `/call [!error]` if generated anyway. Rain is only described when live data reports precipitation.
Headlines never repeat: shown ones are recorded in `~/.config/omarchy/cycle/headlines-used.txt` and skipped.
`cycle expand 'text'` prints a resolved text (errors on stderr).

### Schedules
`settings.schedules.<id> = {"enabled": true, "every_min": 60, "window": "dawn-dusk" | "always"}` (editable on the card).
Slots are aligned to the clock, not intervals: `every_min` 15 → :00 :15 :30 :45, 30 → :00 :30, 60 → hourly at :00,
120 → :00 of even hours, 180 → :00 of hours divisible by 3 (other values fall back to hourly). Next run = next aligned mark inside
the window (dawn → civil dusk by default). Manual runs (menu / Generate button) never shift the slots. If a run is still going
when a mark arrives, that mark is skipped (status “skipped HH:MM”). Fired/skipped marks are recorded as `last_slot` in
`~/.local/state/cycle/schedules.json`.
`cycle-engine.timer` runs `cycle tick` every minute; tick starts due prompts as transient units `cycle-gen-HHMMSS`
(`cycle generate --prompt <id>`). A lock file (`~/.local/state/cycle/generate.lock`, pid) prevents overlapping runs.
Each run re-resolves every variable, generates (timeout ≥ 1200 s), applies the Cycle theme and archives to
`~/Pictures/desktop/YYYY-MM-DD/HHMM.png`; a failed or stale run applies/archives nothing and sends a notification.
`cycle schedules` shows last/next run and status. The old hourly cycle-weather/cycle-generate timers are gone.

### Launcher menu
`Cycle Prompts` (in the Super+Space Cycle menu) lists every prompt by title; each opens Generate / Edit
(`cycle generate --prompt <id>`, `cycle open --prompt <id>`). The rows are generated into
`omarchy-menu.jsonc` by `cycle prompts sync-menu` (run automatically when prompts change), because the Omarchy shell only
supports its built-in providers. `cycle prompts list [--titles]`.

The image command is `generator.command` in settings (default `grok -p {prompt_q} --always-approve --output-format json`) with
`generator.timeout` (default and minimum 1200 s). If generation fails or times out, the current wallpaper is kept.

## Day phases (no fixed clock hours)
Boundaries are recomputed every day from the NOAA sun model for the configured location (`core::sun_events`):

| Phase | Starts at | Variable |
|---|---|---|
| morning | sunrise (sun at −0.833°) | `sunrise` |
| afternoon | solar noon (meridian transit) | `solar_noon` |
| evening | sunset (−0.833°) | `sunset` |
| night | civil dusk, end of civil twilight (−6°) | `civil_dusk` |

Also available but not used as boundaries: `civil_dawn`, `nautical_dusk` (−12°), `astronomical_dusk` (−18°).
`settings.json` → `times` = `{"morning":"sunrise","afternoon":"solar_noon","evening":"sunset","night":"civil_dusk"}`.

Dubai, 15 Oct 2026 (GST): civil dawn 05:54 · **sunrise 06:17** · **solar noon 12:04** · **sunset 17:53** · **civil dusk 18:16** · nautical dusk 18:43 · astronomical dusk 19:09.

## Brightness override and learning (`src/bright.rs`)
- **Override**: every tick Cycle compares the screen brightness with the value it last set. A difference of more than 2 points is a manual change: Cycle keeps it and does not touch brightness again until the next slot (day phase: morning / afternoon / evening / night). Further manual changes in the same slot update the override.
- **Log**: every override is appended to `~/.local/state/cycle/overrides.json` (`at`, `slot`, `base`, `user`). Engine state is in `~/.local/state/cycle/brightness.json`.
- **Learning**: if a slot has 3 or more overrides in the same direction on different days (last override of each day, last 7 days with overrides), the median of those user values becomes the slot's learned base in `~/.local/state/cycle/profile.json`. Cycle moves the effective base towards it by at most 5 points per day (`current` → `target`).
- Commands: `cycle profile`, `cycle reset-profile [SLOT]` (the override log is kept), `cycle brightness-step --slot S --base N [--at YYYY-MM-DDTHH:MM:SS]` (one brightness step, for tests). `CYCLE_STATE_DIR` and `CYCLE_BRIGHTNESS_FILE` redirect state and brightness to test files.
