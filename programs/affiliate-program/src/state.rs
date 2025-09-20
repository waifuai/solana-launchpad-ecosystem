//! # Affiliate Program State Definitions
//!
//! This module defines the state structures used by the affiliate program,
//! including affiliate information, performance tiers, and analytics tracking.
//! These structures store all persistent data for the affiliate system.
//!
//! ## Key Structures
//!
//! - [`AffiliateInfo`]: Main state account for individual affiliates with comprehensive analytics
//! - [`AffiliateAnalytics`]: Daily tracking data for performance analysis
//! - [`PerformanceTier`]: Enumeration of affiliate performance levels
//!
//! ## Performance System
//!
//! The program implements a sophisticated performance tracking system:
//! - **Tier-based progression**: Bronze → Silver → Gold → Platinum based on volume and conversion
//! - **Performance scoring**: Multi-factor scoring system considering volume, conversions, and referrals
//! - **Rate optimization**: AI-suggested commission rates based on performance metrics
//!
//! ## Analytics Features
//!
//! Comprehensive tracking includes:
//! - Volume history (monthly, quarterly, yearly)
//! - Conversion rate tracking
//! - Multi-level referral relationships
//! - Time-based activity monitoring

use anchor_lang::prelude::*;
use genesis_common::constants::*;

/// Performance tier for affiliates based on their performance
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTier {
    /// New or low-performing affiliates
    Bronze,
    /// Moderate performance
    Silver,
    /// Good performance
    Gold,
    /// Exceptional performance
    Platinum,
}

/// State account for a single affiliate with advanced analytics and AI optimization
#[account]
pub struct AffiliateInfo {
    /// The public key of the affiliate's main wallet. This is the authority.
    pub affiliate_key: Pubkey,
    /// The cumulative volume of tokens purchased via this affiliate's referrals.
    pub total_referred_volume: u64,
    /// The commission rate in basis points (bps). For example, 1000 bps is 10.00%.
    pub commission_rate_bps: u16,

    /// Performance analytics
    pub performance_tier: PerformanceTier,
    pub monthly_referred_volume: u64,
    pub quarterly_referred_volume: u64,
    pub yearly_referred_volume: u64,
    pub successful_referrals: u32,
    pub total_clicks: u32,
    pub conversion_rate_bps: u16, // Conversion rate in basis points

    /// AI optimization settings
    pub rate_caps_enabled: bool,
    pub max_commission_rate_bps: u16,
    pub min_commission_rate_bps: u16,
    pub ai_optimization_enabled: bool,

    /// Multi-level referral tracking
    pub referral_level: u8, // 1 = direct, 2 = level 2, etc.
    pub parent_affiliate: Option<Pubkey>,
    pub total_descendants: u32,
    pub active_descendants: u32,

    /// Time tracking
    pub registration_time: i64,
    pub last_activity_time: i64,
    pub last_rate_update_time: i64,
    pub tier_upgrade_time: i64,

    /// Analytics tracking
    pub monthly_volume_history: [u64; 12], // Last 12 months volume
    pub performance_score: u32, // Calculated performance score
}

impl AffiliateInfo {
    /// The total disk space required for an `AffiliateInfo` account in bytes.
    pub const LEN: usize = 32 + 8 + 2 + // Basic fields
        1 + 8 + 8 + 8 + 4 + 4 + 2 + // Performance analytics
        1 + 2 + 2 + 1 + // AI optimization settings
        1 + (1 + 32) + 4 + 4 + // Multi-level referral
        8 + 8 + 8 + 8 + // Time tracking
        (8 * 12) + 4; // Analytics (12 months * 8 bytes + score)

    /// Calculate performance tier based on metrics
    pub fn calculate_performance_tier(&mut self) -> Result<()> {
        let volume = self.total_referred_volume;
        let conversion_rate = self.conversion_rate_bps;

        self.performance_tier = match (volume, conversion_rate) {
            (v, _) if v >= 1_000_000_000 => PerformanceTier::Platinum, // 100M tokens
            (v, c) if v >= 100_000_000 && c >= 500 => PerformanceTier::Gold, // 10M tokens + 5% conversion
            (v, c) if v >= 10_000_000 && c >= 200 => PerformanceTier::Silver, // 1M tokens + 2% conversion
            _ => PerformanceTier::Bronze,
        };

        Ok(())
    }

    /// Update performance score
    pub fn update_performance_score(&mut self) -> Result<()> {
        let volume_score = (self.total_referred_volume / 1_000_000) as u32; // 1M tokens = 1 point
        let conversion_score = (self.conversion_rate_bps / 10) as u32; // 1% conversion = 10 points
        let referral_score = self.successful_referrals / 10; // 10 referrals = 1 point
        let tier_multiplier = match self.performance_tier {
            PerformanceTier::Bronze => 1,
            PerformanceTier::Silver => 2,
            PerformanceTier::Gold => 3,
            PerformanceTier::Platinum => 5,
        };

        self.performance_score = (volume_score + conversion_score + referral_score) * tier_multiplier;
        Ok(())
    }

    /// Check if rate update is allowed based on caps and timing
    pub fn can_update_rate(&self, new_rate: u16, current_time: i64) -> Result<bool> {
        if self.rate_caps_enabled {
            if new_rate < self.min_commission_rate_bps || new_rate > self.max_commission_rate_bps {
                return Ok(false);
            }
        }

        // Rate updates can only happen once per day
        let time_since_last_update = current_time - self.last_rate_update_time;
        Ok(time_since_last_update >= 86400) // 24 hours in seconds
    }

    /// Get suggested rate based on performance tier
    pub fn get_suggested_rate(&self) -> u16 {
        let base_rate = match self.performance_tier {
            PerformanceTier::Bronze => 500,  // 5%
            PerformanceTier::Silver => 750,  // 7.5%
            PerformanceTier::Gold => 1000,   // 10%
            PerformanceTier::Platinum => 1250, // 12.5%
        };

        // Adjust based on conversion rate
        let conversion_adjustment = if self.conversion_rate_bps >= 500 {
            100 // +1% for good conversion
        } else if self.conversion_rate_bps <= 100 {
            -50 // -0.5% for poor conversion
        } else {
            0
        };

        (base_rate as i32 + conversion_adjustment).max(50).min(2000) as u16
    }
}

/// Analytics account for tracking affiliate performance over time
#[account]
pub struct AffiliateAnalytics {
    /// The affiliate this analytics belongs to
    pub affiliate_key: Pubkey,
    /// Daily referral volume for the last 30 days
    pub daily_volume: [u64; 30],
    /// Daily click count for the last 30 days
    pub daily_clicks: [u32; 30],
    /// Last update timestamp
    pub last_update: i64,
    /// Current day index for circular buffers
    pub current_day_index: u8,
}

impl AffiliateAnalytics {
    /// Space required for analytics account
    pub const LEN: usize = 32 + (8 * 30) + (4 * 30) + 8 + 1; // 32 + 240 + 120 + 8 + 1 = 401 bytes

    /// Add daily stats
    pub fn add_daily_stats(&mut self, volume: u64, clicks: u32) {
        self.daily_volume[self.current_day_index as usize] = volume;
        self.daily_clicks[self.current_day_index as usize] = clicks;
        self.current_day_index = ((self.current_day_index as usize + 1) % 30) as u8;
    }

    /// Calculate 30-day moving average volume
    pub fn get_30_day_avg_volume(&self) -> u64 {
        let sum: u64 = self.daily_volume.iter().sum();
        sum / 30
    }
}