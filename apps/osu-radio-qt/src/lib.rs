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
