//! The GDExtension. GDScript draws and lays out; every rule lives in Rust.
//!
//! [`app`] is plain Rust, tested without an engine. [`gd`] only converts it to Godot types.
//! Trainer access goes through `ble_trainer`, the SIM-only chokepoint: nothing here can reach an
//! ERG write.

#![deny(unsafe_code)]

pub mod app;
pub mod gd;

#[allow(unsafe_code)]
mod entry {
    use godot::prelude::*;

    /// The extension's entry point; gdext's registration requires the `unsafe impl`.
    struct BikeCore;

    #[gdextension]
    unsafe impl ExtensionLibrary for BikeCore {}
}
