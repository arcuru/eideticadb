use chrono::{DateTime, Utc};
use eideticadb::data::{Data, CRDT};
use eideticadb::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Todo {
    pub fn new(title: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            completed: false,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    pub fn complete(&mut self) {
        self.completed = true;
        self.completed_at = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoList {
    todos: HashMap<String, Todo>,
}

impl TodoList {
    pub fn new() -> Self {
        Self {
            todos: HashMap::new(),
        }
    }

    pub fn add_todo(&mut self, todo: Todo) {
        self.todos.insert(todo.id.clone(), todo);
    }

    #[allow(dead_code)]
    pub fn get_todo(&self, id: &str) -> Option<&Todo> {
        self.todos.get(id)
    }

    pub fn get_todo_mut(&mut self, id: &str) -> Option<&mut Todo> {
        self.todos.get_mut(id)
    }

    pub fn get_todos(&self) -> Vec<&Todo> {
        self.todos.values().collect()
    }

    #[allow(dead_code)]
    pub fn remove_todo(&mut self, id: &str) -> Option<Todo> {
        self.todos.remove(id)
    }
}

impl Data for TodoList {}

impl CRDT for TodoList {
    fn merge(&self, other: &Self) -> Result<Self, Error> {
        let mut result = self.clone();

        for (id, todo) in other.todos.iter() {
            if let Some(existing_todo) = result.todos.get(id) {
                // For our custom merge, we will always maintain the completion state
                // even if the 'other' list shows it as incomplete.
                // NB: This may not be the desired behavior for a real application, but
                // here is used to show how to customize the merge function.

                // If the todo exists in both, use the completed one
                // or the one with the latest completed_at timestamp
                if todo.completed && !existing_todo.completed {
                    result.todos.insert(id.clone(), todo.clone());
                } else if todo.completed && existing_todo.completed {
                    // Both are completed, use the one with the latest completed_at timestamp
                    if let (Some(todo_completed_at), Some(existing_completed_at)) =
                        (todo.completed_at, existing_todo.completed_at)
                    {
                        if todo_completed_at > existing_completed_at {
                            result.todos.insert(id.clone(), todo.clone());
                        }
                    }
                } else if !todo.completed && existing_todo.completed {
                    // The todo is completed in the earlier list but not the later one.
                    // We don't need to do anything as we will keep the one already in the list.
                }
            } else {
                // If the todo only exists in the other list, add it
                result.todos.insert(id.clone(), todo.clone());
            }
        }

        Ok(result)
    }
}
