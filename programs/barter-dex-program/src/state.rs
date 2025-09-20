use anchor_lang::prelude::*;
use genesis_common::constants::*;

/// Oracle provider types for price feeds
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OracleProvider {
    /// Pyth Network oracle
    Pyth,
    /// Switchboard V2 oracle
    Switchboard,
    /// Custom AI-driven oracle
    AIOracle,
    /// Hybrid approach using multiple oracles
    Hybrid,
}

/// Liquidity pool state with enhanced oracle integration
#[account]
pub struct LiquidityPool {
    /// The mint address of the first token in the pair (token A).
    pub mint_a: Pubkey,
    /// The mint address of the second token in the pair (token B).
    pub mint_b: Pubkey,
    /// The designated authority allowed to push price updates.
    pub oracle_authority: Pubkey,

    /// Enhanced oracle configuration
    pub oracle_provider: OracleProvider,
    pub pyth_price_feed_a: Option<Pubkey>,
    pub pyth_price_feed_b: Option<Pubkey>,
    pub switchboard_feed: Option<Pubkey>,
    pub ai_oracle_program: Option<Pubkey>,

    /// Current price data
    pub oracle_price: u64,
    pub last_oracle_update: i64,
    pub price_confidence: u64, // Confidence interval for price

    /// Multiple price sources for hybrid approach
    pub pyth_price: Option<u64>,
    pub switchboard_price: Option<u64>,
    pub ai_price: Option<u64>,

    /// Price history for volatility calculation (circular buffer)
    pub price_history: [u64; 24], // Last 24 hours (hourly)
    pub history_index: u8,

    /// Liquidity and trading parameters
    pub total_liquidity_a: u64,
    pub total_liquidity_b: u64,
    pub fee_bps: u16, // Trading fee in basis points

    /// Advanced trading features
    pub dynamic_fee_enabled: bool,
    pub volatility_threshold: u64, // Price change threshold to trigger higher fees
    pub last_volatility_update: i64,

    /// Vault bump seeds
    pub vault_a_bump: u8,
    pub vault_b_bump: u8,
}

impl LiquidityPool {
    /// Enhanced space calculation
    pub const LEN: usize = 32 + 32 + 32 + // mint_a, mint_b, oracle_authority
        1 + (1 + 32) + (1 + 32) + (1 + 32) + (1 + 32) + // oracle config
        8 + 8 + 8 + // prices and confidence
        (1 + 8) + (1 + 8) + (1 + 8) + // multiple price sources
        (8 * 24) + 1 + // price history
        8 + 8 + 2 + // liquidity and fees
        1 + 8 + 8 + // dynamic fee settings
        1 + 1; // vault bumps

    /// Calculate weighted average price from multiple sources
    pub fn calculate_weighted_price(&self) -> Result<u64> {
        let mut total_weight: u64 = 0;
        let mut weighted_sum: u128 = 0;

        // Pyth weight: 40% if available
        if let Some(price) = self.pyth_price {
            weighted_sum += price as u128 * 40;
            total_weight += 40;
        }

        // Switchboard weight: 35% if available
        if let Some(price) = self.switchboard_price {
            weighted_sum += price as u128 * 35;
            total_weight += 35;
        }

        // AI price weight: 25% if available
        if let Some(price) = self.ai_price {
            weighted_sum += price as u128 * 25;
            total_weight += 25;
        }

        if total_weight == 0 {
            return Ok(self.oracle_price); // Fallback to last known price
        }

        let weighted_average = (weighted_sum / total_weight as u128) as u64;
        Ok(weighted_average)
    }

    /// Calculate price volatility based on history
    pub fn calculate_volatility(&self) -> Result<u64> {
        if self.history_index == 0 {
            return Ok(0);
        }

        let mut prices = Vec::new();
        for i in 0..self.history_index {
            prices.push(self.price_history[i as usize]);
        }

        if prices.len() < 2 {
            return Ok(0);
        }

        let mean = prices.iter().map(|&p| p as u128).sum::<u128>() / prices.len() as u128;
        let variance = prices.iter()
            .map(|&p| {
                let diff = if p as u128 > mean { p as u128 - mean } else { mean - p as u128 };
                diff * diff
            })
            .sum::<u128>() / prices.len() as u128;

        // Return standard deviation
        let volatility = ((variance as f64).sqrt() * ORACLE_PRICE_PRECISION as f64) as u64;
        Ok(volatility)
    }

    /// Calculate dynamic fee based on volatility
    pub fn calculate_dynamic_fee(&self) -> Result<u16> {
        if !self.dynamic_fee_enabled {
            return Ok(self.fee_bps);
        }

        let volatility = self.calculate_volatility()?;
        let base_fee = self.fee_bps as u64;

        // Increase fee by up to 5x based on volatility
        let volatility_multiplier = if volatility > self.volatility_threshold {
            std::cmp::min(5, (volatility / self.volatility_threshold) as u16)
        } else {
            1
        };

        let dynamic_fee = base_fee * volatility_multiplier as u64;
        Ok(std::cmp::min(dynamic_fee, 1000) as u16) // Cap at 10%
    }

    /// Update price history
    pub fn update_price_history(&mut self, new_price: u64) {
        self.price_history[self.history_index as usize] = new_price;
        self.history_index = ((self.history_index as usize + 1) % 24) as u8;
    }

    /// Check if oracle price is stale
    pub fn is_oracle_stale(&self) -> Result<bool> {
        let current_time = Clock::get()?.unix_timestamp;
        let age = current_time - self.last_oracle_update;
        Ok(age > MAX_ORACLE_AGE_SECONDS)
    }
}

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