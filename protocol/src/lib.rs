//! The pure TOHSENO protocol candidate.
//!
//! This crate defines protocol objects and their exact byte interpretation.
//! It has no terminal, RPC, global filesystem, Apple-signing, server, Studio,
//! or coding-harness policy.

#![forbid(unsafe_code)]

pub mod actions;
pub mod app_metadata;
pub mod builder;
pub mod canonical;
pub mod conformance;
pub mod continuity;
pub mod digest;
pub mod evolution;
pub mod fascia;
pub mod fascia_tree;
pub mod genesis;
pub mod identity;
pub mod lineage;
pub mod ontology;
pub mod public_checkpoint;
pub mod record;
pub mod signature;
pub mod tree_hash;

mod error;
mod text;

pub use error::{ProtocolError, Result};
pub use lineage::{
    adapt_v1_lineage, apply_lineage_actions, reduce_lineage, verify_lineage_segment, LineageAction,
    LineagePayload, SignedLineageAction,
};
pub use ontology::{
    canonical_capability_graph_bytes, capability_graph_digest, organ_acceptance_gate_name,
    ArtifactAvailability, Evolution, EvolutionaryIntent, Expression, Feedback, Genome,
    GenomeAcceptance, GenomeProposal, IntentionRecord, Organ, Ownership, ParentRelation,
    ShotCommitment, TokenAssociation, VerificationResult, VersionRecord,
};
