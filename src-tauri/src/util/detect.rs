use std::process::Command;

pub fn is_tool_installed(tool_name: &str) -> bool {
    Command::new("which")
        .arg(tool_name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn get_tool_path(tool_name: &str) -> Option<String> {
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
    fn test_get_tool_path_existing() {
        let path = get_tool_path("echo");
        assert!(path.is_some());
        assert!(path.unwrap().contains("echo"));
    }

    #[test]
    fn test_get_tool_path_missing() {
        assert!(get_tool_path("nonexistent_tool_xyz_123").is_none());
    }
}
