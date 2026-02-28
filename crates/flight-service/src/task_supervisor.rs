// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Task Supervisor
//!
//! Manages the lifecycle of async tasks including state tracking,
//! failure recording, and restart policy enforcement.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// State of a supervised task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// Metadata about a supervised task.
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub state: TaskState,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub restart_count: u32,
}

/// Supervises a set of named tasks with restart policies.
pub struct TaskSupervisor {
    tasks: HashMap<String, TaskInfo>,
    max_restarts: u32,
    restart_delay_ms: u64,
}

impl TaskSupervisor {
    #[must_use]
    pub fn new(max_restarts: u32, restart_delay_ms: u64) -> Self {
        Self {
            tasks: HashMap::new(),
            max_restarts,
            restart_delay_ms,
        }
    }

    /// Register a new task in `Pending` state.
    pub fn register_task(&mut self, id: &str, name: &str) {
        self.tasks.insert(
            id.to_string(),
            TaskInfo {
                id: id.to_string(),
                name: name.to_string(),
                state: TaskState::Pending,
                started_at: None,
                completed_at: None,
                restart_count: 0,
            },
        );
    }

    /// Transition a task to `Running`.
    pub fn start_task(&mut self, id: &str) -> bool {
        if let Some(task) = self.tasks.get_mut(id) {
            task.state = TaskState::Running;
            task.started_at = Some(epoch_ms());
            task.completed_at = None;
            true
        } else {
            false
        }
    }

    /// Transition a task to `Completed`.
    pub fn complete_task(&mut self, id: &str) -> bool {
        if let Some(task) = self.tasks.get_mut(id) {
            task.state = TaskState::Completed;
            task.completed_at = Some(epoch_ms());
            true
        } else {
            false
        }
    }

    /// Transition a task to `Failed` and increment its restart counter.
    pub fn fail_task(&mut self, id: &str, error: &str) -> bool {
        if let Some(task) = self.tasks.get_mut(id) {
            task.state = TaskState::Failed(error.to_string());
            task.completed_at = Some(epoch_ms());
            task.restart_count += 1;
            true
        } else {
            false
        }
    }

    /// Transition a task to `Cancelled`.
    pub fn cancel_task(&mut self, id: &str) -> bool {
        if let Some(task) = self.tasks.get_mut(id) {
            task.state = TaskState::Cancelled;
            task.completed_at = Some(epoch_ms());
            true
        } else {
            false
        }
    }

    /// Whether the task should be restarted based on the restart policy.
    #[must_use]
    pub fn should_restart(&self, id: &str) -> bool {
        self.tasks.get(id).is_some_and(|t| {
            matches!(t.state, TaskState::Failed(_)) && t.restart_count <= self.max_restarts
        })
    }

    /// The configured restart delay in milliseconds.
    #[must_use]
    pub fn restart_delay_ms(&self) -> u64 {
        self.restart_delay_ms
    }

    /// Tasks currently in `Running` state.
    #[must_use]
    pub fn active_tasks(&self) -> Vec<&TaskInfo> {
        self.tasks
            .values()
            .filter(|t| t.state == TaskState::Running)
            .collect()
    }

    /// Tasks in `Failed` state.
    #[must_use]
    pub fn failed_tasks(&self) -> Vec<&TaskInfo> {
        self.tasks
            .values()
            .filter(|t| matches!(t.state, TaskState::Failed(_)))
            .collect()
    }

    /// Look up a task by id.
    #[must_use]
    pub fn get_task(&self, id: &str) -> Option<&TaskInfo> {
        self.tasks.get(id)
    }

    /// Total number of registered tasks.
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let mut sup = TaskSupervisor::new(3, 1000);
        sup.register_task("t1", "axis-engine");
        let task = sup.get_task("t1").unwrap();
        assert_eq!(task.name, "axis-engine");
        assert_eq!(task.state, TaskState::Pending);
    }

    #[test]
    fn start_sets_running() {
        let mut sup = TaskSupervisor::new(3, 1000);
        sup.register_task("t1", "bus");
        assert!(sup.start_task("t1"));
        assert_eq!(sup.get_task("t1").unwrap().state, TaskState::Running);
        assert!(sup.get_task("t1").unwrap().started_at.is_some());
    }

    #[test]
    fn complete_sets_completed() {
        let mut sup = TaskSupervisor::new(3, 1000);
        sup.register_task("t1", "bus");
        sup.start_task("t1");
        assert!(sup.complete_task("t1"));
        assert_eq!(sup.get_task("t1").unwrap().state, TaskState::Completed);
        assert!(sup.get_task("t1").unwrap().completed_at.is_some());
    }

    #[test]
    fn fail_records_error() {
        let mut sup = TaskSupervisor::new(3, 1000);
        sup.register_task("t1", "hid");
        sup.start_task("t1");
        assert!(sup.fail_task("t1", "device disconnected"));
        assert_eq!(
            sup.get_task("t1").unwrap().state,
            TaskState::Failed("device disconnected".to_string())
        );
        assert_eq!(sup.get_task("t1").unwrap().restart_count, 1);
    }

    #[test]
    fn cancel_sets_cancelled() {
        let mut sup = TaskSupervisor::new(3, 1000);
        sup.register_task("t1", "x");
        sup.start_task("t1");
        assert!(sup.cancel_task("t1"));
        assert_eq!(sup.get_task("t1").unwrap().state, TaskState::Cancelled);
    }

    #[test]
    fn should_restart_within_limit() {
        let mut sup = TaskSupervisor::new(3, 500);
        sup.register_task("t1", "x");
        sup.start_task("t1");
        sup.fail_task("t1", "err");
        assert!(sup.should_restart("t1")); // restart_count == 1, max == 3
    }

    #[test]
    fn should_not_restart_after_max() {
        let mut sup = TaskSupervisor::new(2, 500);
        sup.register_task("t1", "x");
        // Fail 3 times — restart_count reaches 3, exceeds max_restarts (2)
        for _ in 0..3 {
            sup.start_task("t1");
            sup.fail_task("t1", "err");
        }
        assert!(!sup.should_restart("t1"));
    }

    #[test]
    fn should_not_restart_completed() {
        let mut sup = TaskSupervisor::new(3, 500);
        sup.register_task("t1", "x");
        sup.start_task("t1");
        sup.complete_task("t1");
        assert!(!sup.should_restart("t1"));
    }

    #[test]
    fn active_tasks_filters_running() {
        let mut sup = TaskSupervisor::new(3, 500);
        sup.register_task("a", "A");
        sup.register_task("b", "B");
        sup.register_task("c", "C");
        sup.start_task("a");
        sup.start_task("b");
        // c stays Pending
        assert_eq!(sup.active_tasks().len(), 2);
    }

    #[test]
    fn failed_tasks_filters_failed() {
        let mut sup = TaskSupervisor::new(3, 500);
        sup.register_task("a", "A");
        sup.register_task("b", "B");
        sup.start_task("a");
        sup.fail_task("a", "boom");
        sup.start_task("b");
        assert_eq!(sup.failed_tasks().len(), 1);
        assert_eq!(sup.failed_tasks()[0].id, "a");
    }

    #[test]
    fn operations_on_unknown_id_return_false() {
        let mut sup = TaskSupervisor::new(3, 500);
        assert!(!sup.start_task("nope"));
        assert!(!sup.complete_task("nope"));
        assert!(!sup.fail_task("nope", "err"));
        assert!(!sup.cancel_task("nope"));
        assert!(sup.get_task("nope").is_none());
    }

    #[test]
    fn task_count() {
        let mut sup = TaskSupervisor::new(3, 500);
        assert_eq!(sup.task_count(), 0);
        sup.register_task("a", "A");
        sup.register_task("b", "B");
        assert_eq!(sup.task_count(), 2);
    }
}
