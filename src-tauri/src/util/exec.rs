use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug)]
pub enum ExecError {
    Timeout,
    IoError(String),
    NonZeroExit { code: Option<i32>, stderr: String },
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::Timeout => write!(f, "Command timed out"),
            ExecError::IoError(e) => write!(f, "IO error: {}", e),
            ExecError::NonZeroExit { code, stderr } => {
                write!(f, "Exit code {:?}: {}", code, stderr)
            }
        }
    }
}

pub async fn exec_command(
    program: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<String, ExecError> {
    let duration = Duration::from_secs(timeout_secs);

    let future = Command::new(program)
        .args(args)
        .output();

    let output = timeout(duration, future)
        .await
        .map_err(|_| ExecError::Timeout)?
        .map_err(|e| ExecError::IoError(e.to_string()))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| ExecError::IoError(e.to_string()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(ExecError::NonZeroExit {
            code: output.status.code(),
            stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exec_command_success() {
        let result = exec_command("echo", &["hello"], 10).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[tokio::test]
    async fn test_exec_command_nonzero_exit() {
        let result = exec_command("false", &[], 10).await;
        assert!(matches!(result, Err(ExecError::NonZeroExit { .. })));
    }

    #[tokio::test]
    async fn test_exec_command_timeout() {
        let result = exec_command("sleep", &["30"], 1).await;
        assert!(matches!(result, Err(ExecError::Timeout)));
    }

    #[tokio::test]
    async fn test_exec_command_not_found() {
        let result = exec_command("nonexistent_binary_xyz", &[], 10).await;
        assert!(matches!(result, Err(ExecError::IoError(_))));
    }
}
