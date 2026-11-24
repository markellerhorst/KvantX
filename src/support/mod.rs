//! Infrastructure without domain knowledge: hashing, decompression, audit trail, file locations, undo history, number formats.

/// Hash-chained audit trails.
pub mod audit;
/// Number formatting for the user interface.
pub mod fmt;
/// Bounded undo and redo history.
pub mod history;
/// Decompression of zlib streams.
pub mod inflate;
/// Locations of the files KvantX writes.
pub mod paths;
/// SHA-256 hashing.
pub mod sha256;
/// Synthetic data for the test suites.
pub mod testkit;
