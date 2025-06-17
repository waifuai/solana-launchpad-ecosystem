use anchor_lang::prelude::*;

/// Defines the custom errors that the affiliate-program can return.
#[error_code]
pub enum AffiliateError {
    #[msg("The provided commission rate is invalid. It must be between 0 and 10000 basis points.")]
    InvalidRate,
    #[msg("A calculation in the program resulted in an arithmetic overflow.")]
    Overflow,
    #[msg("The signer's public key does not match the required authority for the operation.")]
    AuthorityMismatch,
}