use anchor_client::{Client, Program, Cluster};
use affiliate_program::accounts::SetCommissionRate;
use affiliate_program::instruction::SetCommissionRate as SetCommissionRateInstruction;
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

/// Calls the Gemini 2.5 Pro API to get an optimal commission rate.
///
/// # Arguments
/// * `client` - An HTTP client.
/// * `api_key` - The Gemini API key.
/// * `affiliate_pubkey` - The public key of the affiliate being analyzed.
/// * `current_rate_bps` - The affiliate's current commission rate.
/// * `total_referred_volume` - The affiliate's lifetime referred volume.
///
/// # Returns
/// A `Result` containing the new commission rate in BPS, or an error.
async fn get_commission_rate_from_gemini(
    client: &reqwest::Client,
    api_key: &str,
    affiliate_pubkey: &Pubkey,
    current_rate_bps: u16,
    total_referred_volume: u64,
) -> Result<u16, Box<dyn std::error::Error>> {
    let model = "gemini-2.5-pro";
    let api_url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let prompt = format!(
        "You are a Solana tokenomics expert. Your task is to determine an optimal affiliate commission rate.
        Analyze the following data for the affiliate with public key {}:
        - Current commission rate: {} basis points ({}%).
        - Total referred token volume: {} tokens.

        Based on this data, suggest a new commission rate in basis points.
        - If the volume is high, consider a moderate increase to reward performance.
        - If the volume is low, consider a slight decrease to optimize project costs.
        - Avoid drastic changes. The new rate should be between 500 (5%) and 2000 (20%).
        
        Respond with ONLY a JSON object in the format: {{\"new_rate_bps\": <number>}}",
        affiliate_pubkey,
        current_rate_bps,
        current_rate_bps as f32 / 100.0,
        total_referred_volume
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
                // Extract JSON from the markdown block if present
                let clean_json_str = generated_text.trim().replace("```json", "").replace("```", "").trim().to_string();
                let final_json: serde_json::Value = serde_json::from_str(&clean_json_str)?;
                let new_rate = final_json["new_rate_bps"].as_u64().ok_or("Failed to parse new_rate_bps")? as u16;
                return Ok(new_rate);
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
    
    let affiliate_to_manage = Rc::new(Keypair::new());
    println!("Managing affiliate: {}", affiliate_to_manage.pubkey());

    let client = Client::new(Cluster::Localnet, payer.clone());
    let program: Program = client.program(affiliate_program::id());

    let http_client = reqwest::Client::new();
    println!("\n--- Starting Optimizer Update Cycle ---");
    
    let (affiliate_info_pda, _) = Pubkey::find_program_address(&[b"affiliate_info", affiliate_to_manage.pubkey().as_ref()], &affiliate_program::id());
    
    let info_account: affiliate_program::AffiliateInfo = match program.account(affiliate_info_pda).await {
        Ok(acc) => acc,
        Err(_) => {
            println!("Affiliate not registered. Exiting.");
            return Ok(());
        }
    };

    println!("Fetched on-chain data: rate={} bps, volume={}", info_account.commission_rate_bps, info_account.total_referred_volume);

    match get_commission_rate_from_gemini(&http_client, &api_key, &affiliate_to_manage.pubkey(), info_account.commission_rate_bps, info_account.total_referred_volume).await {
        Ok(new_rate_bps) => {
            println!("Gemini AI suggested new rate: {} bps", new_rate_bps);

            if new_rate_bps == info_account.commission_rate_bps {
                println!("Rate is already optimal. No update needed.");
            } else {
                println!("Sending transaction to update rate...");
                let tx_signature = program
                    .request()
                    .signer(affiliate_to_manage.as_ref())
                    .accounts(SetCommissionRate {
                        affiliate_info: affiliate_info_pda,
                        affiliate_key: affiliate_to_manage.pubkey(),
                    })
                    .args(SetCommissionRateInstruction { new_rate_bps })
                    .send()
                    .await;

                match tx_signature {
                    Ok(sig) => println!("Transaction successful! Signature: {}", sig),
                    Err(e) => eprintln!("Transaction failed: {}", e),
                }
            }
        },
        Err(e) => eprintln!("Failed to get rate from Gemini: {}", e),
    }

    println!("\n--- Update Cycle Complete ---");
    Ok(())
}