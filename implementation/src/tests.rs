// Copyright 2026 Salesforce, Inc. All rights reserved.

//! Test suite.
//!
//! `expr` is tested exhaustively and standalone because it is the component
//! most likely to produce silently wrong behaviour: a mis-evaluated rule is
//! not an error, it is a confident wrong answer reaching a customer.

mod a2d_demo;
mod common;
mod config_tests;
mod contract_tests;
mod delivery_tests;
mod expr_tests;
mod inject_tests;
mod overlay_tests;
mod policy_tests;
mod remote_tests;
mod schema_split_tests;
mod sse_tests;
