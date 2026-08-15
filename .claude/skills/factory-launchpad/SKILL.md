---
name: factory-launchpad
description: Design reference for the removed factory-program Anchor smart contract — an ICO bonding-curve/vesting token launchpad that used to live at programs/factory-program. Not executable; the on-chain Rust implementation was deleted as part of the Rust-to-Python migration since Solana programs cannot run as Python. Read this when you need to recall what the program did or reimplement it in Rust.
---

# Factory Launchpad (design reference, program removed)

## Status

This documents a Solana on-chain program that used to live at `programs/factory-program` in this repository. **It was never deployed to mainnet** (prototype only) and its Rust source was deleted when this repository migrated off Rust, because Solana's runtime only executes Rust or C — there is no Python target for on-chain programs. This file exists purely as a design record in case the program needs to be rebuilt in Rust later. It is not runnable and does not describe current repository behavior.

Program ID (placeholder, never used on a live cluster): `Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS`

## Purpose

An ICO/token-launch program letting an authority create a token sale with configurable bonding-curve pricing, optional linear vesting with a cliff, anti-bot purchase limits, and automatic platform + affiliate fee splitting. Purchases could CPI into the [affiliate-commissions](../affiliate-commissions/SKILL.md) program to pay referral commissions.

## Instructions

- **`create_launch(args: CreateLaunchArgs)`** — Initializes a `LaunchState` PDA (`seeds = ["launch_state", authority, token_mint]`) and mints a new SPL `token_mint` with the `LaunchState` PDA as mint authority. `CreateLaunchArgs` configured: `initial_price`, `slope`, `pricing_model` (`Linear` / `Exponential` / `Fixed` / `DutchAuction`), `max_tokens`, `launch_start_time`/`launch_end_time`, vesting (`vesting_enabled`, `vesting_duration_seconds`, `vesting_cliff_seconds`), anti-bot (`anti_bot_level`, `min_purchase_amount`, `max_purchase_amount`, `purchase_cooldown_seconds`), and fees (`affiliate_fee_bps`, `platform_fee_bps`, `platform_fee_recipient`). Validated: launch start must be in the future, end must be after start, fee bps capped at `MAX_RATE_BPS`, vesting duration bounded and cliff ≤ duration.
- **`buy_tokens(sol_amount, affiliate_key: Option<Pubkey>, enable_vesting: bool)`** — Validates the launch is active and under max supply, applies anti-bot checks (`validate_purchase_amount`), computes the current price via `LaunchState::calculate_current_price()` (bonding-curve formula driven by `pricing_model`/`initial_price`/`slope`/`tokens_sold`), computes `tokens_to_mint`, splits `sol_amount` into a platform fee (transferred to `platform_fee_recipient`), an affiliate fee (only reserved if `affiliate_key` is provided — actually paid via CPI below), and a net amount transferred into the `sol_vault` PDA. Mints `tokens_to_mint` either directly to the buyer's associated token account or into a `VestingSchedule` PDA (`seeds = ["vesting_schedule", launch_state, buyer]`) if `enable_vesting`. If `affiliate_key` is `Some`, CPIs into `affiliate_program::process_commission` (signed by the `LaunchState` PDA) to mint commission tokens to the affiliate. Updates `tokens_sold`, `total_sol_collected`, `total_fees_collected`, `purchase_count`, `last_purchase_timestamp`.
- **`withdraw_sol()`** — Authority-only; drains the entire `sol_vault` PDA balance to the authority. PDA seeds: `["sol_vault", authority, token_mint]`.
- **`claim_vested_tokens(args: ClaimVestedTokensArgs)`** — Computes claimable amount via `VestingSchedule::calculate_claimable_amount(now)` (linear vesting after the cliff) and transfers from the vesting token account to the beneficiary's token account, signed by the `LaunchState` PDA. Updates `claimed_amount` and `last_claim_time`.
- **`update_launch(args: UpdateLaunchArgs)`** — Authority-only; optionally updates `launch_end_time` (must stay in the future), `max_tokens` (must stay ≥ `tokens_sold`), `min_purchase_amount`, `max_purchase_amount`.

## State

- **`LaunchState`** (PDA `["launch_state", authority, token_mint]`): authority, token_mint, `sol_vault_bump`, pricing config (`pricing_model`, `initial_price`, `slope`, `tokens_sold`), vesting config, anti-bot config (`anti_bot_level`, min/max purchase, cooldown, `last_purchase_timestamp`), launch window (`max_tokens`, start/end time), fee config (`affiliate_fee_bps`, `platform_fee_bps`, `platform_fee_recipient`), analytics (`total_sol_collected`, `total_fees_collected`, `purchase_count`).
- **`VestingSchedule`** (PDA `["vesting_schedule", launch_state, buyer]`): `launch_state`, `beneficiary`, `total_amount`, `claimed_amount`, `start_time`, `duration_seconds`, `cliff_seconds`, `last_claim_time`.
- **`PurchaseTracker`** — defined in `state.rs` but never referenced by any instruction; looked like scaffolding for a future, more granular per-wallet anti-bot tracker that was never wired up.

## Dependencies

Depended on `genesis-common` (PDA seed constants, `math_utils::calculate_tokens_to_mint`/`calculate_commission_amount`, safe-checked arithmetic) and CPI'd into `affiliate-program`'s `process_commission` instruction. `genesis-common`'s Rust crate still exists (required by the other on-chain programs, if they're ever rebuilt) — see [genesis-common's Python port](../../shared/genesis_common.py) for the constants/math reimplemented for the off-chain bots.
