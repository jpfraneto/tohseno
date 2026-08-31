//! Public catalog and source-transport policy above the frozen TOHSENO protocol.
//!
//! This crate deliberately does not own a wallet, RPC credential, Apple
//! signing identity, UI, or server. It defines the closed signed catalog
//! object and the deterministic source artifact both sides verify.

#![forbid(unsafe_code)]

pub mod build_profile;
pub mod catalog;
pub mod claim_mark;
pub mod claims;
pub mod claims_activation;
pub mod evidence;
pub mod publication;
pub mod snapshot;

mod error;

pub use error::{NetworkError, Result};
