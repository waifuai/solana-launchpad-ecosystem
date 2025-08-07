use anchor_client::{Client, Program, Cluster};
use affiliate_program::accounts::SetCommissionRate;
use affiliate_program::instruction::SetCommissionRate as SetCommissionRateInstruction;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::{keypair::Keypair, Signer};
use std::fs;
use std::rc::Rc;

#[derive(Serialize)]
struct GoogleGenRequest {
    contents: Vec<GoogleGenContent>,
}
#[derive(Serialize)]
struct GoogleGenContent {
    parts: Vec<GoogleGenPart>,
}
#[derive(Serialize)]
struct GoogleGenPart {
    text: String,
}

#[derive(Deserialize, Debug)]
struct GoogleGenResponse {
    candidates: Vec<GoogleGenCandidate>,
}
#[derive(Deserialize, Debug)]
struct GoogleGenCandidate {
    content: GoogleGenRespContent,
}
#[derive(Deserialize, Debug)]
struct GoogleGenRespContent {
    parts: Vec<GoogleGenRespPart>,
}
#[derive(Deserialize, Debug)]
struct GoogleGenRespPart {
    text: String,
}

/// Provider enum
enum Provider {
    OpenRouter,
    Gemini,
}

fn read_first_line(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn resolve_openrouter_model() -> String {
    let p = dirs::home_dir().unwrap_or_default().join(".model-openrouter");
    read_first_line(&p).unwrap_or_else(|| "openrouter/horizon-beta".to_string())
}

fn resolve_gemini_model() -> String {
    let p = dirs::home_dir().unwrap_or_default().join(".model-gemini");
    read_first_line(&p).unwrap_or_else(|| "gemini-2.5-pro".to_string())
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

fn resolve_gemini_api_key() -> Option<String> {
    if let Ok(v) = std::env::var("GEMINI_API_KEY") {
        let t = v.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let p = dirs::home_dir().unwrap_or_default().join(".api-gemini");
    read_first_line(&p)
}

fn default_provider() -> Provider {
    // Default switched to OpenRouter
    Provider::OpenRouter
}

fn classification_prompt(affiliate_pubkey: &Pubkey, current_rate_bps: u16, total_referred_volume: u64) -> String {
    format!(
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
    )
}

async fn get_commission_rate_openrouter(
    client: &reqwest::Client,
    model_name: &str,
    api_key: &str,
    prompt: &str,
) -> Result<u16, Box<dyn std::error::Error>> {
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
    let new_rate = v["new_rate_bps"]
        .as_u64()
        .ok_or("Failed to parse new_rate_bps")? as u16;
    Ok(new_rate)
}

async fn get_commission_rate_gemini(
    client: &reqwest::Client,
    model_name: &str,
    api_key: &str,
    prompt: &str,
) -> Result<u16, Box<dyn std::error::Error>> {
    let api_url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model_name
    );
    let request_body = GoogleGenRequest {
        contents: vec![GoogleGenContent {
            parts: vec![GoogleGenPart { text: prompt.to_string() }],
        }],
    };
    let res = client.post(&api_url).bearer_auth(api_key).json(&request_body).send().await?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Gemini API error: {}", body).into());
    }
    let data: GoogleGenResponse = res.json().await?;
    let content = data
        .candidates
        .get(0)
        .and_then(|c| c.content.parts.get(0))
        .map(|p| p.text.trim().to_string())
        .ok_or("No content from Gemini")?;
    let clean = content.replace("```json", "").replace("```", "").trim().to_string();
    let v: serde_json::Value = serde_json::from_str(&clean)?;
    let new_rate = v["new_rate_bps"]
        .as_u64()
        .ok_or("Failed to parse new_rate_bps")? as u16;
    Ok(new_rate)
}

async fn get_commission_rate(
    client: &reqwest::Client,
    affiliate_pubkey: &Pubkey,
    current_rate_bps: u16,
    total_referred_volume: u64,
) -> Result<u16, Box<dyn std::error::Error>> {
    let provider = default_provider();
    let prompt = classification_prompt(affiliate_pubkey, current_rate_bps, total_referred_volume);
    match provider {
        Provider::OpenRouter => {
            let model = resolve_openrouter_model();
            let key = resolve_openrouter_api_key().ok_or("Missing OpenRouter API key (OPENROUTER_API_KEY or ~/.api-openrouter)")?;
            get_commission_rate_openrouter(client, &model, &key, &prompt).await
        }
        Provider::Gemini => {
            let model = resolve_gemini_model();
            let key = resolve_gemini_api_key().ok_or("Missing Gemini API key (GEMINI_API_KEY or ~/.api-gemini)")?;
            get_commission_rate_gemini(client, &model, &key, &prompt).await
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payer_kp_path = shellexpand::tilde("~/.config/solana/id.json").to_string();
    let payer = Rc::new(Keypair::from_json(&fs::read_to_string(&payer_kp_path)?)?);

    let affiliate_to_manage = Rc::new(Keypair::new());
    println!("Managing affiliate: {}", affiliate_to_manage.pubkey());

    let client = Client::new(Cluster::Localnet, payer.clone());
    let program: Program = client.program(affiliate_program::id());

    let http_client = reqwest::Client::new();
    println!("\n--- Starting Optimizer Update Cycle ---");

    let (affiliate_info_pda, _) = Pubkey::find_program_address(
        &[b"affiliate_info", affiliate_to_manage.pubkey().as_ref()],
        &affiliate_program::id(),
    );

    let info_account: affiliate_program::AffiliateInfo = match program.account(affiliate_info_pda).await {
        Ok(acc) => acc,
        Err(_) => {
            println!("Affiliate not registered. Exiting.");
            return Ok(());
        }
    };

    println!(
        "Fetched on-chain data: rate={} bps, volume={}",
        info_account.commission_rate_bps, info_account.total_referred_volume
    );

    match get_commission_rate(
        &http_client,
        &affiliate_to_manage.pubkey(),
        info_account.commission_rate_bps,
        info_account.total_referred_volume,
    )
    .await {
        Ok(new_rate_bps) => {
            println!("AI suggested new rate: {} bps", new_rate_bps);
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
        }
        Err(e) => eprintln!("Failed to get rate from provider: {}", e),
    }

    println!("\n--- Update Cycle Complete ---");
    Ok(())
}