use anchor_lang::prelude::constant;

/// Seed for the `LaunchState` PDA in the `factory-program`.
#[constant]
pub const LAUNCH_STATE_SEED: &[u8] = b"launch_state";

/// Seed for the `sol_vault` PDA in the `factory-program`.
#[constant]
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";

/// Seed for the vesting schedule PDA in the `factory-program`.
#[constant]
pub const VESTING_SCHEDULE_SEED: &[u8] = b"vesting_schedule";

/// Seed for the `AffiliateInfo` PDA in the `affiliate-program`.
#[constant]
pub const AFFILIATE_INFO_SEED: &[u8] = b"affiliate_info";

/// Seed for the affiliate analytics PDA in the `affiliate-program`.
#[constant]
pub const AFFILIATE_ANALYTICS_SEED: &[u8] = b"affiliate_analytics";

/// Seed for the `LiquidityPool` PDA in the `barter-dex-program`.
#[constant]
pub const LIQUIDITY_POOL_SEED: &[u8] = b"liquidity_pool";

/// Seed for the token vault PDAs in the `barter-dex-program`.
#[constant]
pub const POOL_VAULT_SEED: &[u8] = b"pool_vault";

/// Seed for the oracle price feed PDA in the `barter-dex-program`.
#[constant]
pub const ORACLE_PRICE_FEED_SEED: &[u8] = b"oracle_price_feed";

/// Mathematical constants for precision and calculations
pub const ORACLE_PRICE_PRECISION: u64 = 1_000_000_000; // 1e9 for price precision
pub const BPS_PRECISION: u64 = 10_000; // 100% = 10,000 basis points
pub const MAX_ORACLE_AGE_SECONDS: i64 = 300; // 5 minutes max oracle staleness
pub const MINIMUM_LIQUIDITY: u64 = 1_000_000; // Minimum liquidity tokens
pub const FEE_BPS: u16 = 30; // 0.3% fee in basis points

/// Security constants
pub const MAX_RATE_BPS: u16 = 2000; // Maximum 20% commission rate
pub const MIN_RATE_BPS: u16 = 50; // Minimum 0.5% commission rate
pub const MAX_VESTING_DURATION_SECONDS: i64 = 31_557_600; // 1 year in seconds
pub const MIN_VESTING_DURATION_SECONDS: i64 = 86_400; // 1 day in seconds

/// Performance optimization constants
pub const MAX_BATCH_SIZE: usize = 100; // Maximum batch processing size
pub const RETRY_ATTEMPTS: u32 = 3; // Number of retry attempts for transactions
pub const TRANSACTION_TIMEOUT_SECONDS: u64 = 30; // Transaction timeout