//! The deterministic simulation contracts and kernel.

pub mod contracts;
pub mod error;
pub mod events;
/// The stepping kernel.
pub mod kernel;
/// The `pcg32-v1` generator and its persisted state.
pub mod pcg32;
pub mod positioning;
/// Rider identity and canonical order.
pub mod rider_id;
pub mod snapshot;
/// Distance-indexed simulation terrain.
pub mod terrain;
pub(crate) mod validate;
