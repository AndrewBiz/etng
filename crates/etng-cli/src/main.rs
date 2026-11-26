//! `etng` command line interface.

mod args;
mod fallback;
mod walk;

use std::collections::{BTreeSet, HashSet};
use std::io::{self, BufWriter, Write};

use etng_core::compat::{EXIFTOOL_COMPAT_VERSION, REPORTED_EXIFTOOL_VERSION};
use etng_core::json::Emitter;
use etng_core::system::{canonical_system, system_fields};
use etng_core::{FallbackMode, FileReport, ReadOptions, Tag, Value, csv, read_file};
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

    if !cli.writes.is_empty() {
        let mut failed = false;
        for path in &files {
            if let Err(err) = etng_core::write::write(path, &cli.writes) {
                eprintln!("etng: {}: {err}", path.display());
                failed = true;
            }
        }
        return if failed { 1 } else { 0 };
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

    if cli.opts.csv {
        let columns = if opts.selection.is_empty() {
            collect_columns(&files, &opts, &mut fallback)
        } else {
            selected_columns(&opts)
        };
        if let Err(err) = csv::write_header(&mut out, &columns) {
            eprintln!("etng: cannot write output: {err}");
            return 1;
        }
        for batch in files.chunks(BATCH_SIZE) {
            let reports: Vec<FileReport> = batch
                .par_iter()
                .map(|path| read_file(path, &opts))
                .collect();
            for mut report in reports {
                process(&mut report, &mut fallback, &opts, &mut exit_code, true);
                if let Err(err) = csv::write_record(&mut out, &report, &columns, &opts) {
                    eprintln!("etng: cannot write output: {err}");
                    return 1;
                }
            }
        }
        if let Err(err) = out.flush() {
            eprintln!("etng: cannot flush output: {err}");
            return 1;
        }
        return exit_code;
    }

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
                process(&mut report, &mut fallback, &opts, &mut exit_code, true);
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
    } else if cli.opts.binary {
        for batch in files.chunks(BATCH_SIZE) {
            let reports: Vec<FileReport> = batch
                .par_iter()
                .map(|path| read_file(path, &opts))
                .collect();
            for mut report in reports {
                process(&mut report, &mut fallback, &opts, &mut exit_code, true);
                if let Err(err) = write_binary(&mut out, &report, &opts) {
                    eprintln!("etng: cannot write output: {err}");
                    return 1;
                }
            }
        }
    } else {
        for batch in files.chunks(BATCH_SIZE) {
            let reports: Vec<FileReport> = batch
                .par_iter()
                .map(|path| read_file(path, &opts))
                .collect();
            for mut report in reports {
                process(&mut report, &mut fallback, &opts, &mut exit_code, true);
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
    diagnostics: bool,
) {
    let forced = opts.fallback == FallbackMode::Always;
    if opts.binary && forced && !opts.quiet && diagnostics {
        eprintln!(
            "etng: {}: fallback disabled in binary mode",
            report.source.display()
        );
    }
    // CR3 is read natively (EXIF/GPS/QuickTime/previews), but nested Canon
    // tables are not complete yet. Enrich with exiftool in auto mode so JSON
    // keeps full parity; binary mode never delegates.
    let cr3_enrich =
        report.system.file_type == "CR3" && opts.fallback == FallbackMode::Auto && !opts.binary;
    let needs_fallback = (forced || report.error.is_some() || cr3_enrich) && !opts.binary;
    if needs_fallback && opts.fallback != FallbackMode::Never {
        let reason = if forced {
            "forced".to_string()
        } else if cr3_enrich {
            "CR3 nested Canon metadata".to_string()
        } else {
            report.error.clone().unwrap_or_default()
        };
        match fallback.delegate(&report.source, opts) {
            Ok(delegate) => {
                if forced || cr3_enrich {
                    // For CR3 JSON the delegate is authoritative until nested
                    // Canon tables are implemented natively; binary mode never
                    // reaches this branch and uses the native payloads.
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
                if !opts.quiet && diagnostics && !cr3_enrich {
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
        if diagnostics {
            eprintln!("etng: {}: {error}", report.source.display());
        }
    }
    for warning in &report.warnings {
        *exit_code = 1;
        if !opts.quiet && diagnostics {
            eprintln!("etng: {}: {warning}", report.source.display());
        }
    }
}

fn collect_columns(
    files: &[std::path::PathBuf],
    opts: &ReadOptions,
    fallback: &mut fallback::Fallback,
) -> Vec<String> {
    let mut keys: BTreeSet<String> = BTreeSet::new();
    let mut ignored = 0;
    for batch in files.chunks(BATCH_SIZE) {
        let reports: Vec<FileReport> = batch.par_iter().map(|path| read_file(path, opts)).collect();
        for mut report in reports {
            process(&mut report, fallback, opts, &mut ignored, false);
            for field in system_fields(&report.system, opts.numeric, opts.binary) {
                if opts.wants(field.group0, field.group1, field.name) {
                    keys.insert(csv::key(opts, field.group0, field.group1, field.name));
                }
            }
            for tag in &report.tags {
                if opts.wants(tag.group0, tag.group1, tag.name) {
                    keys.insert(csv::key(opts, tag.group0, tag.group1, tag.name));
                }
            }
        }
    }
    keys.into_iter().collect()
}

fn selected_columns(opts: &ReadOptions) -> Vec<String> {
    let mut columns = Vec::new();
    for selection in &opts.selection {
        let key = if is_version_selection(selection) {
            if opts.is_excluded("ExifTool", "ExifTool", "ExifToolVersion") {
                continue;
            }
            csv::key(opts, "ExifTool", "ExifTool", "ExifToolVersion")
        } else if let Some((group0, group1, name)) = etng_tags::tables()
            .canonical(selection)
            .or_else(|| canonical_system(selection))
        {
            if opts.is_excluded(group0, group1, name) {
                continue;
            }
            csv::key(opts, group0, group1, name)
        } else {
            selection.clone()
        };
        if !columns.contains(&key) {
            columns.push(key);
        }
    }
    columns
}

fn write_text(out: &mut impl Write, report: &FileReport, opts: &ReadOptions) -> io::Result<()> {
    if let Some(error) = &report.error {
        writeln!(out, "======= {}: {error}", report.source.display())?;
        return Ok(());
    }
    writeln!(out, "======== {}", report.source.display())?;
    for tag in &report.tags {
        if !opts.wants(tag.group0, tag.group1, tag.name) {
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

fn write_binary(out: &mut impl Write, report: &FileReport, opts: &ReadOptions) -> io::Result<()> {
    if report.error.is_some() {
        return Ok(());
    }
    let system = system_fields(&report.system, opts.numeric, opts.binary);
    if opts.selection.is_empty() {
        if !opts.is_excluded("ExifTool", "ExifTool", "ExifToolVersion") {
            out.write_all(REPORTED_EXIFTOOL_VERSION.to_string().as_bytes())?;
        }
        let mut seen = HashSet::new();
        for field in &system {
            if opts.wants(field.group0, field.group1, field.name)
                && seen.insert(csv::key(opts, field.group0, field.group1, field.name))
            {
                out.write_all(field.value.as_bytes())?;
            }
        }
        for tag in &report.tags {
            if opts.wants(tag.group0, tag.group1, tag.name)
                && seen.insert(csv::key(opts, tag.group0, tag.group1, tag.name))
            {
                write_tag_value(out, tag)?;
            }
        }
        return Ok(());
    }

    for selection in &opts.selection {
        if is_version_selection(selection) {
            if !opts.is_excluded("ExifTool", "ExifTool", "ExifToolVersion") {
                out.write_all(REPORTED_EXIFTOOL_VERSION.to_string().as_bytes())?;
            }
            continue;
        }
        if let Some(field) = system.iter().find(|field| {
            !opts.is_excluded(field.group0, field.group1, field.name)
                && selection_matches(selection, field.group0, field.group1, field.name)
        }) {
            out.write_all(field.value.as_bytes())?;
            continue;
        }
        if let Some(tag) = report.tags.iter().find(|tag| {
            !opts.is_excluded(tag.group0, tag.group1, tag.name)
                && selection_matches(selection, tag.group0, tag.group1, tag.name)
        }) {
            write_tag_value(out, tag)?;
        }
    }
    Ok(())
}

fn write_tag_value(out: &mut impl Write, tag: &Tag) -> io::Result<()> {
    match &tag.value {
        Value::Bytes(bytes) => out.write_all(bytes),
        value => out.write_all(value.display().as_bytes()),
    }
}

fn selection_matches(selection: &str, group0: &str, group1: &str, name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    match selection.split_once(':') {
        Some((group, tag)) => {
            tag == name
                && (group.eq_ignore_ascii_case(group0) || group.eq_ignore_ascii_case(group1))
        }
        None => selection == name,
    }
}

fn is_version_selection(selection: &str) -> bool {
    selection == "EXIFTOOLVERSION" || selection == "EXIFTOOL:EXIFTOOLVERSION"
}
