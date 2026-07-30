//! A protocol-driven TOHSENO lineage node.
//!
//! Nodes keep an append-only, content-addressed subset of eligible ordinary
//! lineage as neutral or legacy evidence. They deterministically validate what
//! they possess, rebuild all indexes from canonical actions, and synchronize
//! only when explicitly asked. They do not elect a universal head or gain
//! ownership by storing an action. An unanchored evidence segment may be
//! preserved with unresolved authority; a complete neutral reduction may
//! verify, but active-generation public authority remains unresolved while no
//! release-authorized contract generation is active. Retired v0.7 CREATE2
//! inputs remain available for offline reproduction only and never establish
//! active public authority.

#![forbid(unsafe_code)]

mod error;
mod fs;
mod http;
mod legacy;
mod model;
mod protocol_adapter;
mod store;
mod sync;

pub use error::{NodeError, Result};
pub use http::{router, serve};
pub use legacy::predict_retired_v07_builder_id;
pub use model::{
    ActionReference, ActionValidation, AuthorityStatus, Health, IngestOutcome, IntegrityIssue,
    IntegrityReport, MissingArtifact, MissingParentReference, NodeIdentity, NodeInfo,
    PeerDescription, PeerSyncResult, SegmentStatus, ShotSummary, ShotView, SignedRecordStatus,
    SyncReport, SyncState, ValidationCounts,
};
pub use store::{NodeStore, MAX_ACTIONS_PER_NODE, MAX_ACTION_BYTES};
pub use sync::{Node, Peer, MAX_PEERS};
pub use tohseno_protocol::lineage::{LINEAGE_PROTOCOL, LINEAGE_PROTOCOL_VERSION};

pub const NODE_PROTOCOL: &str = "tohseno.node/2";
