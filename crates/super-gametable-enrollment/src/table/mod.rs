use async_trait::async_trait;

/// Represents a table that can be used to store current connections to players
#[async_trait]
pub trait EnrollmentTable {}
