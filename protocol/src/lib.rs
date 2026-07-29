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
pub mod record;
pub mod signature;
pub mod tree_hash;

mod error;
mod text;

pub use error::{ProtocolError, Result};
