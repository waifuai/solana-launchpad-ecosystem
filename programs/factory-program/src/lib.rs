use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

// CPI client for the affiliate program.
use affiliate_program::cpi::accounts::ProcessCommission;
use affiliate_program::program::AffiliateProgram;
use affiliate_program;

// Shared constants for PDA seeds.
use genesis_common::constants::*;
pub mod state;
pub mod error;

use state::*;
use error::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod factory_program {
    use super::*;

    /// Initializes a new token launch.
    ///
    /// This instruction creates the `LaunchState` account which holds the bonding curve
    /// parameters and also creates the new `token_mint` for which this program's
    /// `launch_state` PDA will be the mint authority.
    ///
    /// # Parameters
    /// - `initial_price`: The base price of the token in lamports (1 SOL = 1,000,000,000 lamports).
    /// - `slope`: The amount the price increases for every one whole token (10^9 units) sold.
    pub fn create_launch(ctx: Context<CreateLaunch>, initial_price: u64, slope: u64) -> Result<()> {
        let state = &mut ctx.accounts.launch_state;
        state.authority = ctx.accounts.authority.key();
        state.token_mint = ctx.accounts.token_mint.key();
        state.sol_vault_bump = *ctx.bumps.get("sol_vault").unwrap();
        state.initial_price = initial_price;
        state.slope = slope;
        state.tokens_sold = 0;
        msg!("New launch created for mint: {}", state.token_mint);
        Ok(())
    }

    /// Executes a token purchase and optionally processes an affiliate commission.
    ///
    /// This instruction calculates the number of tokens to mint based on the `sol_amount` provided
    /// and the current position on the bonding curve. It transfers SOL from the `buyer` to the
    /// `sol_vault` and mints new tokens. If an `affiliate_key` is provided, it makes a CPI call
    /// to the affiliate-program to mint commission tokens.
    ///
    /// # Parameters
    /// - `sol_amount`: The amount of SOL (in lamports) the buyer is spending.
    /// - `affiliate_key`: An optional Pubkey of the referring affiliate. Must match the derived affiliate_info account.
    pub fn buy_tokens(ctx: Context<BuyTokens>, sol_amount: u64, affiliate_key: Option<Pubkey>) -> Result<()> {
        require!(sol_amount > 0, FactoryError::InvalidAmount);
        let state = &mut ctx.accounts.launch_state;

        // --- Bonding Curve Calculation ---
        // The price is calculated using a linear bonding curve formula:
        // current_price = initial_price + (slope * tokens_sold)
        // All calculations use u128 to prevent overflow with large numbers.
        let slope = state.slope as u128;
        let initial_price = state.initial_price as u128;
        let tokens_sold = state.tokens_sold as u128;

        let current_price_per_token = slope.checked_mul(tokens_sold).and_then(|v| v.checked_add(initial_price)).ok_or(FactoryError::Overflow)?;
        require!(current_price_per_token > 0, FactoryError::InvalidAmount);
        
        // Calculate how many tokens can be purchased.
        // We multiply by 10^9 (the token's decimals) before dividing to maintain precision.
        let tokens_to_mint = (sol_amount as u128).checked_mul(1_000_000_000).and_then(|v| v.checked_div(current_price_per_token)).ok_or(FactoryError::Overflow)? as u64;
        require!(tokens_to_mint > 0, FactoryError::InsufficientFunds);

        // Transfer SOL from buyer to the SOL vault PDA.
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.sol_vault.to_account_info(),
                },
            ),
            sol_amount,
        )?;
        
        // Prepare seeds for signing as the `launch_state` PDA.
        let authority_key = state.authority.key();
        let token_mint_key = state.token_mint.key();
        let launch_state_bump = *ctx.bumps.get("launch_state").unwrap();
        let seeds = &[LAUNCH_STATE_SEED.as_ref(), authority_key.as_ref(), token_mint_key.as_ref(), &[launch_state_bump]];
        let signer_seeds = &[&seeds[..]];

        // Mint tokens to the buyer.
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.buyer_token_account.to_account_info(),
                    authority: ctx.accounts.launch_state.to_account_info(),
                },
                signer_seeds,
            ),
            tokens_to_mint,
        )?;

        // If an affiliate key was provided, process the commission via CPI.
        if let Some(key) = affiliate_key {
            require_keys_eq!(key, ctx.accounts.affiliate.key(), FactoryError::AffiliateMismatch);
            
            let cpi_program = ctx.accounts.affiliate_program.to_account_info();
            let cpi_accounts = ProcessCommission {
                launch_state: ctx.accounts.launch_state.to_account_info(),
                affiliate_info: ctx.accounts.affiliate_info.to_account_info(),
                affiliate_token_account: ctx.accounts.affiliate_token_account.to_account_info(),
                token_mint: ctx.accounts.token_mint.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            // The signer for the CPI is the `launch_state` PDA.
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

            affiliate_program::cpi::process_commission(cpi_ctx, tokens_to_mint)?;
        }

        // Update the total number of tokens sold.
        state.tokens_sold = state.tokens_sold.checked_add(tokens_to_mint).ok_or(FactoryError::Overflow)?;
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
    
    #[account(mut)]
    pub buyer: Signer<'info>,

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
    pub affiliate_info: Account<'info, affiliate_program::AffiliateInfo>,

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