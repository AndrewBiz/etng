//! `etng` command line interface.

mod args;
mod fallback;
mod walk;

use std::collections::HashSet;
use std::io::{self, BufWriter, Write};

use etng_core::compat::{EXIFTOOL_COMPAT_VERSION, REPORTED_EXIFTOOL_VERSION};
use etng_core::json::Emitter;
use etng_core::{FallbackMode, FileReport, ReadOptions, SystemFields, Tag, Value, read_file};
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
    } else if cli.opts.binary {
        for batch in files.chunks(BATCH_SIZE) {
            let reports: Vec<FileReport> = batch
                .par_iter()
                .map(|path| read_file(path, &opts))
                .collect();
            for mut report in reports {
                process(&mut report, &mut fallback, &opts, &mut exit_code);
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
    if opts.binary && forced && !opts.quiet {
        eprintln!(
            "etng: {}: fallback disabled in binary mode",
            report.source.display()
        );
    }
    let needs_fallback = (forced || report.error.is_some()) && !opts.binary;
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

fn write_binary(out: &mut impl Write, report: &FileReport, opts: &ReadOptions) -> io::Result<()> {
    if report.error.is_some() {
        return Ok(());
    }
    let system = system_entries(&report.system, opts.numeric, opts.binary);
    if opts.selection.is_empty() {
        out.write_all(REPORTED_EXIFTOOL_VERSION.to_string().as_bytes())?;
        let mut seen = HashSet::new();
        for (group0, group1, name, value) in &system {
            if seen.insert(format!("{group0}:{group1}:{name}")) {
                out.write_all(value.as_bytes())?;
            }
        }
        for tag in &report.tags {
            let key = field_key(opts, tag.group0, tag.group1, tag.name);
            if seen.insert(key) {
                write_tag_value(out, tag)?;
            }
        }
        return Ok(());
    }

    for selection in &opts.selection {
        if is_version_selection(selection) {
            out.write_all(REPORTED_EXIFTOOL_VERSION.to_string().as_bytes())?;
            continue;
        }
        if let Some((_, _, _, value)) = system
            .iter()
            .find(|(group0, group1, name, _)| selection_matches(selection, group0, group1, name))
        {
            out.write_all(value.as_bytes())?;
            continue;
        }
        if let Some(tag) = report
            .tags
            .iter()
            .find(|tag| selection_matches(selection, tag.group0, tag.group1, tag.name))
        {
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

fn field_key(opts: &ReadOptions, group0: &str, group1: &str, name: &str) -> String {
    match opts.group {
        etng_core::GroupMode::None => name.to_string(),
        etng_core::GroupMode::G0 => format!("{group0}:{name}"),
        etng_core::GroupMode::G1 => format!("{group1}:{name}"),
    }
}

fn system_entries(
    system: &SystemFields,
    numeric: bool,
    binary: bool,
) -> Vec<(&'static str, &'static str, &'static str, String)> {
    let file_size = if numeric || binary {
        system.file_size.to_string()
    } else {
        etng_core::formatting::human_size(system.file_size)
    };
    let permissions = if binary {
        format!("{:o}", system.file_mode)
    } else if numeric {
        system.file_mode.to_string()
    } else {
        system.file_permissions.clone()
    };
    let file_type_extension = if binary {
        system.file_type_extension.to_ascii_uppercase()
    } else {
        system.file_type_extension.clone()
    };
    vec![
        ("System", "System", "FileName", system.file_name.clone()),
        ("System", "System", "Directory", system.directory.clone()),
        ("System", "System", "FileSize", file_size),
        (
            "System",
            "System",
            "FileModifyDate",
            system.file_modify_date.clone(),
        ),
        (
            "System",
            "System",
            "FileAccessDate",
            system.file_access_date.clone(),
        ),
        (
            "System",
            "System",
            "FileInodeChangeDate",
            system.file_inode_change_date.clone(),
        ),
        ("System", "System", "FilePermissions", permissions),
        ("File", "File", "FileType", system.file_type.clone()),
        ("File", "File", "FileTypeExtension", file_type_extension),
        ("File", "File", "MIMEType", system.mime_type.clone()),
    ]
}
