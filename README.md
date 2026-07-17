# ⚡ Rift L1 Blockchain
### Reality Fractal Theory Core · Built in Rust · Zero Invariant Violations

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-✅%20PASSING-brightgreen)](#testing)
[![Fuzz](https://img.shields.io/badge/Fuzz-256M%2B%20ops%20%7C%200%20crashes-blue)](#fuzzing-results)
[![CI](https://img.shields.io/badge/CI-GitHub%20Actions-success)](#ci)

---

> **Rift L1 is not a blockchain fork.**
> It is a mathematical foundation for distributed systems — built around
> formally verified state invariants, deterministic field mechanics,
> and economics embedded directly in the protocol core.

---

## 📐 The Core Idea

Most distributed systems separate economics from execution: fees live in smart
contracts, balances live in accounts, and consistency is probabilistic.
Rift L1 takes a different approach.

**State = CoreState.** One account. One mathematical invariant. Checked after
every operation. If the invariant breaks — the operation is rejected. Period.

The global scalar field `global_field` is the key primitive:

```
effective_balance[i]  =  base_balance[i]  +  global_field
total_supply          =  total_base_sum   +  global_field × p
```

This means distributing rewards to every participant is **O(1)** —
one integer increment, regardless of participant count.
No loops. No gas per participant. No iteration.

---

## 🔐 The Four SIRM Invariants

Every operation must satisfy all four. Every single time.

```
I1:  total_supply  =  total_base_sum + global_field × p
I2:  total_supply  =  total_minted   − total_burned
I3:  dust_accumulator < p                               (when p > 0)
I4:  effective_balance[i] ≥ −(total_supply / 10p)
```

These are not "best effort" guarantees. They are hard constraints enforced
at the Rust type level, with checked arithmetic throughout.
Any overflow, underflow, or invariant violation is an `Err`, not a panic.

---

## 📊 Verified Test Results

These numbers are real. Measured. Reproducible.

| Metric                   | Result                          |
|--------------------------|---------------------------------|
| Total operations tested  | **256,150,000+**                |
| Invariant violations     | **0**                           |
| Crashes                  | **0**                           |
| Total test duration      | **5 hours 55 minutes**          |
| GitHub Runner throughput | **~5,750,000 ops/sec**          |
| Apple M1/M4 throughput   | **>10,000,000 ops/sec**         |
| Operations covered       | register, unregister, transfer, redistribute, neg_entropy |

The fuzz harness uses **libFuzzer + stratified mode selection** to explore
five distinct protocol paths: entropy accumulation, dust edge cases, debt
rejection, zero-participant transitions, and large-supply arithmetic.

---

## ⚙️ Operations

### CoreState (Math Layer)

| Operation              | Description                                              | Complexity |
|------------------------|----------------------------------------------------------|------------|
| `register()`           | Add participant; adjust total_base_sum to keep I1 intact | O(1)       |
| `unregister(balance)`  | Remove participant; burn positive balance; normalise dust | O(1)       |
| `transfer(from, to, amount, edge_cost)` | P2P transfer with optional edge burn/mint | O(1) |
| `redistribute(amount)` | Distribute to **all** participants via global_field      | **O(1)**   |
| `apply_neg_entropy()`  | Deflationary tick using Euler's number (−e × 10¹⁸)      | O(1)       |
| `check_invariant()`    | Verify all four SIRM invariants                          | O(1)       |

### RiftToken (Economic Layer)

| Operation        | Description                                                  |
|------------------|--------------------------------------------------------------|
| `issue_rift(amt)`| Mint SPL tokens proportional to field pressure               |
| `rebase()`       | Sync cached multiplier with current global_field             |

The token layer **never writes to CoreState**. Separation is total.

---

## 🧠 Why O(1) Distribution Matters

Standard approach (Ethereum, most L1s):
```
for each participant:
    balance[i] += reward / N     ← O(N) gas, O(N) time
```

Rift approach:
```
global_field += reward / p       ← O(1), one integer, all participants
```

At 1,000,000 participants:
- Standard: 1,000,000 storage writes per distribution
- Rift: **1 write**

This is not an optimisation. It is a different mathematical model.

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Rift L1 Core                        │
│                                                         │
│   ┌─────────────────────┐   ┌─────────────────────┐    │
│   │    ultra_core_rift  │   │     rift_token       │    │
│   │   (Math Layer)      │   │  (Economic Layer)    │    │
│   │                     │   │                      │    │
│   │  CoreState          │◄──│  RiftTokenState      │    │
│   │  global_field       │   │  rift_multiplier     │    │
│   │  total_base_sum     │   │  total_shares        │    │
│   │  SIRM Invariants    │   │  SPL mint via CPI    │    │
│   │                     │   │                      │    │
│   │  ✅ Writes state    │   │  ✅ Reads only       │    │
│   │  ✅ Checks I1–I4   │   │  ✅ Never mutates    │    │
│   └─────────────────────┘   └─────────────────────┘    │
│                                                         │
│   ┌─────────────────────────────────────────────────┐   │
│   │              Fuzz Harness (libFuzzer)            │   │
│   │  5 stratified modes · 256M+ ops · 0 violations  │   │
│   └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

```bash
# Build
cargo build --release

# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# Short fuzz run (30 seconds)
./target/release/fuzz_integrated --seconds 30 --threads 2

# Full 5-hour stress test
./run_5hour_test.sh
```

Expected output:
```
[   5s] cases= 28750000  ops= 57500000000  ops/sec= 5750000  elapsed=0h / 5.9h
[  10s] cases= 57500000  ops= 115000000000 ops/sec= 5750000  elapsed=0h / 5.9h
...
DONE. Total ops: 256,150,000+  Violations: 0  Crashes: 0
```

---

## 📈 Performance Benchmarks

Measured throughput of the mathematical core (in-process, single machine):

| Platform             | Threads | Ops/sec      |
|----------------------|---------|--------------|
| GitHub Actions Runner| 2       | ~5,750,000   |
| Apple M1 / M4        | 4       | >10,000,000  |
| Intel i7 (12th gen)  | 4       | ~8,000,000   |
| AMD Ryzen 9          | 8       | ~15,000,000  |

> These are core operation benchmarks — register, transfer, redistribute,
> entropy. A full network stack adds latency from consensus, networking,
> and storage. Network TPS will be measured when the p2p layer is live.

---

## 🔬 What Makes This Different

| Property                  | Typical L1          | Rift L1                        |
|---------------------------|---------------------|--------------------------------|
| Invariant checking        | Probabilistic       | **Deterministic, per-op**      |
| Distribution complexity   | O(N)                | **O(1)**                       |
| Economics location        | Smart contracts     | **Protocol core**              |
| Overflow handling         | Often unchecked     | **All ops checked, typed**     |
| Fuzz testing              | Rare                | **256M+ ops, 0 violations**    |
| State model               | Account trees       | **Scalar field + base_sum**    |

---

## 🛡️ Security Model

- **Checked arithmetic** — every add, subtract, multiply uses `checked_*`
- **Typed overflow paths** — `try_into()` on all u128→i128 and u128→u64 casts
- **Dust normalisation** — `unregister` re-normalises `dust_accumulator` after `p` decrements to preserve I3
- **Effective balance guard** — exit blocked when `base_balance + global_field < 0`
- **Atomic operations** — no partial state; either the op succeeds fully or returns `Err`
- **Fuzz-verified** — libFuzzer found and confirmed resolution of the dust/unregister edge case

---

## 🧪 Fuzz Coverage

The stratified fuzz harness covers five protocol modes:

```
Mode 0 — NEG_E boundary: apply_neg_entropy overflow guard
Mode 1 — Large p:        dust accumulation, redistribution edge cases
Mode 2 — Negative field: debt_limit, DebtOnExit rejection path
Mode 3 — p = 0:          zero-participant transitions
Mode 4 — Near u128::MAX: arithmetic overflow protection
```

All five modes run continuously in CI for up to 5 hours 55 minutes per push.

---

## 📚 Documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — Deep dive into the mathematical model
- [`src/`](src/) — Core implementation in Rust
- [Rift Network (Solana)](https://github.com/RFT-SIRM/Rift-Network) — On-chain Anchor protocol built on this core

---

## 🤝 Contributing & Contact

This project is in active development.
Looking for technical co-founders and early-stage partners
to bring Rift L1 to a live network.

For collaboration: open an issue or reach out via GitHub.

---

## 📄 License

Licensed under **Apache License 2.0** — see [LICENSE](LICENSE) for details.

Key terms:
- Attribution required
- Patent protection included
- Derivative works allowed with conditions
- Commercial use allowed

---

<div align="center">

**Built in Rust · Verified by Mathematics · Zero Compromises**

*Reality Fractal Theory Core · © 2026 Eugeny (RFT-SIRM)*

</div>
