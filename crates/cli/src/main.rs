//! The `unhoard` command-line tool (F10): scans a folder or drive and prints the largest
//! items as a table or as JSON. It only reads: it never writes to disk and never opens a
//! network connection. Paths in the user's profile are printed as `%USERPROFILE%`.
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
    let Command::Scan {
        path,
        json,
        top,
        depth,
        logical,
    } = cli.command;
    let absolute = std::path::absolute(&path).with_context(|| format!("invalid path {path}"))?;
    let root = absolute
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let roots = system_roots();
    let profile = roots.profile().map(str::to_string);
    let rules =
        Rules::builtin(&roots).context("the known-locations library could not be loaded")?;
    let ctx = ScanContext {
        rules,
        roots,
        now: WindowsFs.now(),
        knowledge: Default::default(),
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
    let volume = WindowsFs.volume(&root).ok();
    let opts = ReportOptions {
        top: usize::from(top),
        depth: usize::from(depth),
        logical,
        profile,
    };
    let report = scan_report(
        &tree,
        volume.as_ref(),
        outcome.partial,
        started.elapsed().as_millis() as u64,
        &ctx,
        &opts,
    );
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
