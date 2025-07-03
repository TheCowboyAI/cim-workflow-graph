//! Infrastructure Layer 1.3: Message Routing Tests for cim-workflow-graph
//! 
//! User Story: As a workflow system, I need to route workflow commands to appropriate handlers
//!
//! Test Requirements:
//! - Verify workflow command routing to correct handlers
//! - Verify handler registration and discovery
//! - Verify fallback handling for unknown commands
//! - Verify routing performance metrics
//!
//! Event Sequence:
//! 1. RouterInitialized
//! 2. HandlerRegistered { command_type, handler_id }
//! 3. CommandRouted { command_type, handler_id }
//! 4. FallbackHandlerInvoked { command_type }
//!
//! ```mermaid
//! graph LR
//!     A[Test Start] --> B[Initialize Router]
//!     B --> C[RouterInitialized]
//!     C --> D[Register Handler]
//!     D --> E[HandlerRegistered]
//!     E --> F[Route Command]
//!     F --> G[CommandRouted]
//!     G --> H[Test Success]
//!     F --> I[Unknown Command]
//!     I --> J[FallbackHandlerInvoked]
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Workflow commands for testing
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowCommand {
    CreateWorkflow {
        name: String,
        description: String,
    },
    AddStep {
        workflow_id: String,
        step_name: String,
        step_type: String,
    },
    StartWorkflow {
        workflow_id: String,
        context: HashMap<String, String>,
    },
    CompleteStep {
        workflow_id: String,
        step_id: String,
        result: String,
    },
    CancelWorkflow {
        workflow_id: String,
        reason: String,
    },
    Unknown {
        command_type: String,
    },
}

/// Command response
#[derive(Debug, Clone, PartialEq)]
pub enum CommandResponse {
    Success { message: String },
    Error { message: String },
    Async { correlation_id: String },
}

/// Handler trait for workflow commands
pub trait WorkflowCommandHandler: Send + Sync {
    fn handle(&self, command: &WorkflowCommand) -> CommandResponse;
    fn command_type(&self) -> &str;
}

/// Mock handler implementation
pub struct MockWorkflowHandler {
    command_type: String,
    response: CommandResponse,
    handled_count: Arc<Mutex<usize>>,
}

impl MockWorkflowHandler {
    pub fn new(command_type: String, response: CommandResponse) -> Self {
        Self {
            command_type,
            response,
            handled_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn get_handled_count(&self) -> usize {
        *self.handled_count.lock().unwrap()
    }
}

impl WorkflowCommandHandler for MockWorkflowHandler {
    fn handle(&self, _command: &WorkflowCommand) -> CommandResponse {
        let mut count = self.handled_count.lock().unwrap();
        *count += 1;
        self.response.clone()
    }

    fn command_type(&self) -> &str {
        &self.command_type
    }
}

/// Fallback handler for unknown commands
pub struct FallbackHandler {
    invoked_count: Arc<Mutex<usize>>,
}

impl FallbackHandler {
    pub fn new() -> Self {
        Self {
            invoked_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn get_invoked_count(&self) -> usize {
        *self.invoked_count.lock().unwrap()
    }
}

impl WorkflowCommandHandler for FallbackHandler {
    fn handle(&self, command: &WorkflowCommand) -> CommandResponse {
        let mut count = self.invoked_count.lock().unwrap();
        *count += 1;
        
        CommandResponse::Error {
            message: format!("Unknown command type: {:?}", command),
        }
    }

    fn command_type(&self) -> &str {
        "fallback"
    }
}

/// Routing events for testing
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingEvent {
    RouterInitialized,
    HandlerRegistered {
        command_type: String,
        handler_id: String,
    },
    CommandRouted {
        command_type: String,
        handler_id: String,
    },
    FallbackHandlerInvoked {
        command_type: String,
    },
    RoutingError {
        message: String,
    },
    HandlerRemoved {
        command_type: String,
    },
}

/// Routing statistics
#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub total_routed: usize,
    pub by_command_type: HashMap<String, usize>,
    pub fallback_count: usize,
    pub average_routing_time: Duration,
}

/// Message router for workflow commands
pub struct WorkflowCommandRouter {
    handlers: HashMap<String, Box<dyn WorkflowCommandHandler>>,
    fallback_handler: Option<Box<dyn WorkflowCommandHandler>>,
    stats: RoutingStats,
    routing_times: Vec<Duration>,
}

impl WorkflowCommandRouter {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            fallback_handler: None,
            stats: RoutingStats {
                total_routed: 0,
                by_command_type: HashMap::new(),
                fallback_count: 0,
                average_routing_time: Duration::ZERO,
            },
            routing_times: Vec::new(),
        }
    }

    pub fn register_handler(
        &mut self,
        handler: Box<dyn WorkflowCommandHandler>,
    ) -> Result<String, String> {
        let command_type = handler.command_type().to_string();
        let handler_id = format!("handler_{command_type}");

        if self.handlers.contains_key(&command_type) {
            return Err(format!("Handler already registered for {command_type}"));
        }

        self.handlers.insert(command_type.clone(), handler);
        Ok(handler_id)
    }

    pub fn set_fallback_handler(&mut self, handler: Box<dyn WorkflowCommandHandler>) {
        self.fallback_handler = Some(handler);
    }

    pub fn route_command(&mut self, command: &WorkflowCommand) -> (CommandResponse, String) {
        let start = Instant::now();
        let command_type = self.get_command_type(command);

        let (response, handler_id) = if let Some(handler) = self.handlers.get(&command_type) {
            let response = handler.handle(command);
            (response, format!("handler_{command_type}"))
        } else if let Some(fallback) = &self.fallback_handler {
            self.stats.fallback_count += 1;
            let response = fallback.handle(command);
            (response, "fallback".to_string())
        } else {
            (
                CommandResponse::Error {
                    message: "No handler found".to_string(),
                },
                "none".to_string(),
            )
        };

        // Update stats
        let routing_time = start.elapsed();
        self.routing_times.push(routing_time);
        self.stats.total_routed += 1;
        *self.stats.by_command_type.entry(command_type).or_insert(0) += 1;
        self.stats.average_routing_time = Duration::from_nanos(
            self.routing_times.iter().map(|d| d.as_nanos()).sum::<u128>() as u64
                / self.routing_times.len() as u64,
        );

        (response, handler_id)
    }

    pub fn remove_handler(&mut self, command_type: &str) -> Result<(), String> {
        self.handlers
            .remove(command_type)
            .ok_or_else(|| format!("Handler not found for {command_type}"))?;
        Ok(())
    }

    pub fn get_stats(&self) -> &RoutingStats {
        &self.stats
    }

    fn get_command_type(&self, command: &WorkflowCommand) -> String {
        match command {
            WorkflowCommand::CreateWorkflow { .. } => "create_workflow".to_string(),
            WorkflowCommand::AddStep { .. } => "add_step".to_string(),
            WorkflowCommand::StartWorkflow { .. } => "start_workflow".to_string(),
            WorkflowCommand::CompleteStep { .. } => "complete_step".to_string(),
            WorkflowCommand::CancelWorkflow { .. } => "cancel_workflow".to_string(),
            WorkflowCommand::Unknown { command_type } => command_type.clone(),
        }
    }
}

/// Event validator for routing testing
pub struct RoutingEventValidator {
    expected_events: Vec<RoutingEvent>,
    captured_events: Vec<RoutingEvent>,
}

impl RoutingEventValidator {
    pub fn new() -> Self {
        Self {
            expected_events: Vec::new(),
            captured_events: Vec::new(),
        }
    }

    pub fn expect_sequence(mut self, events: Vec<RoutingEvent>) -> Self {
        self.expected_events = events;
        self
    }

    pub fn capture_event(&mut self, event: RoutingEvent) {
        self.captured_events.push(event);
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.captured_events.len() != self.expected_events.len() {
            return Err(format!("Event count mismatch: expected {self.expected_events.len(}, got {}"),
                self.captured_events.len()
            ));
        }

        for (i, (expected, actual)) in self.expected_events.iter()
            .zip(self.captured_events.iter())
            .enumerate()
        {
            if expected != actual {
                return Err(format!("Event mismatch at position {i}: expected {:?}, got {:?}", expected, actual));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_initialization() {
        // Arrange
        let mut validator = RoutingEventValidator::new()
            .expect_sequence(vec![
                RoutingEvent::RouterInitialized,
            ]);

        // Act
        let router = WorkflowCommandRouter::new();
        validator.capture_event(RoutingEvent::RouterInitialized);

        // Assert
        assert!(validator.validate().is_ok());
        assert_eq!(router.handlers.len(), 0);
        assert!(router.fallback_handler.is_none());
    }

    #[test]
    fn test_handler_registration() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();
        let mut validator = RoutingEventValidator::new();

        let handler = Box::new(MockWorkflowHandler::new(
            "create_workflow".to_string(),
            CommandResponse::Success {
                message: "Workflow created".to_string(),
            },
        ));

        // Act
        let handler_id = router.register_handler(handler).unwrap();

        // Assert
        assert_eq!(handler_id, "handler_create_workflow");
        validator.capture_event(RoutingEvent::HandlerRegistered {
            command_type: "create_workflow".to_string(),
            handler_id,
        });
    }

    #[test]
    fn test_command_routing() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();
        let mut validator = RoutingEventValidator::new();

        let handler = Box::new(MockWorkflowHandler::new(
            "create_workflow".to_string(),
            CommandResponse::Success {
                message: "Workflow created".to_string(),
            },
        ));

        router.register_handler(handler).unwrap();

        // Act
        let command = WorkflowCommand::CreateWorkflow {
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
        };

        let (response, handler_id) = router.route_command(&command);

        // Assert
        assert_eq!(
            response,
            CommandResponse::Success {
                message: "Workflow created".to_string(),
            }
        );
        assert_eq!(handler_id, "handler_create_workflow");

        validator.capture_event(RoutingEvent::CommandRouted {
            command_type: "create_workflow".to_string(),
            handler_id,
        });
    }

    #[test]
    fn test_multiple_handler_routing() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();

        // Register multiple handlers
        let handlers = vec![
            ("create_workflow", "Workflow created"),
            ("add_step", "Step added"),
            ("start_workflow", "Workflow started"),
        ];

        for (cmd_type, msg) in handlers {
            let handler = Box::new(MockWorkflowHandler::new(
                cmd_type.to_string(),
                CommandResponse::Success {
                    message: msg.to_string(),
                },
            ));
            router.register_handler(handler).unwrap();
        }

        // Act & Assert - Route different commands
        let commands = vec![
            (
                WorkflowCommand::CreateWorkflow {
                    name: "Test".to_string(),
                    description: "Test".to_string(),
                },
                "Workflow created",
            ),
            (
                WorkflowCommand::AddStep {
                    workflow_id: "wf-1".to_string(),
                    step_name: "Step 1".to_string(),
                    step_type: "Manual".to_string(),
                },
                "Step added",
            ),
            (
                WorkflowCommand::StartWorkflow {
                    workflow_id: "wf-1".to_string(),
                    context: HashMap::new(),
                },
                "Workflow started",
            ),
        ];

        for (command, expected_msg) in commands {
            let (response, _) = router.route_command(&command);
            assert_eq!(
                response,
                CommandResponse::Success {
                    message: expected_msg.to_string(),
                }
            );
        }
    }

    #[test]
    fn test_fallback_handler() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();
        let mut validator = RoutingEventValidator::new();

        let fallback = Box::new(FallbackHandler::new());
        router.set_fallback_handler(fallback);

        // Act - Route unknown command
        let command = WorkflowCommand::Unknown {
            command_type: "unknown_command".to_string(),
        };

        let (response, handler_id) = router.route_command(&command);

        // Assert
        assert!(matches!(response, CommandResponse::Error { .. }));
        assert_eq!(handler_id, "fallback");

        validator.capture_event(RoutingEvent::FallbackHandlerInvoked {
            command_type: "unknown_command".to_string(),
        });
    }

    #[test]
    fn test_routing_statistics() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();

        let handler = Box::new(MockWorkflowHandler::new(
            "create_workflow".to_string(),
            CommandResponse::Success {
                message: "Created".to_string(),
            },
        ));
        router.register_handler(handler).unwrap();

        // Act - Route multiple commands
        for i in 0..5 {
            let command = WorkflowCommand::CreateWorkflow {
                name: format!("Workflow {i}"),
                description: "Test".to_string(),
            };
            router.route_command(&command);
        }

        // Assert
        let stats = router.get_stats();
        assert_eq!(stats.total_routed, 5);
        assert_eq!(stats.by_command_type.get("create_workflow"), Some(&5));
        assert!(stats.average_routing_time > Duration::ZERO);
    }

    #[test]
    fn test_handler_removal() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();
        let mut validator = RoutingEventValidator::new();

        let handler = Box::new(MockWorkflowHandler::new(
            "create_workflow".to_string(),
            CommandResponse::Success {
                message: "Created".to_string(),
            },
        ));
        router.register_handler(handler).unwrap();

        // Act
        let result = router.remove_handler("create_workflow");

        // Assert
        assert!(result.is_ok());
        assert!(!router.handlers.contains_key("create_workflow"));

        validator.capture_event(RoutingEvent::HandlerRemoved {
            command_type: "create_workflow".to_string(),
        });
    }

    #[test]
    fn test_concurrent_routing() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();

        // Register handlers for different command types
        let command_types = vec!["create_workflow", "add_step", "start_workflow"];
        
        for cmd_type in &command_types {
            let handler = Box::new(MockWorkflowHandler::new(
                cmd_type.to_string(),
                CommandResponse::Success {
                    message: format!("{cmd_type} handled"),
                },
            ));
            router.register_handler(handler).unwrap();
        }

        // Act - Route commands in sequence (simulating concurrent access)
        let mut responses = Vec::new();
        for i in 0..9 {
            let command = match i % 3 {
                0 => WorkflowCommand::CreateWorkflow {
                    name: format!("Workflow {i}"),
                    description: "Test".to_string(),
                },
                1 => WorkflowCommand::AddStep {
                    workflow_id: format!("wf-{i}"),
                    step_name: format!("Step {i}"),
                    step_type: "Manual".to_string(),
                },
                _ => WorkflowCommand::StartWorkflow {
                    workflow_id: format!("wf-{i}"),
                    context: HashMap::new(),
                },
            };
            
            let (response, _) = router.route_command(&command);
            responses.push(response);
        }

        // Assert
        assert_eq!(responses.len(), 9);
        let stats = router.get_stats();
        assert_eq!(stats.total_routed, 9);
        assert_eq!(stats.by_command_type.len(), 3);
        for cmd_type in &command_types {
            assert_eq!(stats.by_command_type.get(*cmd_type), Some(&3));
        }
    }

    #[test]
    fn test_response_type_detection() {
        // Arrange
        let mut router = WorkflowCommandRouter::new();

        // Register handlers with different response types
        let async_handler = Box::new(MockWorkflowHandler::new(
            "start_workflow".to_string(),
            CommandResponse::Async {
                correlation_id: "corr-123".to_string(),
            },
        ));
        
        let error_handler = Box::new(MockWorkflowHandler::new(
            "cancel_workflow".to_string(),
            CommandResponse::Error {
                message: "Cannot cancel".to_string(),
            },
        ));

        router.register_handler(async_handler).unwrap();
        router.register_handler(error_handler).unwrap();

        // Act & Assert
        let start_cmd = WorkflowCommand::StartWorkflow {
            workflow_id: "wf-1".to_string(),
            context: HashMap::new(),
        };
        let (response, _) = router.route_command(&start_cmd);
        assert!(matches!(response, CommandResponse::Async { .. }));

        let cancel_cmd = WorkflowCommand::CancelWorkflow {
            workflow_id: "wf-1".to_string(),
            reason: "Test".to_string(),
        };
        let (response, _) = router.route_command(&cancel_cmd);
        assert!(matches!(response, CommandResponse::Error { .. }));
    }
} 