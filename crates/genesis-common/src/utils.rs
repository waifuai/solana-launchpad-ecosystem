use anchor_lang::prelude::*;
use std::convert::TryInto;

/// Utility functions for PDA derivation and validation
pub mod pda_utils {
    use super::*;

    /// Derive launch state PDA
    pub fn derive_launch_state_address(
        authority: &Pubkey,
        token_mint: &Pubkey,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                crate::constants::LAUNCH_STATE_SEED.as_ref(),
                authority.as_ref(),
                token_mint.as_ref(),
            ],
            program_id,
        )
    }

    /// Derive SOL vault PDA
    pub fn derive_sol_vault_address(
        authority: &Pubkey,
        token_mint: &Pubkey,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                crate::constants::SOL_VAULT_SEED.as_ref(),
                authority.as_ref(),
                token_mint.as_ref(),
            ],
            program_id,
        )
    }

    /// Derive affiliate info PDA
    pub fn derive_affiliate_info_address(
        affiliate_key: &Pubkey,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                crate::constants::AFFILIATE_INFO_SEED.as_ref(),
                affiliate_key.as_ref(),
            ],
            program_id,
        )
    }

    /// Derive liquidity pool PDA
    pub fn derive_liquidity_pool_address(
        mint_a: &Pubkey,
        mint_b: &Pubkey,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                crate::constants::LIQUIDITY_POOL_SEED.as_ref(),
                mint_a.as_ref(),
                mint_b.as_ref(),
            ],
            program_id,
        )
    }
}

/// Mathematical utility functions for safe calculations
pub mod math_utils {
    use super::*;
    use crate::constants::*;

    /// Safe multiplication with overflow protection
    pub fn safe_mul_u128(a: u128, b: u128) -> Result<u128> {
        a.checked_mul(b).ok_or(error!(crate::ErrorCode::Overflow))
    }

    /// Safe division with zero check
    pub fn safe_div_u128(a: u128, b: u128) -> Result<u128> {
        if b == 0 {
            return err!(crate::ErrorCode::DivisionByZero);
        }
        a.checked_div(b).ok_or(error!(crate::ErrorCode::Overflow))
    }

    /// Safe addition with overflow protection
    pub fn safe_add_u128(a: u128, b: u128) -> Result<u128> {
        a.checked_add(b).ok_or(error!(crate::ErrorCode::Overflow))
    }

    /// Safe subtraction with underflow protection
    pub fn safe_sub_u128(a: u128, b: u128) -> Result<u128> {
        a.checked_sub(b).ok_or(error!(crate::ErrorCode::Underflow))
    }

    /// Calculate commission amount with basis points
    pub fn calculate_commission_amount(
        amount: u64,
        commission_bps: u16,
    ) -> Result<u64> {
        let amount_u128 = amount as u128;
        let commission_bps_u128 = commission_bps as u128;
        let bps_precision_u128 = BPS_PRECISION as u128;

        let commission_amount = safe_mul_u128(amount_u128, commission_bps_u128)?;
        let commission_amount = safe_div_u128(commission_amount, bps_precision_u128)?;

        Ok(commission_amount.try_into().map_err(|_| crate::ErrorCode::Overflow)?)
    }

    /// Calculate price with bonding curve formula
    pub fn calculate_bonding_curve_price(
        initial_price: u64,
        slope: u64,
        tokens_sold: u64,
    ) -> Result<u64> {
        let initial_price_u128 = initial_price as u128;
        let slope_u128 = slope as u128;
        let tokens_sold_u128 = tokens_sold as u128;

        let price_increase = safe_mul_u128(slope_u128, tokens_sold_u128)?;
        let current_price = safe_add_u128(initial_price_u128, price_increase)?;

        Ok(current_price.try_into().map_err(|_| crate::ErrorCode::Overflow)?)
    }

    /// Calculate tokens to mint based on SOL amount and price
    pub fn calculate_tokens_to_mint(
        sol_amount: u64,
        current_price: u64,
    ) -> Result<u64> {
        let sol_amount_u128 = sol_amount as u128;
        let current_price_u128 = current_price as u128;
        let token_decimals_u128 = 1_000_000_000u128; // 9 decimals

        let tokens_to_mint = safe_mul_u128(sol_amount_u128, token_decimals_u128)?;
        let tokens_to_mint = safe_div_u128(tokens_to_mint, current_price_u128)?;

        Ok(tokens_to_mint.try_into().map_err(|_| crate::ErrorCode::Overflow)?)
    }
}

/// Time utility functions
pub mod time_utils {
    use super::*;

    /// Check if oracle price is stale
    pub fn is_oracle_stale(
        last_update: i64,
        max_age_seconds: i64,
    ) -> Result<bool> {
        let current_time = Clock::get()?.unix_timestamp;
        let age = current_time.checked_sub(last_update)
            .ok_or(error!(crate::ErrorCode::InvalidTimestamp))?;
        Ok(age > max_age_seconds)
    }

    /// Check if vesting period has completed
    pub fn is_vesting_complete(
        start_time: i64,
        duration_seconds: i64,
    ) -> Result<bool> {
        let current_time = Clock::get()?.unix_timestamp;
        let end_time = start_time.checked_add(duration_seconds)
            .ok_or(error!(crate::ErrorCode::InvalidTimestamp))?;
        Ok(current_time >= end_time)
    }
}

/// Error codes for shared utilities
#[error_code]
pub enum ErrorCode {
    #[msg("Mathematical overflow occurred")]
    Overflow,
    #[msg("Mathematical underflow occurred")]
    Underflow,
    #[msg("Division by zero")]
    DivisionByZero,
    #[msg("Invalid timestamp provided")]
    InvalidTimestamp,
}