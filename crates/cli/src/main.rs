//! The `unhoard` command-line tool (F10): scans a folder or drive and prints the largest
//! items, the suggestions or the difference between two saved scans, as a table or as JSON.
//! It only reads: it never writes to disk and never opens a network connection. Paths in the
//! user's profile are printed as `%USERPROFILE%`.
#![forbid(unsafe_code)]

use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use unhoard_core::fs::FileSystem;
use unhoard_core::report::{ItemOut, Report, ReportOptions, redact, scan_report};
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_platform::WindowsFs;
use unhoard_platform::system::system_roots;

/// Command-line arguments.
#[derive(Parser)]
#[command(name = "unhoard", about = "See where your disk space went.", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The commands.
#[derive(Subcommand)]
enum Command {
    /// Scan a folder or drive and list its largest items.
    Scan {
        /// The folder or drive to scan, for example C:\ or .
        path: String,
        /// Print JSON (the documented schema) instead of a table.
        #[arg(long)]
        json: bool,
        /// The largest items per level.
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=1000))]
        top: u16,
        /// How many levels below the root.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=8))]
        depth: u8,
        /// Sort by logical size instead of size on disk.
        #[arg(long)]
        logical: bool,
    },
    /// Scan a folder or drive and list what can be cleaned up, with tier and reason.
    Suggestions {
        /// The folder or drive to scan.
        path: String,
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// The most suggestions listed.
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=2000))]
        limit: u16,
    },
    /// Compare two saved `scan --json` reports: what grew or shrank.
    Diff {
        /// The older report.
        old: String,
        /// The newer report.
        new: String,
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Leave out changes smaller than this many megabytes.
        #[arg(long, default_value_t = 100)]
        min_mb: u64,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let profile = std::env::var("USERPROFILE").ok();
            eprintln!("unhoard: {}", redact(&format!("{e:#}"), profile.as_deref()));
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Scan {
            path,
            json,
            top,
            depth,
            logical,
        } => run_scan(&path, json, top, depth, logical),
        Command::Suggestions { path, json, limit } => run_suggestions(&path, json, limit),
        Command::Diff {
            old,
            new,
            json,
            min_mb,
        } => run_diff(&old, &new, json, min_mb),
    }
}

/// Lists the suggestions for a scanned folder or drive, with what Unhoard knows about the
/// installed programs, Steam and Ollama.
fn run_suggestions(path: &str, json: bool, limit: u16) -> Result<()> {
    let (tree, ctx, _, _, _) = scan_path(path, true)?;
    let profile = ctx.roots.profile().map(str::to_string);
    let list = unhoard_core::view::suggestions(&tree, &ctx, usize::from(limit));
    let rows: Vec<serde_json::Value> = list
        .iter()
        .map(|s| {
            serde_json::json!({
                "path": redact(&s.path, profile.as_deref()),
                "onDiskBytes": s.node.on_disk,
                "tier": s.tier,
                "ruleId": s.description.rule_id,
                "actions": s.description.actions,
                "duplicateOf": s.description.duplicate_of,
            })
        })
        .collect();
    let mut out = std::io::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &rows).context("writing JSON")?;
        writeln!(out)?;
    } else {
        for (s, row) in list.iter().zip(&rows) {
            writeln!(
                out,
                "{:>10}  {:<11}  {}  [{}]",
                human(Some(s.node.on_disk)),
                format!("{:?}", s.tier),
                row["path"].as_str().unwrap_or(""),
                s.description.rule_id.as_deref().unwrap_or("-")
            )?;
        }
    }
    Ok(())
}

/// Prints what grew or shrank between two saved `scan --json` reports.
fn run_diff(old: &str, new: &str, json: bool, min_mb: u64) -> Result<()> {
    let read = |p: &str| -> Result<serde_json::Value> {
        let text = std::fs::read_to_string(p).with_context(|| format!("cannot read {p}"))?;
        serde_json::from_str(&text).with_context(|| format!("{p} is not a scan --json report"))
    };
    let min = i64::try_from(min_mb.saturating_mul(1 << 20)).unwrap_or(i64::MAX);
    let changes: Vec<(String, i64)> = unhoard_core::report::diff_reports(&read(old)?, &read(new)?)
        .into_iter()
        .filter(|(_, d)| d.abs() >= min)
        .collect();
    let mut out = std::io::stdout().lock();
    if json {
        let rows: Vec<serde_json::Value> = changes
            .iter()
            .map(|(p, d)| serde_json::json!({"path": p, "deltaBytes": d}))
            .collect();
        serde_json::to_writer_pretty(&mut out, &rows).context("writing JSON")?;
        writeln!(out)?;
    } else {
        for (p, d) in &changes {
            let sign = if *d < 0 { "-" } else { "+" };
            writeln!(out, "{sign}{:>10}  {p}", human(Some(d.unsigned_abs())))?;
        }
    }
    Ok(())
}

/// Scans `path`, with the knowledge about this PC when `knowledge` is set; returns the
/// tree, the context, whether the scan was partial, how long it took and the root.
fn scan_path(
    path: &str,
    knowledge: bool,
) -> Result<(unhoard_core::tree::Tree, ScanContext, bool, u64, String)> {
    let absolute = std::path::absolute(path).with_context(|| format!("invalid path {path}"))?;
    let root = absolute
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let roots = system_roots();
    let rules =
        Rules::builtin(&roots).context("the known-locations library could not be loaded")?;
    let knowledge = if knowledge {
        unhoard_platform::knowledge::gather(&roots)
    } else {
        Default::default()
    };
    let ctx = ScanContext {
        rules,
        roots,
        now: WindowsFs.now(),
        knowledge,
    };
    let control = ScanControl::default();
    let started = Instant::now();
    let done = std::sync::atomic::AtomicBool::new(false);
    let result = std::thread::scope(|s| {
        if std::io::stderr().is_terminal() {
            s.spawn(|| {
                while !done.load(Ordering::Relaxed) {
                    eprint!(
                        "\rScanning… {} files",
                        control.files.load(Ordering::Relaxed)
                    );
                    std::thread::sleep(Duration::from_millis(250));
                }
                eprint!("\r                                        \r");
            });
        }
        let r = scan(&WindowsFs, &root, &ctx, &control);
        done.store(true, Ordering::Relaxed);
        r
    });
    let (tree, outcome) = result.map_err(|e| anyhow::anyhow!("cannot read {root}: {e}"))?;
    Ok((
        tree,
        ctx,
        outcome.partial,
        started.elapsed().as_millis() as u64,
        root,
    ))
}

/// Scans a folder or drive and prints its largest items.
fn run_scan(path: &str, json: bool, top: u16, depth: u8, logical: bool) -> Result<()> {
    let (tree, ctx, partial, millis, root) = scan_path(path, false)?;
    let profile = ctx.roots.profile().map(str::to_string);
    let volume = WindowsFs.volume(&root).ok();
    let opts = ReportOptions {
        top: usize::from(top),
        depth: usize::from(depth),
        logical,
        profile,
    };
    let report = scan_report(&tree, volume.as_ref(), partial, millis, &ctx, &opts);
    let mut out = std::io::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &report).context("writing JSON")?;
        writeln!(out)?;
    } else {
        print_table(&mut out, &report, logical)?;
    }
    Ok(())
}

/// Formats bytes in binary units labelled the way Explorer labels them.
fn human(bytes: Option<u64>) -> String {
    let Some(b) = bytes else {
        return "unknown".into();
    };
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", units[i])
    } else {
        format!("{v:.1} {}", units[i])
    }
}

fn print_items(
    out: &mut impl Write,
    items: &[ItemOut],
    total: u64,
    logical: bool,
    indent: usize,
) -> std::io::Result<()> {
    for it in items {
        let size = if logical {
            it.logical_bytes
        } else {
            it.on_disk_bytes
        };
        let share = size
            .map(|s| {
                if total > 0 {
                    format!("{:5.1}%", s as f64 * 100.0 / total as f64)
                } else {
                    "    –".into()
                }
            })
            .unwrap_or_else(|| "    ?".into());
        let known = it
            .known_location
            .as_ref()
            .map(|k| format!("  [{}]", k.id))
            .unwrap_or_default();
        writeln!(
            out,
            "{:>10}  {share}  {}{}{known}",
            human(size),
            "  ".repeat(indent),
            it.path
        )?;
        print_items(out, &it.children, total, logical, indent + 1)?;
    }
    Ok(())
}

fn print_table(out: &mut impl Write, r: &Report, logical: bool) -> std::io::Result<()> {
    let total = if logical {
        r.totals.logical_bytes
    } else {
        r.totals.on_disk_bytes
    };
    writeln!(
        out,
        "{}  {}  {} files{}",
        r.root,
        human(Some(total)),
        r.totals.files,
        if r.partial { "  (partial)" } else { "" }
    )?;
    print_items(out, &r.items, total, logical, 0)?;
    if let Some(h) = r.hidden_bytes {
        writeln!(
            out,
            "{:>10}          hidden space (used, but not seen by the scan)",
            human(Some(h))
        )?;
    }
    if !r.unscanned.is_empty() {
        writeln!(
            out,
            "{} folder(s) could not be read; use --json for the list.",
            r.unscanned.len()
        )?;
    }
    Ok(())
}
