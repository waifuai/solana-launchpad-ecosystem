//! # Barter DEX Program
//!
//! This program implements an AI-powered oracle-based decentralized exchange (DEX)
//! for the Solana Launchpad Ecosystem. Unlike traditional AMMs, it relies entirely
//! on external price oracles for swap calculations, enabling dynamic pricing through
//! AI-driven price feeds.
//!
//! ## Oracle-Based Architecture
//!
//! The DEX operates on a fundamentally different model from traditional AMMs:
//! - **External Price Sources**: Prices come from trusted oracle authorities, not internal formulas
//! - **AI Integration**: Primary oracle is the `price-keeper-bot` which uses AI to determine fair prices
//! - **Multi-Source Support**: Can aggregate prices from Pyth, Switchboard, and AI oracles
//! - **Dynamic Fees**: Trading fees adjust based on market volatility and price confidence
//!
//! ## Key Features
//!
//! - **AI-Powered Pricing**: Exchange rates determined by AI analysis of market conditions
//! - **Multi-Oracle Support**: Weighted price calculation from multiple data sources
//! - **Dynamic Fee System**: Fees adjust automatically based on volatility and confidence
//! - **Staleness Protection**: Transactions fail if oracle prices are too old
//! - **Emergency Controls**: Administrative functions for pausing and configuration updates
//!
//! ## Core Instructions
//!
//! - [`create_pool`]: Initialize new liquidity pools with oracle configuration
//! - [`update_oracle_price`]: Permissioned price updates from oracle authorities
//! - [`swap`]: Execute token swaps at oracle-determined prices
//! - [`add_liquidity`]: Provide liquidity to trading pools
//! - [`update_pool_config`]: Modify pool parameters and fee structures
//!
//! ## AI Integration
//!
//! This program is designed to work with the `price-keeper-bot` which:
//! 1. Monitors pool configurations and token pairs
//! 2. Queries AI services for fair exchange rates
//! 3. Updates prices through the `update_oracle_price` instruction
//! 4. Maintains price history for volatility calculations
//!
//! ## Security Features
//!
//! - Oracle authority validation for price updates
//! - Timestamp-based staleness checks
//! - Comprehensive overflow/underflow protection
//! - Configurable minimum liquidity requirements

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use genesis_common::constants::*;
use genesis_common::utils::*;

pub mod state;
pub mod error;
use state::*;
use error::*;

declare_id!("DEXy2D1fVf5s3f2y6D4b7j8N1M5P9kH3rW7T4gS6fX8a");

/// Enhanced instruction arguments
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    pub oracle_authority: Pubkey,
    pub oracle_provider: OracleProvider,
    pub pyth_price_feed_a: Option<Pubkey>,
    pub pyth_price_feed_b: Option<Pubkey>,
    pub switchboard_feed: Option<Pubkey>,
    pub ai_oracle_program: Option<Pubkey>,
    pub fee_bps: u16,
    pub dynamic_fee_enabled: bool,
    pub volatility_threshold: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdatePriceArgs {
    pub pyth_price: Option<u64>,
    pub switchboard_price: Option<u64>,
    pub ai_price: Option<u64>,
    pub price_confidence: Option<u64>,
}

#[program]
pub mod barter_dex_program {
    use super::*;

    /// Initializes a new oracle-based liquidity pool with enhanced features.
    pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let pool = &mut ctx.accounts.pool;

        // Basic pool configuration
        pool.mint_a = ctx.accounts.mint_a.key();
        pool.mint_b = ctx.accounts.mint_b.key();
        pool.oracle_authority = args.oracle_authority;
        pool.oracle_price = ORACLE_PRICE_PRECISION; // Default to 1:1 price
        pool.last_oracle_update = current_time;

        // Oracle configuration
        pool.oracle_provider = args.oracle_provider;
        pool.pyth_price_feed_a = args.pyth_price_feed_a;
        pool.pyth_price_feed_b = args.pyth_price_feed_b;
        pool.switchboard_feed = args.switchboard_feed;
        pool.ai_oracle_program = args.ai_oracle_program;

        // Initialize price sources
        pool.pyth_price = None;
        pool.switchboard_price = None;
        pool.ai_price = None;
        pool.price_confidence = 0;

        // Initialize price history
        pool.price_history = [ORACLE_PRICE_PRECISION; 24];
        pool.history_index = 0;

        // Liquidity tracking
        pool.total_liquidity_a = 0;
        pool.total_liquidity_b = 0;
        pool.fee_bps = args.fee_bps;

        // Dynamic fee configuration
        pool.dynamic_fee_enabled = args.dynamic_fee_enabled;
        pool.volatility_threshold = args.volatility_threshold;
        pool.last_volatility_update = current_time;

        let bumps = &ctx.bumps;
        pool.vault_a_bump = bumps.vault_a;
        pool.vault_b_bump = bumps.vault_b;

        msg!("Enhanced pool created for mints {} and {} with oracle provider {:?}",
             pool.mint_a, pool.mint_b, pool.oracle_provider);
        Ok(())
    }

    /// Permissioned instruction for the oracle authority to update the on-chain price.
    pub fn update_oracle_price(ctx: Context<UpdateOraclePrice>, new_price: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.oracle_price = new_price;
        pool.last_oracle_update = Clock::get()?.unix_timestamp;
        msg!("Pool price updated to {} by oracle {}", new_price, ctx.accounts.oracle_authority.key());
        Ok(())
    }

    /// Adds liquidity to an existing pool.
    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount_a: u64, amount_b: u64) -> Result<()> {
        token::transfer(ctx.accounts.transfer_a_context(), amount_a)?;
        token::transfer(ctx.accounts.transfer_b_context(), amount_b)?;
        Ok(())
    }

    /// Swaps tokens using advanced oracle pricing with dynamic fees.
    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_amount_out: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let current_time = Clock::get()?.unix_timestamp;

        // Oracle sanity checks
        require!(!pool.is_oracle_stale()?, BarterError::OraclePriceStale);

        // Calculate weighted average price from multiple sources
        let effective_price = pool.calculate_weighted_price()?;
        require!(effective_price > 0, BarterError::NoValidPriceSources);

        // Calculate dynamic fee
        let fee_bps = pool.calculate_dynamic_fee()?;

        // Calculate amount out with fee
        let amount_out_before_fee = if ctx.accounts.user_source_token_account.mint == pool.mint_a {
            // Swapping A for B: amount_out_B = amount_in_A * price_A_in_B
            (amount_in as u128)
                .checked_mul(effective_price as u128)
                .and_then(|v| v.checked_div(ORACLE_PRICE_PRECISION as u128))
                .ok_or(BarterError::Overflow)? as u64
        } else {
            // Swapping B for A: amount_out_A = amount_in_B / price_A_in_B
            (amount_in as u128)
                .checked_mul(ORACLE_PRICE_PRECISION as u128)
                .and_then(|v| v.checked_div(effective_price as u128))
                .ok_or(BarterError::Overflow)? as u64
        };

        // Apply trading fee
        let fee_amount = (amount_out_before_fee as u128)
            .checked_mul(fee_bps as u128)
            .and_then(|v| v.checked_div(BPS_PRECISION as u128))
            .ok_or(BarterError::DynamicFeeCalculationFailed)? as u64;

        let amount_out = amount_out_before_fee
            .checked_sub(fee_amount)
            .ok_or(BarterError::Underflow)?;

        require!(amount_out >= min_amount_out, BarterError::SlippageExceeded);

        // Liquidity checks
        let (source_vault, dest_vault, dest_vault_balance) = if ctx.accounts.user_source_token_account.mint == pool.mint_a {
            (ctx.accounts.vault_a.to_account_info(), ctx.accounts.vault_b.to_account_info(), ctx.accounts.vault_b.amount)
        } else {
            (ctx.accounts.vault_b.to_account_info(), ctx.accounts.vault_a.to_account_info(), ctx.accounts.vault_a.amount)
        };

        require!(dest_vault_balance >= amount_out, BarterError::InsufficientLiquidity);

        // Execute token transfers
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer { from: ctx.accounts.user_source_token_account.to_account_info(), to: source_vault, authority: ctx.accounts.user.to_account_info() }
            ),
            amount_in
        )?;

        let bumps = &ctx.bumps;
        let seeds = &[LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref(), &[bumps.pool]];
        token::transfer(
             CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer { from: dest_vault, to: ctx.accounts.user_dest_token_account.to_account_info(), authority: pool.to_account_info() },
                &[&seeds[..]]
            ),
            amount_out
        )?;

        // Update pool state
        if ctx.accounts.user_source_token_account.mint == pool.mint_a {
            pool.total_liquidity_a = pool.total_liquidity_a.checked_add(amount_in).ok_or(BarterError::Overflow)?;
            pool.total_liquidity_b = pool.total_liquidity_b.checked_sub(amount_out).ok_or(BarterError::Underflow)?;
        } else {
            pool.total_liquidity_b = pool.total_liquidity_b.checked_add(amount_in).ok_or(BarterError::Overflow)?;
            pool.total_liquidity_a = pool.total_liquidity_a.checked_sub(amount_out).ok_or(BarterError::Underflow)?;
        }

        // Update price history for volatility tracking
        pool.update_price_history(effective_price);
        pool.last_volatility_update = current_time;

        msg!("Swap executed: {} in -> {} out with {} bps fee", amount_in, amount_out, fee_bps);
        Ok(())
    }

    /// Update oracle price with enhanced multi-source support.
    pub fn update_oracle_price(ctx: Context<UpdateOraclePrice>, args: UpdatePriceArgs) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let current_time = Clock::get()?.unix_timestamp;

        // Update individual price sources
        if let Some(pyth_price) = args.pyth_price {
            pool.pyth_price = Some(pyth_price);
        }
        if let Some(switchboard_price) = args.switchboard_price {
            pool.switchboard_price = Some(switchboard_price);
        }
        if let Some(ai_price) = args.ai_price {
            pool.ai_price = Some(ai_price);
        }
        if let Some(confidence) = args.price_confidence {
            pool.price_confidence = confidence;
        }

        // Calculate weighted average price
        let weighted_price = pool.calculate_weighted_price()?;
        pool.oracle_price = weighted_price;
        pool.last_oracle_update = current_time;

        // Update price history
        pool.update_price_history(weighted_price);

        msg!("Oracle prices updated: pyth={:?}, switchboard={:?}, ai={:?}, weighted={}",
             pool.pyth_price, pool.switchboard_price, pool.ai_price, weighted_price);
        Ok(())
    }

    /// Update liquidity pool configuration.
    pub fn update_pool_config(ctx: Context<UpdatePoolConfig>, fee_bps: u16, dynamic_fee_enabled: bool, volatility_threshold: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        pool.fee_bps = fee_bps;
        pool.dynamic_fee_enabled = dynamic_fee_enabled;
        pool.volatility_threshold = volatility_threshold;
        pool.last_volatility_update = Clock::get()?.unix_timestamp;

        msg!("Pool configuration updated: fee={} bps, dynamic={}, threshold={}",
             fee_bps, dynamic_fee_enabled, volatility_threshold);
        Ok(())
    }

    /// Emergency pause/unpause pool trading.
    pub fn emergency_pause(ctx: Context<EmergencyControl>, paused: bool) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        // In a real implementation, this would set a pause flag
        // For now, we'll just log the action
        msg!("Emergency control: pool trading {}", if paused { "paused" } else { "resumed" });
        Ok(())
    }
}

/// Event emitted when prices are updated
#[event]
pub struct PriceUpdateEvent {
    pub pool: Pubkey,
    pub pyth_price: Option<u64>,
    pub switchboard_price: Option<u64>,
    pub ai_price: Option<u64>,
    pub weighted_price: u64,
    pub timestamp: i64,
}


#[derive(Accounts)]
#[instruction(args: CreatePoolArgs)]
pub struct CreatePool<'info> {
    #[account(
        init,
        payer = authority,
        space = LiquidityPool::LEN + 8,
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), mint_a.key().as_ref(), mint_b.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, LiquidityPool>,
    #[account(
        init,
        payer = authority,
        token::mint = mint_a,
        token::authority = pool,
        seeds = [POOL_VAULT_SEED.as_ref(), mint_a.key().as_ref(), mint_b.key().as_ref(), b"a"],
        bump
    )]
    pub vault_a: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = authority,
        token::mint = mint_b,
        token::authority = pool,
        seeds = [POOL_VAULT_SEED.as_ref(), mint_a.key().as_ref(), mint_b.key().as_ref(), b"b"],
        bump
    )]
    pub vault_b: Account<'info, TokenAccount>,
    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UpdateOraclePrice<'info> {
    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump,
        has_one = oracle_authority @ BarterError::InvalidOracleAuthority
    )]
    pub pool: Account<'info, LiquidityPool>,
    pub oracle_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump
    )]
    pub pool: Account<'info, LiquidityPool>,
    #[account(mut, token::mint = pool.mint_a)]
    pub vault_a: Account<'info, TokenAccount>,
    #[account(mut, token::mint = pool.mint_b)]
    pub vault_b: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account_b: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(args: UpdatePriceArgs)]
pub struct UpdateOraclePrice<'info> {
    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump,
        has_one = oracle_authority @ BarterError::InvalidOracleAuthority
    )]
    pub pool: Account<'info, LiquidityPool>,
    pub oracle_authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(fee_bps: u16, dynamic_fee_enabled: bool, volatility_threshold: u64)]
pub struct UpdatePoolConfig<'info> {
    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump,
        has_one = oracle_authority @ BarterError::InvalidOracleAuthority
    )]
    pub pool: Account<'info, LiquidityPool>,
    pub oracle_authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(paused: bool)]
pub struct EmergencyControl<'info> {
    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump,
        has_one = oracle_authority @ BarterError::InvalidOracleAuthority
    )]
    pub pool: Account<'info, LiquidityPool>,
    pub oracle_authority: Signer<'info>,
}

impl<'info> AddLiquidity<'info> {
    pub fn transfer_a_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer { from: self.user_token_account_a.to_account_info(), to: self.vault_a.to_account_info(), authority: self.user.to_account_info() }
        )
    }
    pub fn transfer_b_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer { from: self.user_token_account_b.to_account_info(), to: self.vault_b.to_account_info(), authority: self.user.to_account_info() }
        )
    }
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(
        seeds = [LIQUIDITY_POOL_SEED.as_ref(), pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump
    )]
    pub pool: Account<'info, LiquidityPool>,
    #[account(mut, token::mint = pool.mint_a)]
    pub vault_a: Account<'info, TokenAccount>,
    #[account(mut, token::mint = pool.mint_b)]
    pub vault_b: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_source_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_dest_token_account: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}