//! Core engine of OSC Tool VRC.
//!
//! Everything here talks to VRChat exclusively through its official OSC
//! interface (UDP, default 127.0.0.1:9000). The crate never opens, reads or
//! otherwise touches the VRChat process.

pub mod config;
pub mod engine;
pub mod hr;
pub mod lyrics;
pub mod media;
pub mod osc;
pub mod scanner;
pub mod stats;
pub mod template;

pub use config::Config;
pub use engine::{Engine, Snapshot};
