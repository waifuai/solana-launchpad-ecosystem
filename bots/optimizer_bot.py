"""Affiliate Commission Optimizer Bot.

Part of the Solana Launchpad Ecosystem's AI-hybrid architecture. Dynamically
optimizes affiliate commission rates by analyzing on-chain performance data
and using AI to suggest optimal commission rates.

Workflow:
1. Fetches affiliate data from the on-chain affiliate program.
2. Constructs an AI prompt with performance metrics and the current rate.
3. Queries the OpenRouter API for a commission rate suggestion.
4. Parses the AI response to extract the new rate.
5. Submits a transaction to update the commission rate on-chain if it changed.

The affiliate program itself was removed (Solana on-chain programs cannot be
Python -- see .claude/skills/affiliate-commissions/SKILL.md), so this talks to
whatever program is deployed at PROGRAM_ID using the same account layout and
instruction wire format the original Rust program used.

Configuration:
- API key: env var OPENROUTER_API_KEY, or file ~/.api-openrouter
- Model: file ~/.model-openrouter (defaults to "openrouter/free")
- Wallet: ~/.config/solana/id.json
"""

from __future__ import annotations

import base64
import json
import os
import re
import sys
from pathlib import Path

import httpx
from solders.hash import Hash
from solders.instruction import AccountMeta
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import Transaction

from shared import anchor, genesis_common

PROGRAM_ID = Pubkey.from_string("Aff1aTe111111111111111111111111111111111111")
RPC_URL = "http://127.0.0.1:8899"


def _read_first_line(path: Path) -> str | None:
    try:
        text = path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    return text or None


def _rust_float_display(value: float) -> str:
    """Mimic Rust's `{}` Display for f32/f64, which omits the trailing .0 for whole numbers."""
    s = repr(value)
    return s[:-2] if s.endswith(".0") else s


def resolve_openrouter_model() -> str:
    p = Path.home() / ".model-openrouter"
    return _read_first_line(p) or "openrouter/free"


def resolve_openrouter_api_key() -> str | None:
    v = os.environ.get("OPENROUTER_API_KEY", "").strip()
    if v:
        return v
    p = Path.home() / ".api-openrouter"
    return _read_first_line(p)


def classification_prompt(affiliate_pubkey: Pubkey, current_rate_bps: int, total_referred_volume: int) -> str:
    return (
        "You are a Solana tokenomics expert. Your task is to determine an optimal affiliate commission rate.\n"
        f"Analyze the following data for the affiliate with public key {affiliate_pubkey}:\n"
        f"- Current commission rate: {current_rate_bps} basis points ({_rust_float_display(current_rate_bps / 100.0)}%).\n"
        f"- Total referred token volume: {total_referred_volume} tokens.\n\n"
        "Based on this data, suggest a new commission rate in basis points.\n"
        "- If the volume is high, consider a moderate increase to reward performance.\n"
        "- If the volume is low, consider a slight decrease to optimize project costs.\n"
        "- Avoid drastic changes. The new rate should be between 500 (5%) and 2000 (20%).\n"
        'Respond with ONLY a JSON object in the format: {"new_rate_bps": <number>}'
    )


def _get_via_openrouter(client: httpx.Client, model_name: str, api_key: str, prompt: str) -> dict:
    payload = {
        "model": model_name,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.0,
    }
    res = client.post(
        "https://openrouter.ai/api/v1/chat/completions",
        headers={"Authorization": f"Bearer {api_key}"},
        json=payload,
    )
    if res.status_code // 100 != 2:
        raise RuntimeError(f"OpenRouter API error: {res.text}")
    data = res.json()
    content = data["choices"][0]["message"]["content"].strip()
    clean = re.sub(r"```json|```", "", content).strip()
    return json.loads(clean)


def get_commission_rate(client: httpx.Client, affiliate_pubkey: Pubkey, current_rate_bps: int, total_referred_volume: int) -> int:
    prompt = classification_prompt(affiliate_pubkey, current_rate_bps, total_referred_volume)
    model = resolve_openrouter_model()
    key = resolve_openrouter_api_key()
    if not key:
        raise RuntimeError("Missing OpenRouter API key (OPENROUTER_API_KEY or ~/.api-openrouter)")
    result = _get_via_openrouter(client, model, key, prompt)
    new_rate = result.get("new_rate_bps")
    if not isinstance(new_rate, int):
        raise RuntimeError("Failed to parse new_rate_bps")
    return new_rate


def fetch_affiliate_info(rpc: httpx.Client, affiliate_info_pda: Pubkey) -> tuple[int, int] | None:
    """Returns (commission_rate_bps, total_referred_volume) or None if the account doesn't exist."""
    resp = rpc.post(
        RPC_URL,
        json={
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [str(affiliate_info_pda), {"encoding": "base64"}],
        },
    )
    resp.raise_for_status()
    value = resp.json()["result"]["value"]
    if value is None:
        return None

    data = base64.b64decode(value["data"][0])
    # AffiliateInfo layout after the 8-byte Anchor discriminator:
    #   affiliate_key: Pubkey(32), total_referred_volume: u64(8), commission_rate_bps: u16(2), ...
    total_referred_volume = int.from_bytes(data[8 + 32 : 8 + 32 + 8], "little")
    commission_rate_bps = int.from_bytes(data[8 + 32 + 8 : 8 + 32 + 8 + 2], "little")
    return commission_rate_bps, total_referred_volume


def set_commission_rate_instruction(affiliate_info_pda: Pubkey, affiliate_key: Pubkey, new_rate_bps: int):
    accounts = [
        AccountMeta(affiliate_info_pda, is_signer=False, is_writable=True),
        AccountMeta(affiliate_key, is_signer=True, is_writable=True),
    ]
    return anchor.build_instruction(PROGRAM_ID, "set_commission_rate", anchor.pack_u16(new_rate_bps), accounts)


def main() -> None:
    payer_kp_path = "~/.config/solana/id.json"
    payer = anchor.load_keypair(payer_kp_path)

    affiliate_to_manage = Keypair()
    print(f"Managing affiliate: {affiliate_to_manage.pubkey()}")

    rpc = httpx.Client(timeout=30.0)
    http_client = httpx.Client(timeout=30.0)
    print("\n--- Starting Optimizer Update Cycle ---")

    affiliate_info_pda, _bump = genesis_common.derive_affiliate_info_address(affiliate_to_manage.pubkey(), PROGRAM_ID)

    info = fetch_affiliate_info(rpc, affiliate_info_pda)
    if info is None:
        print("Affiliate not registered. Exiting.")
        return
    commission_rate_bps, total_referred_volume = info

    print(f"Fetched on-chain data: rate={commission_rate_bps} bps, volume={total_referred_volume}")

    try:
        new_rate_bps = get_commission_rate(http_client, affiliate_to_manage.pubkey(), commission_rate_bps, total_referred_volume)
    except Exception as e:
        print(f"Failed to get rate from provider: {e}", file=sys.stderr)
        print("\n--- Update Cycle Complete ---")
        return

    print(f"AI suggested new rate: {new_rate_bps} bps")
    if new_rate_bps == commission_rate_bps:
        print("Rate is already optimal. No update needed.")
    else:
        print("Sending transaction to update rate...")
        ix = set_commission_rate_instruction(affiliate_info_pda, affiliate_to_manage.pubkey(), new_rate_bps)

        blockhash_resp = rpc.post(RPC_URL, json={"jsonrpc": "2.0", "id": 1, "method": "getLatestBlockhash"})
        blockhash_resp.raise_for_status()
        blockhash = blockhash_resp.json()["result"]["value"]["blockhash"]

        tx = Transaction.new_signed_with_payer(
            [ix], payer.pubkey(), [payer, affiliate_to_manage], Hash.from_string(blockhash)
        )

        send_resp = rpc.post(
            RPC_URL,
            json={
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sendTransaction",
                "params": [base64.b64encode(bytes(tx)).decode(), {"encoding": "base64"}],
            },
        )
        result = send_resp.json()
        if "result" in result:
            print(f"Transaction successful! Signature: {result['result']}")
        else:
            print(f"Transaction failed: {result.get('error')}", file=sys.stderr)

    print("\n--- Update Cycle Complete ---")


if __name__ == "__main__":
    main()
