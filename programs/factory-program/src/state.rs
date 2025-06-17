use anchor_lang::prelude::*;

/// State account for a token launch. This account stores all the configuration
/// and live data for a single ICO. It is a PDA.
/// PDA seeds: `[b"launch_state", authority.key().as_ref(), token_mint.key().as_ref()]`
#[account]
pub struct LaunchState {
    /// The public key of the authority allowed to withdraw funds from the SOL vault.
    pub authority: Pubkey,
    /// The public key of the SPL Token mint for this launch. This program is the mint authority.
    pub token_mint: Pubkey,
    /// The bump seed for the `sol_vault` PDA, used for signing withdrawals.
    pub sol_vault_bump: u8,
    /// The starting price for one whole token (10^9 units), in lamports.
    pub initial_price: u64,
    /// The rate at which the price increases per whole token sold (the slope of the bonding curve).
    pub slope: u64,
    /// The cumulative number of tokens sold so far (in whole token units).
    pub tokens_sold: u64,
}

impl LaunchState {
    /// The total disk space required for a `LaunchState` account in bytes.
    /// Pubkey (32) + Pubkey (32) + u8 (1) + u64 (8) + u64 (8) + u64 (8) = 89 bytes.
    pub const LEN: usize = 32 + 32 + 1 + 8 + 8 + 8;
}