use anchor_lang::prelude::constant;

/// Seed for the `LaunchState` PDA in the `factory-program`.
#[constant]
pub const LAUNCH_STATE_SEED: &[u8] = b"launch_state";

/// Seed for the `sol_vault` PDA in the `factory-program`.
#[constant]
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";

/// Seed for the `AffiliateInfo` PDA in the `affiliate-program`.
#[constant]
pub const AFFILIATE_INFO_SEED: &[u8] = b"affiliate_info";

/// Seed for the `LiquidityPool` PDA in the `barter-dex-program`.
#[constant]
pub const LIQUIDITY_POOL_SEED: &[u8] = b"liquidity_pool";

/// Seed for the token vault PDAs in the `barter-dex-program`.
#[constant]
pub const POOL_VAULT_SEED: &[u8] = b"pool_vault";