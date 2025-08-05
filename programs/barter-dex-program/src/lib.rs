use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use genesis_common::constants::*;

pub mod state;
pub mod error;
use state::*;
use error::*;

declare_id!("DEXy2D1fVf5s3f2y6D4b7j8N1M5P9kH3rW7T4gS6fX8a");

#[program]
pub mod barter_dex_program {
    use super::*;

    /// Initializes a new oracle-based liquidity pool.
    pub fn create_pool(ctx: Context<CreatePool>, oracle_authority: Pubkey) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.mint_a = ctx.accounts.mint_a.key();
        pool.mint_b = ctx.accounts.mint_b.key();
        pool.oracle_authority = oracle_authority;
        pool.oracle_price = ORACLE_PRICE_PRECISION; // Default to 1:1 price
        pool.last_oracle_update = Clock::get()?.unix_timestamp;
        // Anchor 0.31: retrieve bumps from generated struct rather than ctx.bumps.get(...)
        // Anchor 0.31: use ctx.bumps map instead of Accounts::bumps()
        let bumps = &ctx.bumps;
        pool.vault_a_bump = bumps.vault_a;
        pool.vault_b_bump = bumps.vault_b;
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

    /// Swaps tokens based on the current on-chain oracle price.
    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_amount_out: u64) -> Result<()> {
        let pool = &ctx.accounts.pool;
        
        // --- Oracle Sanity Checks ---
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time.checked_sub(pool.last_oracle_update).unwrap() < MAX_ORACLE_AGE_SECONDS, BarterError::OraclePriceStale);

        // --- Price Calculation based on Oracle ---
        // This logic replaces the constant product formula.
        let amount_out = if ctx.accounts.user_source_token_account.mint == pool.mint_a {
            // Swapping A for B: amount_out_B = amount_in_A * price_A_in_B
            (amount_in as u128)
                .checked_mul(pool.oracle_price as u128)
                .and_then(|v| v.checked_div(ORACLE_PRICE_PRECISION as u128))
                .ok_or(BarterError::Overflow)? as u64
        } else {
            // Swapping B for A: amount_out_A = amount_in_B / price_A_in_B
            (amount_in as u128)
                .checked_mul(ORACLE_PRICE_PRECISION as u128)
                .and_then(|v| v.checked_div(pool.oracle_price as u128))
                .ok_or(BarterError::Overflow)? as u64
        };
        
        require!(amount_out >= min_amount_out, BarterError::SlippageExceeded);

        let (source_vault, dest_vault, dest_vault_balance) = if ctx.accounts.user_source_token_account.mint == pool.mint_a {
            (ctx.accounts.vault_a.to_account_info(), ctx.accounts.vault_b.to_account_info(), ctx.accounts.vault_b.amount)
        } else {
            (ctx.accounts.vault_b.to_account_info(), ctx.accounts.vault_a.to_account_info(), ctx.accounts.vault_a.amount)
        };

        // --- Liquidity Check ---
        require!(dest_vault_balance >= amount_out, BarterError::InsufficientLiquidity);
        
        // --- Token Transfers ---
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer { from: ctx.accounts.user_source_token_account.to_account_info(), to: source_vault, authority: ctx.accounts.user.to_account_info() }
            ),
            amount_in
        )?;

        // Anchor 0.31: use ctx.bumps map instead of Accounts::bumps()
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

        Ok(())
    }
}


#[derive(Accounts)]
#[instruction(oracle_authority: Pubkey)]
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