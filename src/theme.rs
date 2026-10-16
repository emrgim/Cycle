//! Omarchy theme integration: palette from an image (colors.toml) and the real Omarchy themed CSS.
use crate::core::home;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn hls(c: [f64; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (c[0] / 255.0, c[1] / 255.0, c[2] / 255.0);
    let mx = r.max(g).max(b); let mn = r.min(g).min(b); let l = (mx + mn) / 2.0;
    if mx == mn { return (0.0, l, 0.0); }
    let s = if l <= 0.5 { (mx - mn) / (mx + mn) } else { (mx - mn) / (2.0 - mx - mn) };
    let (rc, gc, bc) = ((mx - r) / (mx - mn), (mx - g) / (mx - mn), (mx - b) / (mx - mn));
    let h = if r == mx { bc - gc } else if g == mx { 2.0 + rc - bc } else { 4.0 + gc - rc };
    (((h / 6.0) % 1.0 + 1.0) % 1.0, l, s)
}
fn rgb(h: f64, l: f64, s: f64) -> [f64; 3] {
    let (h, l, s) = (h.rem_euclid(1.0), l.clamp(0.0, 1.0), s.clamp(0.0, 1.0));
    if s == 0.0 { return [l * 255.0; 3]; }
    let m2 = if l <= 0.5 { l * (1.0 + s) } else { l + s - l * s }; let m1 = 2.0 * l - m2;
    let v = |hue: f64| { let hue = hue.rem_euclid(1.0);
        (if hue < 1.0 / 6.0 { m1 + (m2 - m1) * hue * 6.0 } else if hue < 0.5 { m2 } else if hue < 2.0 / 3.0 { m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0 } else { m1 }) * 255.0 };
    [v(h + 1.0 / 3.0), v(h), v(h - 1.0 / 3.0)]
}
fn hex(c: [f64; 3]) -> String { format!("#{:02x}{:02x}{:02x}", c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8) }

/// 8-colour k-means on a 256px thumbnail, then the same derivation rules as before.
pub fn palette_from_image(p: &Path) -> Result<String, String> {
    let im = image::open(p).map_err(|e| e.to_string())?.thumbnail(256, 256).to_rgb8();
    let px: Vec<[f64; 3]> = im.pixels().map(|q| [q[0] as f64, q[1] as f64, q[2] as f64]).collect();
    if px.is_empty() { return Err("empty image".into()); }
    let mut sorted = px.clone(); sorted.sort_by(|a, b| (a[0] + a[1] + a[2]).partial_cmp(&(b[0] + b[1] + b[2])).unwrap());
    let k = 8; let mut cent: Vec<[f64; 3]> = (0..k).map(|i| sorted[(i * 2 + 1) * sorted.len() / (2 * k)]).collect();
    let mut asg = vec![0usize; px.len()];
    for _ in 0..12 {
        for (i, q) in px.iter().enumerate() {
            asg[i] = (0..k).min_by(|&a, &b| d2(q, &cent[a]).partial_cmp(&d2(q, &cent[b])).unwrap()).unwrap();
        }
        let mut sum = vec![[0.0; 3]; k]; let mut n = vec![0usize; k];
        for (i, q) in px.iter().enumerate() { let c = asg[i]; n[c] += 1; for j in 0..3 { sum[c][j] += q[j]; } }
        for c in 0..k { if n[c] > 0 { for j in 0..3 { cent[c][j] = sum[c][j] / n[c] as f64; } } }
    }
    let mut n = vec![0usize; k]; for &a in &asg { n[a] += 1; }
    let mut cols: Vec<(usize, [f64; 3])> = (0..k).filter(|&c| n[c] > 0).map(|c| (n[c], cent[c])).collect();
    cols.sort_by(|a, b| b.0.cmp(&a.0));
    let tot: usize = cols.iter().map(|c| c.0).sum();
    let lum = cols.iter().map(|(n, c)| *n as f64 * (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2])).sum::<f64>() / tot as f64 / 255.0;
    let light = lum > 0.55; let dom = hls(cols[0].1); let n0 = cols[0].0 as f64;
    let acc = cols.iter().max_by(|a, b| { let sa = hls(a.1).2 * (0.3 + (a.0 as f64 / n0).min(1.0)); let sb = hls(b.1).2 * (0.3 + (b.0 as f64 / n0).min(1.0)); sa.partial_cmp(&sb).unwrap() }).unwrap().1;
    let (ah, _al, as_) = hls(acc);
    let ds = dom.2.min(0.25); let pick = |a: f64, b: f64| if light { a } else { b };
    let l = pick(0.38, 0.68); let s = as_.clamp(0.45, 0.75);
    let mut c: Vec<(String, String)> = vec![
        ("accent".into(), hex(rgb(ah, l, s))), ("selection".into(), hex(rgb(ah, pick(0.82, 0.28), s * 0.6))), ("muted".into(), hex(rgb(dom.0, pick(0.55, 0.5), 0.08))), ("".into(), "".into()),
        ("background".into(), hex(rgb(dom.0, pick(0.96, 0.09), ds))), ("dark_background".into(), hex(rgb(dom.0, pick(0.93, 0.07), ds))),
        ("darker_background".into(), hex(rgb(dom.0, pick(0.89, 0.05), ds))), ("lighter_background".into(), hex(rgb(dom.0, pick(0.80, 0.16), ds))), ("".into(), "".into()),
        ("foreground".into(), hex(rgb(dom.0, pick(0.12, 0.90), 0.12))), ("dark_foreground".into(), hex(rgb(dom.0, pick(0.75, 0.6), 0.1))),
        ("light_foreground".into(), hex(rgb(dom.0, pick(0.12, 0.90), 0.12))), ("bright_foreground".into(), hex(rgb(dom.0, pick(0.0, 1.0), 0.0))), ("".into(), "".into())];
    let hues = [("red", 0.0), ("yellow", 0.13), ("green", 0.33), ("cyan", 0.5), ("blue", 0.6), ("magenta", 0.83)];
    let adj = |h: f64| if (h - ah).abs() < 0.15 { h * 0.85 + ah * 0.15 } else { h };
    for (nm, h) in hues { c.push((nm.into(), hex(rgb(adj(h), l, s)))); }
    c.push(("".into(), "".into()));
    for (nm, h) in hues { c.push((format!("bright_{nm}"), hex(rgb(adj(h), l + pick(-0.08, 0.1), s)))); }
    let mut out = format!("mode = \"{}\"\n\n", if light { "light" } else { "dark" });
    for (k, v) in c { if k.is_empty() { out.push('\n') } else { out += &format!("{k} = \"{v}\"\n") } }
    Ok(out)
}
fn d2(a: &[f64; 3], b: &[f64; 3]) -> f64 { (0..3).map(|i| (a[i] - b[i]).powi(2)).sum() }

// ---------- Omarchy look ----------
pub fn current_theme_dir() -> std::path::PathBuf { home().join(".local/state/omarchy/current/theme") }
pub fn template_path() -> std::path::PathBuf { home().join(".config/omarchy/themed/cycle.css.tpl") }
pub fn colors() -> HashMap<String, String> {
    let t = fs::read_to_string(current_theme_dir().join("colors.toml")).unwrap_or_default();
    Regex::new(r#"(?m)^(\w+)\s*=\s*"([^"]+)""#).unwrap().captures_iter(&t).map(|c| (c[1].to_string(), c[2].to_string())).collect()
}
/// CSS rendered by Omarchy (current/theme/cycle.css) when up to date, else render the template ourselves.
pub fn css() -> String {
    let r = current_theme_dir().join("cycle.css"); let tpl = template_path();
    let mt = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    if let (Some(a), b) = (mt(&r), mt(&tpl)) { if b.map(|b| a >= b).unwrap_or(true) { if let Ok(s) = fs::read_to_string(&r) { return s; } } }
    let c = colors();
    let t = fs::read_to_string(&tpl).unwrap_or_default();
    Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap().replace_all(&t, |m: &regex::Captures| c.get(&m[1]).cloned().unwrap_or("#888888".into())).into_owned()
}
