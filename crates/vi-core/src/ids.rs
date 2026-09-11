//! ULID-based identifiers. One newtype per entity so ids cannot be mixed up.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub ulid::Ulid);

        impl $name {
            /// A fresh, time-ordered random id.
            pub fn new() -> Self {
                Self(ulid::Ulid::generate())
            }

            /// The nil id (all zeros). Used as a sentinel, never stored.
            pub const fn nil() -> Self {
                Self(ulid::Ulid::nil())
            }

            /// Parse from the 26-character Crockford base32 form.
            pub fn parse(s: &str) -> Result<Self, ulid::DecodeError> {
                ulid::Ulid::from_string(s).map(Self)
            }

            /// Underlying 128-bit value.
            pub const fn as_u128(&self) -> u128 {
                self.0 .0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl FromStr for $name {
            type Err = ulid::DecodeError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }

        impl From<ulid::Ulid> for $name {
            fn from(u: ulid::Ulid) -> Self {
                Self(u)
            }
        }
    };
}

define_id!(
    /// Identifies a [`crate::model::Video`].
    VideoId
);
define_id!(
    /// Identifies a [`crate::model::Track`].
    TrackId
);
define_id!(
    /// Identifies a [`crate::model::Segment`].
    SegmentId
);
define_id!(
    /// Identifies a [`crate::model::FrameSample`].
    FrameSampleId
);
define_id!(
    /// Identifies a transcript or OCR span.
    SpanId
);
define_id!(
    /// Identifies a [`crate::model::Description`].
    DescriptionId
);
define_id!(
    /// Identifies an [`crate::model::Entity`].
    EntityId
);
define_id!(
    /// Identifies an [`crate::model::Event`].
    EventId
);
define_id!(
    /// Identifies an [`crate::model::Embedding`] row.
    EmbeddingId
);
define_id!(
    /// Identifies a [`crate::model::Provenance`] row.
    ProvenanceId
);
define_id!(
    /// Identifies an indexing job.
    JobId
);
define_id!(
    /// Identifies an index (the whole directory).
    IndexId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip_through_strings_and_json() {
        let id = VideoId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 26);
        assert_eq!(VideoId::parse(&s).unwrap(), id);
        let j = serde_json::to_string(&id).unwrap();
        assert_eq!(j, format!("\"{s}\""));
        assert_eq!(serde_json::from_str::<VideoId>(&j).unwrap(), id);
    }

    #[test]
    fn ids_are_time_ordered() {
        let a = JobId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = JobId::new();
        assert!(a < b);
    }
}
