//! # Genesis Common Crate
//!
//! This crate provides shared resources and utilities for the Solana Launchpad Ecosystem.
//! It serves as the foundation library used by all on-chain programs to ensure consistency
//! and reduce code duplication across the ecosystem.
//!
//! ## Purpose
//!
//! The genesis-common crate centralizes:
//! - **PDA seeds**: Standardized seeds for Program Derived Addresses across all programs
//! - **Mathematical utilities**: Safe arithmetic operations with overflow protection
//! - **Time utilities**: Helper functions for timestamp validation and vesting calculations
//! - **Error definitions**: Common error types shared across programs
//!
//! ## Architecture
//!
//! This crate is designed to be used by all on-chain programs in the ecosystem:
//! - `programs/factory-program`
//! - `programs/affiliate-program`
//! - `programs/barter-dex-program`
//!
//! ## Modules
//!
//! - [`constants`]: Program Derived Address (PDA) seeds and system-wide constants
//! - [`utils`]: Utility functions for math operations, time handling, and PDA derivation

/// This crate provides shared constants, specifically PDA seeds,
/// to be used across all on-chain programs in the ecosystem.
/// This ensures consistency and prevents typos when deriving PDAs.
pub mod constants;

/// Utility functions for common operations across programs
pub mod utils;