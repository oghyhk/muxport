use std::collections::VecDeque;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::{Child, Command};
use tracing::info;

#[derive(Error, Debug)]
pub enum SupervisorError {
    #[error("Failed to spawn child process: {0}")]
    SpawnFailed(String),
    #[error("Crash loop detected: {0} failures in past 10 minutes")]
    CrashLoopDetected(usize),
    #[error("Child process not running")]
    NotRunning,
    #[error("Child process is already running with PID {0}")]
    AlreadyRunning(u32),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Idle,
    Starting,
    Running(u32), // PID
    CrashLooping,
    Stopped,
}

pub struct ProcessSupervisor {
    command_path: String,
    args: Vec<String>,
    crash_history: VecDeque<Instant>,
    max_crashes_in_window: usize,
    window_duration: Duration,
    current_child: Option<Child>,
}

impl ProcessSupervisor {
    pub fn new(command_path: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command_path: command_path.into(),
            args,
            crash_history: VecDeque::new(),
            max_crashes_in_window: 5,
            window_duration: Duration::from_secs(600), // 10 minutes
            current_child: None,
        }
    }

    pub fn is_crash_looping(&mut self) -> bool {
        let now = Instant::now();
        while let Some(&time) = self.crash_history.front() {
            if now.duration_since(time) > self.window_duration {
                self.crash_history.pop_front();
            } else {
                break;
            }
        }
        self.crash_history.len() >= self.max_crashes_in_window
    }

    pub fn record_crash(&mut self) {
        self.crash_history.push_back(Instant::now());
    }

    pub async fn spawn(&mut self) -> Result<u32, SupervisorError> {
        if let Some(child) = self.current_child.as_mut() {
            match child.try_wait()? {
                None => {
                    return Err(SupervisorError::AlreadyRunning(
                        child.id().unwrap_or_default(),
                    ));
                }
                Some(status) => {
                    if !status.success() {
                        self.record_crash();
                    }
                    self.current_child = None;
                }
            }
        }
        if self.is_crash_looping() {
            return Err(SupervisorError::CrashLoopDetected(self.crash_history.len()));
        }

        let mut cmd = Command::new(&self.command_path);
        cmd.args(&self.args);
        let child = cmd.spawn().map_err(|e| SupervisorError::SpawnFailed(e.to_string()))?;
        let pid = child.id().ok_or_else(|| SupervisorError::SpawnFailed("No PID assigned".into()))?;

        self.current_child = Some(child);
        info!(pid = pid, command = %self.command_path, "Spawned supervised child process");
        Ok(pid)
    }

    pub async fn stop(&mut self) -> Result<(), SupervisorError> {
        if let Some(mut child) = self.current_child.take() {
            info!("Sending kill signal to child process");
            child.kill().await?;
        }
        Ok(())
    }

    pub fn compute_backoff_delay(retry_count: u32) -> Duration {
        let base_secs = 1u64.checked_shl(retry_count.min(6)).unwrap_or(64);
        let capped = base_secs.min(60);
        Duration::from_secs(capped)
    }

    pub async fn check_exit(
        &mut self,
    ) -> Result<Option<std::process::ExitStatus>, SupervisorError> {
        if let Some(ref mut child) = self.current_child {
            match child.try_wait()? {
                Some(status) => {
                    if !status.success() {
                        self.record_crash();
                    }
                    self.current_child = None;
                    Ok(Some(status))
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub async fn wait_for_exit(&mut self) -> Result<std::process::ExitStatus, SupervisorError> {
        let mut child = self
            .current_child
            .take()
            .ok_or(SupervisorError::NotRunning)?;
        let status = child.wait().await?;
        if !status.success() {
            self.record_crash();
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_loop_detection() {
        let mut supervisor = ProcessSupervisor::new("echo", vec![]);
        assert!(!supervisor.is_crash_looping());

        for _ in 0..5 {
            supervisor.record_crash();
        }
        assert!(supervisor.is_crash_looping());
    }

    #[test]
    fn test_exponential_backoff_capping() {
        assert_eq!(ProcessSupervisor::compute_backoff_delay(0), Duration::from_secs(1));
        assert_eq!(ProcessSupervisor::compute_backoff_delay(1), Duration::from_secs(2));
        assert_eq!(ProcessSupervisor::compute_backoff_delay(2), Duration::from_secs(4));
        assert_eq!(ProcessSupervisor::compute_backoff_delay(10), Duration::from_secs(60));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn duplicate_spawn_is_rejected_and_requested_stop_is_not_a_crash() {
        let mut supervisor =
            ProcessSupervisor::new("sh", vec!["-c".into(), "sleep 30".into()]);
        let pid = supervisor.spawn().await.unwrap();
        assert!(pid > 0);
        assert!(matches!(
            supervisor.spawn().await,
            Err(SupervisorError::AlreadyRunning(running_pid)) if running_pid == pid
        ));
        supervisor.stop().await.unwrap();
        assert!(supervisor.crash_history.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn only_unsuccessful_natural_exit_counts_as_a_crash() {
        let mut successful =
            ProcessSupervisor::new("sh", vec!["-c".into(), "exit 0".into()]);
        successful.spawn().await.unwrap();
        assert!(successful.wait_for_exit().await.unwrap().success());
        assert!(successful.crash_history.is_empty());

        let mut failed =
            ProcessSupervisor::new("sh", vec!["-c".into(), "exit 7".into()]);
        failed.spawn().await.unwrap();
        assert!(!failed.wait_for_exit().await.unwrap().success());
        assert_eq!(failed.crash_history.len(), 1);
    }
}
