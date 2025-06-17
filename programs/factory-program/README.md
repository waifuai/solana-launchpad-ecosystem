# Program: factory-program

## Purpose
This program implements an ICO (Initial Coin Offering) launchpad. It allows a project authority to create a new token where the price is determined by a linear bonding curve. It is the mint authority for the token it creates and is responsible for processing purchases and CPI-calling the affiliate program to handle commissions.

## State Accounts

### 1. `LaunchState`
- **PDA Seeds**: `["launch_state", authority_pubkey, token_mint_pubkey]`
- **Purpose**: Stores the configuration and live state of a single ICO launch.
- **Fields**:
    - `authority: Pubkey` - The wallet authorized to withdraw SOL.
    - `token_mint: Pubkey` - The mint address of the token being sold.
    - `sol_vault_bump: u8` - The bump seed for the SOL vault PDA.
    - `initial_price: u64` - The starting price of the token in lamports per token.
    - `slope: u64` - The value by which the price increases for each token sold.
    - `tokens_sold: u64` - The total number of tokens sold to date.

## Instructions

### 1. `create_launch`
- **Description**: Initializes a new ICO. Creates the `LaunchState` account and the `token_mint`.
- **Parameters**:
    - `initial_price: u64`
    - `slope: u64`

### 2. `buy_tokens`
- **Description**: Allows a user to buy tokens by sending SOL. Calculates the token amount based on the current price from the bonding curve, transfers SOL to the vault, and mints tokens to the buyer. If an affiliate is provided, it triggers a CPI call.
- **Parameters**:
    - `sol_amount: u64`
    - `affiliate_key: Option<Pubkey>`

### 3. `withdraw_sol`
- **Description**: Allows the authority to withdraw all accumulated SOL from the vault.
- **Parameters**: None.

## Errors

- `InvalidAmount`: Input amount is zero or invalid.
- `Overflow`: A mathematical calculation resulted in an overflow.
- `InsufficientFunds`: The SOL amount is too small to purchase any tokens at the current price.
- `AuthorityMismatch`: The signer of `withdraw_sol` is not the launch authority.
- `AffiliateMismatch`: The provided `affiliate_key` does not match the public key in the `affiliate_info` account.