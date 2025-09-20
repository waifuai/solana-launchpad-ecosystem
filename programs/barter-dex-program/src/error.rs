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