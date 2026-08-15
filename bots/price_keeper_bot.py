"""DEX Price Keeper Bot.

An AI-powered price oracle for the barter DEX program in the Solana Launchpad
Ecosystem. Continuously monitors liquidity pools and updates on-chain prices
using AI-generated exchange rates.

Workflow:
1. Fetches all liquidity pools from the barter DEX program.
2. For each pool, constructs an AI prompt with the two token mint addresses.
3. Queries the OpenRouter API for an exchange rate.
4. Parses the AI response (9-decimal fixed point).
5. Submits a transaction to update the on-chain oracle price for each pool.

The barter-dex-program itself was removed (Solana on-chain programs cannot be
Python -- see .claude/skills/barter-dex-oracle/SKILL.md), so this talks to
whatever program is deployed at PROGRAM_ID using the same account layout and
instruction wire format the original Rust program used. Uses the program's
simple single-price update_oracle_price(new_price: u64) instruction, since
that was the version this bot's Rust source actually called.

Configuration:
- API key: env var OPENROUTER_API_KEY, or file ~/.api-openrouter
- Model: file ~/.model-openrouter (defaults to "openrouter/free")
- Wallet / oracle authority: ~/.config/solana/id.json
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
from solders.pubkey import Pubkey
from solders.transaction import Transaction

from shared import anchor

PROGRAM_ID = Pubkey.from_string("DEXy2D1fVf5s3f2y6D4b7j8N1M5P9kH3rW7T4gS6fX8a")
RPC_URL = "http://127.0.0.1:8899"

# LiquidityPool's Anchor account discriminator, for filtering getProgramAccounts.
_LIQUIDITY_POOL_DISCRIMINATOR = anchor.account_discriminator("LiquidityPool")


def _read_first_line(path: Path) -> str | None:
    try:
        text = path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    return text or None


def resolve_openrouter_model() -> str:
    p = Path.home() / ".model-openrouter"
    return _read_first_line(p) or "openrouter/free"


def resolve_openrouter_api_key() -> str | None:
    v = os.environ.get("OPENROUTER_API_KEY", "").strip()
    if v:
        return v
    p = Path.home() / ".api-openrouter"
    return _read_first_line(p)


def price_prompt(mint_a: Pubkey, mint_b: Pubkey) -> str:
    return (
        "You are a decentralized exchange price oracle. Your task is to provide the fair market exchange "
        "rate between two Solana tokens.\n"
        f"- Token A Mint: {mint_a}\n"
        f"- Token B Mint: {mint_b}\n\n"
        "Determine the price of 1 whole unit of Token A in terms of Token B.\n"
        "Provide the price as a u64 integer with 9 decimal places of precision. For example, a price of 1.5 "
        "means you should return 1500000000. A price of 1.0 is 1000000000.\n\n"
        "For this simulation, assume Token A is slightly more valuable than Token B. Return a price between "
        "1.1 and 1.3.\n\n"
        'Respond with ONLY a JSON object in the format: {"price_of_a_in_b": <u64_number>}'
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


def get_exchange_rate(client: httpx.Client, mint_a: Pubkey, mint_b: Pubkey) -> int:
    prompt = price_prompt(mint_a, mint_b)
    model = resolve_openrouter_model()
    key = resolve_openrouter_api_key()
    if not key:
        raise RuntimeError("Missing OpenRouter API key (OPENROUTER_API_KEY or ~/.api-openrouter)")
    result = _get_via_openrouter(client, model, key, prompt)
    price = result.get("price_of_a_in_b")
    if not isinstance(price, int):
        raise RuntimeError("Failed to parse price_of_a_in_b")
    return price


def fetch_liquidity_pools(rpc: httpx.Client) -> list[tuple[Pubkey, Pubkey, Pubkey]]:
    """Returns [(pool_pubkey, mint_a, mint_b), ...] for every LiquidityPool account."""
    resp = rpc.post(
        RPC_URL,
        json={
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getProgramAccounts",
            "params": [
                str(PROGRAM_ID),
                {
                    "encoding": "base64",
                    "filters": [{"memcmp": {"offset": 0, "bytes": base64.b64encode(_LIQUIDITY_POOL_DISCRIMINATOR).decode()}}],
                },
            ],
        },
    )
    resp.raise_for_status()
    result = resp.json().get("result") or []

    pools = []
    for entry in result:
        pool_pubkey = Pubkey.from_string(entry["pubkey"])
        data = base64.b64decode(entry["account"]["data"][0])
        # LiquidityPool layout after the 8-byte Anchor discriminator: mint_a: Pubkey(32), mint_b: Pubkey(32), ...
        mint_a = Pubkey.from_bytes(data[8 : 8 + 32])
        mint_b = Pubkey.from_bytes(data[8 + 32 : 8 + 64])
        pools.append((pool_pubkey, mint_a, mint_b))
    return pools


def update_oracle_price_instruction(pool_pda: Pubkey, oracle_authority: Pubkey, new_price: int):
    accounts = [
        AccountMeta(pool_pda, is_signer=False, is_writable=True),
        AccountMeta(oracle_authority, is_signer=True, is_writable=False),
    ]
    return anchor.build_instruction(PROGRAM_ID, "update_oracle_price", anchor.pack_u64(new_price), accounts)


def main() -> None:
    payer_kp_path = "~/.config/solana/id.json"
    payer = anchor.load_keypair(payer_kp_path)
    oracle_authority = anchor.load_keypair(payer_kp_path)
    print(f"Price Keeper starting with authority: {oracle_authority.pubkey()}")

    rpc = httpx.Client(timeout=30.0)
    http_client = httpx.Client(timeout=30.0)

    print("\n--- Starting Price Keeper Update Cycle ---")

    pools = fetch_liquidity_pools(rpc)
    if not pools:
        print("No liquidity pools found. Exiting.")
        return

    for pool_pda, mint_a, mint_b in pools:
        print(f"\nProcessing pool for {mint_a} <-> {mint_b}")

        try:
            new_price = get_exchange_rate(http_client, mint_a, mint_b)
        except Exception as e:
            print(f"Failed to get price from provider for pool {pool_pda}: {e}", file=sys.stderr)
            continue

        print(f"AI suggested new price: {new_price}")
        print("Sending transaction to update on-chain price...")

        ix = update_oracle_price_instruction(pool_pda, oracle_authority.pubkey(), new_price)

        blockhash_resp = rpc.post(RPC_URL, json={"jsonrpc": "2.0", "id": 1, "method": "getLatestBlockhash"})
        blockhash_resp.raise_for_status()
        blockhash = blockhash_resp.json()["result"]["value"]["blockhash"]

        tx = Transaction.new_signed_with_payer(
            [ix], payer.pubkey(), [payer, oracle_authority], Hash.from_string(blockhash)
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
            print(f"Price update successful! Signature: {result['result']}")
        else:
            print(f"Price update transaction failed: {result.get('error')}", file=sys.stderr)

    print("\n--- Update Cycle Complete ---")


if __name__ == "__main__":
    main()
