use chrono::Utc;

use crate::error::{Result, SeeError};
use crate::io::{read_json, write_json};
use crate::types::paths::TeamDir;
use crate::types::{TaskItem, TaskStatus};

/// File-backed task board operating on `tasklist.json`.
pub struct TaskBoard {
    team_dir: TeamDir,
}

impl TaskBoard {
    pub fn new(team_dir: TeamDir) -> Self {
        Self { team_dir }
    }

    /// Load all tasks from disk.
    fn load_tasks(&self) -> Result<Vec<TaskItem>> {
        let path = self.team_dir.tasklist();
        if !path.exists() {
            return Ok(Vec::new());
        }
        read_json(&path)
    }

    /// Save all tasks to disk.
    fn save_tasks(&self, tasks: &[TaskItem]) -> Result<()> {
        write_json(&self.team_dir.tasklist(), &tasks.to_vec())
    }

    /// Create a new task.
    pub fn create_task(
        &self,
        title: &str,
        description: &str,
        created_by: &str,
    ) -> Result<TaskItem> {
        let mut tasks = self.load_tasks()?;
        let now = Utc::now().to_rfc3339();

        let task = TaskItem {
            id: generate_task_id(),
            title: title.to_owned(),
            description: description.to_owned(),
            status: TaskStatus::Pending,
            assigned_to: None,
            depends_on: Vec::new(),
            result: None,
            created_by: created_by.to_owned(),
            created_at: now.clone(),
            updated_at: now,
        };

        tasks.push(task.clone());
        self.save_tasks(&tasks)?;
        Ok(task)
    }

    /// List tasks, optionally filtered by status.
    pub fn list_tasks(&self, status: Option<TaskStatus>) -> Result<Vec<TaskItem>> {
        let tasks = self.load_tasks()?;
        match status {
            Some(s) => Ok(tasks.into_iter().filter(|t| t.status == s).collect()),
            None => Ok(tasks),
        }
    }

    /// Claim a task (set status=claimed + assigned_to).
    pub fn claim_task(&self, task_id: &str, agent_id: &str) -> Result<TaskItem> {
        let mut tasks = self.load_tasks()?;
        let task = find_task_mut(&mut tasks, task_id)?;

        if task.status != TaskStatus::Pending {
            return Err(SeeError::Team {
                message: format!("task '{task_id}' is not pending (status: {:?})", task.status),
            });
        }

        task.status = TaskStatus::Claimed;
        task.assigned_to = Some(agent_id.to_owned());
        task.updated_at = Utc::now().to_rfc3339();

        let result = task.clone();
        self.save_tasks(&tasks)?;
        Ok(result)
    }

    /// Assign a task to an agent (without changing status).
    pub fn assign_task(&self, task_id: &str, agent_id: &str) -> Result<TaskItem> {
        let mut tasks = self.load_tasks()?;
        let task = find_task_mut(&mut tasks, task_id)?;

        task.assigned_to = Some(agent_id.to_owned());
        task.updated_at = Utc::now().to_rfc3339();

        let result = task.clone();
        self.save_tasks(&tasks)?;
        Ok(result)
    }

    /// Complete a task (set status=done).
    pub fn complete_task(
        &self,
        task_id: &str,
        agent_id: &str,
        result_text: &str,
    ) -> Result<TaskItem> {
        let mut tasks = self.load_tasks()?;
        let task = find_task_mut(&mut tasks, task_id)?;

        if task.assigned_to.as_deref() != Some(agent_id) {
            return Err(SeeError::Team {
                message: format!("task '{task_id}' is not assigned to '{agent_id}'"),
            });
        }

        task.status = TaskStatus::Done;
        task.result = Some(result_text.to_owned());
        task.updated_at = Utc::now().to_rfc3339();

        let result = task.clone();
        self.save_tasks(&tasks)?;
        Ok(result)
    }

    /// Update arbitrary fields on a task.
    pub fn update_task(
        &self,
        task_id: &str,
        status: Option<TaskStatus>,
        assigned_to: Option<&str>,
        result_text: Option<&str>,
    ) -> Result<TaskItem> {
        let mut tasks = self.load_tasks()?;
        let task = find_task_mut(&mut tasks, task_id)?;

        if let Some(s) = status {
            task.status = s;
        }
        if let Some(a) = assigned_to {
            task.assigned_to = Some(a.to_owned());
        }
        if let Some(r) = result_text {
            task.result = Some(r.to_owned());
        }
        task.updated_at = Utc::now().to_rfc3339();

        let result = task.clone();
        self.save_tasks(&tasks)?;
        Ok(result)
    }
}

fn find_task_mut<'a>(tasks: &'a mut [TaskItem], task_id: &str) -> Result<&'a mut TaskItem> {
    tasks
        .iter_mut()
        .find(|t| t.id == task_id)
        .ok_or_else(|| SeeError::NotFound {
            what: format!("task '{task_id}'"),
        })
}

fn generate_task_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{nanos:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ensure_workspace;
    use crate::types::paths::WorkspaceDir;
    use crate::types::TeamMember;
    use tempfile::TempDir;

    fn setup() -> (TempDir, TeamDir) {
        let tmp = TempDir::new().unwrap();
        let ws = WorkspaceDir::new(tmp.path());
        ensure_workspace(&ws).unwrap();

        let team = crate::team::create_team(
            &ws,
            "Test",
            vec![TeamMember {
                id: "a1".into(),
                role: "dev".into(),
                endpoint: None,
            }],
            None,
        )
        .unwrap();

        (tmp, ws.team(&team.id))
    }

    #[test]
    fn create_and_list_tasks() {
        let (_tmp, team_dir) = setup();
        let board = TaskBoard::new(team_dir);

        board.create_task("Task 1", "Do stuff", "a1").unwrap();
        board.create_task("Task 2", "More stuff", "a1").unwrap();

        let all = board.list_tasks(None).unwrap();
        assert_eq!(all.len(), 2);

        let pending = board.list_tasks(Some(TaskStatus::Pending)).unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn claim_and_complete_task() {
        let (_tmp, team_dir) = setup();
        let board = TaskBoard::new(team_dir);

        let task = board.create_task("T1", "desc", "a1").unwrap();
        let claimed = board.claim_task(&task.id, "a1").unwrap();
        assert_eq!(claimed.status, TaskStatus::Claimed);
        assert_eq!(claimed.assigned_to.as_deref(), Some("a1"));

        let done = board.complete_task(&task.id, "a1", "finished").unwrap();
        assert_eq!(done.status, TaskStatus::Done);
        assert_eq!(done.result.as_deref(), Some("finished"));
    }

    #[test]
    fn claim_non_pending_fails() {
        let (_tmp, team_dir) = setup();
        let board = TaskBoard::new(team_dir);

        let task = board.create_task("T1", "desc", "a1").unwrap();
        board.claim_task(&task.id, "a1").unwrap();

        let result = board.claim_task(&task.id, "a1");
        assert!(result.is_err());
    }

    #[test]
    fn complete_wrong_agent_fails() {
        let (_tmp, team_dir) = setup();
        let board = TaskBoard::new(team_dir);

        let task = board.create_task("T1", "desc", "a1").unwrap();
        board.claim_task(&task.id, "a1").unwrap();

        let result = board.complete_task(&task.id, "wrong-agent", "done");
        assert!(result.is_err());
    }

    #[test]
    fn update_task_fields() {
        let (_tmp, team_dir) = setup();
        let board = TaskBoard::new(team_dir);

        let task = board.create_task("T1", "desc", "a1").unwrap();
        let updated = board
            .update_task(&task.id, Some(TaskStatus::InProgress), Some("a1"), None)
            .unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
        assert_eq!(updated.assigned_to.as_deref(), Some("a1"));
    }
}
