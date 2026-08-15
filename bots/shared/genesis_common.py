"""Shared constants and utilities mirroring the (Rust-only) genesis-common crate.

genesis-common lives at solana-launchpad-ecosystem/crates/genesis-common and is
required by the on-chain Anchor programs -- it cannot disappear from Rust. This
module reimplements the pieces the off-chain Python bots need (PDA seeds, fee
math, staleness checks) so the bots don't have to depend on the Rust crate.
"""

from __future__ import annotations

import time

from solders.pubkey import Pubkey

# ============================================================================
# PDA seeds
# ============================================================================

LAUNCH_STATE_SEED = b"launch_state"
SOL_VAULT_SEED = b"sol_vault"
VESTING_SCHEDULE_SEED = b"vesting_schedule"
AFFILIATE_INFO_SEED = b"affiliate_info"
AFFILIATE_ANALYTICS_SEED = b"affiliate_analytics"
LIQUIDITY_POOL_SEED = b"liquidity_pool"
POOL_VAULT_SEED = b"pool_vault"
ORACLE_PRICE_FEED_SEED = b"oracle_price_feed"

# ============================================================================
# Mathematical constants
# ============================================================================

ORACLE_PRICE_PRECISION = 1_000_000_000  # 1e9 for price precision
BPS_PRECISION = 10_000  # 100% = 10,000 basis points
MAX_ORACLE_AGE_SECONDS = 300  # 5 minutes max oracle staleness
MINIMUM_LIQUIDITY = 1_000_000  # Minimum liquidity tokens
FEE_BPS = 30  # 0.3% fee in basis points

# ============================================================================
# Security constants
# ============================================================================

MAX_RATE_BPS = 2000  # Maximum 20% commission rate
MIN_RATE_BPS = 50  # Minimum 0.5% commission rate
MAX_VESTING_DURATION_SECONDS = 31_557_600  # 1 year in seconds
MIN_VESTING_DURATION_SECONDS = 86_400  # 1 day in seconds

# ============================================================================
# Performance optimization constants
# ============================================================================

MAX_BATCH_SIZE = 100
RETRY_ATTEMPTS = 3
TRANSACTION_TIMEOUT_SECONDS = 30

U64_MAX = 2**64 - 1


class Overflow(Exception):
    """Mirrors genesis_common::ErrorCode::Overflow / Underflow."""


# ============================================================================
# pda_utils
# ============================================================================


def derive_launch_state_address(authority: Pubkey, token_mint: Pubkey, program_id: Pubkey) -> tuple[Pubkey, int]:
    return Pubkey.find_program_address([LAUNCH_STATE_SEED, bytes(authority), bytes(token_mint)], program_id)


def derive_sol_vault_address(authority: Pubkey, token_mint: Pubkey, program_id: Pubkey) -> tuple[Pubkey, int]:
    return Pubkey.find_program_address([SOL_VAULT_SEED, bytes(authority), bytes(token_mint)], program_id)


def derive_affiliate_info_address(affiliate_key: Pubkey, program_id: Pubkey) -> tuple[Pubkey, int]:
    return Pubkey.find_program_address([AFFILIATE_INFO_SEED, bytes(affiliate_key)], program_id)


def derive_liquidity_pool_address(mint_a: Pubkey, mint_b: Pubkey, program_id: Pubkey) -> tuple[Pubkey, int]:
    return Pubkey.find_program_address([LIQUIDITY_POOL_SEED, bytes(mint_a), bytes(mint_b)], program_id)


# ============================================================================
# math_utils
# ============================================================================


def _check_u64(value: int) -> int:
    if value < 0 or value > U64_MAX:
        raise Overflow(f"value {value} does not fit in u64")
    return value


def calculate_commission_amount(amount: int, commission_bps: int) -> int:
    """Safe-checked equivalent of genesis_common::math_utils::calculate_commission_amount."""
    return _check_u64((amount * commission_bps) // BPS_PRECISION)


def calculate_bonding_curve_price(initial_price: int, slope: int, tokens_sold: int) -> int:
    """Safe-checked equivalent of genesis_common::math_utils::calculate_bonding_curve_price."""
    return _check_u64(initial_price + slope * tokens_sold)


def calculate_tokens_to_mint(sol_amount: int, current_price: int) -> int:
    """Safe-checked equivalent of genesis_common::math_utils::calculate_tokens_to_mint."""
    token_decimals = 1_000_000_000  # 9 decimals
    if current_price == 0:
        raise Overflow("division by zero")
    return _check_u64((sol_amount * token_decimals) // current_price)


# ============================================================================
# time_utils
# ============================================================================


def is_oracle_stale(last_update: int, max_age_seconds: int = MAX_ORACLE_AGE_SECONDS) -> bool:
    return (int(time.time()) - last_update) > max_age_seconds


def is_vesting_complete(start_time: int, duration_seconds: int) -> bool:
    return int(time.time()) >= (start_time + duration_seconds)
