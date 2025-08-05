#![cfg(test)]

use anchor_lang::{InstructionData, ToAccountMetas, prelude::*};
use anchor_spl::token::spl_token;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::Transaction,
};

async fn get_token_account(
    context: &mut ProgramTestContext,
    pubkey: &Pubkey,
) -> spl_token::state::Account {
    let account_data = context
        .banks_client
        .get_account(*pubkey)
        .await
        .expect("get_account request failed")
        .expect("token account not found");
    spl_token::state::Account::unpack_from_slice(&account_data.data).expect("unpack token account")
}

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
    context.banks_client.process_transaction(tx).await.expect("airdrop tx failed");
}

// Minimal smoke test that mirrors the end-to-end flow from tests/integration.rs,
// but runs under plain `cargo test` without requiring Anchor CLI test-bpf.
#[tokio::test]
async fn smoke_full_flow_with_affiliate() {
    // Register both programs with their entrypoints in the in-memory bank.
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

    // Actors.
    let authority = context.payer.pubkey();
    let token_mint_kp = Keypair::new();
    let affiliate = Keypair::new();
    let buyer = Keypair::new();

    // Fund affiliate and buyer.
    airdrop(&mut context, &affiliate.pubkey(), 1_000_000_000).await; // 1 SOL
    airdrop(&mut context, &buyer.pubkey(), 2_000_000_000).await; // 2 SOL

    // Derive PDAs consistent with on-chain programs.
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

    // 1) Create launch.
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
        }
        .to_account_metas(None),
        data: factory_program::instruction::CreateLaunch {
            initial_price: 100_000_000, // 0.1 SOL per token
            slope: 10_000_000,
        }
        .data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[create_launch_ix],
        Some(&context.payer.pubkey()),
        &[&context.payer, &token_mint_kp],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.expect("create_launch failed");

    // 2) Register affiliate.
    let register_ix = Instruction {
        program_id: affiliate_program::id(),
        accounts: affiliate_program::accounts::RegisterAffiliate {
            affiliate_info: affiliate_info_pda,
            affiliate: affiliate.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: affiliate_program::instruction::RegisterAffiliate {}.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[register_ix],
        Some(&affiliate.pubkey()),
        &[&affiliate],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.expect("register_affiliate failed");

    // 3) Buyer purchases tokens with affiliate referral.
    let sol_to_spend = 1_000_000_000; // 1 SOL
    let buyer_ata = anchor_spl::associated_token::get_associated_token_address(
        &buyer.pubkey(),
        &token_mint_kp.pubkey(),
    );
    let affiliate_ata = anchor_spl::associated_token::get_associated_token_address(
        &affiliate.pubkey(),
        &token_mint_kp.pubkey(),
    );

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
        }
        .to_account_metas(None),
        data: factory_program::instruction::BuyTokens {
            sol_amount: sol_to_spend,
            affiliate_key: Some(affiliate.pubkey()),
        }
        .data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[buy_ix],
        Some(&buyer.pubkey()),
        &[&buyer],
        context.last_blockhash,
    );
    context.banks_client.process_transaction(tx).await.expect("buy_tokens failed");

    // Assertions: Buyer gets 10 tokens; Affiliate gets 1 token; SOL vault has the 1 SOL.
    let buyer_token_account = get_token_account(&mut context, &buyer_ata).await;
    assert_eq!(
        buyer_token_account.amount,
        1_000_000_000 * 10,
        "Buyer should receive 10 tokens"
    );

    let affiliate_token_account = get_token_account(&mut context, &affiliate_ata).await;
    assert_eq!(
        affiliate_token_account.amount,
        1_000_000_000 * 1,
        "Affiliate should receive 1 token commission"
    );

    let vault_balance = context
        .banks_client
        .get_balance(sol_vault_pda)
        .await
        .expect("get_balance failed");
    assert_eq!(
        vault_balance, sol_to_spend,
        "SOL vault should contain the 1 SOL spent by the buyer"
    );
}