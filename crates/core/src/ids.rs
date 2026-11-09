//! Typed identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! typed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            pub fn from_str(s: &str) -> Result<Self, crate::error::Error> {
                if s.is_empty() {
                    return Err(crate::error::Error::InvalidId(s.to_string()));
                }
                Ok(Self(s.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $prefix, self.0)
            }
        }
    };
}

typed_id!(AgentId, "agent");
typed_id!(ChannelId, "channel");
typed_id!(MessageId, "msg");
typed_id!(QueueId, "queue");
typed_id!(RoutineId, "routine");
typed_id!(TurnId, "turn");
