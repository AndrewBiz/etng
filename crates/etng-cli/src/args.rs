//! ExifTool-style argument parsing.

use etng_core::{FallbackMode, GroupMode, ReadOptions};

#[derive(Debug)]
pub struct Cli {
    pub json: bool,
    pub opts: ReadOptions,
    pub recursive: bool,
    pub extensions: Vec<String>,
    pub threads: Option<usize>,
    pub inputs: Vec<String>,
    pub show_help: bool,
    pub show_version: bool,
    pub list: bool,
    pub listw: bool,
    pub writes: Vec<(String, String)>,
    fallback_explicit: bool,
}

impl Cli {
    fn new() -> Self {
        Self {
            json: false,
            opts: ReadOptions::default(),
            recursive: false,
            extensions: Vec::new(),
            threads: None,
            inputs: Vec::new(),
            show_help: false,
            show_version: false,
            list: false,
            listw: false,
            writes: Vec::new(),
            fallback_explicit: false,
        }
    }
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli::new();
    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            cli.inputs.extend(iter);
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            cli.inputs.push(arg);
            continue;
        }
        match arg.as_str() {
            "-j" | "-json" | "--json" => cli.json = true,
            "-G" | "--group0" => cli.opts.group = GroupMode::G0,
            "-G1" | "--group1" => cli.opts.group = GroupMode::G1,
            "-s" => cli.opts.short = true,
            "-n" => cli.opts.numeric = true,
            "-q" => cli.opts.quiet = true,
            "-b" | "-binary" | "--binary" => cli.opts.binary = true,
            "-csv" | "--csv" => cli.opts.csv = true,
            "-x" | "--exclude" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{arg} requires a tag name"))?;
                cli.opts.exclude.push(normalize_exclude(&value));
            }
            "-r" => cli.recursive = true,
            "-overwrite_original" => {}
            "-ver" => cli.show_version = true,
            "-list" => cli.list = true,
            "-listw" => cli.listw = true,
            "-h" | "--help" => cli.show_help = true,
            "-ext" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "-ext requires an extension".to_string())?;
                cli.extensions.push(normalize_ext(&value));
            }
            "--threads" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--threads requires a number".to_string())?;
                cli.threads = Some(parse_threads(&value)?);
            }
            "--fallback" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--fallback requires auto|never|always".to_string())?;
                cli.opts.fallback = parse_fallback(&value)?;
                cli.fallback_explicit = true;
            }
            other => {
                if let Some(value) = other.strip_prefix("--threads=") {
                    cli.threads = Some(parse_threads(value)?);
                } else if let Some(value) = other.strip_prefix("--fallback=") {
                    cli.opts.fallback = parse_fallback(value)?;
                    cli.fallback_explicit = true;
                } else if let Some(value) = other.strip_prefix("-ext=") {
                    cli.extensions.push(normalize_ext(value));
                } else if let Some(value) = other.strip_prefix("--exclude=") {
                    cli.opts.exclude.push(normalize_exclude(value));
                } else if let Some(value) = other.strip_prefix("-x")
                    && !value.is_empty()
                {
                    cli.opts.exclude.push(normalize_exclude(value));
                } else if let Some((name, value)) = other.trim_start_matches('-').split_once('=')
                    && etng_core::write::is_writable(name)
                {
                    cli.writes.push((name.to_string(), value.to_string()));
                } else {
                    let selection = other.trim_start_matches('-').to_string();
                    if selection.is_empty() {
                        return Err(format!("unknown option {other}"));
                    }
                    if !tag_exists(&selection) {
                        return Err(format!("unknown option or tag {other}"));
                    }
                    cli.opts.selection.push(selection.to_ascii_uppercase());
                }
            }
        }
    }
    if !cli.fallback_explicit
        && let Ok(value) = std::env::var("ETNG_FALLBACK")
    {
        cli.opts.fallback = parse_fallback(&value)?;
    }
    Ok(cli)
}

fn normalize_ext(value: &str) -> String {
    value.trim_start_matches('.').to_ascii_lowercase()
}

fn normalize_exclude(value: &str) -> String {
    value.trim_start_matches('-').to_ascii_uppercase()
}

fn parse_threads(value: &str) -> Result<usize, String> {
    let threads: usize = value
        .parse()
        .map_err(|_| format!("invalid thread count: {value}"))?;
    if threads == 0 {
        return Err("--threads must be greater than zero".to_string());
    }
    Ok(threads)
}

fn parse_fallback(value: &str) -> Result<FallbackMode, String> {
    match value {
        "auto" => Ok(FallbackMode::Auto),
        "never" => Ok(FallbackMode::Never),
        "always" => Ok(FallbackMode::Always),
        other => Err(format!(
            "invalid fallback mode {other}: expected auto|never|always"
        )),
    }
}

fn tag_exists(selection: &str) -> bool {
    if selection.eq_ignore_ascii_case("EXIFTOOLVERSION")
        || selection.eq_ignore_ascii_case("EXIFTOOL:EXIFTOOLVERSION")
    {
        return true;
    }
    let tables = etng_tags::tables();
    match selection.split_once(':') {
        Some((group, name)) => tables.has_group_name(group, name),
        None => tables.has_name(selection),
    }
}

pub const HELP: &str = "\
etng — ExifTool нового поколения (чтение и запись дат)

Usage: etng [OPTIONS] FILE|DIR|PATTERN...

Options:
  -j, -json        JSON-вывод, совместимый с `exiftool -j`
  -G, -G1          префикс группы family 0 / family 1
  -s               принимать флаг exiftool (имена полей не меняются)
  -n               машинные значения без печатных конверсий
  -q               подавить некритичные предупреждения
  -b, -binary      сырые значения: байты в stdout, base64 в JSON
  -csv             CSV-таблица: SourceFile + колонки тегов
  -x TAG           исключить тег (повторяемый), -x GROUP:TAG
  -r               рекурсивный обход каталогов
  -ext EXT         фильтр по расширению (можно повторять)
  -TAG, -GROUP:TAG выбрать конкретные теги
  --threads N      число потоков (по умолчанию — число ядер)
  --fallback MODE  auto|never|always (по умолчанию auto)
  -TAG=VALUE       записать дату: DateTimeOriginal, CreateDate, ModifyDate,
                   FileModifyDate, AllDates (JPEG/TIFF/HEIC/CR3/MOV);
                   MWG: City, Country, State, Location, Keywords, Creator,
                   Copyright, CountryCode, WorldRegion, CodedCharacterSet,
                   ImageUniqueID, Collections=name|uri; GPSLatitude,
                   GPSLatitudeRef, GPSLongitude, GPSLongitudeRef, GPSAltitude,
                   GPSAltitudeRef (JPEG).
                   Keywords/Creator можно повторять
  -ver             версия совместимости с exiftool
  -h, --help       эта справка

Formats: JPEG, HEIC/HEIF, TIFF, DNG, CR2, NEF
Exit codes: 0 — успех, 1 — предупреждения/частичные ошибки, 2 — ошибка использования
";

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Result<Cli, String> {
        parse(values.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_json_groups_and_selection() {
        let cli = args(&["-j", "-G1", "-Make", "--threads", "4", "a.jpg"]).unwrap();
        assert!(cli.json);
        assert_eq!(cli.opts.group, GroupMode::G1);
        assert_eq!(cli.opts.selection, vec!["MAKE"]);
        assert_eq!(cli.threads, Some(4));
        assert_eq!(cli.inputs, vec!["a.jpg"]);
    }

    #[test]
    fn unknown_flag_is_an_error() {
        assert!(args(&["--definitely-unknown", "a.jpg"]).is_err());
    }

    #[test]
    fn zero_threads_is_an_error() {
        assert!(args(&["--threads", "0", "a.jpg"]).is_err());
    }

    #[test]
    fn parses_date_assignments() {
        let cli = args(&[
            "-DateTimeOriginal=2024:05:06 07:08:09",
            "-FileModifyDate=2024:05:06 07:08:09",
            "a.jpg",
        ])
        .unwrap();
        assert_eq!(cli.writes.len(), 2);
        assert_eq!(cli.writes[0].0, "DateTimeOriginal");
        assert_eq!(cli.inputs, vec!["a.jpg"]);
    }

    #[test]
    fn fallback_modes_parse() {
        let cli = args(&["--fallback=never", "a.jpg"]).unwrap();
        assert_eq!(cli.opts.fallback, FallbackMode::Never);
    }
}
