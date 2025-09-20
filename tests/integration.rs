//! # Integration Tests for Solana Launchpad Ecosystem
//!
//! This module contains comprehensive integration tests that verify the end-to-end
//! functionality of the AI-Hybrid Solana Launchpad Ecosystem. These tests run
//! against the actual deployed programs using the Solana Program Test framework.
//!
//! ## Test Coverage
//!
//! The integration tests cover the complete user journey:
//! 1. **Program Setup**: Initialize factory and affiliate programs in test environment
//! 2. **ICO Launch Creation**: Create token launches with bonding curve pricing
//! 3. **Affiliate Registration**: Register affiliates with commission structures
//! 4. **Token Purchase Flow**: Complete purchase with affiliate referral commissions
//! 5. **Balance Verification**: Validate token distributions and SOL transfers
//!
//! ## Key Test Scenarios
//!
//! - **Full Flow Test**: Complete workflow from launch creation to affiliate commission
//! - **Multi-Program Integration**: Tests interaction between factory and affiliate programs
//! - **Economic Validation**: Verifies correct token amounts, commissions, and pricing
//! - **PDA Address Calculation**: Ensures consistent Program Derived Address generation
//!
//! ## Test Environment
//!
//! Tests run in the Solana Program Test environment with:
//! - In-memory blockchain simulation
//! - Program deployment and registration
//! - Account creation and funding
//! - Transaction processing and validation
//!
//! ## Usage
//!
//! Run integration tests with:
//! ```bash
//! cargo test-bpf -- --nocapture
//! ```

#![cfg(feature = "test-bpf")]

use anchor_lang::{prelude::*, InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::Transaction,
};

/// Helper function to get the deserialized data of a token account.
async fn get_token_account(
    context: &mut ProgramTestContext,
    pubkey: &Pubkey,
) -> spl_token::state::Account {
    let account_data = context
        .banks_client
        .get_account(*pubkey)
        .await
        .unwrap()
        .unwrap();
    spl_token::state::Account::unpack_from_slice(&account_data.data).unwrap()
}

/// Helper function to airdrop lamports to a specified account.
async fn airdrop(context: &mut ProgramTestContext, receiver: &Pubkey, amount: u64) {
    let tx = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(
            &context.payer.pubkey(),
            receiver,
            amount,
        )],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.unwrap();
}

#[tokio::test]
async fn test_full_flow_with_affiliate() {
    // --- SETUP: Initialize test environment and actors ---
    // Initialize the Solana program test environment with both the factory and affiliate programs.
    let mut pt = ProgramTest::new(
        "factory_program",
        factory_program::id(),
        processor!(factory_program::entry),
    );
    pt.add_program(
        "affiliate_program",
        affiliate_program::id(),
        processor!(affiliate_program::entry),
    );
    let mut context = pt.start_with_context().await;

    // Define actors: the project authority, an affiliate, and a buyer.
    let authority = context.payer.pubkey();
    let token_mint_kp = Keypair::new();
    let affiliate = Keypair::new();
    let buyer = Keypair::new();
    // Airdrop SOL to the affiliate and buyer to pay for transactions and the token purchase.
    airdrop(&mut context, &affiliate.pubkey(), 1_000_000_000).await;
    airdrop(&mut context, &buyer.pubkey(), 2_000_000_000).await;

    // --- SETUP: Calculate all necessary PDA addresses ---
    let (launch_state_pda, _) = Pubkey::find_program_address(
        &[b"launch_state", authority.as_ref(), token_mint_kp.pubkey().as_ref()],
        &factory_program::id(),
    );
    let (sol_vault_pda, _) = Pubkey::find_program_address(
        &[b"sol_vault", authority.as_ref(), token_mint_kp.pubkey().as_ref()],
        &factory_program::id(),
    );
    let (affiliate_info_pda, _) = Pubkey::find_program_address(
        &[b"affiliate_info", affiliate.pubkey().as_ref()],
        &affiliate_program::id(),
    );
    
    // --- GIVEN: A registered affiliate and a live ICO ---
    // Step 1: Create the ICO Launch.
    // The initial price is 0.1 SOL (100,000,000 lamports) per token, with a small slope.
    let create_launch_ix = Instruction {
        program_id: factory_program::id(),
        accounts: factory_program::accounts::CreateLaunch {
            launch_state: launch_state_pda,
            token_mint: token_mint_kp.pubkey(),
            sol_vault: sol_vault_pda,
            authority,
            system_program: system_program::id(),
            token_program: spl_token::id(),
            rent: sysvar::rent::id(),
        }.to_account_metas(None),
        data: factory_program::instruction::CreateLaunch {
            initial_price: 100_000_000,
            slope: 10_000_000,
        }.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[create_launch_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer, &token_mint_kp],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.unwrap();

    // Step 2: Register the Affiliate.
    // The affiliate is registered with a default 10% commission.
    let register_ix = Instruction {
        program_id: affiliate_program::id(),
        accounts: affiliate_program::accounts::RegisterAffiliate {
            affiliate_info: affiliate_info_pda,
            affiliate: affiliate.pubkey(),
            system_program: system_program::id(),
        }.to_account_metas(None),
        data: affiliate_program::instruction::RegisterAffiliate {}.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[register_ix],
        Some(&affiliate.pubkey()),
        &[&affiliate],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.unwrap();

    // --- WHEN: A buyer purchases tokens using the affiliate's referral ---
    let sol_to_spend = 1_000_000_000; // 1 SOL.
    // Calculate associated token account addresses.
    let buyer_ata = anchor_spl::associated_token::get_associated_token_address(&buyer.pubkey(), &token_mint_kp.pubkey());
    let affiliate_ata = anchor_spl::associated_token::get_associated_token_address(&affiliate.pubkey(), &token_mint_kp.pubkey());
    
    let buy_ix = Instruction {
        program_id: factory_program::id(),
        accounts: factory_program::accounts::BuyTokens {
            launch_state: launch_state_pda,
            token_mint: token_mint_kp.pubkey(),
            sol_vault: sol_vault_pda,
            buyer_token_account: buyer_ata,
            buyer: buyer.pubkey(),
            affiliate: affiliate.pubkey(),
            affiliate_info: affiliate_info_pda,
            affiliate_token_account: affiliate_ata,
            affiliate_program: affiliate_program::id(),
            system_program: system_program::id(),
            token_program: spl_token::id(),
            associated_token_program: anchor_spl::associated_token::ID,
            rent: sysvar::rent::id(),
        }.to_account_metas(None),
        data: factory_program::instruction::BuyTokens {
            sol_amount: sol_to_spend,
            affiliate_key: Some(affiliate.pubkey()),
        }.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[buy_ix],
        Some(&buyer.pubkey()),
        &[&buyer],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.unwrap();

    // --- THEN: The buyer, affiliate, and vault balances should be correct ---
    // ASSERTION 1: Buyer receives the correct amount of tokens.
    // With 1 SOL spent at a price of 0.1 SOL/token, the buyer should get 10 tokens (10 * 10^9 units).
    let buyer_token_account = get_token_account(&mut context, &buyer_ata).await;
    assert_eq!(buyer_token_account.amount, 1_000_000_000 * 10, "Buyer should receive 10 tokens");

    // ASSERTION 2: Affiliate receives the correct commission.
    // With a 10% commission on a 10 token purchase, the affiliate should get 1 token (1 * 10^9 units).
    let affiliate_token_account = get_token_account(&mut context, &affiliate_ata).await;
    assert_eq!(affiliate_token_account.amount, 1_000_000_000 * 1, "Affiliate should receive 1 token commission");

    // ASSERTION 3: The SOL vault has received the payment.
    let vault_balance = context.banks_client.get_balance(sol_vault_pda).await.unwrap();
    assert_eq!(vault_balance, sol_to_spend, "SOL vault should contain the 1 SOL spent by the buyer");
}