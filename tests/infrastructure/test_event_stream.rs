//! Infrastructure Layer 1.2: Event Stream Tests for cim-workflow-graph
//! 
//! User Story: As a workflow system, I need to persist workflow events with CID chains for integrity
//!
//! Test Requirements:
//! - Verify workflow event persistence with CID calculation
//! - Verify CID chain integrity for workflow transitions
//! - Verify workflow event replay from store
//! - Verify workflow snapshot creation and restoration
//!
//! Event Sequence:
//! 1. WorkflowEventStoreInitialized
//! 2. WorkflowEventPersisted { event_id, cid, previous_cid }
//! 3. CIDChainValidated { start_cid, end_cid, length }
//! 4. WorkflowEventsReplayed { count, workflow_id }
//!
//! ```mermaid
//! graph LR
//!     A[Test Start] --> B[Initialize Store]
//!     B --> C[WorkflowEventStoreInitialized]
//!     C --> D[Create Workflow Event]
//!     D --> E[WorkflowEventPersisted]
//!     E --> F[Validate CID Chain]
//!     F --> G[CIDChainValidated]
//!     G --> H[Replay Events]
//!     H --> I[WorkflowEventsReplayed]
//!     I --> J[Test Success]
//! ```

use std::collections::HashMap;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

/// Mock CID representation for testing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cid(String);

impl Cid {
    pub fn new(data: &[u8]) -> Self {
        // Simple mock CID calculation
        let hash = data.iter().fold(0u64, |acc, &b| acc.wrapping_add(b as u64));
        Self(format!("Qm{:x}", hash))
    }
}

/// Workflow domain events for testing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowDomainEvent {
    WorkflowCreated {
        workflow_id: String,
        name: String,
        description: String,
        timestamp: SystemTime,
    },
    StepAdded {
        workflow_id: String,
        step_id: String,
        name: String,
        step_type: String,
        timestamp: SystemTime,
    },
    WorkflowStarted {
        workflow_id: String,
        context: HashMap<String, String>,
        timestamp: SystemTime,
    },
    StepCompleted {
        workflow_id: String,
        step_id: String,
        result: String,
        timestamp: SystemTime,
    },
    WorkflowCompleted {
        workflow_id: String,
        status: String,
        timestamp: SystemTime,
    },
}

/// Event store events for testing
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowEventStoreEvent {
    WorkflowEventStoreInitialized,
    WorkflowEventPersisted {
        event_id: String,
        cid: Cid,
        previous_cid: Option<Cid>,
    },
    CIDChainValidated {
        start_cid: Cid,
        end_cid: Cid,
        length: usize,
    },
    WorkflowEventsReplayed {
        count: usize,
        workflow_id: String,
    },
    SnapshotCreated {
        snapshot_cid: Cid,
        event_count: usize,
    },
    SnapshotRestored {
        snapshot_cid: Cid,
        restored_count: usize,
    },
}

/// Event with CID chain
#[derive(Debug, Clone)]
pub struct ChainedWorkflowEvent {
    pub event_id: String,
    pub event: WorkflowDomainEvent,
    pub cid: Cid,
    pub previous_cid: Option<Cid>,
    pub sequence: u64,
}

/// Mock event store for workflow events
pub struct MockWorkflowEventStore {
    events: Vec<ChainedWorkflowEvent>,
    snapshots: HashMap<Cid, Vec<ChainedWorkflowEvent>>,
}

impl MockWorkflowEventStore {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            snapshots: HashMap::new(),
        }
    }

    pub fn append_event(
        &mut self,
        event: WorkflowDomainEvent,
    ) -> Result<(String, Cid, Option<Cid>), String> {
        let event_id = format!("evt_{}", self.events.len());
        let previous_cid = self.events.last().map(|e| e.cid.clone());
        
        // Calculate CID including previous CID
        let event_data = format!("{:?}{:?}", event, previous_cid);
        let cid = Cid::new(event_data.as_bytes());
        
        let sequence = self.events.len() as u64;
        
        let chained_event = ChainedWorkflowEvent {
            event_id: event_id.clone(),
            event,
            cid: cid.clone(),
            previous_cid: previous_cid.clone(),
            sequence,
        };
        
        self.events.push(chained_event);
        
        Ok((event_id, cid, previous_cid))
    }

    pub fn validate_chain(&self) -> Result<(Cid, Cid, usize), String> {
        if self.events.is_empty() {
            return Err("No events to validate".to_string());
        }

        // Validate each event's CID chain
        for i in 1..self.events.len() {
            let current = &self.events[i];
            let previous = &self.events[i - 1];
            
            if current.previous_cid.as_ref() != Some(&previous.cid) {
                return Err(format!(
                    "Chain broken at sequence {}: expected {:?}, got {:?}",
                    i, previous.cid, current.previous_cid
                ));
            }
        }

        let start_cid = self.events.first().unwrap().cid.clone();
        let end_cid = self.events.last().unwrap().cid.clone();
        let length = self.events.len();

        Ok((start_cid, end_cid, length))
    }

    pub fn replay_events(&self, workflow_id: &str) -> Vec<ChainedWorkflowEvent> {
        self.events
            .iter()
            .filter(|e| match &e.event {
                WorkflowDomainEvent::WorkflowCreated { workflow_id: id, .. } => id == workflow_id,
                WorkflowDomainEvent::StepAdded { workflow_id: id, .. } => id == workflow_id,
                WorkflowDomainEvent::WorkflowStarted { workflow_id: id, .. } => id == workflow_id,
                WorkflowDomainEvent::StepCompleted { workflow_id: id, .. } => id == workflow_id,
                WorkflowDomainEvent::WorkflowCompleted { workflow_id: id, .. } => id == workflow_id,
            })
            .cloned()
            .collect()
    }

    pub fn create_snapshot(&mut self) -> Result<Cid, String> {
        if self.events.is_empty() {
            return Err("No events to snapshot".to_string());
        }

        let snapshot_data = format!("{:?}", self.events);
        let snapshot_cid = Cid::new(snapshot_data.as_bytes());
        
        self.snapshots.insert(snapshot_cid.clone(), self.events.clone());
        
        Ok(snapshot_cid)
    }

    pub fn restore_from_snapshot(&mut self, snapshot_cid: &Cid) -> Result<usize, String> {
        match self.snapshots.get(snapshot_cid) {
            Some(events) => {
                self.events = events.clone();
                Ok(events.len())
            }
            None => Err("Snapshot not found".to_string()),
        }
    }
}

/// Event stream validator for workflow event store testing
pub struct WorkflowEventStreamValidator {
    expected_events: Vec<WorkflowEventStoreEvent>,
    captured_events: Vec<WorkflowEventStoreEvent>,
}

impl WorkflowEventStreamValidator {
    pub fn new() -> Self {
        Self {
            expected_events: Vec::new(),
            captured_events: Vec::new(),
        }
    }

    pub fn expect_sequence(mut self, events: Vec<WorkflowEventStoreEvent>) -> Self {
        self.expected_events = events;
        self
    }

    pub fn capture_event(&mut self, event: WorkflowEventStoreEvent) {
        self.captured_events.push(event);
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.captured_events.len() != self.expected_events.len() {
            return Err(format!(
                "Event count mismatch: expected {}, got {}",
                self.expected_events.len(),
                self.captured_events.len()
            ));
        }

        for (i, (expected, actual)) in self.expected_events.iter()
            .zip(self.captured_events.iter())
            .enumerate()
        {
            if expected != actual {
                return Err(format!(
                    "Event mismatch at position {}: expected {:?}, got {:?}",
                    i, expected, actual
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_event_store_initialization() {
        // Arrange
        let mut validator = WorkflowEventStreamValidator::new()
            .expect_sequence(vec![
                WorkflowEventStoreEvent::WorkflowEventStoreInitialized,
            ]);

        // Act
        let store = MockWorkflowEventStore::new();
        validator.capture_event(WorkflowEventStoreEvent::WorkflowEventStoreInitialized);

        // Assert
        assert!(validator.validate().is_ok());
        assert_eq!(store.events.len(), 0);
    }

    #[test]
    fn test_workflow_event_persistence_with_cid() {
        // Arrange
        let mut store = MockWorkflowEventStore::new();
        let mut validator = WorkflowEventStreamValidator::new();

        // Act
        let event = WorkflowDomainEvent::WorkflowCreated {
            workflow_id: "wf-123".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            timestamp: SystemTime::now(),
        };

        let (event_id, cid, previous_cid) = store.append_event(event).unwrap();

        // Assert
        assert!(previous_cid.is_none()); // First event has no previous
        assert!(!event_id.is_empty());
        
        validator.capture_event(WorkflowEventStoreEvent::WorkflowEventPersisted {
            event_id,
            cid,
            previous_cid,
        });
    }

    #[test]
    fn test_workflow_lifecycle_cid_chain() {
        // Arrange
        let mut store = MockWorkflowEventStore::new();
        let mut validator = WorkflowEventStreamValidator::new();
        let workflow_id = "wf-lifecycle";

        // Act - Create workflow lifecycle events
        let event1 = WorkflowDomainEvent::WorkflowCreated {
            workflow_id: workflow_id.to_string(),
            name: "Lifecycle Test".to_string(),
            description: "Testing workflow lifecycle".to_string(),
            timestamp: SystemTime::now(),
        };

        let event2 = WorkflowDomainEvent::StepAdded {
            workflow_id: workflow_id.to_string(),
            step_id: "step-1".to_string(),
            name: "First Step".to_string(),
            step_type: "Manual".to_string(),
            timestamp: SystemTime::now(),
        };

        let event3 = WorkflowDomainEvent::WorkflowStarted {
            workflow_id: workflow_id.to_string(),
            context: HashMap::new(),
            timestamp: SystemTime::now(),
        };

        let event4 = WorkflowDomainEvent::StepCompleted {
            workflow_id: workflow_id.to_string(),
            step_id: "step-1".to_string(),
            result: "Success".to_string(),
            timestamp: SystemTime::now(),
        };

        let event5 = WorkflowDomainEvent::WorkflowCompleted {
            workflow_id: workflow_id.to_string(),
            status: "Completed".to_string(),
            timestamp: SystemTime::now(),
        };

        store.append_event(event1).unwrap();
        store.append_event(event2).unwrap();
        store.append_event(event3).unwrap();
        store.append_event(event4).unwrap();
        store.append_event(event5).unwrap();

        // Validate chain
        let (start_cid, end_cid, length) = store.validate_chain().unwrap();

        // Assert
        assert_eq!(length, 5);
        assert_ne!(start_cid, end_cid);
        
        validator.capture_event(WorkflowEventStoreEvent::CIDChainValidated {
            start_cid,
            end_cid,
            length,
        });
    }

    #[test]
    fn test_workflow_event_replay() {
        // Arrange
        let mut store = MockWorkflowEventStore::new();
        let mut validator = WorkflowEventStreamValidator::new();
        let workflow_id = "wf-replay";

        // Add events for multiple workflows
        store.append_event(WorkflowDomainEvent::WorkflowCreated {
            workflow_id: workflow_id.to_string(),
            name: "Replay Test".to_string(),
            description: "Testing replay".to_string(),
            timestamp: SystemTime::now(),
        }).unwrap();

        store.append_event(WorkflowDomainEvent::WorkflowCreated {
            workflow_id: "other-workflow".to_string(),
            name: "Other Workflow".to_string(),
            description: "Different workflow".to_string(),
            timestamp: SystemTime::now(),
        }).unwrap();

        store.append_event(WorkflowDomainEvent::StepAdded {
            workflow_id: workflow_id.to_string(),
            step_id: "step-1".to_string(),
            name: "Test Step".to_string(),
            step_type: "Automated".to_string(),
            timestamp: SystemTime::now(),
        }).unwrap();

        // Act
        let replayed = store.replay_events(workflow_id);

        // Assert
        assert_eq!(replayed.len(), 2); // Only events for the specific workflow
        
        validator.capture_event(WorkflowEventStoreEvent::WorkflowEventsReplayed {
            count: replayed.len(),
            workflow_id: workflow_id.to_string(),
        });
    }

    #[test]
    fn test_workflow_snapshot_creation_and_restoration() {
        // Arrange
        let mut store = MockWorkflowEventStore::new();
        let mut validator = WorkflowEventStreamValidator::new();

        // Add some events
        for i in 0..3 {
            store.append_event(WorkflowDomainEvent::WorkflowCreated {
                workflow_id: format!("wf-{}", i),
                name: format!("Workflow {}", i),
                description: "Test workflow".to_string(),
                timestamp: SystemTime::now(),
            }).unwrap();
        }

        // Act - Create snapshot
        let snapshot_cid = store.create_snapshot().unwrap();
        
        validator.capture_event(WorkflowEventStoreEvent::SnapshotCreated {
            snapshot_cid: snapshot_cid.clone(),
            event_count: 3,
        });

        // Clear events and restore
        store.events.clear();
        let restored_count = store.restore_from_snapshot(&snapshot_cid).unwrap();

        // Assert
        assert_eq!(restored_count, 3);
        assert_eq!(store.events.len(), 3);
        
        validator.capture_event(WorkflowEventStoreEvent::SnapshotRestored {
            snapshot_cid,
            restored_count,
        });
    }

    #[test]
    fn test_broken_chain_detection() {
        // Arrange
        let mut store = MockWorkflowEventStore::new();

        // Add valid events
        store.append_event(WorkflowDomainEvent::WorkflowCreated {
            workflow_id: "wf-1".to_string(),
            name: "Workflow 1".to_string(),
            description: "Test".to_string(),
            timestamp: SystemTime::now(),
        }).unwrap();

        store.append_event(WorkflowDomainEvent::StepAdded {
            workflow_id: "wf-1".to_string(),
            step_id: "step-1".to_string(),
            name: "Step 1".to_string(),
            step_type: "Manual".to_string(),
            timestamp: SystemTime::now(),
        }).unwrap();

        // Manually break the chain
        if let Some(event) = store.events.get_mut(1) {
            event.previous_cid = Some(Cid::new(b"broken"));
        }

        // Act
        let result = store.validate_chain();

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Chain broken"));
    }

    #[test]
    fn test_step_completion_event() {
        // Arrange
        let mut store = MockWorkflowEventStore::new();

        // Act
        let event = WorkflowDomainEvent::StepCompleted {
            workflow_id: "wf-test".to_string(),
            step_id: "step-complete".to_string(),
            result: "Success with output data".to_string(),
            timestamp: SystemTime::now(),
        };

        let (event_id, cid, _) = store.append_event(event.clone()).unwrap();

        // Assert
        assert_eq!(store.events.len(), 1);
        match &store.events[0].event {
            WorkflowDomainEvent::StepCompleted { result, .. } => {
                assert_eq!(result, "Success with output data");
            }
            _ => panic!("Wrong event type"),
        }
        assert_eq!(store.events[0].event_id, event_id);
        assert_eq!(store.events[0].cid, cid);
    }
} 