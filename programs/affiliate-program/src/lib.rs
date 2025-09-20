//! # Affiliate Program
//!
//! This program manages an AI-optimized affiliate system for the Solana Launchpad Ecosystem.
//! It enables dynamic commission rate management through intelligent analysis of affiliate
//! performance metrics, creating a responsive and merit-based referral system.
//!
//! ## Core Functionality
//!
//! The affiliate program provides:
//! - **Dynamic Commission Rates**: Rates adjust based on AI analysis of performance metrics
//! - **Multi-level Referrals**: Support for hierarchical affiliate structures
//! - **Performance Analytics**: Comprehensive tracking of referral volume, conversion rates, and tier progression
//! - **AI Integration Points**: Multiple instructions designed for AI bot interaction
//!
//! ## Key Instructions
//!
//! - [`register_affiliate`]: Creates affiliate accounts with configurable parameters
//! - [`set_commission_rate`]: Basic rate setting (legacy compatibility)
//! - [`update_commission_rate_ai`]: AI-optimized rate updates with validation
//! - [`process_commission`]: CPI-only commission processing for token launches
//! - [`update_analytics`]: Performance data updates for AI analysis
//! - [`get_ai_suggested_rate`]: Query current AI-suggested rates
//!
//! ## AI Integration
//!
//! This program is designed to work with the `optimizer-bot` which:
//! 1. Analyzes affiliate performance data on-chain
//! 2. Queries AI services for optimal commission rates
//! 3. Updates rates through the `update_commission_rate_ai` instruction
//!
//! ## Security Features
//!
//! - Rate caps and minimum bounds to prevent abuse
//! - Time-based restrictions on rate updates
//! - Authority validation for all sensitive operations
//! - Comprehensive error handling with custom error codes

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, MintTo, TokenAccount};
use genesis_common::constants::*;
use genesis_common::utils::*;

pub mod state;
pub mod error;

use state::*;
use error::*;

declare_id!("Aff1aTe111111111111111111111111111111111111"); // 32-byte base58 placeholder for local tests

/// Enhanced instruction arguments
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RegisterAffiliateArgs {
    pub parent_affiliate: Option<Pubkey>,
    pub referral_level: u8,
    pub rate_caps_enabled: bool,
    pub max_commission_rate_bps: u16,
    pub min_commission_rate_bps: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateCommissionRateArgs {
    pub new_rate_bps: u16,
    pub ai_suggested: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateAnalyticsArgs {
    pub volume: u64,
    pub clicks: u32,
}

#[program]
pub mod affiliate_program {
    use super::*;

    /// Creates an `AffiliateInfo` account for the signer, registering them as an affiliate with enhanced features.
    pub fn register_affiliate(ctx: Context<RegisterAffiliate>, args: RegisterAffiliateArgs) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let info = &mut ctx.accounts.affiliate_info;

        // Validate referral level
        require!(args.referral_level > 0 && args.referral_level <= 5, AffiliateError::InvalidReferralLevel);

        // Validate parent affiliate if provided
        if let Some(parent) = args.parent_affiliate {
            require!(parent != ctx.accounts.affiliate.key(), AffiliateError::CircularReferral);
            // Additional validation would check if parent exists
        }

        // Initialize basic fields
        info.affiliate_key = ctx.accounts.affiliate.key();
        info.total_referred_volume = 0;
        info.commission_rate_bps = 1000; // Default to 10% commission

        // Initialize performance analytics
        info.performance_tier = PerformanceTier::Bronze;
        info.monthly_referred_volume = 0;
        info.quarterly_referred_volume = 0;
        info.yearly_referred_volume = 0;
        info.successful_referrals = 0;
        info.total_clicks = 0;
        info.conversion_rate_bps = 0;
        info.performance_score = 0;

        // Initialize AI optimization settings
        info.rate_caps_enabled = args.rate_caps_enabled;
        info.max_commission_rate_bps = if args.rate_caps_enabled { args.max_commission_rate_bps } else { MAX_RATE_BPS };
        info.min_commission_rate_bps = if args.rate_caps_enabled { args.min_commission_rate_bps } else { MIN_RATE_BPS };
        info.ai_optimization_enabled = true;

        // Initialize multi-level referral tracking
        info.referral_level = args.referral_level;
        info.parent_affiliate = args.parent_affiliate;
        info.total_descendants = 0;
        info.active_descendants = 0;

        // Initialize time tracking
        info.registration_time = current_time;
        info.last_activity_time = current_time;
        info.last_rate_update_time = current_time;
        info.tier_upgrade_time = current_time;

        // Initialize monthly volume history
        info.monthly_volume_history = [0; 12];

        msg!("Enhanced affiliate {} registered with tier: {:?}, level: {}",
             info.affiliate_key, info.performance_tier, info.referral_level);
        Ok(())
    }

    /// Allows an affiliate to set their own commission rate.
    /// In a production system, this would likely be restricted to a program admin.
    /// # Parameters
    /// - `new_rate_bps`: The new commission rate in basis points (0-10000).
    pub fn set_commission_rate(ctx: Context<SetCommissionRate>, new_rate_bps: u16) -> Result<()> {
        require!(new_rate_bps <= 10000, AffiliateError::InvalidRate);
        ctx.accounts.affiliate_info.commission_rate_bps = new_rate_bps;
        msg!("Commission rate for {} set to {} bps", ctx.accounts.affiliate_key.key(), new_rate_bps);
        Ok(())
    }

    /// Processes a commission payment for an affiliate.
    /// This instruction is designed to be called via CPI from another program (e.g., `factory-program`).
    /// It calculates the commission and mints the corresponding tokens to the affiliate.
    /// # Parameters
    /// - `purchased_tokens`: The total amount of tokens the referred user purchased.
    pub fn process_commission(ctx: Context<ProcessCommission>, purchased_tokens: u64) -> Result<()> {
        let affiliate_info = &mut ctx.accounts.affiliate_info;
        let commission_bps = affiliate_info.commission_rate_bps as u128;

        // Calculate commission amount: (purchased_tokens * rate) / 10000
        let commission_amount = (purchased_tokens as u128)
            .checked_mul(commission_bps)
            .and_then(|v| v.checked_div(10000))
            .ok_or(AffiliateError::Overflow)? as u64;

        // Mint commission tokens to the affiliate.
        // The mint authority is the `launch_state` PDA from the factory program,
        // which is passed in and must sign this CPI call.
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.affiliate_token_account.to_account_info(),
                    authority: ctx.accounts.launch_state.to_account_info(),
                }
            ),
            commission_amount
        )?;

        // Update the affiliate's lifetime referral volume.
        affiliate_info.total_referred_volume = affiliate_info.total_referred_volume
            .checked_add(purchased_tokens)
            .ok_or(AffiliateError::Overflow)?;

        msg!("Processed commission of {} tokens for affiliate {}", commission_amount, affiliate_info.affiliate_key);

        // Update analytics
        affiliate_info.monthly_referred_volume = affiliate_info.monthly_referred_volume
            .checked_add(purchased_tokens)
            .ok_or(AffiliateError::Overflow)?;
        affiliate_info.successful_referrals = affiliate_info.successful_referrals
            .checked_add(1)
            .ok_or(AffiliateError::Overflow)?;
        affiliate_info.last_activity_time = Clock::get()?.unix_timestamp;

        // Recalculate performance metrics
        affiliate_info.calculate_performance_tier()?;
        affiliate_info.update_performance_score()?;

        Ok(())
    }

    /// AI-optimized commission rate update with validation
    pub fn update_commission_rate_ai(ctx: Context<UpdateCommissionRate>, args: UpdateCommissionRateArgs) -> Result<()> {
        let info = &mut ctx.accounts.affiliate_info;
        let current_time = Clock::get()?.unix_timestamp;

        // Validate rate is within allowed range
        require!(args.new_rate_bps >= MIN_RATE_BPS && args.new_rate_bps <= MAX_RATE_BPS, AffiliateError::InvalidRate);

        // Check rate caps if enabled
        if info.rate_caps_enabled {
            require!(args.new_rate_bps >= info.min_commission_rate_bps, AffiliateError::RateBelowMinCap);
            require!(args.new_rate_bps <= info.max_commission_rate_bps, AffiliateError::RateExceedsMaxCap);
        }

        // Check if update is allowed
        require!(info.can_update_rate(args.new_rate_bps, current_time)?, AffiliateError::RateUpdateNotAllowed);

        info.commission_rate_bps = args.new_rate_bps;
        info.last_rate_update_time = current_time;

        msg!("AI-optimized commission rate for {} updated to {} bps (AI suggested: {})",
             info.affiliate_key, args.new_rate_bps, args.ai_suggested);
        Ok(())
    }

    /// Update affiliate analytics data
    pub fn update_analytics(ctx: Context<UpdateAnalytics>, args: UpdateAnalyticsArgs) -> Result<()> {
        let analytics = &mut ctx.accounts.analytics;
        let current_time = Clock::get()?.unix_timestamp;

        // Update daily stats
        analytics.add_daily_stats(args.volume, args.clicks);
        analytics.last_update = current_time;

        // Update affiliate info with aggregated data
        let affiliate_info = &mut ctx.accounts.affiliate_info;
        affiliate_info.total_referred_volume = affiliate_info.total_referred_volume
            .checked_add(args.volume)
            .ok_or(AffiliateError::Overflow)?;
        affiliate_info.total_clicks = affiliate_info.total_clicks
            .checked_add(args.clicks)
            .ok_or(AffiliateError::Overflow)?;

        // Recalculate conversion rate
        if affiliate_info.total_clicks > 0 {
            affiliate_info.conversion_rate_bps = ((affiliate_info.successful_referrals as u64 * BPS_PRECISION) / affiliate_info.total_clicks as u64) as u16;
        }

        // Update performance metrics
        affiliate_info.calculate_performance_tier()?;
        affiliate_info.update_performance_score()?;

        msg!("Analytics updated for affiliate {}", affiliate_info.affiliate_key);
        Ok(())
    }

    /// Get AI-suggested commission rate based on performance
    pub fn get_ai_suggested_rate(ctx: Context<GetAISuggestedRate>) -> Result<()> {
        let info = &ctx.accounts.affiliate_info;
        let suggested_rate = info.get_suggested_rate();

        msg!("AI suggested rate for affiliate {}: {} bps (current: {} bps)",
             info.affiliate_key, suggested_rate, info.commission_rate_bps);

        // Emit event for off-chain processing
        emit!(AISuggestedRateEvent {
            affiliate_key: info.affiliate_key,
            current_rate_bps: info.commission_rate_bps,
            suggested_rate_bps: suggested_rate,
            performance_tier: info.performance_tier,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

/// Event emitted when AI suggests a new commission rate
#[event]
pub struct AISuggestedRateEvent {
    pub affiliate_key: Pubkey,
    pub current_rate_bps: u16,
    pub suggested_rate_bps: u16,
    pub performance_tier: PerformanceTier,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct RegisterAffiliate<'info> {
    #[account(
        init,
        payer = affiliate,
        space = AffiliateInfo::LEN + 8,
        seeds = [AFFILIATE_INFO_SEED.as_ref(), affiliate.key().as_ref()],
        bump
    )]
    pub affiliate_info: Account<'info, AffiliateInfo>,
    #[account(mut)]
    pub affiliate: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetCommissionRate<'info> {
    #[account(
        mut,
        has_one = affiliate_key @ AffiliateError::AuthorityMismatch
    )]
    pub affiliate_info: Account<'info, AffiliateInfo>,
    
    // The affiliate is the authority that can change their own rate.
    #[account(mut)]
    pub affiliate_key: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProcessCommission<'info> {
    /// CHECK: This is the `launch_state` account from the `factory-program`.
    /// It is the mint authority for the token. Its authority is verified by the
    /// SPL Token program when `mint_to` is called with this account as a signer.
    pub launch_state: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [AFFILIATE_INFO_SEED.as_ref(), affiliate_info.affiliate_key.as_ref()],
        bump
    )]
    pub affiliate_info: Account<'info, AffiliateInfo>,

    /// CHECK: This is the affiliate's token account. It is checked by the SPL Token program.
    #[account(mut)]
    pub affiliate_token_account: AccountInfo<'info>,

    /// CHECK: This is the token mint. It is checked by the SPL Token program.
    #[account(mut)]
    pub token_mint: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(args: UpdateCommissionRateArgs)]
pub struct UpdateCommissionRate<'info> {
    #[account(
        mut,
        seeds = [AFFILIATE_INFO_SEED.as_ref(), affiliate.key().as_ref()],
        bump,
        has_one = affiliate_key @ AffiliateError::AuthorityMismatch
    )]
    pub affiliate_info: Account<'info, AffiliateInfo>,

    #[account(mut)]
    pub affiliate: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(args: UpdateAnalyticsArgs)]
pub struct UpdateAnalytics<'info> {
    #[account(
        mut,
        seeds = [AFFILIATE_INFO_SEED.as_ref(), affiliate.key().as_ref()],
        bump
    )]
    pub affiliate_info: Account<'info, AffiliateInfo>,

    #[account(
        init_if_needed,
        payer = affiliate,
        space = AffiliateAnalytics::LEN + 8,
        seeds = [AFFILIATE_ANALYTICS_SEED.as_ref(), affiliate.key().as_ref()],
        bump
    )]
    pub analytics: Account<'info, AffiliateAnalytics>,

    #[account(mut)]
    pub affiliate: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetAISuggestedRate<'info> {
    #[account(
        seeds = [AFFILIATE_INFO_SEED.as_ref(), affiliate.key().as_ref()],
        bump
    )]
    pub affiliate_info: Account<'info, AffiliateInfo>,

    pub affiliate: Signer<'info>,
}