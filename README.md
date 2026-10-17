# Cycle

> **Your display, in step with the sun.** A time-of-day brightness, theme and wallpaper engine for Omarchy (Arch Linux + Hyprland).

The pictures below are the previous desktop window. Version 2 is the same workflow inside the Omarchy shell.

![Previous Cycle window, Now tab](screenshots/tab-now.png)

**Cycle** follows the day on **[Omarchy](https://omarchy.org)**. Brightness and colour temperature follow the real sun position, a generated wallpaper and matching colour theme reflect live conditions outside, and when you adjust something yourself, Cycle respects it and learns from it.

### Why it matters
Most auto-brightness or night-light tools either fight you (they re-apply their value a minute after you change it) or make you edit config files to change their defaults. Cycle keeps your manual change for the rest of the time slot, and if you keep correcting it the same way, it adopts your preference as the new default.

### Key features
- **Sun-based slots:** **morning, afternoon, evening, night**, with boundaries computed daily from sunrise, solar noon, sunset and civil dusk at your location. No fixed clock hours, and it follows the seasons.
- **Manual override kept until the next slot:** change brightness yourself and Cycle holds your value until the slot changes.
- **Every override logged:** time, slot, Cycle's base value and your value, in `~/.local/state/cycle/overrides.json`.
- **Learns your preference:** 3+ overrides in the same direction on different days, and the median becomes that slot's learned brightness (approached at most 5 points per day).
- **Live-data wallpapers + palette theme:** prompts you write are filled with live weather, air quality, sea, sun position and news headlines, sent to an image generator you configure, and applied as the wallpaper of a generated **Cycle** Omarchy theme whose colour palette is extracted from the image.
- **Weather rules:** presets for hot (≥ 40 °C), rain and sandstorm (dust ≥ 150) conditions, from Open-Meteo (no API key).
- **Bar, panel, and command line:** an icon-only bar widget, a shell window, and a scriptable command with JSON output.
- **No separate build:** the marketplace copy is the running plugin. A shell timer inside the plugin replaces the old user timer.

## Contents
- [Key features](#key-features)
- [Requirements](#requirements)
- [Add from the marketplace](#add-from-the-marketplace)
- [First-run quickstart](#first-run-quickstart)
- [Usage (command line)](#usage-command-line)
- [Screenshots](#screenshots)
- [Omarchy integration](#omarchy-integration)
- [How it works](#how-it-works)
- [Fresh copy: what to expect](#fresh-copy-what-to-expect)
- [Generator command](#generator-command)
- [Prompts](#prompts)
- [Day phases](#day-phases-no-fixed-clock-hours)
- [Brightness override and learning](#brightness-override-and-learning)
- [Where things live](#where-things-live)
- [Remove](#remove)
- [Troubleshooting](#troubleshooting)
- [Testing](#testing)
- [License](#license)

## Requirements
- **Omarchy** (Arch Linux + Hyprland). Cycle calls Omarchy's own tools: `omarchy-theme-set`, `omarchy-theme-bg-set`, `omarchy-theme-current`, `omarchy-notification-send`, and `hyprctl hyprsunset` for colour temperature.
- **brightnessctl** for reading and setting the backlight (`brightnessctl -m`, `brightnessctl set N%`).
- Optional: **NetworkManager** (`nmcli`) or `iwgetid` for Wi-Fi-based location rules.
- Optional: a logged-in image command such as the [Grok CLI](https://github.com/xai-org/grok-cli), or any other command that writes a PNG. See [Generator command](#generator-command).
- Optional: `~/.local/bin/omarchy-display`. Presets apply dark/light mode, preset brightness, warmth and night shift through it. It is not part of this plugin; without it those preset steps are skipped, while slot brightness (`brightnessctl`) and theme/wallpaper changes still run.
- The command line prefers `node` when it is present, and otherwise runs through `qml`. The bar widget, background service, and window do not need `node`.

## Add from the marketplace
```text
omarchy plugin add https://github.com/emrgim/Cycle.git --enable
```
Omarchy copies the plugin into `~/.config/omarchy/plugins/emrgim.cycle` and enables it. Automatic Cycle stays off until you turn it on. The bar icon is placed from the manifest (`center`). Open the window from the bar or with `cycle app`.

The command is also available as `~/.config/omarchy/plugins/emrgim.cycle/bin/cycle`. On first use Cycle links `~/.local/bin/cycle` to that file when the destination is missing.

## First-run quickstart
1. **Open the window** from the bar icon, or run `cycle app` (Super+Space → Cycle).
2. **Enable Automatic Cycle** on the **Now** tab (same as `cycle engine on`).
3. **Location** is detected once, the first time Cycle starts with nothing saved. Change it on **Settings**, or press **Detect location** to look it up again.
4. **Set the generator** on **Settings → Generation** if you want wallpapers.
5. **Create a prompt** on the **Prompt** tab, for example `A wide city skyline at /time, sky /sun, weather /weather`. Check the live preview, then press **Generate**.
6. **Schedule it** on the **Schedule** tab: pick 15, 30, 60, 120 or 180 minutes and a window (dawn–dusk or always).

## Usage (command line)
```text
cycle app                         open the window (tabs: Now, Data, Prompt, Schedule, Settings)
cycle tick [--force]              one engine step (what the timer runs every minute)
cycle apply-now                   apply the current slot immediately
cycle resolve | state             show the resolved slot/preset | current display state (JSON)
cycle apply-preset NAME           apply a preset once
cycle hold NAME | hold automatic  pin a preset | return to automatic
cycle engine on|off | engine-is-on
cycle generate [--prompt ID]      live data → prompt → image → Cycle theme → archive
cycle schedules                   last/next run and status of each prompt schedule
cycle prompts [list [--titles]|sync-menu]
cycle open --prompt ID            open the window on a prompt
cycle live | prompt | expand TEXT print live data | the prompt | a resolved text
cycle theme-gen [--force] [--apply] | test-gen
cycle profile | reset-profile [SLOT]
cycle brightness-step --slot S --base N [--at ISO]
```
Unknown commands exit 2. `cycle engine-is-on` exits 0 when Automatic Cycle is on and 1 when it is off.

## Screenshots
These captures are the previous desktop window, taken on SuperOmi. The shell window has the same five tabs.

### Now
Circadian curve for today (colour temperature and brightness from the sun's elevation), the current slot and preset, and the next update.
![Previous window, Now tab](screenshots/tab-now.png)

### Data
Live data used by Cycle (Open-Meteo weather, air quality, marine, sun position).
![Previous window, Data tab](screenshots/tab-data.png)

### Prompt
Prompt cards for the generated wallpaper, with the inline composition language.
![Previous window, Prompt tab](screenshots/tab-prompt.png)

### Schedule
Clock-aligned generation slots, the next run and timing.
![Previous window, Schedule tab](screenshots/tab-schedule.png)

### Settings
Automatic mode and fixed presets (theme, wallpaper, brightness, warmth) that you can hold.
![Previous window, Settings tab](screenshots/tab-settings.png)

## Omarchy integration
- The bar widget `emrgim.cycle` sits in the **center** of the Omarchy bar. It uses the same `BarIconButton` style as the other bar icons.
  - **Left click** opens the Cycle window.
  - **Right click** applies the current slot immediately.
- A kept-loaded service runs the same minute timer, so scheduled prompts still fire when the window is closed.
- `cycle tick`, `cycle state`, `cycle profile`, and `cycle apply-now` print JSON or a short status line, so a local agent can read state or trigger a step.
- Changing prompts rewrites the Cycle rows in `~/.config/omarchy/extensions/omarchy-menu.jsonc` between the generated markers.

## How it works
- After enable, the bar widget (or the kept-loaded service) starts a timer aligned to the minute. Each minute runs the same step as `cycle tick`.
- That step resolves the current slot from today's sun events and loads that slot's preset.
- Brightness compares the real backlight with the value Cycle last set:
  - a gap of more than 2 points is recorded as an override and held until the slot changes;
  - otherwise Cycle applies the base value, which may be a learned one.
- State files live in `~/.local/state/cycle/`: `brightness.json`, `overrides.json`, `profile.json`, `schedules.json`, `weather.json`, `last.json`, `theme.json`, `generate.lock`, and `open-prompt`. The saved place is `location` in `settings.json` (`auto` from the one-time setup, `manual` when you edit it, `unknown` when setup could not detect one).

## Fresh copy: what to expect
Enabling the plugin **generates nothing until you configure it**.

- **Automatic Cycle is off.** Turn it on in the **Now** tab or with `cycle engine on`. The minute timer keeps running either way so a prompt schedule can still fire; the display engine itself does nothing until it is on, unless you pass `--force` or press Apply now.
- **The styles folder is empty.** `~/.config/omarchy/cycle/styles` has no prompt files until you create one.
- **Location is detected once** on that first start, from a privacy-respecting HTTPS IP geolocation lookup (city, coordinates, timezone), then stored. A location you save yourself is never replaced on restart, when the network changes, or when Cycle is updated. If the lookup fails and nothing was saved, Settings says **location unknown, set it in Settings**.
- **Generation window** defaults to **06:00–18:00**.
- **Default RSS feed:** Google News in English until a city is detected; the first successful detection points the feed at that city. **Open-Meteo** needs no API key.
- **Without a working generator command**, generation fails and the **current wallpaper is kept**.

Brightness slots, sun-based phases, Open-Meteo data and the override/learning loop still work without image generation.

## Prompts
Prompts are text files `~/.config/omarchy/cycle/styles/<id>.txt` (titles in `settings.prompt_titles`, the active one in `styles/active`).
Each prompt has a title, id, editor, slash suggestions, error lines, a live preview, and Use / Generate / Duplicate / Delete. Saving one schedule turns the others off.

### Composition language
```text
prompt   := { text | call | legacy }
call     := "/" name { "/" arg }
name     := [a-z] [a-z0-9_]*
arg      := [A-Za-z0-9_+%-]+
legacy   := "{{" name "}}"
args     := { keyword }
keyword  := "city" value | "query" value | "today" | "tomorrow" | "yesterday"
```
A call must start the text or follow whitespace or one of `( [ , ; : " '`. Inside a city or query value, `_`, `+`, and `%20` become a space.

| Command | Args | Value |
|---|---|---|
| `/weather` | city, today/tomorrow/yesterday | live weather (current) or daily forecast for that day |
| `/air` | city, day | AQI, PM2.5, PM10, dust, AOD |
| `/sun` | city, day | phase, elevation, azimuth, shadow direction, sky colour |
| `/dawn` `/sunrise` `/solar_noon` `/sunset` `/dusk` | city, day | civil dawn (−6°), sunrise (−0.833°), transit, sunset, civil dusk (−6°) |
| `/location` | city | name and coordinates (Open-Meteo geocoding) |
| `/news` | city, query | LED billboard headline (enabled feeds; with args a Google News search) |
| `/breaking` | – | Google News top story, falls back to `/news` |
| `/phase` `/time` `/date` `/now` `/today` `/tomorrow` `/yesterday` | – | day phase / clock / dates |
| `/style` | – | text of the style preset `settings.prompt_style` (default gits-clean) |
| `/image1`… | – | reference image paths from `settings.images` |

Examples: `/weather/city/Genova/tomorrow`, `/sunrise/tomorrow`, `/sun/city/Tokyo`, `/air/city/Genova`, `/news/query/expo_city`.
With no saved location, `/location` and the sun commands say to set it in Settings. After that, network results are cached for the run. Unknown commands stay as `/call [!error]`. Rain is only described when live data reports precipitation.
Headlines never repeat: shown ones are recorded in `~/.config/omarchy/cycle/headlines-used.txt` (last 300) and skipped.
`cycle expand 'text'` prints a resolved text and writes errors to stderr.

`{{location}}`, `{{temperature}}`, `{{condition}}`, and `{{phase}}` are filled before the slash language.

### Schedules
`settings.schedules.<id> = {"enabled": true, "every_min": 60, "window": "dawn-dusk" | "always", "anticipate": false}`.
Slots are aligned to the clock: 15 → :00 :15 :30 :45, 30 → :00 :30, 60 → hourly at :00, 120 → :00 of even hours, 180 → :00 of hours divisible by 3 (other values fall back to hourly). The next run is the next aligned mark inside the window (dawn to civil dusk by default). Manual runs do not move `last_slot`.
If a generation is still running when its mark has not yet passed, Cycle waits. If the mark has passed and the previous run is still going, that mark is skipped and the status records it.
The lock file `~/.local/state/cycle/generate.lock` holds the process id. A lock whose process is gone is treated as stale.
Each run re-resolves every variable, generates (timeout at least 1200 s), applies the Cycle theme and archives to `~/Pictures/desktop/YYYY-MM-DD/HHMM.png`. A run that must not apply (`CYCLE_NO_APPLY`) archives as `HHMM-{slug}.png` instead. A failed run keeps the current wallpaper and sends a notification.
`cycle schedules` shows last/next run and status.

### Launcher menu
Prompt rows are written into `omarchy-menu.jsonc` by `cycle prompts sync-menu` (also after a prompt change in the window). Each prompt offers Generate and Edit. `cycle prompts list [--titles]`.

## Generator command
On **Settings**, the Generation section stores:

| Field | What it is |
|---|---|
| **Prompt template** | Text used when building the image prompt |
| **Generator command** | Command Cycle runs to produce the wallpaper PNG |
| **Timeout (s)** | Timeout for that command; the effective value is at least 1200 |

The same values live in `~/.config/omarchy/cycle/settings.json` under `generator`.

Cycle runs the command template with `bash -c`. Placeholders are replaced in one pass, and the prompt and output path are passed as environment values, never spliced into the command text:

| Placeholder | Meaning |
|---|---|
| `{prompt_q}` | `"$CYCLE_PROMPT"` |
| `{prompt}` | `"$CYCLE_PROMPT"` |
| `{out}` | `"$CYCLE_OUT"` |

Other braces, such as `{other}`, stay as written. Headline text is stripped of control characters and of `` ` ``, `$`, `\`, `{`, and `}` before it can become prompt text. The command must write a PNG at the output path. If it fails or times out, the current wallpaper stays and a notification is sent.

The default command string is:

```text
grok -p {prompt_q} --always-approve --output-format json
```

That names the [Grok CLI](https://github.com/xai-org/grok-cli) only as a default. Cycle does not fetch or sign in to it. Any other command that writes a PNG to the output path is a valid replacement.

## Day phases (no fixed clock hours)
Boundaries are recomputed every day from the NOAA sun model for the configured coordinates. The clock used for "now" is the machine's timezone.

| Phase | Starts at | Variable |
|---|---|---|
| morning | sunrise (sun at −0.833°) | `sunrise` |
| afternoon | solar noon (last minute of maximum elevation) | `solar_noon` |
| evening | sunset (−0.833°) | `sunset` |
| night | civil dusk (−6°) | `civil_dusk` |

Also available: `civil_dawn`, `nautical_dusk` (−12°), `astronomical_dusk` (−18°).
`settings.json` → `times` defaults to `{"morning":"sunrise","afternoon":"solar_noon","evening":"sunset","night":"civil_dusk"}`. A value may be one of those event names or a `HH:MM` clock.

Sample for 15 Oct 2026 at 25.2048°N, 55.2708°E, timezone offset +4: civil dawn 05:54 · **sunrise 06:17** · **solar noon 12:04** · **sunset 17:53** · **civil dusk 18:16** · nautical dusk 18:43 · astronomical dusk 19:09.

## Brightness override and learning
- **Override:** every tick compares the backlight with the value Cycle last set. A difference of more than 2 points is a manual change. Cycle keeps it until the next slot. Further changes in the same slot update the override.
- **Log:** each override is appended to `~/.local/state/cycle/overrides.json` (`at`, `slot`, `base`, `user`).
- **Learning:** 3 or more same-direction overrides on different days, using the last override of each day within the last 7 days, set the slot's learned base to their median. Cycle moves toward it by at most 5 points per day. An even count rounds halves away from zero for the median of positive values (standard half-up).
- Commands: `cycle profile`, `cycle reset-profile [SLOT]` (the override log is kept), `cycle brightness-step --slot S --base N [--at YYYY-MM-DDTHH:MM:SS]`. `CYCLE_STATE_DIR` and `CYCLE_BRIGHTNESS_FILE` redirect state and the brightness file.

Colour temperature is applied with `hyprctl hyprsunset`. At 6500 K or above Cycle asks hyprsunset for identity. If hyprsunset is not running, Cycle starts it directly.

## Where things live
| What | Path |
|---|---|
| Plugin tree | `~/.config/omarchy/plugins/emrgim.cycle` |
| Command | `bin/cycle` in that tree, and `~/.local/bin/cycle` when the link is created |
| Settings | `~/.config/omarchy/cycle/settings.json` |
| Presets and cycles | `presets/*.json`, `cycles/*.json` next to settings |
| Prompts | `styles/*.txt`, `styles/active` |
| Live snapshot | `~/.config/omarchy/cycle/weather-now.json` |
| State | `~/.local/state/cycle/` |
| Generated images | `~/.local/share/cycle/generated` |
| Generated theme | `~/.config/omarchy/themes/cycle` |
| Archive | `~/Pictures/desktop/YYYY-MM-DD/HHMM.png` |
| Launcher menu | `~/.config/omarchy/extensions/omarchy-menu.jsonc` |

Engine source is `js/engine.js`, shared by the shell and by `node js/cli.mjs`. The shell host is `EngineHost.qml`. The window is `Panel.qml`, the bar icon is `Widget.qml`, and the kept-loaded timer is `Service.qml`.

## Remove
```text
omarchy plugin remove emrgim.cycle
```
That drops the plugin tree. Settings, prompts, logs, and generated images stay until you remove them:

```text
rm -rf ~/.config/omarchy/cycle ~/.local/state/cycle ~/.local/share/cycle ~/.config/omarchy/themes/cycle
rm -f ~/.local/bin/cycle
```
Also remove leftover `cycle.*` rows from `omarchy-menu.jsonc` if they remain. Switch to another Omarchy theme first if the Cycle theme is active.

## Troubleshooting
- **Nothing changes during the day.** Automatic Cycle is off until you enable it. `cycle engine on`, then `cycle tick --force` to see one step.
- **Brightness does not change.** `brightnessctl -m` must work for your user. A manual change is held until the next slot.
- **Generation fails.** `cycle test-gen` or `cycle generate`. The generator must write a PNG to the output path. On failure the current wallpaper stays and a notification is sent.
- **No prompts or schedules.** The styles folder starts empty.
- **Wrong sunrise or sunset.** Set location on Settings. The sample above uses timezone offset +4.
- **Learning went the wrong way.** `cycle reset-profile [SLOT]` clears learned values. The override log stays.
- **Weather data.** Open-Meteo needs no key. Image generation uses only the command you configured.
- **Optional display helper.** Without `omarchy-display`, dark/light, preset brightness, warmth, and night shift are skipped. Theme, wallpaper, and `brightnessctl` still run.

## Testing
```text
node test/engine.test.mjs
node test/qml-plaintext.test.mjs
```
The engine checks cover the prompt language, the sun sample, generator placeholder isolation (including a headline that must not become shell syntax), brightness override logging, a fresh settings seed, first-run geolocation (including a failed lookup), and PNG palette extraction.

`node test/fetch-bounds.test.mjs` checks that fetches are HTTPS and that the only `XMLHttpRequest` goes through the capped helper.

The QML check fails if a `Text`, `Label`, `TextArea`, or `TextEdit` shows non-literal data without `textFormat: Text.PlainText`, or if a combo box, button, or delegate shows a non-literal model or label without a plain-text `Text`. SSIDs, location names, weather and other API text, headlines, and file names stay literal characters.

## License
Cycle is released under the [MIT License](LICENSE).
