use anchor_lang::prelude::*;

/// The price precision for the oracle price. A price of 1,000,000,000 means 1 token A = 1 token B.
pub const ORACLE_PRICE_PRECISION: u64 = 1_000_000_000;
/// The maximum age of an oracle price in seconds before it is considered stale. (e.g., 5 minutes)
pub const MAX_ORACLE_AGE_SECONDS: i64 = 300;

/// State account for a liquidity pool. This is an oracle-based pool.
/// PDA seeds: `[b"liquidity_pool", mint_a.key().as_ref(), mint_b.key().as_ref()]`
#[account]
pub struct LiquidityPool {
    /// The mint address of the first token in the pair (token A).
    pub mint_a: Pubkey,
    /// The mint address of the second token in the pair (token B).
    pub mint_b: Pubkey,
    /// The designated authority allowed to push price updates.
    pub oracle_authority: Pubkey,
    /// The AI-provided price of token A in terms of token B, with 9 decimals of precision.
    pub oracle_price: u64,
    /// The Unix timestamp of the last successful price update.
    pub last_oracle_update: i64,
    /// The bump seed for `vault_a`.
    pub vault_a_bump: u8,
    /// The bump seed for `vault_b`.
    pub vault_b_bump: u8,
}

impl LiquidityPool {
    /// The total disk space required for a `LiquidityPool` account in bytes.
    /// Pubkey(32)*3 + u64(8) + i64(8) + u8(1) + u8(1) = 96 + 16 + 2 = 114 bytes.
    pub const LEN: usize = 32 + 32 + 32 + 8 + 8 + 1 + 1;
}