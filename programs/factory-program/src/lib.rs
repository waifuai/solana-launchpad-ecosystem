//! # Factory Program - ICO Launchpad
//!
//! This program implements an advanced Initial Coin Offering (ICO) launchpad
//! for the Solana Launchpad Ecosystem. It enables the creation and management
//! of token launches with sophisticated pricing models, vesting schedules,
//! and anti-bot protection mechanisms.
//!
//! ## Core Functionality
//!
//! The factory program serves as the central hub for token launches:
//! - **Multi-Modal Pricing**: Support for linear, exponential, fixed, and Dutch auction pricing
//! - **Advanced Vesting**: Configurable vesting schedules with cliffs and linear distribution
//! - **Anti-Bot Protection**: Multi-level protection against automated trading bots
//! - **Affiliate Integration**: Seamless integration with the affiliate program for referral commissions
//! - **Platform Fees**: Configurable platform and affiliate fee structures
//!
//! ## Key Features
//!
//! - **Bonding Curve Pricing**: Dynamic price adjustment based on tokens sold
//! - **Vesting Schedules**: Linear vesting with configurable cliffs and durations
//! - **Anti-Bot Measures**: Purchase limits, cooldowns, and amount validation
//! - **Cross-Program Integration**: Direct CPI calls to affiliate program for commission processing
//! - **Launch Analytics**: Comprehensive tracking of sales, fees, and purchase metrics
//!
//! ## Core Instructions
//!
//! - [`create_launch`]: Initialize new token launches with full configuration
//! - [`buy_tokens`]: Process token purchases with anti-bot validation and affiliate commissions
//! - [`withdraw_sol`]: Authority-only withdrawal of collected SOL funds
//! - [`claim_vested_tokens`]: Claim tokens from vesting schedules
//! - [`update_launch`]: Modify launch parameters post-creation
//!
//! ## Security Features
//!
//! - Comprehensive overflow/underflow protection using genesis-common utilities
//! - Time-based launch constraints with start/end validation
//! - Authority-based access control for sensitive operations
//! - Anti-bot protection with configurable severity levels
//! - Fee calculation validation and recipient verification
//!
//! ## Integration Points
//!
//! This program integrates with:
//! - **Affiliate Program**: For commission processing and referral tracking
//! - **Genesis Common**: For shared utilities, constants, and safe math operations
//! - **SPL Token Program**: For minting and token account management

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

// CPI client for the affiliate program.
use affiliate_program::cpi::accounts::ProcessCommission;
use affiliate_program::program::AffiliateProgram;
use affiliate_program;

// Shared constants and utilities
use genesis_common::constants::*;
use genesis_common::utils::*;
pub mod state;
pub mod error;

use state::*;
use error::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// Enhanced instruction to create a launch with advanced configuration
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateLaunchArgs {
    pub initial_price: u64,
    pub slope: u64,
    pub pricing_model: PricingModel,
    pub max_tokens: u64,
    pub launch_start_time: i64,
    pub launch_end_time: i64,
    pub vesting_enabled: bool,
    pub vesting_duration_seconds: i64,
    pub vesting_cliff_seconds: i64,
    pub anti_bot_level: AntiBotLevel,
    pub min_purchase_amount: u64,
    pub max_purchase_amount: u64,
    pub purchase_cooldown_seconds: i64,
    pub affiliate_fee_bps: u16,
    pub platform_fee_bps: u16,
    pub platform_fee_recipient: Pubkey,
}

/// Instruction to claim vested tokens
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ClaimVestedTokensArgs {
    pub amount: u64,
}

/// Instruction to update launch configuration
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateLaunchArgs {
    pub new_end_time: Option<i64>,
    pub new_max_tokens: Option<u64>,
    pub new_min_purchase_amount: Option<u64>,
    pub new_max_purchase_amount: Option<u64>,
}

#[program]
pub mod factory_program {
    use super::*;

    /// Initializes a new token launch with advanced configuration.
    ///
    /// This instruction creates the `LaunchState` account which holds the bonding curve
    /// parameters and also creates the new `token_mint` for which this program's
    /// `launch_state` PDA will be the mint authority.
    ///
    /// # Parameters
    /// - `args`: Configuration arguments for the launch including pricing, vesting, and anti-bot settings
    pub fn create_launch(ctx: Context<CreateLaunch>, args: CreateLaunchArgs) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        require!(args.launch_start_time >= current_time, FactoryError::InvalidLaunchTime);
        require!(args.launch_end_time > args.launch_start_time, FactoryError::InvalidLaunchTime);
        require!(args.affiliate_fee_bps <= MAX_RATE_BPS, FactoryError::InvalidFeeConfig);
        require!(args.platform_fee_bps <= MAX_RATE_BPS, FactoryError::InvalidFeeConfig);

        if args.vesting_enabled {
            require!(args.vesting_duration_seconds >= MIN_VESTING_DURATION_SECONDS, FactoryError::InvalidVestingParams);
            require!(args.vesting_duration_seconds <= MAX_VESTING_DURATION_SECONDS, FactoryError::InvalidVestingParams);
            require!(args.vesting_cliff_seconds <= args.vesting_duration_seconds, FactoryError::InvalidVestingParams);
        }

        let state = &mut ctx.accounts.launch_state;
        state.authority = ctx.accounts.authority.key();
        state.token_mint = ctx.accounts.token_mint.key();

        let bumps = &ctx.bumps;
        state.sol_vault_bump = bumps.sol_vault;

        // Pricing configuration
        state.pricing_model = args.pricing_model;
        state.initial_price = args.initial_price;
        state.slope = args.slope;
        state.tokens_sold = 0;

        // Vesting configuration
        state.vesting_enabled = args.vesting_enabled;
        state.vesting_duration_seconds = args.vesting_duration_seconds;
        state.vesting_cliff_seconds = args.vesting_cliff_seconds;

        // Anti-bot configuration
        state.anti_bot_level = args.anti_bot_level;
        state.min_purchase_amount = args.min_purchase_amount;
        state.max_purchase_amount = args.max_purchase_amount;
        state.purchase_cooldown_seconds = args.purchase_cooldown_seconds;
        state.last_purchase_timestamp = current_time;

        // Launch constraints
        state.max_tokens = args.max_tokens;
        state.launch_start_time = args.launch_start_time;
        state.launch_end_time = args.launch_end_time;

        // Fee configuration
        state.affiliate_fee_bps = args.affiliate_fee_bps;
        state.platform_fee_bps = args.platform_fee_bps;
        state.platform_fee_recipient = args.platform_fee_recipient;

        // Initialize analytics
        state.total_sol_collected = 0;
        state.total_fees_collected = 0;
        state.purchase_count = 0;

        msg!("Enhanced launch created for mint: {} with pricing model: {:?}",
             state.token_mint, state.pricing_model);
        Ok(())
    }

    /// Executes a token purchase with advanced pricing and anti-bot measures.
    ///
    /// This instruction calculates the number of tokens to mint based on the `sol_amount` provided
    /// and the current pricing model. It includes anti-bot validation, fee processing, and
    /// optional vesting schedule creation.
    ///
    /// # Parameters
    /// - `sol_amount`: The amount of SOL (in lamports) the buyer is spending.
    /// - `affiliate_key`: An optional Pubkey of the referring affiliate.
    /// - `enable_vesting`: Whether to create a vesting schedule for the purchased tokens.
    pub fn buy_tokens(
        ctx: Context<BuyTokens>,
        sol_amount: u64,
        affiliate_key: Option<Pubkey>,
        enable_vesting: bool,
    ) -> Result<()> {
        require!(sol_amount > 0, FactoryError::InvalidAmount);
        let state = &mut ctx.accounts.launch_state;

        // Validate launch is active and within constraints
        require!(state.is_launch_active()?, FactoryError::LaunchNotActive);
        require!(!state.is_max_supply_reached(), FactoryError::MaxSupplyReached);

        // Anti-bot validation
        state.validate_purchase_amount(sol_amount)?;

        // Calculate current price based on pricing model
        let current_price_per_token = state.calculate_current_price()?;
        require!(current_price_per_token > 0, FactoryError::InvalidAmount);

        // Calculate tokens to mint
        let tokens_to_mint = math_utils::calculate_tokens_to_mint(sol_amount, current_price_per_token)?;
        require!(tokens_to_mint > 0, FactoryError::InsufficientFunds);

        // Check if we exceed max tokens
        let new_total_supply = state.tokens_sold.checked_add(tokens_to_mint)
            .ok_or(FactoryError::Overflow)?;
        require!(new_total_supply <= state.max_tokens, FactoryError::MaxSupplyReached);

        // Calculate fees
        let platform_fee = if state.platform_fee_bps > 0 {
            math_utils::calculate_commission_amount(sol_amount, state.platform_fee_bps)?
        } else {
            0
        };

        let affiliate_fee = if let Some(_) = affiliate_key {
            math_utils::calculate_commission_amount(sol_amount, state.affiliate_fee_bps)?
        } else {
            0
        };

        let net_sol_amount = sol_amount.checked_sub(platform_fee)
            .and_then(|v| v.checked_sub(affiliate_fee))
            .ok_or(FactoryError::FeeCalculationOverflow)?;

        // Transfer platform fee if applicable
        if platform_fee > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.platform_fee_recipient.to_account_info(),
                    },
                ),
                platform_fee,
            )?;
        }

        // Transfer net SOL to vault
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.sol_vault.to_account_info(),
                },
            ),
            net_sol_amount,
        )?;

        // Prepare PDA seeds for signing
        let authority_key = state.authority;
        let token_mint_key = state.token_mint;
        let launch_state_bump = ctx.bumps.launch_state;
        let seeds = &[
            LAUNCH_STATE_SEED.as_ref(),
            authority_key.as_ref(),
            token_mint_key.as_ref(),
            &[launch_state_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        // Mint tokens to buyer (or to vesting schedule if enabled)
        let token_destination = if enable_vesting {
            ctx.accounts.vesting_schedule.to_account_info()
        } else {
            ctx.accounts.buyer_token_account.to_account_info()
        };

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: token_destination,
                    authority: state.to_account_info(),
                },
                signer_seeds,
            ),
            tokens_to_mint,
        )?;

        // Initialize vesting schedule if requested
        if enable_vesting {
            let vesting_schedule = &mut ctx.accounts.vesting_schedule;
            vesting_schedule.launch_state = state.key();
            vesting_schedule.beneficiary = ctx.accounts.buyer.key();
            vesting_schedule.total_amount = tokens_to_mint;
            vesting_schedule.claimed_amount = 0;
            vesting_schedule.start_time = Clock::get()?.unix_timestamp;
            vesting_schedule.duration_seconds = state.vesting_duration_seconds;
            vesting_schedule.cliff_seconds = state.vesting_cliff_seconds;
            vesting_schedule.last_claim_time = vesting_schedule.start_time;
        }

        // Process affiliate commission if provided
        if let Some(key) = affiliate_key {
            require_keys_eq!(key, ctx.accounts.affiliate.key(), FactoryError::AffiliateMismatch);

            let cpi_program = ctx.accounts.affiliate_program.to_account_info();
            let cpi_accounts = ProcessCommission {
                launch_state: state.to_account_info(),
                affiliate_info: ctx.accounts.affiliate_info.to_account_info(),
                affiliate_token_account: ctx.accounts.affiliate_token_account.to_account_info(),
                token_mint: ctx.accounts.token_mint.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
            affiliate_program::cpi::process_commission(cpi_ctx, tokens_to_mint)?;
        }

        // Update state
        state.tokens_sold = new_total_supply;
        state.total_sol_collected = state.total_sol_collected.checked_add(net_sol_amount)
            .ok_or(FactoryError::Overflow)?;
        state.total_fees_collected = state.total_fees_collected.checked_add(platform_fee)
            .ok_or(FactoryError::Overflow)?;
        state.purchase_count = state.purchase_count.checked_add(1)
            .ok_or(FactoryError::Overflow)?;
        state.last_purchase_timestamp = Clock::get()?.unix_timestamp;

        msg!("Purchase completed: {} tokens minted for {} lamports", tokens_to_mint, sol_amount);
        Ok(())
    }
    
    /// Allows the authority of the launch to withdraw all collected SOL.
    pub fn withdraw_sol(ctx: Context<WithdrawSol>) -> Result<()> {
        let state = &ctx.accounts.launch_state;
        let sol_vault = &mut ctx.accounts.sol_vault;
        let authority = &ctx.accounts.authority;
        let lamports_to_withdraw = sol_vault.lamports();
        require!(lamports_to_withdraw > 0, FactoryError::InvalidAmount);
        
        // Prepare seeds for the SOL vault PDA to sign the transfer.
        let seeds = &[SOL_VAULT_SEED.as_ref(), state.authority.as_ref(), state.token_mint.as_ref(), &[state.sol_vault_bump]];
        let signer = &[&seeds[..]];
        
        // Transfer all lamports from the vault to the authority.
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: sol_vault.to_account_info(),
                    to: authority.to_account_info(),
                },
                signer
            ),
            lamports_to_withdraw
        )?;
        Ok(())
    }

    /// Claim vested tokens from a vesting schedule.
    pub fn claim_vested_tokens(ctx: Context<ClaimVestedTokens>, _args: ClaimVestedTokensArgs) -> Result<()> {
        let vesting = &mut ctx.accounts.vesting_schedule;
        let current_time = Clock::get()?.unix_timestamp;

        // Calculate claimable amount
        let claimable_amount = vesting.calculate_claimable_amount(current_time)?;
        require!(claimable_amount > 0, FactoryError::NoTokensToClaim);

        // Prepare PDA seeds for signing
        let launch_state = &ctx.accounts.launch_state;
        let authority_key = launch_state.authority;
        let token_mint_key = launch_state.token_mint;
        let launch_state_bump = ctx.bumps.launch_state;
        let seeds = &[
            LAUNCH_STATE_SEED.as_ref(),
            authority_key.as_ref(),
            token_mint_key.as_ref(),
            &[launch_state_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        // Transfer tokens from vesting schedule to beneficiary
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.vesting_token_account.to_account_info(),
                    to: ctx.accounts.beneficiary_token_account.to_account_info(),
                    authority: ctx.accounts.launch_state.to_account_info(),
                },
                signer_seeds,
            ),
            claimable_amount,
        )?;

        // Update vesting schedule
        vesting.claimed_amount = vesting.claimed_amount.checked_add(claimable_amount)
            .ok_or(FactoryError::Overflow)?;
        vesting.last_claim_time = current_time;

        msg!("Claimed {} vested tokens", claimable_amount);
        Ok(())
    }

    /// Update launch configuration (authority only).
    pub fn update_launch(ctx: Context<UpdateLaunch>, args: UpdateLaunchArgs) -> Result<()> {
        let state = &mut ctx.accounts.launch_state;

        if let Some(new_end_time) = args.new_end_time {
            require!(new_end_time > Clock::get()?.unix_timestamp, FactoryError::InvalidLaunchTime);
            state.launch_end_time = new_end_time;
        }

        if let Some(new_max_tokens) = args.new_max_tokens {
            require!(new_max_tokens >= state.tokens_sold, FactoryError::InvalidAmount);
            state.max_tokens = new_max_tokens;
        }

        if let Some(new_min_purchase) = args.new_min_purchase_amount {
            state.min_purchase_amount = new_min_purchase;
        }

        if let Some(new_max_purchase) = args.new_max_purchase_amount {
            state.max_purchase_amount = new_max_purchase;
        }

        msg!("Launch configuration updated");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateLaunch<'info> {
    #[account(
        init,
        payer = authority,
        space = LaunchState::LEN + 8,
        seeds = [LAUNCH_STATE_SEED.as_ref(), authority.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    pub launch_state: Account<'info, LaunchState>,

    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = launch_state
    )]
    pub token_mint: Account<'info, Mint>,
    
    #[account(
        seeds = [SOL_VAULT_SEED.as_ref(), authority.key().as_ref(), token_mint.key().as_ref()],
        bump
    )]
    /// CHECK: This is a PDA used as a SOL vault. Its address is derived and verified by seeds.
    pub sol_vault: SystemAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(sol_amount: u64, affiliate_key: Option<Pubkey>)]
pub struct BuyTokens<'info> {
    #[account(
        mut,
        seeds = [LAUNCH_STATE_SEED.as_ref(), launch_state.authority.as_ref(), launch_state.token_mint.as_ref()],
        bump
    )]
    pub launch_state: Account<'info, LaunchState>,

    #[account(mut, address = launch_state.token_mint)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED.as_ref(), launch_state.authority.as_ref(), launch_state.token_mint.as_ref()],
        bump = launch_state.sol_vault_bump
    )]
    /// CHECK: Vault address is derived from seeds and verified by Anchor.
    pub sol_vault: SystemAccount<'info>,
    
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = token_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = VestingSchedule::LEN + 8,
        seeds = [
            VESTING_SCHEDULE_SEED.as_ref(),
            launch_state.key().as_ref(),
            buyer.key().as_ref()
        ],
        bump
    )]
    pub vesting_schedule: Account<'info, VestingSchedule>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = token_mint,
        associated_token::authority = vesting_schedule
    )]
    pub vesting_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        address = launch_state.platform_fee_recipient
    )]
    pub platform_fee_recipient: SystemAccount<'info>,

    /// --- Affiliate Accounts (Optional) ---
    /// CHECK: The affiliate's main wallet account. Its public key is used as a seed.
    #[account(mut)]
    pub affiliate: AccountInfo<'info>,

    /// The affiliate's state account from the affiliate program.
    #[account(
        seeds = [AFFILIATE_INFO_SEED.as_ref(), affiliate.key().as_ref()],
        bump,
        seeds::program = affiliate_program.key()
    )]
    // Use the AffiliateInfo account type from the affiliate program crate
    pub affiliate_info: Account<'info, affiliate_program::state::AffiliateInfo>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = token_mint,
        associated_token::authority = affiliate
    )]
    pub affiliate_token_account: Account<'info, TokenAccount>,
    
    pub affiliate_program: Program<'info, AffiliateProgram>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(
        seeds = [LAUNCH_STATE_SEED.as_ref(), authority.key().as_ref(), launch_state.token_mint.as_ref()],
        bump,
        has_one = authority @ FactoryError::AuthorityMismatch
    )]
    pub launch_state: Account<'info, LaunchState>,

    #[account(
        mut,
        seeds = [SOL_VAULT_SEED.as_ref(), authority.key().as_ref(), launch_state.token_mint.as_ref()],
        bump = launch_state.sol_vault_bump
    )]
    /// CHECK: Vault address is derived from seeds and verified by Anchor.
    pub sol_vault: SystemAccount<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(args: ClaimVestedTokensArgs)]
pub struct ClaimVestedTokens<'info> {
    #[account(
        mut,
        seeds = [LAUNCH_STATE_SEED.as_ref(), launch_state.authority.as_ref(), launch_state.token_mint.as_ref()],
        bump
    )]
    pub launch_state: Account<'info, LaunchState>,

    #[account(
        mut,
        seeds = [
            VESTING_SCHEDULE_SEED.as_ref(),
            launch_state.key().as_ref(),
            vesting_schedule.beneficiary.as_ref()
        ],
        bump,
        has_one = launch_state @ FactoryError::VestingScheduleNotFound,
        has_one = beneficiary @ FactoryError::AuthorityMismatch
    )]
    pub vesting_schedule: Account<'info, VestingSchedule>,

    #[account(
        mut,
        associated_token::mint = launch_state.token_mint,
        associated_token::authority = vesting_schedule
    )]
    pub vesting_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = launch_state.token_mint,
        associated_token::authority = beneficiary
    )]
    pub beneficiary_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub beneficiary: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(args: UpdateLaunchArgs)]
pub struct UpdateLaunch<'info> {
    #[account(
        mut,
        seeds = [LAUNCH_STATE_SEED.as_ref(), authority.key().as_ref(), launch_state.token_mint.as_ref()],
        bump,
        has_one = authority @ FactoryError::AuthorityMismatch
    )]
    pub launch_state: Account<'info, LaunchState>,

    #[account(mut)]
    pub authority: Signer<'info>,
}