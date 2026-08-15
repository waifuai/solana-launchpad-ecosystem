---
name: affiliate-commissions
description: Design reference for the removed affiliate-program Anchor smart contract — a tiered, AI-tunable referral/commission system that used to live at programs/affiliate-program. Not executable; the on-chain Rust implementation was deleted as part of the Rust-to-Python migration since Solana programs cannot run as Python. Read this when you need to recall what the program did or reimplement it in Rust.
---

# Affiliate Commissions (design reference, program removed)

## Status

This documents a Solana on-chain program that used to live at `programs/affiliate-program` in this repository. **It was never deployed to mainnet** (prototype only) and its Rust source was deleted when this repository migrated off Rust, because Solana's runtime only executes Rust or C — there is no Python target for on-chain programs. This file exists purely as a design record in case the program needs to be rebuilt in Rust later. It is not runnable and does not describe current repository behavior.

Program ID declared in code (`declare_id!`): `Aff1aTe111111111111111111111111111111111111`

**Known bug (present at time of removal):** `Anchor.toml` listed a *different* program ID, `AFFLiateiGR4sC1VbN9s3M1hA9gRPEc2iEM5y1N2u1j6a`, for every cluster (localnet/devnet/mainnet). The two never agreed, which would have broken any build/deploy relying on `Anchor.toml`'s ID matching the compiled program's `declare_id!`.

## Purpose

A referral/commission system for the [factory-launchpad](../factory-launchpad/SKILL.md) program: affiliates register, earn commission on referred purchases (paid via CPI from `factory-program`), and have their commission rate tunable — either by themselves directly, or by an off-chain AI bot (`optimizer-bot`) within admin-configured caps and a cooldown. Tracks tiered performance analytics (Bronze → Platinum) from rolling volume/click data.

## Instructions

- **`register_affiliate(args: RegisterAffiliateArgs)`** — Creates an `AffiliateInfo` PDA (`seeds = ["affiliate_info", affiliate]`) for the signer. `args`: `parent_affiliate: Option<Pubkey>` (multi-level referral support, must not be the signer — no cycle check beyond that), `referral_level` (1-5), `rate_caps_enabled`, `max_commission_rate_bps`, `min_commission_rate_bps` (only applied if caps enabled, otherwise defaults to the global `MAX_RATE_BPS`/`MIN_RATE_BPS`). Initializes commission rate to 1000 bps (10%), tier to `Bronze`, all volume/analytics counters to 0, and a 12-slot `monthly_volume_history` array.
- **`set_commission_rate(new_rate_bps: u16)`** — Legacy self-service rate setter, capped only at ≤10000 bps (100%). No caps/cooldown enforcement — noted in the original doc comment as something that "in a production system... would likely be restricted to a program admin."
- **`process_commission(purchased_tokens: u64)`** — **CPI-only**, called by `factory-program::buy_tokens`. Computes `commission = purchased_tokens * commission_rate_bps / 10000` and mints that many tokens to the affiliate's token account, with the caller-supplied `launch_state` PDA (from factory-program) as signing mint authority. Updates `total_referred_volume`, `monthly_referred_volume`, `successful_referrals`, `last_activity_time`, then recalculates tier/performance score.
- **`update_commission_rate_ai(args: UpdateCommissionRateArgs)`** — The AI-bot rate-update path. `args`: `new_rate_bps`, `ai_suggested: bool` (informational flag, logged but not otherwise checked). Validates the new rate against global `MIN_RATE_BPS`/`MAX_RATE_BPS`, then against the affiliate's own caps if `rate_caps_enabled`, then against `AffiliateInfo::can_update_rate()` which enforces a cooldown (24h) since `last_rate_update_time`.
- **`update_analytics(args: UpdateAnalyticsArgs)`** — `args`: `volume: u64`, `clicks: u32`. Rolls the new volume/clicks into a 30-day circular buffer (`AffiliateAnalytics::add_daily_stats`), then adds volume/clicks into `AffiliateInfo`'s lifetime totals, recomputes `conversion_rate_bps = successful_referrals * BPS_PRECISION / total_clicks`, and recalculates tier/score.
- **`get_ai_suggested_rate()`** — Read-only; computes a suggested rate from `AffiliateInfo::get_suggested_rate()` (tier-based) and emits an `AISuggestedRateEvent { affiliate_key, current_rate_bps, suggested_rate_bps, performance_tier, timestamp }` for off-chain consumption (i.e. by `optimizer-bot`).

## State

- **`AffiliateInfo`** (PDA `["affiliate_info", affiliate]`): `affiliate_key`, `commission_rate_bps`, `performance_tier` (`Bronze`/`Silver`/`Gold`/`Platinum`), volume fields (`total_referred_volume`, `monthly_referred_volume`, `quarterly_referred_volume`, `yearly_referred_volume`, `monthly_volume_history: [u64; 12]`), engagement (`successful_referrals`, `total_clicks`, `conversion_rate_bps`, `performance_score`), rate governance (`rate_caps_enabled`, `max_commission_rate_bps`, `min_commission_rate_bps`, `ai_optimization_enabled`), multi-level referral (`referral_level`, `parent_affiliate`, `total_descendants`, `active_descendants`), timestamps (`registration_time`, `last_activity_time`, `last_rate_update_time`, `tier_upgrade_time`).
- **`AffiliateAnalytics`** — 30-day rolling circular buffers of daily volume/clicks, plus `last_update`.

## Dependencies

Depended on `genesis-common` for `MAX_RATE_BPS`/`MIN_RATE_BPS`/`BPS_PRECISION` constants and safe-checked arithmetic. Called via CPI from [factory-launchpad](../factory-launchpad/SKILL.md)'s `buy_tokens`. Its `optimizer_bot.py` Python port (in `bots/`) is the off-chain AI-rate-tuning consumer described here — see [genesis-common's Python port](../../bots/shared/genesis_common.py).
