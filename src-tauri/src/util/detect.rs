use std::path::Path;
use std::process::Command;

/// Common macOS paths not included in the default app PATH
const EXTRA_PATHS: &[&str] = &[
    "/usr/local/bin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
];

/// Check if a tool is installed, searching both PATH and common macOS locations
pub fn is_tool_installed(tool_name: &str) -> bool {
    find_tool(tool_name).is_some()
}

/// Find the full path to a tool, checking PATH and common macOS locations
pub fn find_tool(tool_name: &str) -> Option<String> {
    // If it's already an absolute path, just check existence
    if tool_name.starts_with('/') {
        return if Path::new(tool_name).exists() {
            Some(tool_name.to_string())
        } else {
            None
        };
    }

    // Try `which` first (works when PATH is set correctly, e.g. in dev mode)
    if let Some(path) = get_tool_path_via_which(tool_name) {
        return Some(path);
    }

    // Fall back to checking common macOS install locations
    for dir in EXTRA_PATHS {
        let full_path = format!("{}/{}", dir, tool_name);
        if Path::new(&full_path).exists() {
            return Some(full_path);
        }
    }

    None
}

fn get_tool_path_via_which(tool_name: &str) -> Option<String> {
    Command::new("which")
        .arg(tool_name)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_existing_tool() {
        assert!(is_tool_installed("echo"));
    }

    #[test]
    fn test_detect_missing_tool() {
        assert!(!is_tool_installed("nonexistent_tool_xyz_123"));
    }

    #[test]
    fn test_find_tool_existing() {
        let path = find_tool("echo");
        assert!(path.is_some());
        assert!(path.unwrap().contains("echo"));
    }

    #[test]
    fn test_find_tool_missing() {
        assert!(find_tool("nonexistent_tool_xyz_123").is_none());
    }

    #[test]
    fn test_find_tool_absolute_path() {
        assert!(find_tool("/bin/ls").is_some());
        assert!(find_tool("/nonexistent/path/tool").is_none());
    }
}
