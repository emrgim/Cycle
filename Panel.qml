import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import "js/engine.js" as Cycle

Item {
  id: root
  property string noteText: ""
  property var curveData: null
  property var liveData: null
  property var settingsData: null
  property var promptList: []
  property string activeId: ""
  property string editingId: ""
  property var scheduleStatus: null
  property bool filling: false
  property int feedEdit: -1

  function ensureHost() {
    if (!Cycle.isArmed()) Cycle.arm(engineHost.api)
  }
  function note(msg) { root.noteText = String(msg) }
  function tabIndex(name) {
    if (name === "data") return 1
    if (name === "prompt") return 2
    if (name === "schedule") return 3
    if (name === "presets" || name === "settings") return 4
    return 0
  }
  function open(payloadJson) {
    ensureHost()
    var tab = ""
    try {
      if (payloadJson) {
        var parsed = JSON.parse(String(payloadJson))
        if (parsed && parsed.tab) tab = String(parsed.tab)
      }
    } catch (e) {}
    var fromEnv = Quickshell.env("CYCLE_TAB")
    if (!tab && fromEnv) tab = fromEnv
    window.visible = true
    if (tab) tabs.currentIndex = tabIndex(tab)
    reload()
    Cycle.takeOpenPrompt().then(function(id) {
      if (!id) return
      tabs.currentIndex = 2
      root.editingId = id
      reloadPrompts()
    })
  }
  function close() { window.visible = false }

  function reload() {
    reloadNow()
    reloadData()
    reloadPrompts()
    reloadSchedule()
    reloadSettings()
  }
  function reloadNow() {
    Cycle.curve(30).then(function(d) {
      root.curveData = d
      graph.requestPaint()
      var hold = d.hold ? ("HOLD '" + d.hold + "'") : (d.target_k + " K \u00b7 brightness " + d.target_b + "% \u00b7 sun " + d.elev + "\u00b0")
      nowLine.text = hold + " \u00b7 " + (d.phase || "") + " \u2192 " + (d.preset || "\u2014")
      var sun = d.sun || {}
      nextLine.text = "sunrise " + (sun.sunrise || "") + ", sunset " + (sun.sunset || "") + " \u00b7 engine: " + (d.enabled ? ("next check " + d.next_tick) : "off (manual only)")
      var res = d.resolve || {}
      var w = res.weather || {}
      var weather = w.error ? String(w.error) : (w.temperature + "\u00b0C, " + (w.condition || "") + ", dust " + w.dust)
      var loc = res.location || {}
      resolveText.text = "Phase: " + (res.phase || "") + "\nDay type: " + (res.day_type || "") + "\nWi-Fi: " + (res.ssid || "\u2014") + "\nLocation: " + (loc.name || "") + " " + (loc.note || "") + "\nWeather: " + weather + "\nCycle: " + (res.cycle || "") + "\nPreset: " + (res.preset || "\u2014") + "\nWhy: " + (res.source || "")
      Cycle.listNames(Cycle.presetsDir(), "json").then(function(names) {
        var opts = ["Automatic"].concat(names)
        root.filling = true
        holdBox.model = opts
        var cur = d.hold || "Automatic"
        var idx = opts.indexOf(cur)
        holdBox.currentIndex = idx < 0 ? 0 : idx
        root.filling = false
      })
      engineSwitch.checked = !!d.enabled
    })
  }
  function reloadData() {
    Cycle.storedLive().then(function(d) { root.liveData = d; paintLive(d) })
  }
  function paintLive(d) {
    d = d || {}
    var w = d.weather || {}
    var a = d.air || {}
    var m = d.marine || {}
    function v(x, unit) { return (x === null || x === undefined || x === "") ? "\u2014" : (x + unit) }
    liveText.text = "Fetched: " + (d.fetched_at || "not yet") + "\n"
      + "Summary: " + Cycle.weatherText(d) + "\n"
      + "Sun: " + Cycle.sunText(d) + "\n"
      + "Temperature: " + v(w.temperature_2m, " \u00b0C") + " (feels " + v(w.apparent_temperature, " \u00b0C") + ")\n"
      + "Humidity: " + v(w.relative_humidity_2m, " %") + " \u00b7 dew " + v(w.dew_point_2m, " \u00b0C") + "\n"
      + "Condition: " + Cycle.condition(w.weather_code || 0) + " (WMO " + (w.weather_code === undefined ? "" : w.weather_code) + ")\n"
      + "Rain: " + v(w.precipitation, " mm") + " \u00b7 " + v(w.rain, " mm") + "\n"
      + "Cloud: " + v(w.cloud_cover, "%") + "\n"
      + "Wind: " + v(w.wind_speed_10m, " km/h") + " from " + (w.wind_direction_10m === undefined ? "" : w.wind_direction_10m) + "\u00b0\n"
      + "Air: US AQI " + (a.us_aqi === undefined ? "\u2014" : a.us_aqi) + " \u00b7 PM2.5 " + (a.pm2_5 === undefined ? "\u2014" : a.pm2_5) + " \u00b7 dust " + (a.dust === undefined ? "\u2014" : a.dust) + "\n"
      + "Sea: waves " + v(m.wave_height, " m") + " \u00b7 water " + v(m.sea_surface_temperature, " \u00b0C") + "\n"
      + "Headline: " + (d.headline || "\u2014") + (d.headline_source ? (" (" + d.headline_source + ")") : "")
    paintFeeds()
  }
  function paintFeeds() {
    var s = root.settingsData
    var feeds = (s && s.feeds) || []
    var lines = []
    for (var i = 0; i < feeds.length; i++) lines.push((feeds[i].enabled === false ? "off" : "on") + "  " + (feeds[i].name || "feed") + "  " + (feeds[i].url || ""))
    feedList.text = lines.length ? lines.join("\n") : "No feeds"
  }
  function reloadPrompts() {
    Cycle.settings().then(function(s) {
      root.settingsData = s
      return Cycle.promptNames().then(function(names) {
        return Cycle.activePrompt().then(function(active) {
          root.promptList = names
          root.activeId = active || ""
          if (!root.editingId || names.indexOf(root.editingId) < 0) root.editingId = active || (names[0] || "")
          root.filling = true
          promptBox.model = names
          promptBox.currentIndex = Math.max(0, names.indexOf(root.editingId))
          root.filling = false
          if (!root.editingId) {
            titleField.text = ""
            idField.text = ""
            promptEdit.text = ""
            errorLabel.text = ""
            previewLabel.text = ""
            return
          }
          idField.text = root.editingId
          titleField.text = Cycle.titleFrom(root.editingId, s)
          Cycle.promptText(root.editingId).then(function(t) {
            promptEdit.text = t
            refreshPromptMeta()
          })
        })
      })
    })
  }
  function refreshPromptMeta() {
    var s = root.settingsData || {}
    var images = s.images ? s.images.length : 0
    var errs = Cycle.promptErrors(promptEdit.text, images)
    errorLabel.text = errs.join("\n")
    errorLabel.visible = errs.length > 0
    var hits = Cycle.completeCall(tailToken(promptEdit.text), imageNames(images))
    suggestRow.model = hits.slice(0, 8)
    previewTimer.restart()
  }
  function imageNames(n) {
    var out = []
    for (var i = 1; i <= n; i++) out.push("image" + i)
    return out
  }
  function tailToken(text) {
    var m = /(?:^|[\s(\[,;:"'])(\/[A-Za-z0-9_/+%-]*)$/.exec(text || "")
    return m ? m[1] : ""
  }
  function reloadSchedule() {
    Cycle.statusAll().then(function(st) {
      root.scheduleStatus = st
      return Cycle.settings()
    }).then(function(s) {
      root.settingsData = s
      var names = root.promptList
      root.filling = true
      schedPrompt.model = names
      var chosen = scheduledName(s)
      schedPrompt.currentIndex = Math.max(0, names.indexOf(chosen))
      root.filling = false
      applyScheduleForm(s, chosen)
    })
  }
  function scheduledName(s) {
    var m = (s && s.schedules) || {}
    var keys = Object.keys(m)
    for (var i = 0; i < keys.length; i++) if (m[keys[i]] && m[keys[i]].enabled) return keys[i]
    return root.activeId || (root.promptList[0] || "")
  }
  function applyScheduleForm(s, name) {
    var sc = (s && s.schedules && s.schedules[name]) || { enabled: false, every_min: 60, window: "dawn-dusk", anticipate: false }
    root.filling = true
    schedOn.checked = !!sc.enabled
    var modes = [15, 30, 60, 120, 180]
    var every = modes.indexOf(sc.every_min)
    modeBox.currentIndex = every < 0 ? 2 : every
    windowBox.currentIndex = sc.window === "always" ? 1 : 0
    anticipateSwitch.checked = !!sc.anticipate
    root.filling = false
    var info = (root.scheduleStatus && root.scheduleStatus[name]) || {}
    schedInfo.text = "Next: " + (info.next_run || "\u2014") + "\nLast: " + (info.last_run || "\u2014") + "\nStatus: " + (info.status || "\u2014") + "\nLead: " + (info.lead_secs || 0) + " s"
  }
  function currentSchedule() {
    var modes = [15, 30, 60, 120, 180]
    return {
      enabled: schedOn.checked,
      every_min: modes[modeBox.currentIndex] || 60,
      window: windowBox.currentIndex === 1 ? "always" : "dawn-dusk",
      anticipate: anticipateSwitch.checked
    }
  }
  function reloadSettings() {
    Cycle.settings().then(function(s) {
      root.settingsData = s
      paintFeeds()
      locName.text = (s.location && s.location.name) || ""
      locLat.text = s.location ? String(s.location.lat) : ""
      locLon.text = s.location ? String(s.location.lon) : ""
      var times = s.times || {}
      phaseMorning.text = times.morning || "sunrise"
      phaseAfternoon.text = times.afternoon || "solar_noon"
      phaseEvening.text = times.evening || "sunset"
      phaseNight.text = times.night || "civil_dusk"
      genTemplate.text = (s.generator && s.generator.template) || ""
      genCommand.text = (s.generator && s.generator.command) || ""
      genTimeout.text = String((s.generator && s.generator.timeout) || 1200)
      schedFrom.text = String(s.schedule && typeof s.schedule.from === "number" ? s.schedule.from : 6)
      schedTo.text = String(s.schedule && typeof s.schedule.to === "number" ? s.schedule.to : 18)
      weatherOn.checked = !(s.weather && s.weather.enabled === false)
      var rules = (s.weather && s.weather.rules) || []
      var lines = []
      for (var i = 0; i < rules.length; i++) {
        var r = rules[i]
        lines.push((r.name || "rule") + ": if " + (r.when || "") + " \u2192 " + (r.preset || "") + (r.generate ? " + wallpaper" : ""))
      }
      rulesText.text = lines.join("\n")
      var today = new Date()
      var now = Cycle.sunEvents({ y: today.getFullYear(), mo: today.getMonth() + 1, d: today.getDate() }, s)
      sunToday.text = (s.location && s.location.name ? s.location.name : "Location") + " today: sunrise " + (now.sunrise || "") + " \u00b7 solar noon " + (now.solar_noon || "") + " \u00b7 sunset " + (now.sunset || "") + " \u00b7 civil dusk " + (now.civil_dusk || "")
      return Cycle.listNames(Cycle.cyclesDir(), "json").then(function(cycles) {
        root.filling = true
        weekdayBox.model = cycles
        weekendBox.model = cycles
        cycleEdit.model = cycles
        var wc = (s.cycles && s.cycles.weekday) || "weekday"
        var ec = (s.cycles && s.cycles.weekend) || "weekend"
        weekdayBox.currentIndex = Math.max(0, cycles.indexOf(wc))
        weekendBox.currentIndex = Math.max(0, cycles.indexOf(ec))
        wifiCycle.model = cycles
        root.filling = false
        return Cycle.listNames(Cycle.presetsDir(), "json")
      }).then(function(presets) {
        root.filling = true
        presetBox.model = presets
        presetBox.currentIndex = presets.length ? 0 : -1
        cyclePreset.model = presets
        root.filling = false
        loadPreset(presets[0] || "")
        Cycle.wifiSsid().then(function(ssid) { wifiLabel.text = ssid ? ("Current Wi-Fi: " + ssid) : "Current Wi-Fi: none" })
      })
    })
  }
  function loadPreset(name) {
    if (!name) return
    Cycle.preset(name).then(function(p) {
      p = p || {}
      presetTheme.text = p.theme || ""
      presetWallpaper.text = p.wallpaper || ""
      presetBright.text = (typeof p.brightness === "number") ? String(p.brightness) : ""
      presetWarmth.text = (typeof p.warmth === "number") ? String(p.warmth) : "4500"
      presetNight.checked = !!p.night_shift
      presetBrightOn.checked = typeof p.brightness === "number"
      var dark = p.dark === true ? "dark" : (p.dark === false ? "light" : "keep")
      root.filling = true
      darkBox.currentIndex = dark === "dark" ? 1 : (dark === "light" ? 2 : 0)
      root.filling = false
    })
  }
  function presetPayload() {
    var dark = darkBox.currentIndex === 1 ? true : (darkBox.currentIndex === 2 ? false : null)
    var bright = presetBrightOn.checked ? parseInt(presetBright.text, 10) : null
    var warmth = parseInt(presetWarmth.text, 10)
    return {
      label: presetBox.currentText || "Preset",
      theme: presetTheme.text.trim() ? presetTheme.text.trim() : null,
      wallpaper: presetWallpaper.text.trim() ? presetWallpaper.text.trim() : null,
      brightness: (bright === null || isNaN(bright)) ? null : bright,
      night_shift: presetNight.checked,
      warmth: isNaN(warmth) ? 4500 : warmth,
      dark: dark
    }
  }

  EngineHost {
    id: engineHost
    onMinute: function() { Cycle.tick(false) }
  }
  Component.onCompleted: ensureHost()

  FloatingWindow {
    id: window
    title: "Cycle"
    implicitWidth: 720
    implicitHeight: 780
    color: "#14151c"
    visible: false

    ColumnLayout {
      anchors.fill: parent
      anchors.margins: 12
      spacing: 8

      TabBar {
        id: tabs
        Layout.fillWidth: true
        background: Rectangle { color: "#1c1e28" }
        TabButton { text: "Now" }
        TabButton { text: "Data" }
        TabButton { text: "Prompt" }
        TabButton { text: "Schedule" }
        TabButton { text: "Settings" }
      }

      StackLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        currentIndex: tabs.currentIndex

        Flickable {
          contentWidth: width
          contentHeight: nowCol.implicitHeight
          clip: true
          Column {
            id: nowCol
            width: parent.width
            spacing: 8
            Label { text: "Circadian curve"; color: "#f2efe9"; font.bold: true }
            Label { id: nowLine; wrapMode: Text.WordWrap; width: parent.width; color: "#f2efe9" }
            Label { id: nextLine; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
            Canvas {
              id: graph
              width: parent.width
              height: 210
              onPaint: {
                var ctx = getContext("2d")
                ctx.clearRect(0, 0, width, height)
                ctx.fillStyle = "#1c1e28"
                ctx.fillRect(0, 0, width, height)
                var d = root.curveData
                if (!d || !d.points) return
                var l = 44, r = 40, t = 12, b = 22
                var ww = width - l - r
                var hh = height - t - b
                function x(m) { return l + ww * m / 1440 }
                ctx.fillStyle = "#a8a39a"
                ctx.font = "10px sans-serif"
                for (var hr = 0; hr <= 24; hr += 3) ctx.fillText((hr < 10 ? "0" : "") + hr, x(hr * 60) - 7, height - 6)
                function line(idx, lo, hi, color) {
                  ctx.strokeStyle = color
                  ctx.lineWidth = 2
                  ctx.beginPath()
                  for (var i = 0; i < d.points.length; i++) {
                    var px = x(d.points[i][0])
                    var py = t + hh * (1 - (d.points[i][idx] - lo) / (hi - lo))
                    if (i === 0) ctx.moveTo(px, py)
                    else ctx.lineTo(px, py)
                  }
                  ctx.stroke()
                }
                line(1, 2500, 6600, "#e0a050")
                line(2, 0, 100, "#4c90e0")
                var nx = x(d.now || 0)
                ctx.strokeStyle = "#e23b4a"
                ctx.beginPath()
                ctx.moveTo(nx, t)
                ctx.lineTo(nx, t + hh)
                ctx.stroke()
              }
            }
            RowLayout {
              width: parent.width
              Label { text: "Automatic Cycle"; color: "#f2efe9" }
              Switch {
                id: engineSwitch
                onToggled: {
                  if (root.filling) return
                  Cycle.setEngine(checked).then(function() { root.note(checked ? "Engine on" : "Engine off"); reloadNow() })
                }
              }
            }
            RowLayout {
              width: parent.width
              Label { text: "Preset"; color: "#f2efe9" }
              ComboBox {
                id: holdBox
                Layout.fillWidth: true
                model: ["Automatic"]
                onActivated: {
                  if (root.filling) return
                  Cycle.setHold(currentText === "Automatic" ? "automatic" : currentText).then(function() {
                    root.note("Preset: " + currentText)
                    reloadNow()
                  })
                }
              }
            }
            RowLayout {
              Button {
                text: "Apply now"
                onClicked: Cycle.tick(true).then(function() { root.note("Applied"); reloadNow() })
              }
              Button {
                text: "Generate"
                onClicked: {
                  root.note("Generating wallpaper from live data")
                  Cycle.generateAt(false, null, null).then(function() { root.note("Generation finished"); reload() })
                }
              }
            }
            Label { text: "Resolution"; color: "#f2efe9"; font.bold: true }
            Label { id: resolveText; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
          }
        }

        Flickable {
          contentWidth: width
          contentHeight: dataCol.implicitHeight
          clip: true
          Column {
            id: dataCol
            width: parent.width
            spacing: 8
            RowLayout {
              Label { text: "Live data"; color: "#f2efe9"; font.bold: true }
              Button {
                text: "Refresh"
                onClicked: Cycle.refreshLive().then(function(d) { root.liveData = d; paintLive(d); root.note("Live data refreshed") })
              }
            }
            Label { id: liveText; wrapMode: Text.WordWrap; width: parent.width; color: "#f2efe9" }
            Label { text: "RSS feeds"; color: "#f2efe9"; font.bold: true }
            Label { id: feedList; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
            TextField { id: feedIndex; placeholderText: "Index to edit (blank = new)"; width: parent.width; color: "#f2efe9"; placeholderTextColor: "#a8a39a" }
            TextField { id: feedName; placeholderText: "Name"; width: parent.width; color: "#f2efe9"; placeholderTextColor: "#a8a39a" }
            TextField { id: feedUrl; placeholderText: "Feed address"; width: parent.width; color: "#f2efe9"; placeholderTextColor: "#a8a39a" }
            Switch { id: feedEnabled; text: "Enabled"; checked: true }
            RowLayout {
              Button {
                text: "Load"
                onClicked: {
                  var idx = parseInt(feedIndex.text, 10)
                  var feeds = (root.settingsData && root.settingsData.feeds) || []
                  if (isNaN(idx) || idx < 0 || idx >= feeds.length) { root.note("No feed at that index"); return }
                  root.feedEdit = idx
                  feedName.text = feeds[idx].name || ""
                  feedUrl.text = feeds[idx].url || ""
                  feedEnabled.checked = feeds[idx].enabled !== false
                }
              }
              Button {
                text: "Save feed"
                onClicked: {
                  var url = feedUrl.text.trim()
                  if (url.indexOf("http") !== 0) { root.note("Enter an http(s) feed address"); return }
                  var name = feedName.text.trim() || url
                  var idx = feedIndex.text.trim() === "" ? -1 : parseInt(feedIndex.text, 10)
                  var on = feedEnabled.checked
                  Cycle.updateSettings(function(s) {
                    if (!s.feeds) s.feeds = []
                    if (!isNaN(idx) && idx >= 0 && idx < s.feeds.length) { s.feeds[idx].name = name; s.feeds[idx].url = url; s.feeds[idx].enabled = on }
                    else s.feeds.push({ name: name, url: url, enabled: on })
                  }).then(function() { root.feedEdit = -1; root.note("Feed saved"); reloadSettings() })
                }
              }
              Button {
                text: "Remove"
                onClicked: {
                  var idx = parseInt(feedIndex.text, 10)
                  Cycle.updateSettings(function(s) {
                    if (!s.feeds) return
                    if (!isNaN(idx) && idx >= 0 && idx < s.feeds.length) s.feeds.splice(idx, 1)
                  }).then(function() { root.note("Feed removed"); reloadSettings() })
                }
              }
            }
          }
        }

        Flickable {
          contentWidth: width
          contentHeight: promptCol.implicitHeight
          clip: true
          Column {
            id: promptCol
            width: parent.width
            spacing: 8
            Label { text: root.editingId && root.editingId === root.activeId ? "Prompt  \u2713 active" : "Prompt"; color: "#f2efe9"; font.bold: true }
            ComboBox {
              id: promptBox
              width: parent.width
              model: []
              onActivated: {
                if (root.filling) return
                root.editingId = currentText
                reloadPrompts()
              }
            }
            RowLayout {
              Button {
                text: "New"
                onClicked: {
                  var i = 1
                  function exists(names, id) { for (var k = 0; k < names.length; k++) if (names[k] === id) return true; return false }
                  var id = "prompt-" + i
                  while (exists(root.promptList, id)) { i++; id = "prompt-" + i }
                  var body = "A wide 16:9 desktop wallpaper of /location in the /phase. Light: /sun. Weather: /weather. Air: /air. In the style: /style. LED billboard shows: /breaking. No characters, no other text."
                  Cycle.savePrompt(id, body).then(function() { return Cycle.syncMenu() }).then(function() {
                    root.editingId = id
                    root.note("Created " + id)
                    reloadPrompts()
                  })
                }
              }
              Button {
                text: "Use"
                enabled: root.editingId !== "" && root.editingId !== root.activeId
                onClicked: Cycle.setActivePrompt(root.editingId).then(function() { return Cycle.syncMenu() }).then(function() { root.note("Active prompt: " + root.editingId); reloadPrompts() })
              }
              Button {
                text: "Generate"
                onClicked: {
                  root.note("Generating from " + (root.editingId || "the active prompt"))
                  Cycle.generateAt(false, root.editingId || null, null).then(function() { root.note("Generation finished") })
                }
              }
              Button {
                text: "Duplicate"
                onClicked: {
                  var base = root.editingId
                  if (!base) return
                  Cycle.promptText(base).then(function(t) {
                    var copy = base + "-copy"
                    var n = 2
                    function exists(id) { for (var k = 0; k < root.promptList.length; k++) if (root.promptList[k] === id) return true; return false }
                    while (exists(copy)) { copy = base + "-copy" + n; n++ }
                    return Cycle.savePrompt(copy, t).then(function() {
                      return Cycle.setTitle(copy, Cycle.titleFrom(base, root.settingsData || {}) + " (copy)")
                    }).then(function() { return Cycle.syncMenu() }).then(function() {
                      root.editingId = copy
                      root.note("Duplicated as " + copy)
                      reloadPrompts()
                    })
                  })
                }
              }
              Button {
                text: "Delete"
                enabled: root.editingId !== "" && root.editingId !== root.activeId
                onClicked: {
                  var id = root.editingId
                  Cycle.deletePrompt(id).then(function() {
                    return Cycle.updateSettings(function(s) { if (s.schedules) delete s.schedules[id] })
                  }).then(function() { return Cycle.syncMenu() }).then(function() {
                    root.editingId = ""
                    root.note("Deleted " + id)
                    reloadPrompts()
                  })
                }
              }
              Button { text: "Schedule\u2026"; onClicked: tabs.currentIndex = 3 }
            }
            Label { text: "Title"; color: "#a8a39a" }
            TextField { id: titleField; width: parent.width; color: "#f2efe9" }
            Label { text: "Id"; color: "#a8a39a" }
            TextField { id: idField; width: parent.width; color: "#f2efe9" }
            ScrollView {
              width: parent.width
              height: 160
              TextArea {
                id: promptEdit
                wrapMode: Text.WordWrap
                color: "#f2efe9"
                onTextChanged: refreshPromptMeta()
              }
            }
            Row {
              id: suggestRow
              spacing: 4
              property var model: []
              Repeater {
                model: suggestRow.model
                Button {
                  text: modelData
                  onClicked: {
                    var tok = tailToken(promptEdit.text)
                    var cur = tok.split("/").pop()
                    var add = text.slice(cur.length)
                    promptEdit.text = promptEdit.text + add
                  }
                }
              }
            }
            Label { id: errorLabel; wrapMode: Text.WordWrap; width: parent.width; color: "#e06c75"; visible: false }
            Button {
              text: "Save"
              onClicked: {
                var nn = Cycle.safeName(idField.text)
                if (!nn) { root.note("Give the prompt an id"); return }
                var old = root.editingId
                var body = promptEdit.text
                Cycle.savePrompt(nn, body).then(function() {
                  if (old && old !== nn) {
                    return Cycle.deletePrompt(old).then(function() {
                      if (root.activeId === old) return Cycle.setActivePrompt(nn)
                    }).then(function() {
                      return Cycle.updateSettings(function(s) {
                        if (s.schedules && s.schedules[old]) {
                          s.schedules[nn] = s.schedules[old]
                          delete s.schedules[old]
                        }
                      })
                    })
                  }
                }).then(function() { return Cycle.setTitle(nn, titleField.text) })
                  .then(function() { return Cycle.syncMenu() })
                  .then(function() {
                    root.editingId = nn
                    var errs = Cycle.promptErrors(body, (root.settingsData && root.settingsData.images) ? root.settingsData.images.length : 0)
                    root.note(errs.length ? ("Saved " + nn + " \u2013 " + errs.length + " error(s)") : ("Saved " + nn))
                    reloadPrompts()
                  })
              }
            }
            Label { text: "Live preview"; color: "#f2efe9"; font.bold: true }
            Label { id: previewLabel; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
            Timer {
              id: previewTimer
              interval: 500
              repeat: false
              onTriggered: {
                var text = promptEdit.text
                Cycle.settings().then(function(s) { return Cycle.expandPlaceholders(text, s) }).then(function(out) { previewLabel.text = out })
              }
            }
          }
        }

        Flickable {
          contentWidth: width
          contentHeight: schedCol.implicitHeight
          clip: true
          Column {
            id: schedCol
            width: parent.width
            spacing: 8
            Label { text: "Mode"; color: "#f2efe9"; font.bold: true }
            Label { text: "Clock-aligned slots. Manual runs do not move them."; color: "#a8a39a"; wrapMode: Text.WordWrap; width: parent.width }
            ComboBox { id: schedPrompt; width: parent.width; model: []; onActivated: { if (!root.filling) applyScheduleForm(root.settingsData, currentText) } }
            Switch { id: schedOn; text: "Scheduled" }
            ComboBox { id: modeBox; width: parent.width; model: ["15 min", "30 min", "60 min", "120 min", "180 min"] }
            ComboBox { id: windowBox; width: parent.width; model: ["dawn\u2013dusk", "always"] }
            Switch { id: anticipateSwitch; text: "Anticipate generation delay" }
            Label { id: schedInfo; color: "#f2efe9"; wrapMode: Text.WordWrap; width: parent.width }
            Button {
              text: "Save schedule"
              onClicked: {
                var name = schedPrompt.currentText
                if (!name) { root.note("Create a prompt first"); return }
                Cycle.saveSchedule(name, currentSchedule()).then(function() { root.note("Schedule saved"); reloadSchedule() })
              }
            }
          }
        }

        Flickable {
          contentWidth: width
          contentHeight: setCol.implicitHeight
          clip: true
          Column {
            id: setCol
            width: parent.width
            spacing: 8
            Label { id: sunToday; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
            Label { text: "Phase boundaries"; color: "#f2efe9"; font.bold: true }
            TextField { id: phaseMorning; placeholderText: "Morning starts at"; width: parent.width; color: "#f2efe9" }
            TextField { id: phaseAfternoon; placeholderText: "Afternoon starts at"; width: parent.width; color: "#f2efe9" }
            TextField { id: phaseEvening; placeholderText: "Evening starts at"; width: parent.width; color: "#f2efe9" }
            TextField { id: phaseNight; placeholderText: "Night starts at"; width: parent.width; color: "#f2efe9" }
            Button {
              text: "Save phases"
              onClicked: {
                var names = ["sunrise", "solar_noon", "sunset", "civil_dusk", "civil_dawn", "nautical_dusk", "astronomical_dusk"]
                var fields = [phaseMorning.text.trim(), phaseAfternoon.text.trim(), phaseEvening.text.trim(), phaseNight.text.trim()]
                for (var i = 0; i < fields.length; i++) {
                  if (names.indexOf(fields[i]) < 0 && !/^\d{1,2}:\d{2}$/.test(fields[i])) {
                    root.note("Use a sun event or HH:MM")
                    return
                  }
                }
                Cycle.updateSettings(function(s) {
                  s.times = { morning: fields[0], afternoon: fields[1], evening: fields[2], night: fields[3] }
                }).then(function() { root.note("Saved"); reloadSettings() })
              }
            }
            Label { text: "Day type"; color: "#f2efe9"; font.bold: true }
            ComboBox {
              id: weekdayBox
              width: parent.width
              model: []
              onActivated: { if (!root.filling) Cycle.updateSettings(function(s) { if (!s.cycles) s.cycles = {}; s.cycles.weekday = weekdayBox.currentText }).then(function() { root.note("Saved") }) }
            }
            ComboBox {
              id: weekendBox
              width: parent.width
              model: []
              onActivated: { if (!root.filling) Cycle.updateSettings(function(s) { if (!s.cycles) s.cycles = {}; s.cycles.weekend = weekendBox.currentText }).then(function() { root.note("Saved") }) }
            }
            Label { text: "Location"; color: "#f2efe9"; font.bold: true }
            TextField { id: locName; placeholderText: "Name"; width: parent.width; color: "#f2efe9" }
            TextField { id: locLat; placeholderText: "Latitude"; width: parent.width; color: "#f2efe9" }
            TextField { id: locLon; placeholderText: "Longitude"; width: parent.width; color: "#f2efe9" }
            Button {
              text: "Save location"
              onClicked: Cycle.updateSettings(function(s) {
                var lat = parseFloat(locLat.text)
                var lon = parseFloat(locLon.text)
                s.location = { name: locName.text.trim() || "Dubai", lat: isNaN(lat) ? 25.2048 : lat, lon: isNaN(lon) ? 55.2708 : lon, source: "fixed" }
              }).then(function() { root.note("Saved"); reloadSettings() })
            }
            Label { id: wifiLabel; color: "#a8a39a"; wrapMode: Text.WordWrap; width: parent.width }
            ComboBox { id: wifiCycle; width: parent.width; model: [] }
            Button {
              text: "Map current Wi-Fi"
              onClicked: Cycle.wifiSsid().then(function(ssid) {
                if (!ssid) { root.note("No Wi-Fi network"); return }
                var cycle = wifiCycle.currentText
                return Cycle.updateSettings(function(s) { if (!s.wifi) s.wifi = {}; s.wifi[ssid] = cycle }).then(function() { root.note("Mapped " + ssid); reloadSettings() })
              })
            }
            Label { text: "Weather rules"; color: "#f2efe9"; font.bold: true }
            Switch {
              id: weatherOn
              text: "Use weather rules"
              onToggled: { if (!root.filling) Cycle.updateSettings(function(s) { if (!s.weather) s.weather = {}; s.weather.enabled = weatherOn.checked }).then(function() { root.note("Saved") }) }
            }
            Label { id: rulesText; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
            TextField { id: ruleIndex; placeholderText: "Rule index"; text: "0"; width: parent.width; color: "#f2efe9" }
            TextField { id: ruleWhen; placeholderText: "Condition, e.g. temperature >= 40"; width: parent.width; color: "#f2efe9" }
            TextField { id: rulePreset; placeholderText: "Preset id"; width: parent.width; color: "#f2efe9" }
            Switch { id: ruleGenerate; text: "Generate wallpaper with this rule" }
            RowLayout {
              Button {
                text: "Load rule"
                onClicked: {
                  var idx = parseInt(ruleIndex.text, 10) || 0
                  var rules = (root.settingsData && root.settingsData.weather && root.settingsData.weather.rules) || []
                  if (idx < 0 || idx >= rules.length) { root.note("No rule at that index"); return }
                  ruleWhen.text = rules[idx].when || ""
                  rulePreset.text = rules[idx].preset || ""
                  ruleGenerate.checked = !!rules[idx].generate
                }
              }
              Button {
                text: "Save rule"
                onClicked: {
                  var idx = parseInt(ruleIndex.text, 10) || 0
                  var when = ruleWhen.text
                  var preset = rulePreset.text.trim()
                  var gen = ruleGenerate.checked
                  Cycle.updateSettings(function(s) {
                    if (!s.weather || !s.weather.rules || idx < 0 || idx >= s.weather.rules.length) return
                    if (when.trim()) s.weather.rules[idx].when = when.trim()
                    if (preset) s.weather.rules[idx].preset = preset
                    s.weather.rules[idx].generate = gen
                  }).then(function() { root.note("Saved"); reloadSettings() })
                }
              }
            }
            Label { text: "Generation"; color: "#f2efe9"; font.bold: true }
            Label { text: "Night window hours (from / to). Placeholders: {{location}} {{temperature}} {{condition}} {{phase}} and {prompt_q} {out}."; wrapMode: Text.WordWrap; width: parent.width; color: "#a8a39a" }
            TextField { id: schedFrom; placeholderText: "From hour"; width: parent.width; color: "#f2efe9" }
            TextField { id: schedTo; placeholderText: "To hour"; width: parent.width; color: "#f2efe9" }
            TextField { id: genTemplate; placeholderText: "Prompt template"; width: parent.width; color: "#f2efe9" }
            TextField { id: genCommand; placeholderText: "Generator command"; width: parent.width; color: "#f2efe9" }
            TextField { id: genTimeout; placeholderText: "Timeout seconds"; width: parent.width; color: "#f2efe9" }
            RowLayout {
              Button {
                text: "Save generation"
                onClicked: Cycle.updateSettings(function(s) {
                  if (!s.generator) s.generator = {}
                  s.generator.template = genTemplate.text
                  s.generator.command = genCommand.text
                  var to = parseInt(genTimeout.text, 10)
                  s.generator.timeout = isNaN(to) ? 1200 : to
                  if (!s.schedule) s.schedule = {}
                  s.schedule.from = parseInt(schedFrom.text, 10) || 0
                  s.schedule.to = parseInt(schedTo.text, 10) || 0
                }).then(function() { root.note("Saved") })
              }
              Button {
                text: "Test generation"
                onClicked: {
                  root.note("Generating\u2026")
                  Cycle.settings().then(function(s) {
                    return Cycle.resolveState(true).then(function(res) {
                      return Cycle.generateImage("test", Cycle.themeContext(res), s)
                    })
                  }).then(function(path) { root.note(path ? ("Wrote " + path) : "Generation failed") })
                }
              }
            }
            Label { text: "Preset values"; color: "#f2efe9"; font.bold: true }
            ComboBox {
              id: presetBox
              width: parent.width
              model: []
              onActivated: { if (!root.filling) loadPreset(currentText) }
            }
            TextField { id: presetTheme; placeholderText: "Theme (empty keeps current)"; width: parent.width; color: "#f2efe9" }
            TextField { id: presetWallpaper; placeholderText: "Wallpaper path"; width: parent.width; color: "#f2efe9" }
            Switch { id: presetBrightOn; text: "Set brightness" }
            TextField { id: presetBright; placeholderText: "Brightness %"; width: parent.width; color: "#f2efe9" }
            Switch { id: presetNight; text: "Night Shift" }
            TextField { id: presetWarmth; placeholderText: "Warmth (K)"; width: parent.width; color: "#f2efe9" }
            ComboBox { id: darkBox; width: parent.width; model: ["Appearance: keep", "Dark", "Light"] }
            RowLayout {
              Button { text: "Try"; onClicked: Cycle.applyPreset(presetPayload()).then(function() { root.note("Applied") }) }
              Button {
                text: "Save preset"
                onClicked: {
                  var name = presetBox.currentText
                  if (!name) return
                  Cycle.savePreset(name, presetPayload()).then(function() { root.note("Saved " + name) })
                }
              }
              Button {
                text: "Hold this"
                onClicked: {
                  var name = presetBox.currentText
                  if (!name) return
                  Cycle.setHold(name).then(function() { root.note("Holding " + name); reloadNow() })
                }
              }
            }
            TextField { id: newPresetName; placeholderText: "New preset id"; width: parent.width; color: "#f2efe9"; placeholderTextColor: "#a8a39a" }
            Button {
              text: "Create preset"
              onClicked: {
                var name = Cycle.safeName(newPresetName.text)
                if (!name) return
                var body = presetPayload()
                body.label = name
                Cycle.savePreset(name, body).then(function() { root.note("Created " + name); reloadSettings() })
              }
            }
            Label { text: "Cycle phases"; color: "#f2efe9"; font.bold: true }
            ComboBox { id: cycleEdit; width: parent.width; model: [] }
            ComboBox { id: cyclePhase; width: parent.width; model: ["morning", "afternoon", "evening", "night"] }
            ComboBox { id: cyclePreset; width: parent.width; model: [] }
            Button {
              text: "Save phase mapping"
              onClicked: {
                var name = cycleEdit.currentText
                var phase = cyclePhase.currentText
                var preset = cyclePreset.currentText
                if (!name || !preset) return
                Cycle.cycleOf(name).then(function(c) {
                  c = c || { label: name, phases: {} }
                  if (!c.phases) c.phases = {}
                  c.phases[phase] = preset
                  return Cycle.saveCycle(name, c)
                }).then(function() { root.note(name + ": " + phase + " \u2192 " + preset) })
              }
            }
            TextField { id: newCycleName; placeholderText: "New cycle id"; width: parent.width; color: "#f2efe9"; placeholderTextColor: "#a8a39a" }
            Button {
              text: "Create cycle"
              onClicked: {
                var name = Cycle.safeName(newCycleName.text)
                if (!name) return
                Cycle.saveCycle(name, { label: name, phases: { morning: "day", afternoon: "day", evening: "evening", night: "night" } }).then(function() {
                  root.note("Created " + name)
                  reloadSettings()
                })
              }
            }
          }
        }
      }

      Label {
        Layout.fillWidth: true
        text: root.noteText
        color: "#e0a050"
        elide: Text.ElideRight
      }
    }
  }
}
