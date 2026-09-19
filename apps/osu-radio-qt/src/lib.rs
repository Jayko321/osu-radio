#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]
//! Qt bridge and runtime glue. Shared application behavior lives in osu-radio-client.
pub mod bridge;
pub mod runtime;

// CXX-Qt inherited-method declarations cannot carry must_use attributes.
#[allow(clippy::must_use_candidate)]
pub mod app_bridge;
