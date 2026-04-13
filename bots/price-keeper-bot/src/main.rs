//! # DEX Price Keeper Bot
//!
//! This bot serves as an AI-powered price oracle for the barter DEX program
//! in the Solana Launchpad Ecosystem. It continuously monitors liquidity pools
//! and updates on-chain prices using AI-generated exchange rates.
//!
//! ## Functionality
//!
//! The bot implements the following workflow:
//! 1. Fetches all active liquidity pools from the barter DEX program
//! 2. For each pool, constructs AI prompts with token mint addresses
//! 3. Queries the OpenRouter API for exchange rate calculations
//! 4. Parses AI responses to extract precise price data (with 9 decimal precision)
//! 5. Submits transactions to update oracle prices on-chain for each pool
//!
//! ## Oracle Role
//!
//! This bot acts as the authoritative price oracle for the barter DEX system:
//! - Only the bot's designated oracle authority can update prices
//! - Prices are used directly by the DEX for swap calculations
//! - Stale prices are automatically rejected by the DEX program
//!
//! ## AI Integration
//!
//! The bot uses OpenRouter for price calculations.
//!
//! ## Configuration
//!
//! API keys can be configured via:
//! - Environment variable `OPENROUTER_API_KEY`
//! - File in user home directory `~/.api-openrouter`
//!
//! Model preferences can be set via:
//! - `~/.model-openrouter` for OpenRouter model selection

use anchor_client::{Client, Program, Cluster};
use barter_dex_program::accounts::UpdateOraclePrice;
use barter_dex_program::instruction::UpdateOraclePrice as UpdateOraclePriceInstruction;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::{keypair::Keypair, Signer};
use std::fs;
use std::rc::Rc;

fn read_first_line(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn resolve_openrouter_model() -> String {
    let p = dirs::home_dir().unwrap_or_default().join(".model-openrouter");
    read_first_line(&p).unwrap_or_else(|| "openrouter/free".to_string())
}

fn resolve_openrouter_api_key() -> Option<String> {
    if let Ok(v) = std::env::var("OPENROUTER_API_KEY") {
        let t = v.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let p = dirs::home_dir().unwrap_or_default().join(".api-openrouter");
    read_first_line(&p)
}

fn price_prompt(mint_a: &Pubkey, mint_b: &Pubkey) -> String {
    format!(
        "You are a decentralized exchange price oracle. Your task is to provide the fair market exchange rate between two Solana tokens.
- Token A Mint: {}
- Token B Mint: {}

Determine the price of 1 whole unit of Token A in terms of Token B.
Provide the price as a u64 integer with 9 decimal places of precision. For example, a price of 1.5 means you should return 1500000000. A price of 1.0 is 1000000000.

For this simulation, assume Token A is slightly more valuable than Token B. Return a price between 1.1 and 1.3.

Respond with ONLY a JSON object in the format: {{\\\"price_of_a_in_b\\\": <u64_number>}}",
        mint_a, mint_b
    )
}

async fn get_price_openrouter(
    client: &reqwest::Client,
    model_name: &str,
    api_key: &str,
    prompt: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    #[derive(Serialize)]
    struct ORMsg {
        role: String,
        content: String,
    }
    #[derive(Serialize)]
    struct ORPayload {
        model: String,
        messages: Vec<ORMsg>,
        temperature: f32,
    }
    #[derive(Deserialize)]
    struct ORChoiceMsg {
        content: Option<String>,
    }
    #[derive(Deserialize)]
    struct ORChoice {
        message: ORChoiceMsg,
    }
    #[derive(Deserialize)]
    struct ORResp {
        choices: Vec<ORChoice>,
    }

    let payload = ORPayload {
        model: model_name.to_string(),
        messages: vec![ORMsg {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        temperature: 0.0,
    };
    let res = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("OpenRouter API error: {}", body).into());
    }
    let data: ORResp = res.json().await?;
    let content = data
        .choices
        .get(0)
        .and_then(|c| c.message.content.as_ref())
        .map(|s| s.trim().to_string())
        .ok_or("No content from OpenRouter")?;
    let clean = content.replace("```json", "").replace("```", "").trim().to_string();
    let v: serde_json::Value = serde_json::from_str(&clean)?;
    let price = v["price_of_a_in_b"]
        .as_u64()
        .ok_or("Failed to parse price_of_a_in_b")?;
    Ok(price)
}

async fn get_exchange_rate(
    client: &reqwest::Client,
    mint_a: &Pubkey,
    mint_b: &Pubkey,
) -> Result<u64, Box<dyn std::error::Error>> {
    let prompt = price_prompt(mint_a, mint_b);
    let model = resolve_openrouter_model();
    let key = resolve_openrouter_api_key().ok_or("Missing OpenRouter API key (OPENROUTER_API_KEY or ~/.api-openrouter)")?;
    get_price_openrouter(client, &model, &key, &prompt).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payer_kp_path = shellexpand::tilde("~/.config/solana/id.json").to_string();
    let payer = Rc::new(Keypair::from_json(&fs::read_to_string(&payer_kp_path)?)?);

    let oracle_authority = Rc::new(Keypair::from_json(&std::fs::read_to_string(&payer_kp_path)?)?);
    println!("Price Keeper starting with authority: {}", oracle_authority.pubkey());

    let client = Client::new(Cluster::Localnet, payer.clone());
    let program: Program = client.program(barter_dex_program::id());
    let http_client = reqwest::Client::new();

    println!("\n--- Starting Price Keeper Update Cycle ---");

    let pool_accounts: Vec<(Pubkey, barter_dex_program::LiquidityPool)> = program.accounts(vec![]).await?;
    if pool_accounts.is_empty() {
        println!("No liquidity pools found. Exiting.");
        return Ok(());
    }

    for (pool_pda, pool_data) in pool_accounts {
        println!("\nProcessing pool for {} <-> {}", pool_data.mint_a, pool_data.mint_b);

        match get_exchange_rate(&http_client, &pool_data.mint_a, &pool_data.mint_b).await {
            Ok(new_price) => {
                println!("AI suggested new price: {}", new_price);
                println!("Sending transaction to update on-chain price...");
                let tx_signature = program
                    .request()
                    .signer(oracle_authority.as_ref())
                    .accounts(UpdateOraclePrice {
                        pool: pool_pda,
                        oracle_authority: oracle_authority.pubkey(),
                    })
                    .args(UpdateOraclePriceInstruction { new_price })
                    .send()
                    .await;

                match tx_signature {
                    Ok(sig) => println!("Price update successful! Signature: {}", sig),
                    Err(e) => eprintln!("Price update transaction failed: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to get price from provider for pool {}: {}", pool_pda, e),
        }
    }

    println!("\n--- Update Cycle Complete ---");
    Ok(())
}