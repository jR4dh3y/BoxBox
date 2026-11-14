//! `--profile-summary`: ranks a recorded trace so the slow parts stand out.

use std::collections::HashMap;
use std::fmt::Write;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::MAIN_TID;

/// A frame longer than this misses a 60 Hz refresh.
const FRAME_BUDGET_MS: f64 = 16.7;
/// UI-thread work longer than this is felt as a hitch.
const UI_BLOCK_MS: f64 = 4.0;

#[derive(Deserialize)]
struct Event {
    ph: String,
    name: String,
    #[serde(default)]
    tid: u64,
    #[serde(default)]
    ts: f64,
    #[serde(default)]
    dur: f64,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    cat: String,
    #[serde(default)]
    args: Option<Args>,
}

#[derive(Default, Deserialize)]
struct Args {
    #[serde(default)]
    detail: Option<String>,
}

/// One measured occurrence, in milliseconds.
struct Sample {
    name: String,
    ms: f64,
    detail: Option<String>,
    on_ui_thread: bool,
}

pub fn run(path: &str) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?;
    print!("{}", report(&text)?);
    Ok(())
}

fn parse(text: &str) -> Result<Vec<Event>> {
    // The recorder streams; a trace from a running or killed app has no closing bracket.
    let body = text.trim_end().trim_end_matches(']').trim_end().trim_end_matches(',');
    serde_json::from_str(&format!("{body}]")).context("not a BoxBox profile trace")
}

fn report(text: &str) -> Result<String> {
    let events = parse(text)?;
    let mut journeys = Vec::new();
    let mut spans = Vec::new();
    let mut frames = Vec::new();
    let mut begun: HashMap<u64, (f64, Option<String>)> = HashMap::new();
    for event in events {
        let detail = event.args.and_then(|args| args.detail);
        match (event.ph.as_str(), event.cat.as_str()) {
            ("X", "frame") => frames.push(event.dur / 1e3),
            ("X", _) => spans.push(Sample { name: event.name, ms: event.dur / 1e3, detail, on_ui_thread: event.tid == MAIN_TID }),
            ("b", _) => {
                begun.insert(event.id.unwrap_or_default(), (event.ts, detail));
            }
            ("e", _) => {
                if let Some((start, detail)) = begun.remove(&event.id.unwrap_or_default()) {
                    journeys.push(Sample { name: event.name, ms: (event.ts - start) / 1e3, detail, on_ui_thread: true });
                }
            }
            _ => {}
        }
    }

    let mut out = String::new();
    section(&mut out, "Journeys: input to painted frame, slowest p95 first", &journeys, |stats| -stats.p95);
    slowest(&mut out, "Slowest journeys", &journeys, 10);
    section(&mut out, "Spans: work on any thread, most total time first", &spans, |stats| -stats.total);
    let blocking: Vec<&Sample> = spans.iter().filter(|span| span.on_ui_thread && span.ms >= UI_BLOCK_MS).collect();
    let _ = writeln!(out, "\nUI thread blocked over {UI_BLOCK_MS} ms: {}", blocking.len());
    for span in top(blocking, 10) {
        let _ = writeln!(out, "  {:>9}  {}{}", ms(span.ms), span.name, detail(span));
    }
    if !frames.is_empty() {
        let stats = Stats::of(frames.clone());
        let late = frames.iter().filter(|&&frame| frame > FRAME_BUDGET_MS).count();
        let _ = writeln!(
            out,
            "\nFrames (render to paint): {}  p50 {}  p95 {}  max {}  over {FRAME_BUDGET_MS} ms: {late}",
            stats.count, ms(stats.p50), ms(stats.p95), ms(stats.max)
        );
    }
    Ok(out)
}

struct Stats {
    count: usize,
    total: f64,
    p50: f64,
    p95: f64,
    max: f64,
}

impl Stats {
    fn of(mut values: Vec<f64>) -> Self {
        values.sort_by(f64::total_cmp);
        let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
        Self { count: values.len(), total: values.iter().sum(), p50: at(0.5), p95: at(0.95), max: at(1.0) }
    }
}

fn section(out: &mut String, title: &str, samples: &[Sample], rank: impl Fn(&Stats) -> f64) {
    let mut groups: HashMap<&str, Vec<f64>> = HashMap::new();
    for sample in samples {
        groups.entry(&sample.name).or_default().push(sample.ms);
    }
    let mut rows: Vec<(&str, Stats)> = groups.into_iter().map(|(name, values)| (name, Stats::of(values))).collect();
    rows.sort_by(|a, b| rank(&a.1).total_cmp(&rank(&b.1)));
    let _ = writeln!(out, "\n{title}");
    if rows.is_empty() {
        let _ = writeln!(out, "  none recorded");
        return;
    }
    let _ = writeln!(out, "  {:<24} {:>6} {:>10} {:>9} {:>9} {:>9}", "name", "count", "total", "p50", "p95", "max");
    for (name, stats) in rows {
        let _ = writeln!(
            out,
            "  {:<24} {:>6} {:>10} {:>9} {:>9} {:>9}",
            name, stats.count, ms(stats.total), ms(stats.p50), ms(stats.p95), ms(stats.max)
        );
    }
}

fn slowest(out: &mut String, title: &str, samples: &[Sample], limit: usize) {
    if samples.is_empty() {
        return;
    }
    let _ = writeln!(out, "\n{title}");
    for sample in top(samples.iter().collect(), limit) {
        let _ = writeln!(out, "  {:>9}  {}{}", ms(sample.ms), sample.name, detail(sample));
    }
}

fn top(mut samples: Vec<&Sample>, limit: usize) -> Vec<&Sample> {
    samples.sort_by(|a, b| b.ms.total_cmp(&a.ms));
    samples.truncate(limit);
    samples
}

fn detail(sample: &Sample) -> String {
    sample.detail.as_deref().map(|detail| format!("  {detail}")).unwrap_or_default()
}

fn ms(value: f64) -> String {
    if value >= 1000.0 {
        format!("{:.2} s", value / 1000.0)
    } else {
        format!("{value:.1} ms")
    }
}

#[cfg(test)]
mod tests {
    use super::report;

    #[test]
    fn ranks_a_trace_cut_off_mid_stream() {
        let trace = r#"[
{"ph":"M","name":"thread_name","pid":1,"tid":1,"args":{"name":"main (UI)"}},
{"ph":"X","name":"api.list","cat":"span","pid":1,"tid":2,"ts":0,"dur":3000,"args":{"detail":"home"}},
{"ph":"X","name":"api.stat","cat":"span","pid":1,"tid":1,"ts":0,"dur":9000},
{"ph":"b","name":"open folder","cat":"journey","id":4,"pid":1,"tid":1,"ts":100,"args":{"detail":"home"}},
{"ph":"e","name":"open folder","cat":"journey","id":4,"pid":1,"tid":1,"ts":20100,"args":{"detail":"home"}},
{"ph":"X","name":"frame","cat":"frame","pid":1,"tid":1,"ts":0,"dur":20000},
"#;
        let text = report(trace).unwrap();
        assert!(text.contains("open folder"), "{text}");
        assert!(text.contains("20.0 ms  open folder  home"), "{text}");
        assert!(text.contains("UI thread blocked over 4 ms: 1"), "{text}");
        assert!(text.contains("9.0 ms  api.stat"), "{text}");
        assert!(text.contains("over 16.7 ms: 1"), "{text}");
    }
}
