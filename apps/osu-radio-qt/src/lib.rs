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
#[expect(
    clippy::unnecessary_box_returns,
    reason = "CXX-Qt generated constructor declarations return Box to transfer Rust state through the C++ ABI"
)]
pub mod app_bridge;

#[allow(clippy::must_use_candidate)]
#[expect(
    clippy::unnecessary_box_returns,
    reason = "CXX-Qt generated constructor declarations return Box to transfer Rust state through the C++ ABI"
)]
pub mod qt_view_models;
