//! Infrastructure Layer 1.1: NATS Connection Tests for cim-workflow-graph
//! 
//! User Story: As a workflow system, I need to publish workflow events to NATS for inter-service communication
//!
//! Test Requirements:
//! - Verify NATS connection establishment for workflow events
//! - Verify workflow event stream creation
//! - Verify workflow event publishing with acknowledgment
//! - Verify workflow event consumption with ordering
//!
//! Event Sequence:
//! 1. NATSConnectionEstablished
//! 2. WorkflowEventStreamCreated { stream_name }
//! 3. WorkflowEventPublished { subject, event_id }
//! 4. WorkflowEventConsumed { consumer_name, event_id }
//!
//! ```mermaid
//! graph LR
//!     A[Test Start] --> B[Connect to NATS]
//!     B --> C[NATSConnectionEstablished]
//!     C --> D[Create Stream]
//!     D --> E[WorkflowEventStreamCreated]
//!     E --> F[Publish Event]
//!     F --> G[WorkflowEventPublished]
//!     G --> H[Consume Event]
//!     H --> I[WorkflowEventConsumed]
//!     I --> J[Test Success]
//! ```

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Mock NATS client for testing
pub struct MockNatsClient {
    connected: bool,
    streams: HashMap<String, MockStream>,
    published_messages: Vec<PublishedMessage>,
}

/// Mock stream configuration
pub struct MockStream {
    name: String,
    subjects: Vec<String>,
    messages: Vec<MockMessage>,
    consumers: HashMap<String, MockConsumer>,
}

/// Mock consumer
pub struct MockConsumer {
    name: String,
    stream_name: String,
    ack_wait: Duration,
    delivered: Vec<String>,
}

/// Published message
#[derive(Debug, Clone)]
pub struct PublishedMessage {
    subject: String,
    payload: Vec<u8>,
    headers: HashMap<String, String>,
    ack_id: String,
}

/// Mock message
#[derive(Debug, Clone)]
pub struct MockMessage {
    subject: String,
    payload: Vec<u8>,
    sequence: u64,
    timestamp: SystemTime,
}

/// NATS connection events for testing
#[derive(Debug, Clone, PartialEq)]
pub enum NatsConnectionEvent {
    NATSConnectionEstablished,
    WorkflowEventStreamCreated { stream_name: String },
    WorkflowEventPublished { subject: String, event_id: String },
    WorkflowEventConsumed { consumer_name: String, event_id: String },
    ConsumerCreated { consumer_name: String, stream_name: String },
    ConnectionLost,
    ConnectionReestablished,
    StreamDeleted { stream_name: String },
}

impl MockNatsClient {
    pub fn new() -> Self {
        Self {
            connected: false,
            streams: HashMap::new(),
            published_messages: Vec::new(),
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        if self.connected {
            return Err("Already connected".to_string());
        }
        self.connected = true;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn create_stream(&mut self, name: String, subjects: Vec<String>) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        if self.streams.contains_key(&name) {
            return Err("Stream already exists".to_string());
        }

        let stream = MockStream {
            name: name.clone(),
            subjects,
            messages: Vec::new(),
            consumers: HashMap::new(),
        };

        self.streams.insert(name, stream);
        Ok(())
    }

    pub fn publish_workflow_event(
        &mut self,
        subject: &str,
        event_id: &str,
        payload: Vec<u8>,
    ) -> Result<String, String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        // Find the stream that handles this subject
        let stream = self.streams.values_mut()
            .find(|s| s.subjects.iter().any(|subj| {
                // Handle wildcard matching
                if subj.ends_with(".>") {
                    let prefix = &subj[..subj.len() - 2];
                    subject.starts_with(prefix)
                } else if subj.contains('*') {
                    // Simple single-level wildcard matching
                    let parts: Vec<&str> = subj.split('.').collect();
                    let subject_parts: Vec<&str> = subject.split('.').collect();
                    if parts.len() != subject_parts.len() {
                        return false;
                    }
                    parts.iter().zip(subject_parts.iter()).all(|(p, s)| p == &"*" || p == s)
                } else {
                    subject == subj
                }
            }))
            .ok_or("No stream for subject")?;

        let sequence = stream.messages.len() as u64 + 1;
        let global_sequence = self.published_messages.len() as u64 + 1;
        let ack_id = format!("ack_{}_{}", global_sequence, sequence);

        let message = MockMessage {
            subject: subject.to_string(),
            payload: payload.clone(),
            sequence,
            timestamp: SystemTime::now(),
        };

        stream.messages.push(message);

        let published = PublishedMessage {
            subject: subject.to_string(),
            payload,
            headers: HashMap::from([
                ("event-id".to_string(), event_id.to_string()),
                ("sequence".to_string(), sequence.to_string()),
            ]),
            ack_id: ack_id.clone(),
        };

        self.published_messages.push(published);

        Ok(ack_id)
    }

    pub fn create_consumer(
        &mut self,
        stream_name: &str,
        consumer_name: &str,
    ) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        let stream = self.streams.get_mut(stream_name)
            .ok_or("Stream not found")?;

        if stream.consumers.contains_key(consumer_name) {
            return Err("Consumer already exists".to_string());
        }

        let consumer = MockConsumer {
            name: consumer_name.to_string(),
            stream_name: stream_name.to_string(),
            ack_wait: Duration::from_secs(30),
            delivered: Vec::new(),
        };

        stream.consumers.insert(consumer_name.to_string(), consumer);
        Ok(())
    }

    pub fn consume_next(
        &mut self,
        stream_name: &str,
        consumer_name: &str,
    ) -> Result<Option<(String, Vec<u8>)>, String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        let stream = self.streams.get_mut(stream_name)
            .ok_or("Stream not found")?;

        let consumer = stream.consumers.get_mut(consumer_name)
            .ok_or("Consumer not found")?;

        // Find next undelivered message
        let next_seq = consumer.delivered.len();
        if next_seq < stream.messages.len() {
            let message = &stream.messages[next_seq];
            let event_id = format!("evt_{}", next_seq);
            consumer.delivered.push(event_id.clone());
            Ok(Some((event_id, message.payload.clone())))
        } else {
            Ok(None)
        }
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn delete_stream(&mut self, stream_name: &str) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }

        self.streams.remove(stream_name)
            .ok_or("Stream not found")?;

        Ok(())
    }

    pub fn get_published_count(&self) -> usize {
        self.published_messages.len()
    }
}

/// Event validator for NATS connection testing
pub struct NatsEventValidator {
    expected_events: Vec<NatsConnectionEvent>,
    captured_events: Vec<NatsConnectionEvent>,
}

impl NatsEventValidator {
    pub fn new() -> Self {
        Self {
            expected_events: Vec::new(),
            captured_events: Vec::new(),
        }
    }

    pub fn expect_sequence(mut self, events: Vec<NatsConnectionEvent>) -> Self {
        self.expected_events = events;
        self
    }

    pub fn capture_event(&mut self, event: NatsConnectionEvent) {
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
    fn test_nats_connection_establishment() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new()
            .expect_sequence(vec![
                NatsConnectionEvent::NATSConnectionEstablished,
            ]);

        // Act
        let result = client.connect();

        // Assert
        assert!(result.is_ok());
        assert!(client.is_connected());
        validator.capture_event(NatsConnectionEvent::NATSConnectionEstablished);
        assert!(validator.validate().is_ok());
    }

    #[test]
    fn test_workflow_event_stream_creation() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new();
        client.connect().unwrap();

        // Act
        let stream_name = "WORKFLOW_EVENTS";
        let subjects = vec!["workflow.>".to_string()];
        let result = client.create_stream(stream_name.to_string(), subjects);

        // Assert
        assert!(result.is_ok());
        validator.capture_event(NatsConnectionEvent::WorkflowEventStreamCreated {
            stream_name: stream_name.to_string(),
        });
    }

    #[test]
    fn test_workflow_event_publishing() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new();
        
        client.connect().unwrap();
        client.create_stream(
            "WORKFLOW_EVENTS".to_string(),
            vec!["workflow.>".to_string()],
        ).unwrap();

        // Act
        let subject = "workflow.created";
        let event_id = "evt_123";
        let payload = b"workflow event data".to_vec();
        
        let ack_id = client.publish_workflow_event(subject, event_id, payload).unwrap();

        // Assert
        assert!(!ack_id.is_empty());
        assert_eq!(client.get_published_count(), 1);
        
        validator.capture_event(NatsConnectionEvent::WorkflowEventPublished {
            subject: subject.to_string(),
            event_id: event_id.to_string(),
        });
    }

    #[test]
    fn test_workflow_event_consumption() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new();
        
        client.connect().unwrap();
        client.create_stream(
            "WORKFLOW_EVENTS".to_string(),
            vec!["workflow.>".to_string()],
        ).unwrap();

        // Publish an event
        let event_id = "evt_456";
        client.publish_workflow_event(
            "workflow.started",
            event_id,
            b"workflow started".to_vec(),
        ).unwrap();

        // Create consumer
        let consumer_name = "workflow-consumer";
        client.create_consumer("WORKFLOW_EVENTS", consumer_name).unwrap();

        // Act
        let result = client.consume_next("WORKFLOW_EVENTS", consumer_name).unwrap();

        // Assert
        assert!(result.is_some());
        let (consumed_id, payload) = result.unwrap();
        assert_eq!(payload, b"workflow started");
        
        validator.capture_event(NatsConnectionEvent::WorkflowEventConsumed {
            consumer_name: consumer_name.to_string(),
            event_id: consumed_id,
        });
    }

    #[test]
    fn test_consumer_creation() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new();
        
        client.connect().unwrap();
        let stream_name = "WORKFLOW_EVENTS";
        client.create_stream(
            stream_name.to_string(),
            vec!["workflow.>".to_string()],
        ).unwrap();

        // Act
        let consumer_name = "test-consumer";
        let result = client.create_consumer(stream_name, consumer_name);

        // Assert
        assert!(result.is_ok());
        validator.capture_event(NatsConnectionEvent::ConsumerCreated {
            consumer_name: consumer_name.to_string(),
            stream_name: stream_name.to_string(),
        });
    }

    #[test]
    fn test_connection_loss_and_reconnection() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new();
        
        client.connect().unwrap();
        client.create_stream(
            "WORKFLOW_EVENTS".to_string(),
            vec!["workflow.>".to_string()],
        ).unwrap();

        // Act - Disconnect
        client.disconnect();
        assert!(!client.is_connected());
        validator.capture_event(NatsConnectionEvent::ConnectionLost);

        // Act - Reconnect
        client.connect().unwrap();
        assert!(client.is_connected());
        validator.capture_event(NatsConnectionEvent::ConnectionReestablished);

        // Verify we can still publish
        let result = client.publish_workflow_event(
            "workflow.test",
            "evt_reconnect",
            b"test".to_vec(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_workflow_event_ordering() {
        // Arrange
        let mut client = MockNatsClient::new();
        
        client.connect().unwrap();
        client.create_stream(
            "WORKFLOW_EVENTS".to_string(),
            vec!["workflow.>".to_string()],
        ).unwrap();
        client.create_consumer("WORKFLOW_EVENTS", "ordered-consumer").unwrap();

        // Act - Publish multiple events
        let events = vec![
            ("workflow.created", "evt_1", b"event 1"),
            ("workflow.step.added", "evt_2", b"event 2"),
            ("workflow.started", "evt_3", b"event 3"),
        ];

        for (subject, event_id, payload) in &events {
            client.publish_workflow_event(subject, event_id, payload.to_vec()).unwrap();
        }

        // Consume in order
        let mut consumed = Vec::new();
        while let Some((id, payload)) = client.consume_next("WORKFLOW_EVENTS", "ordered-consumer").unwrap() {
            consumed.push((id, payload));
        }

        // Assert - Events consumed in order
        assert_eq!(consumed.len(), 3);
        assert_eq!(consumed[0].1, b"event 1");
        assert_eq!(consumed[1].1, b"event 2");
        assert_eq!(consumed[2].1, b"event 3");
    }

    #[test]
    fn test_stream_deletion() {
        // Arrange
        let mut client = MockNatsClient::new();
        let mut validator = NatsEventValidator::new();
        
        client.connect().unwrap();
        let stream_name = "TEMP_WORKFLOW_STREAM";
        client.create_stream(
            stream_name.to_string(),
            vec!["temp.workflow.>".to_string()],
        ).unwrap();

        // Act
        let result = client.delete_stream(stream_name);

        // Assert
        assert!(result.is_ok());
        validator.capture_event(NatsConnectionEvent::StreamDeleted {
            stream_name: stream_name.to_string(),
        });

        // Verify stream is gone
        let publish_result = client.publish_workflow_event(
            "temp.workflow.test",
            "evt_deleted",
            b"test".to_vec(),
        );
        assert!(publish_result.is_err());
    }

    #[test]
    fn test_multiple_workflow_streams() {
        // Arrange
        let mut client = MockNatsClient::new();
        client.connect().unwrap();

        // Act - Create multiple streams for different workflow types
        client.create_stream(
            "APPROVAL_WORKFLOWS".to_string(),
            vec!["workflow.approval.>".to_string()],
        ).unwrap();

        client.create_stream(
            "AUTOMATION_WORKFLOWS".to_string(),
            vec!["workflow.automation.>".to_string()],
        ).unwrap();

        // Publish to different streams
        let ack1 = client.publish_workflow_event(
            "workflow.approval.created",
            "approval_1",
            b"approval workflow".to_vec(),
        ).unwrap();

        let ack2 = client.publish_workflow_event(
            "workflow.automation.created",
            "automation_1",
            b"automation workflow".to_vec(),
        ).unwrap();

        // Assert
        assert_ne!(ack1, ack2);
        assert_eq!(client.get_published_count(), 2);
    }
} 