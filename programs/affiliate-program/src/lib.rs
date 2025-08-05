use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, MintTo, TokenAccount};
use genesis_common::constants::*;

pub mod state;
pub mod error;

use state::*;
use error::*;

declare_id!("Aff1aTe111111111111111111111111111111111111"); // 32-byte base58 placeholder for local tests

#[program]
pub mod affiliate_program {
    use super::*;

    /// Creates an `AffiliateInfo` account for the signer, registering them as an affiliate.
    pub fn register_affiliate(ctx: Context<RegisterAffiliate>) -> Result<()> {
        let info = &mut ctx.accounts.affiliate_info;
        info.affiliate_key = ctx.accounts.affiliate.key();
        info.total_referred_volume = 0;
        info.commission_rate_bps = 1000; // Default to 10% commission.
        msg!("Affiliate {} registered with a default 10% rate", info.affiliate_key);
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
        Ok(())
    }
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