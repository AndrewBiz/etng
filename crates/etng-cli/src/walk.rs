//! Input expansion: files, directories, glob patterns.

use std::path::{Path, PathBuf};

pub fn expand(
    inputs: &[String],
    recursive: bool,
    extensions: &[String],
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for input in inputs {
        if input.contains(['*', '?', '[']) {
            let matches = glob::glob(input).map_err(|err| format!("{input}: {err}"))?;
            for entry in matches {
                match entry {
                    Ok(path) => push_path(&path, &mut files, extensions),
                    Err(err) => return Err(err.to_string()),
                }
            }
            continue;
        }
        let path = PathBuf::from(input);
        if !path.exists() {
            return Err(format!("{input}: file not found"));
        }
        if path.is_dir() {
            collect_dir(&path, recursive, extensions, &mut files)?;
        } else if extension_allowed(&path, extensions) {
            files.push(path);
        }
    }
    Ok(files)
}

fn push_path(path: &Path, files: &mut Vec<PathBuf>, extensions: &[String]) {
    if path.is_dir() {
        let _ = collect_dir(path, false, extensions, files);
    } else if extension_allowed(path, extensions) {
        files.push(path.to_path_buf());
    }
}

fn collect_dir(
    dir: &Path,
    recursive: bool,
    extensions: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut children: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| err.to_string())?;
        children.push(entry.path());
    }
    children.sort();
    for child in children {
        if child.is_dir() {
            if recursive {
                collect_dir(&child, recursive, extensions, files)?;
            }
        } else if extension_allowed(&child, extensions) {
            files.push(child);
        }
    }
    Ok(())
}

fn extension_allowed(path: &Path, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    extensions.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn expands_directories_and_extension_filters() {
        let dir = std::env::temp_dir().join(format!("etng-walk-{}", std::process::id()));
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("a.JPG"), b"x").unwrap();
        fs::write(dir.join("b.heic"), b"x").unwrap();
        fs::write(dir.join("nested/c.jpg"), b"x").unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();

        let flat = expand(&[dir.display().to_string()], false, &[]).unwrap();
        assert_eq!(flat.len(), 3);

        let filtered = expand(&[dir.display().to_string()], true, &["jpg".to_string()]).unwrap();
        assert_eq!(filtered.len(), 2);
        assert!(
            filtered
                .iter()
                .all(|p| p.extension().unwrap() == "JPG" || p.extension().unwrap() == "jpg")
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_input_is_an_error() {
        assert!(expand(&["/definitely/missing".to_string()], false, &[]).is_err());
    }
}
