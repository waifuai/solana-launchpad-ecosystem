---
name: barter-dex-oracle
description: Design reference for the removed barter-dex-program Anchor smart contract — an oracle-priced (not AMM-formula) DEX that used to live at programs/barter-dex-program. Not executable; the on-chain Rust implementation was deleted as part of the Rust-to-Python migration since Solana programs cannot run as Python. Read this when you need to recall what the program did or reimplement it in Rust.
---

# Barter DEX Oracle (design reference, program removed)

## Status

This documents a Solana on-chain program that used to live at `programs/barter-dex-program` in this repository. **It was never deployed to mainnet** (prototype only) and its Rust source was deleted when this repository migrated off Rust, because Solana's runtime only executes Rust or C — there is no Python target for on-chain programs. This file exists purely as a design record in case the program needs to be rebuilt in Rust later. It is not runnable and does not describe current repository behavior.

Program ID (placeholder, never used on a live cluster): `DEXy2D1fVf5s3f2y6D4b7j8N1M5P9kH3rW7T4gS6fX8a`

**Known bug (present at time of removal, more severe than a simple oversight):** `lib.rs` declared **two** functions named `update_oracle_price` inside the same `#[program] mod barter_dex_program { ... }` block — one taking a plain `new_price: u64` (single-source), the other taking a full `UpdatePriceArgs` (multi-source, weighted). `state.rs`/`lib.rs` correspondingly declared **two** `LiquidityPool` / `UpdateOraclePrice` struct definitions. Two items with the same name in the same Rust module is a hard compile error (duplicate-definition), not a "last one wins" situation — **the program as committed would not have compiled at all.** Whoever picks this back up needs to delete one of each duplicate pair (the simple single-price version looks like an earlier iteration superseded by the multi-source version below it) before this can build.

## Purpose

An oracle-priced DEX, not a constant-product/AMM-formula DEX: swap prices come entirely from an external, permissioned "oracle authority" (intended to be the off-chain `price-keeper-bot`), rather than being derived from pool reserves. The multi-source design (the surviving/intended version, per the duplicate-definition bug above) blends up to three price feeds — Pyth (40% weight), Switchboard (35%), and an AI oracle (25%) — into a single weighted price, and applies a dynamic, volatility-based trading fee.

## Instructions

- **`create_pool(args: CreatePoolArgs)`** — Initializes a `LiquidityPool` PDA (`seeds = ["liquidity_pool", mint_a, mint_b]`) plus two token vault PDAs (`seeds = ["pool_vault", mint_a, mint_b, "a"/"b"]`). `args`: `oracle_authority`, `oracle_provider` (enum), optional `pyth_price_feed_a`/`pyth_price_feed_b`/`switchboard_feed`/`ai_oracle_program` addresses, `fee_bps`, `dynamic_fee_enabled`, `volatility_threshold`. Seeds `oracle_price` to `ORACLE_PRICE_PRECISION` (1:1) and a 24-slot `price_history` ring buffer, all also at that default.
- **`update_oracle_price`** (oracle-authority-only, gated by `has_one = oracle_authority`) — the intended surviving version takes `args: UpdatePriceArgs { pyth_price, switchboard_price, ai_price, price_confidence: all Option<u64> }`, merges any provided sources into the pool's stored per-source prices, recomputes `oracle_price` as the weighted blend via `calculate_weighted_price()`, and appends to the 24-slot `price_history`. (The other, non-compiling duplicate took a bare `new_price: u64` and just overwrote `oracle_price` directly with no weighting/history handling — see the bug note above.)
- **`add_liquidity(amount_a, amount_b)`** — Straight token transfers from the user's token accounts into `vault_a`/`vault_b`. No LP-share/mint accounting — this is not a standard AMM liquidity-provider model, just vault top-up.
- **`swap(amount_in, min_amount_out)`** — Checks `!pool.is_oracle_stale()` (rejects if the last oracle update exceeds `MAX_ORACLE_AGE_SECONDS`), computes `effective_price = pool.calculate_weighted_price()`, computes a `dynamic_fee` via `pool.calculate_dynamic_fee()` (driven by recent price volatility vs. `volatility_threshold` when `dynamic_fee_enabled`), converts `amount_in` to `amount_out_before_fee` using the oracle price (direction depends on whether the source token is `mint_a` or `mint_b`), subtracts the fee, and requires `amount_out >= min_amount_out` (slippage protection) and sufficient destination-vault balance. Transfers `amount_in` into the source vault and `amount_out` out of the destination vault (signed by the `LiquidityPool` PDA), updates `total_liquidity_a`/`total_liquidity_b`, and records the price into `price_history`.
- **`update_pool_config(fee_bps, dynamic_fee_enabled, volatility_threshold)`** — Oracle-authority-only; updates the fee/volatility parameters.
- **`emergency_pause(paused: bool)`** — **No-op beyond logging.** The doc comment even says "In a real implementation, this would set a pause flag" — no pause flag exists on `LiquidityPool` and no instruction actually checks one, so this instruction currently does nothing to stop trading.

## State

- **`LiquidityPool`** (PDA `["liquidity_pool", mint_a, mint_b]`) — also duplicated (see bug note); the intended multi-source version holds: `mint_a`/`mint_b`, `oracle_authority`, `oracle_provider`, optional Pyth/Switchboard/AI feed addresses and their last-seen prices (`pyth_price`, `switchboard_price`, `ai_price`, all `Option<u64>`), `price_confidence`, blended `oracle_price`, `last_oracle_update`, 24-slot `price_history` + `history_index`, `total_liquidity_a`/`total_liquidity_b`, `fee_bps`, `dynamic_fee_enabled`, `volatility_threshold`, `last_volatility_update`, vault PDA bumps.

## Dependencies

Depended on `genesis-common` for `ORACLE_PRICE_PRECISION`, `BPS_PRECISION`, `MAX_ORACLE_AGE_SECONDS`, and safe-checked arithmetic. Its `price_keeper_bot.py` Python port (in `bots/`) is the off-chain price-oracle consumer described here — see [genesis-common's Python port](../../bots/shared/genesis_common.py).
