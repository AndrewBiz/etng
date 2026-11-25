//! `etng` command line interface.

mod args;
mod fallback;
mod walk;

use std::io::{self, BufWriter, Write};

use etng_core::compat::EXIFTOOL_COMPAT_VERSION;
use etng_core::json::Emitter;
use etng_core::{FallbackMode, FileReport, ReadOptions, read_file};
use rayon::prelude::*;

const BATCH_SIZE: usize = 64;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = match args::parse(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("etng: {message}");
            eprintln!("Try 'etng --help' for usage.");
            return 2;
        }
    };
    if cli.show_help {
        print!("{}", args::HELP);
        return 0;
    }
    if cli.show_version {
        println!("{EXIFTOOL_COMPAT_VERSION}");
        return 0;
    }
    if cli.inputs.is_empty() {
        eprintln!("etng: no input files");
        eprintln!("Try 'etng --help' for usage.");
        return 2;
    }

    let files = match walk::expand(&cli.inputs, cli.recursive, &cli.extensions) {
        Ok(files) => files,
        Err(message) => {
            eprintln!("etng: {message}");
            return 2;
        }
    };
    if files.is_empty() {
        eprintln!("etng: no files matched");
        return 1;
    }

    if let Some(threads) = cli.threads {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global();
    }

    let opts = cli.opts.clone();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut fallback = fallback::Fallback::new();
    let mut exit_code = 0;

    if cli.json {
        let mut emitter = Emitter::new(&mut out, opts.clone());
        if let Err(err) = emitter.begin() {
            eprintln!("etng: cannot write output: {err}");
            return 1;
        }
        for batch in files.chunks(BATCH_SIZE) {
            let reports: Vec<FileReport> = batch
                .par_iter()
                .map(|path| read_file(path, &opts))
                .collect();
            for mut report in reports {
                process(&mut report, &mut fallback, &opts, &mut exit_code);
                if let Err(err) = emitter.record(&report) {
                    eprintln!("etng: cannot write output: {err}");
                    return 1;
                }
            }
        }
        if let Err(err) = emitter.finish() {
            eprintln!("etng: cannot write output: {err}");
            return 1;
        }
    } else {
        for batch in files.chunks(BATCH_SIZE) {
            let reports: Vec<FileReport> = batch
                .par_iter()
                .map(|path| read_file(path, &opts))
                .collect();
            for mut report in reports {
                process(&mut report, &mut fallback, &opts, &mut exit_code);
                if let Err(err) = write_text(&mut out, &report, &opts) {
                    eprintln!("etng: cannot write output: {err}");
                    return 1;
                }
            }
        }
    }

    if let Err(err) = out.flush() {
        eprintln!("etng: cannot flush output: {err}");
        return 1;
    }
    exit_code
}

fn process(
    report: &mut FileReport,
    fallback: &mut fallback::Fallback,
    opts: &ReadOptions,
    exit_code: &mut i32,
) {
    let forced = opts.fallback == FallbackMode::Always;
    let needs_fallback = forced || report.error.is_some();
    if needs_fallback && opts.fallback != FallbackMode::Never {
        let reason = if forced {
            "forced".to_string()
        } else {
            report.error.clone().unwrap_or_default()
        };
        match fallback.delegate(&report.source, opts) {
            Ok(delegate) => {
                if forced {
                    report.tags = delegate.tags;
                } else {
                    fallback::merge(report, delegate.clone());
                }
                if report.system.file_type.is_empty() || report.system.file_type == "Unknown" {
                    report.system.file_type = delegate.system.file_type;
                    report.system.file_type_extension = delegate.system.file_type_extension;
                    report.system.mime_type = delegate.system.mime_type;
                }
                report.error = None;
                if !opts.quiet {
                    eprintln!(
                        "etng: {}: delegated to exiftool ({reason})",
                        report.source.display()
                    );
                }
            }
            Err(message) => report.warnings.push(format!("fallback: {message}")),
        }
    }

    if let Some(error) = &report.error {
        *exit_code = 1;
        eprintln!("etng: {}: {error}", report.source.display());
    }
    for warning in &report.warnings {
        *exit_code = 1;
        if !opts.quiet {
            eprintln!("etng: {}: {warning}", report.source.display());
        }
    }
}

fn write_text(out: &mut impl Write, report: &FileReport, opts: &ReadOptions) -> io::Result<()> {
    if let Some(error) = &report.error {
        writeln!(out, "======= {}: {error}", report.source.display())?;
        return Ok(());
    }
    writeln!(out, "======== {}", report.source.display())?;
    for tag in &report.tags {
        if !opts.selects(tag.group0, tag.group1, tag.name) {
            continue;
        }
        let value = if opts.numeric {
            tag.value.display()
        } else {
            tag.print.clone().unwrap_or_else(|| tag.value.display())
        };
        writeln!(out, "{:<32}: {value}", tag.name)?;
    }
    Ok(())
}
