# Program: affiliate-program

## Purpose
This program manages an on-chain affiliate system. It allows wallets to register as affiliates, stores their commission rate, and has an instruction that can be called via CPI by other programs (like `factory-program`) to process commission payouts.

## State Accounts

### 1. `AffiliateInfo`
- **PDA Seeds**: `["affiliate_info", affiliate_pubkey]`
- **Purpose**: Stores the data for a single affiliate.
- **Fields**:
    - `affiliate_key: Pubkey` - The public key of the affiliate's wallet.
    - `total_referred_volume: u64` - The cumulative amount of tokens purchased through this affiliate's referrals.
    - `commission_rate_bps: u16` - The commission rate in basis points (e.g., 1000 = 10%).

## Instructions

### 1. `register_affiliate`
- **Description**: Creates a new `AffiliateInfo` account for the signing wallet, registering them as an affiliate with a default commission rate.
- **Parameters**: None.

### 2. `set_commission_rate`
- **Description**: Allows an affiliate (or a designated authority, though here it's the affiliate themselves) to update their own commission rate. In a real-world scenario, this would likely be restricted to a program admin.
- **Parameters**:
    - `new_rate_bps: u16`

### 3. `process_commission`
- **Description**: **This is a CPI-only instruction.** It calculates the commission amount based on the affiliate's rate and the purchased token amount. It then signs a `mint_to` instruction to issue the commission tokens to the affiliate's token account. The mint authority is the `launch_state` account from the calling program.
- **Parameters**:
    - `purchased_tokens: u64`

## Errors

- `InvalidRate`: The provided commission rate is out of the valid range (0-10000).
- `Overflow`: A mathematical calculation resulted in an overflow.
- `AuthorityMismatch`: The signer does not have authority over the account.