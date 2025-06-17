use anchor_client::{Client, Program, Cluster};
use barter_dex_program::accounts::UpdateOraclePrice;
use barter_dex_program::instruction::UpdateOraclePrice as UpdateOraclePriceInstruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::{keypair::Keypair, Signer};
use std::rc::Rc;
use std::time::Duration;
use tokio::time::sleep;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize, Debug)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize, Debug)]
struct ResponsePart {
    text: String,
}

/// Calls the Gemini 2.5 Pro API to get a fair market exchange rate.
///
/// # Arguments
/// * `client` - An HTTP client.
/// * `api_key` - The Gemini API key.
/// * `mint_a` - Public key of the first token.
/// * `mint_b` - Public key of the second token.
///
/// # Returns
/// A `Result` containing the price of token A in terms of token B, with 9 decimals of precision.
async fn get_exchange_rate_from_gemini(
    client: &reqwest::Client,
    api_key: &str,
    mint_a: &Pubkey,
    mint_b: &Pubkey,
) -> Result<u64, Box<dyn std::error::Error>> {
    let model = "gemini-2.5-pro";
    let api_url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let prompt = format!(
        "You are a decentralized exchange price oracle. Your task is to provide the fair market exchange rate between two Solana tokens.
        - Token A Mint: {}
        - Token B Mint: {}

        Determine the price of 1 whole unit of Token A in terms of Token B.
        Provide the price as a u64 integer with 9 decimal places of precision. For example, a price of 1.5 means you should return 1_500_000_000. A price of 1.0 is 1_000_000_000.

        (In a real scenario, you would be provided with more context, such as project descriptions, liquidity depths, and recent trade history.)
        
        For this simulation, assume Token A is slightly more valuable than Token B. Return a price between 1.1 and 1.3.
        
        Respond with ONLY a JSON object in the format: {{\"price_of_a_in_b\": <u64_number>}}",
        mint_a, mint_b
    );

    let request_body = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part {
                text: prompt,
            }],
        }],
    };

    let res = client.post(&api_url).bearer_auth(api_key).json(&request_body).send().await?;

    if res.status().is_success() {
        let gemini_response: GeminiResponse = res.json().await?;
        if let Some(candidate) = gemini_response.candidates.get(0) {
            if let Some(part) = candidate.content.parts.get(0) {
                let generated_text = &part.text;
                let clean_json_str = generated_text.trim().replace("```json", "").replace("```", "").trim().to_string();
    
                let final_json: serde_json::Value = serde_json::from_str(&clean_json_str)?;
                let new_price = final_json["price_of_a_in_b"].as_u64().ok_or("Failed to parse price_of_a_in_b")?;

                return Ok(new_price);
            }
        }
    } else {
        let error_body = res.text().await?;
        return Err(format!("Gemini API error: {}", error_body).into());
    }

    Err("No response from Gemini API".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key_path = shellexpand::tilde("~/.api-gemini").to_string();
    let api_key = fs::read_to_string(api_key_path)?.trim().to_string();

    let payer_kp_path = shellexpand::tilde("~/.config/solana/id.json").to_string();
    let payer = Rc::new(Keypair::from_json(&fs::read_to_string(&payer_kp_path)?)?);
    
    // This keypair must be the designated oracle_authority for the pools it manages.
    let oracle_authority = Rc::new(Keypair::from_json(&std::fs::read_to_string(&payer_kp_path)?)?);
    println!("Price Keeper Bot starting with authority: {}", oracle_authority.pubkey());

    let client = Client::new(Cluster::Localnet, payer.clone());
    let program: Program = client.program(barter_dex_program::id());
    let http_client = reqwest::Client::new();

    println!("Price Keeper Bot Started with Gemini AI Engine. Press Ctrl+C to exit.");

    loop {
        println!("\n--- Price Keeper Cycle ---");
        
        // Step 1: Fetch all liquidity pool accounts.
        let pool_accounts: Vec<(Pubkey, barter_dex_program::LiquidityPool)> = program.accounts(vec![]).await?;
        if pool_accounts.is_empty() {
            println!("No liquidity pools found. Skipping cycle.");
            sleep(Duration::from_secs(60)).await;
            continue;
        }

        for (pool_pda, pool_data) in pool_accounts {
            println!("Processing pool for {} <-> {}", pool_data.mint_a, pool_data.mint_b);

            // Step 2: Call Gemini API to get the new exchange rate.
            match get_exchange_rate_from_gemini(&http_client, &api_key, &pool_data.mint_a, &pool_data.mint_b).await {
                Ok(new_price) => {
                    println!("Gemini AI suggested new price: {}", new_price);
                    
                    // Step 3: Send transaction to update the on-chain oracle price.
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
                },
                Err(e) => eprintln!("Failed to get price from Gemini for pool {}: {}", pool_pda, e),
            }
        }

        sleep(Duration::from_secs(60)).await;
    }
}