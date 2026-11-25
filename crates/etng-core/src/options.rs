//! Reader options shared by the library and the CLI.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupMode {
    /// No group prefix (plain ExifTool output).
    #[default]
    None,
    /// `-G`
    G0,
    /// `-G1`
    G1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FallbackMode {
    /// Delegate only unsupported formats and fatal parse failures.
    #[default]
    Auto,
    /// Never call the system exiftool.
    Never,
    /// Always delegate, for baseline comparisons.
    Always,
}

#[derive(Clone, Debug)]
pub struct ReadOptions {
    pub group: GroupMode,
    /// `-n`: machine values, no print conversion.
    pub numeric: bool,
    /// `-s`: accepted for compatibility; JSON keys are unchanged.
    pub short: bool,
    pub quiet: bool,
    /// `-b`: emit binary values raw (text) or as `base64:` strings (JSON).
    pub binary: bool,
    /// `-csv`: one row per file with a header.
    pub csv: bool,
    /// Selected tags, normalized to uppercase: `TAG` or `GROUP:TAG`.
    pub selection: Vec<String>,
    /// Excluded tags, normalized to uppercase: `TAG` or `GROUP:TAG`.
    pub exclude: Vec<String>,
    pub fallback: FallbackMode,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            group: GroupMode::None,
            numeric: false,
            short: false,
            quiet: false,
            binary: false,
            csv: false,
            selection: Vec::new(),
            exclude: Vec::new(),
            fallback: FallbackMode::Auto,
        }
    }
}

impl ReadOptions {
    pub fn selects(&self, group0: &str, group1: &str, name: &str) -> bool {
        if self.selection.is_empty() {
            return true;
        }
        let name = name.to_ascii_uppercase();
        self.selection.iter().any(|sel| match sel.split_once(':') {
            Some((group, tag)) => {
                tag == name
                    && (group.eq_ignore_ascii_case(group0) || group.eq_ignore_ascii_case(group1))
            }
            None => *sel == name,
        })
    }

    pub fn wants(&self, group0: &str, group1: &str, name: &str) -> bool {
        self.selects(group0, group1, name) && !self.is_excluded(group0, group1, name)
    }

    pub fn is_excluded(&self, group0: &str, group1: &str, name: &str) -> bool {
        if self.exclude.is_empty() {
            return false;
        }
        let name = name.to_ascii_uppercase();
        self.exclude.iter().any(|sel| match sel.split_once(':') {
            Some((group, tag)) => {
                tag == name
                    && (group.eq_ignore_ascii_case(group0) || group.eq_ignore_ascii_case(group1))
            }
            None => *sel == name,
        })
    }

    pub fn wants_system_fields(&self) -> bool {
        self.selection.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_matches_group_and_name() {
        let opts = ReadOptions {
            selection: vec!["IFD0:MAKE".to_string()],
            ..Default::default()
        };
        assert!(opts.selects("EXIF", "IFD0", "Make"));
        assert!(!opts.selects("EXIF", "IFD0", "Model"));
        assert!(!opts.selects("EXIF", "ExifIFD", "Make"));
    }

    #[test]
    fn empty_selection_selects_everything() {
        assert!(ReadOptions::default().selects("File", "File", "FileName"));
    }
}
