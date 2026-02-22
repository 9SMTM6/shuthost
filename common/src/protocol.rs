//! Protocol types for agent-coordinator communication.
//!
//! - Agent-to-coordinator messages use miniserde for serialization (agent) and serde for deserialization (coordinator).

use core::str::FromStr;

#[cfg(feature = "agent")]
use alloc::borrow::Cow;

#[cfg(feature = "agent")]
use miniserde::{Serialize as MiniSerialize, ser};
#[cfg(feature = "coordinator")]
use serde::{Deserialize, Serialize};

/// Data carried in a startup announcement from an agent.
///
/// Kept separate so that the surrounding enum can carry an explicit message
/// type tag when we support additional broadcast kinds in the future.
#[derive(Debug, Clone, PartialEq, Eq)]
// miniserde serialization for agent
#[cfg_attr(feature = "agent", derive(MiniSerialize))]
// serde deserialization for coordinator
#[cfg_attr(feature = "coordinator", derive(Deserialize, Serialize))]
pub struct StartupBroadcast {
    pub hostname: String,
    pub agent_version: String,
    pub port: u16,
    pub mac_address: String,
    pub ip_address: String,
    pub timestamp: u64,
}

/// Message sent from agent to coordinator over the UDP broadcast channel.
///
/// Currently only a single agent-startup packet is defined, but the enum
/// wrapper makes future extensions (e.g. heartbeat, shutdown notice) easier.
#[derive(Debug, Clone, PartialEq, Eq)]
// serde deserialization for coordinator
#[cfg_attr(feature = "coordinator", derive(Deserialize, Serialize))]
#[serde(tag = "type", content = "payload")]
pub enum BroadcastMessage {
    /// Agent startup announcement
    AgentStartup(StartupBroadcast),
}

/// Manual miniserde serialization needed.
/// Miniserdes derive doesnt support enums.
/// This mirrors the `#[serde(tag = "type", content = "payload")]` representation.
#[cfg(feature = "agent")]
impl MiniSerialize for BroadcastMessage {
    fn begin(&self) -> ser::Fragment<'_> {
        match self {
            &BroadcastMessage::AgentStartup(ref payload) => {
                // build a small map with two entries
                struct BMsgMap<'payload> {
                    payload: &'payload StartupBroadcast,
                    parse_step: usize,
                }

                impl ser::Map for BMsgMap<'_> {
                    fn next(&mut self) -> Option<(Cow<'_, str>, &dyn MiniSerialize)> {
                        self.parse_step += 1;
                        match self.parse_step {
                            1 => Some((Cow::Borrowed("type"), &"AgentStartup")),
                            2 => Some((Cow::Borrowed("payload"), self.payload)),
                            _ => None,
                        }
                    }
                }

                ser::Fragment::Map(Box::new(BMsgMap {
                    payload,
                    parse_step: 0,
                }))
            }
        }
    }
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

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match *self {
                    $($name::$variant => write!(f, "{}", $str),)*
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "agent")]
    use miniserde::json;

    #[cfg(feature = "coordinator")]
    #[test]
    fn coordinator_message_serialization() {
        let msg = CoordinatorMessage::Shutdown;
        let serialized = msg.to_string();
        assert_eq!(serialized, "shutdown");
    }

    #[cfg(feature = "agent")]
    #[test]
    fn agent_message_deserialization() {
        use core::str::FromStr as _;

        let message = "shutdown";
        let deserialized = CoordinatorMessage::from_str(message).unwrap();
        assert_eq!(deserialized, CoordinatorMessage::Shutdown);
    }

    #[cfg(feature = "agent")]
    #[test]
    fn broadcast_message_serialization() {
        let startup = StartupBroadcast {
            hostname: "h".into(),
            agent_version: "v".into(),
            port: 1234,
            mac_address: "aa:bb".into(),
            ip_address: "1.2.3.4".into(),
            timestamp: 0,
        };
        let msg = BroadcastMessage::AgentStartup(startup.clone());
        let serialized = json::to_string(&msg);
        // should contain the correct tag and some payload fields
        assert!(serialized.contains("\"type\":\"AgentStartup\""));
        assert!(serialized.contains("\"hostname\":\"h\""));
    }

    #[cfg(feature = "coordinator")]
    #[test]
    fn broadcast_message_deserialization() {
        let json = r#"{"type":"AgentStartup","payload":{"hostname":"h","agent_version":"v","port":1234,"mac_address":"aa:bb","ip_address":"1.2.3.4","timestamp":0}}"#;
        let msg: BroadcastMessage = serde_json::from_str(json).unwrap();
        let BroadcastMessage::AgentStartup(s) = msg;
        assert_eq!(s.hostname, "h");
        assert_eq!(s.port, 1234);
    }
}
