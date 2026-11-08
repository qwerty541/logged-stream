//! The crate's unit tests.
//!
//! The whole tree is compiled only under `#[cfg(test)]`, via the gated `mod tests;` declaration
//! in [`lib.rs`](../lib.rs), so the submodules need no `cfg` attribute of their own.
//!
//! Each library module's tests are in a file of the same name. The `LoggedStream` suite is split
//! by the behavior it covers: `stream` keeps the basics, and the `stream_*` files cover filtering,
//! errors, lifecycle events and asynchronous IO. `helpers` holds the scaffolding they share: the
//! `LoggedStream` suite's test doubles, and the doubles and assertions other files use too.
//!
//! The tests live in `crate::tests`, outside the modules they test, so they can reach only `pub`
//! and `pub(crate)` items: a test that needs a private item needs that item made `pub(crate)`,
//! as `EventFilterDebug` is.

mod buffer_formatter;
mod event;
mod filter;
mod helpers;
mod logger;
mod stream;
mod stream_async_io;
mod stream_errors;
mod stream_filtering;
mod stream_lifecycle;
