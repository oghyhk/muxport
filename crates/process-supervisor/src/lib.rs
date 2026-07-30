use std::collections::VecDeque;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::{Child, Command};
use tracing::{info};

#[derive(Error, Debug)]
pub enum SupervisorError {
    #[error("Failed to spawn child process: {0}")]
    SpawnFailed(String),
    #[error("Crash loop detected: {0} failures in past 10 minutes")]
    CrashLoopDetected(usize),
    #[error("Child process not running")]
    NotRunning,
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
            let _ = child.kill().await;
        }
        Ok(())
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
}
