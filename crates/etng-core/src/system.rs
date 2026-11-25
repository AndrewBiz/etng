//! System-field names, ordering and values shared by CSV and raw output.

use crate::formatting::human_size;
use crate::model::SystemFields;

pub struct SystemField {
    pub group0: &'static str,
    pub group1: &'static str,
    pub name: &'static str,
    pub value: String,
}

/// Canonical system fields in ExifTool order (no `ExifToolVersion`).
pub const NAMES: [(&str, &str, &str); 10] = [
    ("System", "System", "FileName"),
    ("System", "System", "Directory"),
    ("System", "System", "FileSize"),
    ("System", "System", "FileModifyDate"),
    ("System", "System", "FileAccessDate"),
    ("System", "System", "FileInodeChangeDate"),
    ("System", "System", "FilePermissions"),
    ("File", "File", "FileType"),
    ("File", "File", "FileTypeExtension"),
    ("File", "File", "MIMEType"),
];

pub fn system_fields(system: &SystemFields, numeric: bool, binary: bool) -> Vec<SystemField> {
    let file_size = if numeric || binary {
        system.file_size.to_string()
    } else {
        human_size(system.file_size)
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
        SystemField {
            group0: "System",
            group1: "System",
            name: "FileName",
            value: system.file_name.clone(),
        },
        SystemField {
            group0: "System",
            group1: "System",
            name: "Directory",
            value: system.directory.clone(),
        },
        SystemField {
            group0: "System",
            group1: "System",
            name: "FileSize",
            value: file_size,
        },
        SystemField {
            group0: "System",
            group1: "System",
            name: "FileModifyDate",
            value: system.file_modify_date.clone(),
        },
        SystemField {
            group0: "System",
            group1: "System",
            name: "FileAccessDate",
            value: system.file_access_date.clone(),
        },
        SystemField {
            group0: "System",
            group1: "System",
            name: "FileInodeChangeDate",
            value: system.file_inode_change_date.clone(),
        },
        SystemField {
            group0: "System",
            group1: "System",
            name: "FilePermissions",
            value: permissions,
        },
        SystemField {
            group0: "File",
            group1: "File",
            name: "FileType",
            value: system.file_type.clone(),
        },
        SystemField {
            group0: "File",
            group1: "File",
            name: "FileTypeExtension",
            value: file_type_extension,
        },
        SystemField {
            group0: "File",
            group1: "File",
            name: "MIMEType",
            value: system.mime_type.clone(),
        },
    ]
}

/// Resolve a system field selection to its canonical name and groups.
pub fn canonical_system(selection: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let (group, name) = match selection.split_once(':') {
        Some((group, name)) => (Some(group), name),
        None => (None, selection),
    };
    NAMES.iter().find_map(|(group0, group1, canonical)| {
        let matches = canonical.eq_ignore_ascii_case(name)
            && group.is_none_or(|group| {
                group.eq_ignore_ascii_case(group0) || group.eq_ignore_ascii_case(group1)
            });
        matches.then_some((*group0, *group1, *canonical))
    })
}
