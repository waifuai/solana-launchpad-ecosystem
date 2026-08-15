"""Minimal Anchor wire-format helpers.

The original Rust bots used anchor-client, which builds instructions from a
generated IDL. There is no IDL here (the on-chain programs were removed --
see the SKILL.md design docs under .claude/skills/), so this hand-builds the
same wire format anchor-client would have produced: an 8-byte sighash
discriminator (sha256("global:<method>")[:8] for instructions,
sha256("account:<Type>")[:8] for accounts) followed by borsh-encoded args.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.pubkey import Pubkey


def instruction_discriminator(method_name: str) -> bytes:
    return hashlib.sha256(f"global:{method_name}".encode()).digest()[:8]


def account_discriminator(type_name: str) -> bytes:
    return hashlib.sha256(f"account:{type_name}".encode()).digest()[:8]


def load_keypair(path: str) -> Keypair:
    """Load a Solana CLI keypair file (JSON array of 64 secret-key bytes)."""
    raw = Path(path).expanduser().read_text(encoding="utf-8")
    return Keypair.from_bytes(bytes(json.loads(raw)))


def build_instruction(program_id: Pubkey, method_name: str, args: bytes, accounts: list[AccountMeta]) -> Instruction:
    data = instruction_discriminator(method_name) + args
    return Instruction(program_id, data, accounts)


def pack_u16(value: int) -> bytes:
    return value.to_bytes(2, "little")


def pack_u64(value: int) -> bytes:
    return value.to_bytes(8, "little")
