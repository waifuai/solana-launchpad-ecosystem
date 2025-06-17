use anchor_lang::prelude::*;

/// State account for a single affiliate. It is a PDA.
/// PDA seeds: `[b"affiliate_info", affiliate.key().as_ref()]`
#[account]
pub struct AffiliateInfo {
    /// The public key of the affiliate's main wallet. This is the authority.
    pub affiliate_key: Pubkey,
    /// The cumulative volume of tokens purchased via this affiliate's referrals.
    /// This is a lifetime statistic.
    pub total_referred_volume: u64,
    /// The commission rate in basis points (bps). For example, 1000 bps is 10.00%.
    pub commission_rate_bps: u16,
}

impl AffiliateInfo {
    /// The total disk space required for an `AffiliateInfo` account in bytes.
    /// Pubkey (32) + u64 (8) + u16 (2) = 42 bytes.
    pub const LEN: usize = 32 + 8 + 2;
}