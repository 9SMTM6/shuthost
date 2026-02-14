//! Protocol types for agent-coordinator communication.
//!
//! - Agent-to-coordinator messages use miniserde for serialization (agent) and serde for deserialization (coordinator).
//! - Coordinator-to-agent messages use miniserde for deserialization (agent) and serde for serialization (coordinator).

use std::str::FromStr;

#[cfg(feature = "agent")]
use miniserde::Serialize as MiniSerialize;
#[cfg(feature = "coordinator")]
use serde::{Deserialize, Serialize};

/// Message sent from agent to coordinator.
#[derive(Debug, Clone, PartialEq, Eq)]
// miniserde serialization for agent
#[cfg_attr(feature = "agent", derive(MiniSerialize))]
// serde deserialization for coordinator
#[cfg_attr(feature = "coordinator", derive(Deserialize, Serialize))]
pub struct AgentMessage {
    pub hostname: String,
    pub status: String,
    pub timestamp: u64,
}

// Macro to define the enum from variant => string mappings
macro_rules! define_enum_with_str {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $str:literal
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )*
        }

        impl ToString for $name {
            fn to_string(&self) -> String {
                match self {
                    $($name::$variant => $str.to_string(),)*
                }
            }
        }

        impl FromStr for $name {
            type Err = ();

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($str => Ok($name::$variant),)*
                    _ => Err(()),
                }
            }
        }
    };
}

define_enum_with_str! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    /// Enum for messages sent from coordinator to agent.
    pub enum CoordinatorMessage {
        /// Request agent status
        Status => "status",
        /// Request agent to shutdown
        Shutdown => "shutdown",
        /// Request agent to abort service
        Abort => "abort",
    }
}

mod tests {
    #[cfg(feature = "coordinator")]
    #[test]
    fn test_coordinator_message_serialization() {
        let msg = super::CoordinatorMessage::Shutdown;
        let serialized = msg.to_string();
        assert_eq!(serialized, "shutdown");
    }

    #[cfg(feature = "agent")]
    #[test]
    fn test_agent_message_deserialization() {
        use std::str::FromStr;

        let message = "shutdown";
        let deserialized = super::CoordinatorMessage::from_str(&message).unwrap();
        assert_eq!(deserialized, super::CoordinatorMessage::Shutdown);
    }
}
