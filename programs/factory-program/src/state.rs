use anchor_lang::prelude::*;
use genesis_common::constants::*;

/// Pricing model enumeration for different launch strategies
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PricingModel {
    /// Linear bonding curve: price = initial_price + (slope * tokens_sold)
    LinearBondingCurve,
    /// Exponential bonding curve: price = initial_price * (1 + slope)^tokens_sold
    ExponentialBondingCurve,
    /// Fixed price: constant price regardless of tokens sold
    FixedPrice,
    /// Dutch auction: price decreases over time
    DutchAuction,
}

/// Anti-bot protection level
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum AntiBotLevel {
    /// No anti-bot measures
    None,
    /// Basic: minimum purchase limits and time delays
    Basic,
    /// Advanced: proof of work, rate limiting, and wallet analysis
    Advanced,
    /// Maximum: full KYC integration and comprehensive checks
    Maximum,
}

/// State account for a token launch with advanced features
#[account]
pub struct LaunchState {
    /// The public key of the authority allowed to withdraw funds from the SOL vault.
    pub authority: Pubkey,
    /// The public key of the SPL Token mint for this launch. This program is the mint authority.
    pub token_mint: Pubkey,
    /// The bump seed for the `sol_vault` PDA, used for signing withdrawals.
    pub sol_vault_bump: u8,

    /// Pricing configuration
    pub pricing_model: PricingModel,
    /// The starting price for one whole token (10^9 units), in lamports.
    pub initial_price: u64,
    /// The rate at which the price increases per whole token sold (slope for linear, multiplier for exponential).
    pub slope: u64,
    /// The cumulative number of tokens sold so far (in whole token units).
    pub tokens_sold: u64,

    /// Vesting configuration
    pub vesting_enabled: bool,
    pub vesting_duration_seconds: i64,
    pub vesting_cliff_seconds: i64,

    /// Anti-bot protection settings
    pub anti_bot_level: AntiBotLevel,
    pub min_purchase_amount: u64,
    pub max_purchase_amount: u64,
    pub purchase_cooldown_seconds: i64,
    pub last_purchase_timestamp: i64,

    /// Launch constraints
    pub max_tokens: u64,
    pub launch_start_time: i64,
    pub launch_end_time: i64,

    /// Fee configuration
    pub affiliate_fee_bps: u16,
    pub platform_fee_bps: u16,
    pub platform_fee_recipient: Pubkey,

    /// Analytics and tracking
    pub total_sol_collected: u64,
    pub total_fees_collected: u64,
    pub purchase_count: u64,
}

impl LaunchState {
    /// The total disk space required for a `LaunchState` account in bytes.
    pub const LEN: usize = 32 + 32 + 1 + // authority, token_mint, sol_vault_bump
        1 + 8 + 8 + 8 + // pricing_model, initial_price, slope, tokens_sold
        1 + 8 + 8 + // vesting_enabled, vesting_duration, vesting_cliff
        1 + 8 + 8 + 8 + 8 + // anti_bot_level, min/max_purchase, cooldown, last_purchase
        8 + 8 + 8 + // max_tokens, launch_start/end_time
        2 + 2 + 32 + // affiliate_fee, platform_fee, platform_recipient
        8 + 8 + 8; // total_sol, total_fees, purchase_count

    /// Check if the launch is currently active
    pub fn is_launch_active(&self) -> Result<bool> {
        let current_time = Clock::get()?.unix_timestamp;
        Ok(current_time >= self.launch_start_time && current_time <= self.launch_end_time)
    }

    /// Check if maximum token supply has been reached
    pub fn is_max_supply_reached(&self) -> bool {
        self.tokens_sold >= self.max_tokens
    }

    /// Calculate current price based on pricing model
    pub fn calculate_current_price(&self) -> Result<u64> {
        match self.pricing_model {
            PricingModel::LinearBondingCurve => {
                genesis_common::utils::math_utils::calculate_bonding_curve_price(
                    self.initial_price,
                    self.slope,
                    self.tokens_sold,
                )
            }
            PricingModel::ExponentialBondingCurve => {
                // For exponential: price = initial_price * (1 + slope)^tokens_sold
                // Using approximation for on-chain computation
                let multiplier = self.slope as u128;
                let tokens_sold_u128 = self.tokens_sold as u128;
                let initial_price_u128 = self.initial_price as u128;

                let exponential_factor = multiplier.checked_pow(tokens_sold_u128 as u32)
                    .ok_or(error!(FactoryError::Overflow))?;

                let current_price_u128 = initial_price_u128.checked_mul(exponential_factor)
                    .ok_or(error!(FactoryError::Overflow))?;

                Ok(current_price_u128.try_into().map_err(|_| FactoryError::Overflow)?)
            }
            PricingModel::FixedPrice => Ok(self.initial_price),
            PricingModel::DutchAuction => {
                // For Dutch auction, price decreases over time
                let current_time = Clock::get()?.unix_timestamp;
                let time_elapsed = current_time.saturating_sub(self.launch_start_time);
                let total_duration = self.launch_end_time.saturating_sub(self.launch_start_time);

                if total_duration == 0 {
                    return Ok(self.initial_price);
                }

                let price_reduction = ((self.initial_price as u128) * (time_elapsed as u128)) / (total_duration as u128);
                let current_price = self.initial_price.saturating_sub(price_reduction as u64);

                Ok(std::cmp::max(current_price, self.slope)) // slope acts as minimum price
            }
        }
    }

    /// Validate purchase amount against anti-bot rules
    pub fn validate_purchase_amount(&self, amount: u64) -> Result<()> {
        match self.anti_bot_level {
            AntiBotLevel::None => {},
            _ => {
                require!(amount >= self.min_purchase_amount, FactoryError::PurchaseAmountTooLow);
                require!(amount <= self.max_purchase_amount, FactoryError::PurchaseAmountTooHigh);

                if self.anti_bot_level >= AntiBotLevel::Advanced {
                    let current_time = Clock::get()?.unix_timestamp;
                    let time_since_last_purchase = current_time - self.last_purchase_timestamp;
                    require!(time_since_last_purchase >= self.purchase_cooldown_seconds,
                            FactoryError::PurchaseCooldownActive);
                }
            }
        }
        Ok(())
    }
}

/// Vesting schedule account for tracking token vesting
#[account]
pub struct VestingSchedule {
    /// The launch state this vesting schedule belongs to
    pub launch_state: Pubkey,
    /// The beneficiary who will receive the vested tokens
    pub beneficiary: Pubkey,
    /// Total amount of tokens to be vested
    pub total_amount: u64,
    /// Amount of tokens already claimed
    pub claimed_amount: u64,
    /// Vesting start timestamp
    pub start_time: i64,
    /// Total vesting duration in seconds
    pub duration_seconds: i64,
    /// Cliff period in seconds (no tokens before this)
    pub cliff_seconds: i64,
    /// Last claim timestamp
    pub last_claim_time: i64,
}

impl VestingSchedule {
    /// Space required for vesting schedule account
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8; // 104 bytes

    /// Calculate vested amount at current time
    pub fn calculate_vested_amount(&self, current_time: i64) -> Result<u64> {
        if current_time < self.start_time + self.cliff_seconds {
            return Ok(0);
        }

        let time_since_start = current_time - self.start_time;
        if time_since_start >= self.duration_seconds {
            return Ok(self.total_amount);
        }

        // Linear vesting after cliff
        let vesting_time = self.duration_seconds - self.cliff_seconds;
        let vested_amount = ((self.total_amount as u128) * (time_since_start as u128)) / (vesting_time as u128);

        Ok(vested_amount as u64)
    }

    /// Calculate claimable amount
    pub fn calculate_claimable_amount(&self, current_time: i64) -> Result<u64> {
        let vested_amount = self.calculate_vested_amount(current_time)?;
        Ok(vested_amount.saturating_sub(self.claimed_amount))
    }
}

/// Purchase tracking for anti-bot measures
#[account]
pub struct PurchaseTracker {
    /// The buyer who made the purchase
    pub buyer: Pubkey,
    /// Last purchase timestamp
    pub last_purchase_time: i64,
    /// Total amount purchased by this buyer
    pub total_purchased: u64,
    /// Number of purchases made by this buyer
    pub purchase_count: u32,
}

impl PurchaseTracker {
    /// Space required for purchase tracker account
    pub const LEN: usize = 32 + 8 + 8 + 4; // 52 bytes
}