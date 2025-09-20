//! # Barter DEX Program Error Definitions
//!
//! This module defines all custom error codes used by the oracle-based DEX program.
//! These errors provide detailed feedback for various failure conditions encountered
//! during DEX operations, oracle interactions, and liquidity management.
//!
//! ## Error Categories
//!
//! - **Trading Errors**: Slippage, liquidity, and swap execution failures
//! - **Oracle Errors**: Price feed validation, staleness, and authority issues
//! - **Fee Calculation**: Dynamic fee computation and validation errors
//! - **Pool Management**: Configuration and state management issues
//!
//! ## Oracle-Specific Errors
//!
//! Specialized errors for multi-oracle integration:
//! - Price feed validation for Pyth, Switchboard, and AI oracles
//! - Confidence interval validation for price reliability
//! - Multi-source aggregation failures
//!
//! ## Usage
//!
//! All errors are defined using Anchor's `#[error_code]` attribute and
//! include descriptive messages that will be returned to users when
//! transactions fail. Error codes are automatically generated and can
//! be used for programmatic error handling and monitoring.

use anchor_lang::prelude::*;

/// Defines the custom errors that the barter-dex-program can return.
#[error_code]
pub enum BarterError {
    #[msg("The calculated swap amount is less than the minimum amount out specified, indicating slippage tolerance was exceeded.")]
    SlippageExceeded,
    #[msg("The liquidity pool does not have enough tokens to fulfill the requested swap.")]
    InsufficientLiquidity,
    #[msg("A calculation in the program resulted in an arithmetic overflow.")]
    Overflow,
    #[msg("Mathematical underflow occurred.")]
    Underflow,
    #[msg("A provided token account has a mint that does not match the expected mint for this pool.")]
    InvalidMint,
    #[msg("The signer is not the designated oracle authority for this pool.")]
    InvalidOracleAuthority,
    #[msg("The oracle price is too old and has not been updated recently. The DEX is paused until a new price is pushed.")]
    OraclePriceStale,

    // Oracle integration errors
    #[msg("Pyth oracle price feed not found or invalid.")]
    PythPriceFeedNotFound,
    #[msg("Switchboard oracle feed not found or invalid.")]
    SwitchboardFeedNotFound,
    #[msg("AI oracle program not found or invalid.")]
    AIOracleProgramNotFound,
    #[msg("Failed to fetch price from oracle.")]
    OraclePriceFetchFailed,
    #[msg("Oracle price confidence interval too high.")]
    OraclePriceConfidenceTooHigh,
    #[msg("No valid price sources available.")]
    NoValidPriceSources,

    // Dynamic fee errors
    #[msg("Dynamic fee calculation failed.")]
    DynamicFeeCalculationFailed,
    #[msg("Fee exceeds maximum allowed limit.")]
    FeeExceedsMaximum,
    #[msg("Invalid volatility calculation.")]
    InvalidVolatilityCalculation,

    // Pool management errors
    #[msg("Pool is currently paused.")]
    PoolPaused,
    #[msg("Insufficient liquidity for token.")]
    InsufficientTokenLiquidity,
    #[msg("Pool configuration is invalid.")]
    InvalidPoolConfiguration,
    #[msg("Price history is not available.")]
    PriceHistoryNotAvailable,
}