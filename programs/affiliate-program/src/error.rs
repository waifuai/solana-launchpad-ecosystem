//! # Affiliate Program Error Definitions
//!
//! This module defines all custom error codes used by the affiliate program.
//! These errors provide detailed feedback for various failure conditions
//! encountered during affiliate operations.
//!
//! ## Error Categories
//!
//! - **Rate Management**: Errors related to commission rate validation and updates
//! - **Analytics**: Errors for performance tracking and tier management
//! - **Referrals**: Errors for multi-level referral validation
//! - **Time Constraints**: Errors for timestamp and timing validation
//! - **Account Management**: Errors for account initialization and state
//!
//! ## Usage
//!
//! All errors are defined using Anchor's `#[error_code]` attribute and
//! include descriptive messages that will be returned to users when
//! transactions fail. Error codes are automatically generated and can
//! be used for programmatic error handling.

use anchor_lang::prelude::*;

/// Defines the custom errors that the affiliate-program can return.
#[error_code]
pub enum AffiliateError {
    #[msg("The provided commission rate is invalid. It must be between 0 and 10000 basis points.")]
    InvalidRate,
    #[msg("A calculation in the program resulted in an arithmetic overflow.")]
    Overflow,
    #[msg("Mathematical underflow occurred.")]
    Underflow,
    #[msg("The signer's public key does not match the required authority for the operation.")]
    AuthorityMismatch,

    // Rate cap and timing errors
    #[msg("Commission rate exceeds maximum allowed cap.")]
    RateExceedsMaxCap,
    #[msg("Commission rate is below minimum allowed cap.")]
    RateBelowMinCap,
    #[msg("Rate update is not allowed at this time.")]
    RateUpdateNotAllowed,

    // Analytics and performance errors
    #[msg("Analytics data not found or invalid.")]
    AnalyticsNotFound,
    #[msg("Performance tier upgrade not allowed.")]
    TierUpgradeNotAllowed,
    #[msg("Invalid performance metrics.")]
    InvalidPerformanceMetrics,

    // Multi-level referral errors
    #[msg("Invalid referral level specified.")]
    InvalidReferralLevel,
    #[msg("Parent affiliate not found.")]
    ParentAffiliateNotFound,
    #[msg("Circular referral relationship detected.")]
    CircularReferral,

    // Time-related errors
    #[msg("Invalid timestamp provided.")]
    InvalidTimestamp,
    #[msg("Operation is outside allowed time window.")]
    OutsideTimeWindow,

    // Account validation errors
    #[msg("Affiliate account not initialized.")]
    AccountNotInitialized,
    #[msg("Affiliate account already exists.")]
    AccountAlreadyExists,
}