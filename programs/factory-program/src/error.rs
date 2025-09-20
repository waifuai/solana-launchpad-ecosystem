use anchor_lang::prelude::*;

/// Defines the custom errors that the factory-program can return.
#[error_code]
pub enum FactoryError {
    #[msg("Invalid amount provided. Amount must be greater than zero.")]
    InvalidAmount,
    #[msg("A calculation in the program resulted in an arithmetic overflow.")]
    Overflow,
    #[msg("Mathematical underflow occurred.")]
    Underflow,
    #[msg("Division by zero attempted.")]
    DivisionByZero,
    #[msg("Insufficient SOL funds to complete the purchase at the current token price.")]
    InsufficientFunds,
    #[msg("The signer's public key does not match the authority stored in the launch state.")]
    AuthorityMismatch,
    #[msg("The provided affiliate public key does not match the key stored in the affiliate info account.")]
    AffiliateMismatch,

    // Launch state errors
    #[msg("Launch is not currently active.")]
    LaunchNotActive,
    #[msg("Maximum token supply has been reached.")]
    MaxSupplyReached,
    #[msg("Invalid launch time configuration.")]
    InvalidLaunchTime,
    #[msg("Invalid pricing model specified.")]
    InvalidPricingModel,

    // Vesting errors
    #[msg("Vesting schedule not found or invalid.")]
    VestingScheduleNotFound,
    #[msg("No tokens available to claim yet.")]
    NoTokensToClaim,
    #[msg("Vesting period has not completed.")]
    VestingNotComplete,
    #[msg("Invalid vesting parameters.")]
    InvalidVestingParams,

    // Anti-bot errors
    #[msg("Purchase amount is below minimum allowed.")]
    PurchaseAmountTooLow,
    #[msg("Purchase amount exceeds maximum allowed.")]
    PurchaseAmountTooHigh,
    #[msg("Purchase cooldown is still active.")]
    PurchaseCooldownActive,
    #[msg("Anti-bot validation failed.")]
    AntiBotValidationFailed,

    // Fee errors
    #[msg("Invalid fee configuration.")]
    InvalidFeeConfig,
    #[msg("Fee calculation overflow.")]
    FeeCalculationOverflow,

    // Time-related errors
    #[msg("Invalid timestamp provided.")]
    InvalidTimestamp,
    #[msg("Operation is outside allowed time window.")]
    OutsideTimeWindow,

    // Account validation errors
    #[msg("Invalid account state for operation.")]
    InvalidAccountState,
    #[msg("Account not initialized.")]
    AccountNotInitialized,
}