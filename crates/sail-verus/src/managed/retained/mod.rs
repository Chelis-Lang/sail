//! Content-bound retained-proof building blocks (proposed ADR 0050).
//!
//! Deliberately disconnected from the driver until the actual GHOST/EXEC
//! consumer, closed macro policy and native ABA gate qualify together. The old
//! event-based retained-proof path is not imported or enabled here.

mod archive;
mod compiler;
mod dependencies;
