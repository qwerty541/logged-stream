//! The crate's unit tests.
//!
//! The whole tree is compiled only under `#[cfg(test)]`, via the gated `mod tests;` declaration
//! in [`lib.rs`](../lib.rs), so the submodules need no `cfg` attribute of their own.
//!
//! A library module's tests go in the file of the same name (`record` has none yet). The
//! `LoggedStream` suite is split by the behavior it covers, each file keeping that behavior's
//! synchronous and asynchronous tests together: `stream` holds the basics, and the `stream_*`
//! files cover filtering, errors and lifecycle events. `helpers` holds what two or more files
//! use; a test double or assertion that only one file uses stays private to that file.
//!
//! These tests live in `crate::tests`, outside the modules they test, so they see only what the
//! crate root sees: an item private to its module is out of reach until it is made
//! `pub(crate)`, as `EventFilterDebug` is.

mod buffer_formatter;
mod event;
mod filter;
mod helpers;
mod logger;
mod stream;
mod stream_errors;
mod stream_filtering;
mod stream_lifecycle;
