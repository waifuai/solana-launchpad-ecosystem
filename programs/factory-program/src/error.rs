use anchor_lang::prelude::*;

/// Defines the custom errors that the factory-program can return.
#[error_code]
pub enum FactoryError {
    #[msg("Invalid amount provided. Amount must be greater than zero.")]
    InvalidAmount,
    #[msg("A calculation in the program resulted in an arithmetic overflow.")]
    Overflow,
    #[msg("Insufficient SOL funds to complete the purchase at the current token price.")]
    InsufficientFunds,
    #[msg("The signer's public key does not match the authority stored in the launch state.")]
    AuthorityMismatch,
    #[msg("The provided affiliate public key does not match the key stored in the affiliate info account.")]
    AffiliateMismatch,
}