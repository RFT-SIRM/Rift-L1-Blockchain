use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Constants
// ============================================================================
const MAX_PARTICIPANTS: u64 = 100000;
const NEG_E: i128 = -2_718_281_828_459_045_235i128;
const NEG_E_MAX_P: i128 = 6_827_960_122_002_486_635i128;
const MAX_EDGE_COST: i128 = 1_000_000_000i128;
const MIN_FIELD_PRESSURE: u128 = 1_000_000;
const FOUNDER_SHARE_BPS: u16 = 314;
const MAX_FEE_BPS: u16 = 10;
const NUM_ACCOUNTS: usize = 12;
const OPS_PER_CASE: usize = 2_000;
const HISTORY_CAPACITY: usize = 500;

// ============================================================================
// State
// ============================================================================

#[derive(Clone, Debug)]
struct CoreState {
    global_field: i128,
    total_base_sum: i128,
    total_supply: u128,
    total_minted: u128,
    total_burned: u128,
    participants_count: u64,
    dust_accumulator: u128,
    paused: bool,
}

#[derive(Clone, Debug)]
struct RiftTokenState {
    total_shares: u64,
    rift_multiplier: u128,
    fee_bps: u16,
}

#[derive(Clone, Copy, Debug, Default)]
struct Account {
    base_balance: i128,
}

#[derive(Debug, Clone)]
enum Op {
    Register,
    Unregister { idx: usize },
    Transfer { from: usize, to: usize, amount: u64, edge_cost: i128 },
    Redistribute { amount: u64 },
    ApplyNegEntropy,
    SetPaused { paused: bool },
    IssueRift { amount: u64 },
    Rebase,
}

#[derive(Debug)]
struct InvariantError(String);

impl CoreState {
    fn new() -> Self {
        CoreState {
            global_field: 0,
            total_base_sum: 0,
            total_supply: 0,
            total_minted: 0,
            total_burned: 0,
            participants_count: 0,
            dust_accumulator: 0,
            paused: false,
        }
    }

    fn check_invariant(&self) -> Result<(), InvariantError> {
        let field_contrib = self
            .global_field
            .checked_mul(self.participants_count as i128)
            .ok_or_else(|| InvariantError("overflow: field_contrib".into()))?;

        let expected = self
            .total_base_sum
            .checked_add(field_contrib)
            .ok_or_else(|| InvariantError("overflow: expected".into()))?;

        let supply_signed = self.total_supply as i128;
        if supply_signed != expected {
            return Err(InvariantError(format!(
                "SIRM invariant violated: supply({}) != base_sum({}) + field({}) * p({}) = {}",
                supply_signed, self.total_base_sum, self.global_field, self.participants_count, expected
            )));
        }

        if self.total_minted < self.total_burned {
            return Err(InvariantError(format!(
                "minted({}) < burned({})",
                self.total_minted, self.total_burned
            )));
        }

        let net_supply = self
            .total_minted
            .checked_sub(self.total_burned)
            .ok_or_else(|| InvariantError("overflow: net_supply".into()))?;

        if self.total_supply != net_supply {
            return Err(InvariantError(format!(
                "supply({}) != minted-burned({})",
                self.total_supply, net_supply
            )));
        }

        if self.participants_count > 0 && self.dust_accumulator >= self.participants_count as u128 {
            return Err(InvariantError(format!(
                "dust({}) >= participants({})",
                self.dust_accumulator, self.participants_count
            )));
        }

        if self.total_supply > i128::MAX as u128 {
            return Err(InvariantError("total_supply exceeds i128::MAX".into()));
        }

        Ok(())
    }

    fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0;
        h = h.wrapping_mul(31).wrapping_add(self.global_field as u64);
        h = h.wrapping_mul(31).wrapping_add(self.total_base_sum as u64);
        h = h.wrapping_mul(31).wrapping_add(self.total_supply as u64);
        h = h.wrapping_mul(31).wrapping_add(self.participants_count);
        h = h.wrapping_mul(31).wrapping_add(self.dust_accumulator as u64);
        h
    }

    fn debt_limit(&self) -> Result<i128, InvariantError> {
        let factor = (self.participants_count as i128)
            .checked_mul(10)
            .ok_or_else(|| InvariantError("overflow: debt_limit factor".into()))?;
        if factor == 0 {
            return Ok(-1);
        }
        let limit = (self.total_supply as i128)
            .checked_div(factor)
            .ok_or_else(|| InvariantError("overflow: debt_limit div".into()))?;
        Ok(-limit)
    }

    fn register(&mut self) -> Result<(), InvariantError> {
        if self.participants_count >= MAX_PARTICIPANTS {
            return Ok(());
        }
        self.participants_count = self
            .participants_count
            .checked_add(1)
            .ok_or_else(|| InvariantError("overflow: participants_count++".into()))?;
        self.total_base_sum = self
            .total_base_sum
            .checked_sub(self.global_field)
            .ok_or_else(|| InvariantError("overflow: total_base_sum (register)".into()))?;
        self.check_invariant()
    }

    fn unregister(&mut self, base_balance: i128) -> Result<bool, InvariantError> {
        if self.participants_count == 0 {
            return Ok(false);
        }
        let effective = base_balance
            .checked_add(self.global_field)
            .ok_or_else(|| InvariantError("overflow: effective_balance".into()))?;
        if effective < 0 {
            return Ok(false);
        }

        if base_balance > 0 {
            let burn = base_balance as u128;
            if self.total_supply < burn {
                return Ok(false);
            }
            self.total_supply = self
                .total_supply
                .checked_sub(burn)
                .ok_or_else(|| InvariantError("overflow: total_supply (unreg burn)".into()))?;
            self.total_burned = self
                .total_burned
                .checked_add(burn)
                .ok_or_else(|| InvariantError("overflow: total_burned".into()))?;
        } else if base_balance < 0 {
            let mint = base_balance.unsigned_abs();
            self.total_supply = self
                .total_supply
                .checked_add(mint)
                .ok_or_else(|| InvariantError("overflow: total_supply (unreg mint)".into()))?;
            self.total_minted = self
                .total_minted
                .checked_add(mint)
                .ok_or_else(|| InvariantError("overflow: total_minted".into()))?;
        }

        self.total_base_sum = self
            .total_base_sum
            .checked_sub(base_balance)
            .ok_or_else(|| InvariantError("overflow: total_base_sum sub".into()))?
            .checked_add(self.global_field)
            .ok_or_else(|| InvariantError("overflow: total_base_sum add".into()))?;

        self.participants_count = self
            .participants_count
            .checked_sub(1)
            .ok_or_else(|| InvariantError("underflow: participants_count--".into()))?;

        if self.participants_count > 0 && self.dust_accumulator >= self.participants_count as u128 {
            self.dust_accumulator = self
                .dust_accumulator
                .checked_rem(self.participants_count as u128)
                .ok_or_else(|| InvariantError("div-by-zero: dust normalize".into()))?;
        } else if self.participants_count == 0 {
            self.dust_accumulator = 0;
        }

        self.check_invariant()?;
        Ok(true)
    }

    fn transfer(
        &mut self,
        from: &mut Account,
        to: &mut Account,
        amount: u64,
        edge_cost: i128,
    ) -> Result<(), InvariantError> {
        if self.paused || amount == 0 {
            return Ok(());
        }

        let amt = amount as i128;
        let new_from = match from
            .base_balance
            .checked_sub(amt)
            .and_then(|v| v.checked_sub(edge_cost))
        {
            Some(v) => v,
            None => return Ok(()),
        };

        let debt_limit = self.debt_limit()?;
        if new_from < debt_limit {
            return Ok(());
        }

        let new_to = match to.base_balance.checked_add(amt) {
            Some(v) => v,
            None => return Ok(()),
        };

        from.base_balance = new_from;
        to.base_balance = new_to;

        if edge_cost != 0 {
            self.total_base_sum = self
                .total_base_sum
                .checked_sub(edge_cost)
                .ok_or_else(|| InvariantError("overflow: total_base_sum (edge)".into()))?;

            if edge_cost > 0 {
                let burn = edge_cost as u128;
                if self.total_supply < burn {
                    return Err(InvariantError(format!(
                        "SupplyUnderflow on edge burn: supply({}) < burn({})",
                        self.total_supply, burn
                    )));
                }
                self.total_supply = self
                    .total_supply
                    .checked_sub(burn)
                    .ok_or_else(|| InvariantError("overflow: total_supply (edge burn)".into()))?;
                self.total_burned = self
                    .total_burned
                    .checked_add(burn)
                    .ok_or_else(|| InvariantError("overflow: total_burned (edge)".into()))?;
            } else {
                let mint = edge_cost.unsigned_abs();
                self.total_supply = self
                    .total_supply
                    .checked_add(mint)
                    .ok_or_else(|| InvariantError("overflow: total_supply (edge mint)".into()))?;
                self.total_minted = self
                    .total_minted
                    .checked_add(mint)
                    .ok_or_else(|| InvariantError("overflow: total_minted (edge)".into()))?;
            }
        }

        self.check_invariant()
    }

    fn redistribute(&mut self, amount: u64) -> Result<(), InvariantError> {
        if self.paused || self.participants_count == 0 {
            return Ok(());
        }
        let p_u128 = self.participants_count as u128;
        let total = match (amount as u128).checked_add(self.dust_accumulator) {
            Some(v) => v,
            None => return Ok(()),
        };
        let q = total / p_u128;
        let r = total % p_u128;

        let q_i128: i128 = match q.try_into() {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        self.global_field = self
            .global_field
            .checked_add(q_i128)
            .ok_or_else(|| InvariantError("overflow: global_field (redistribute)".into()))?;

        let distributed = q
            .checked_mul(p_u128)
            .ok_or_else(|| InvariantError("overflow: distributed".into()))?;

        self.total_supply = self
            .total_supply
            .checked_add(distributed)
            .ok_or_else(|| InvariantError("overflow: total_supply (redistribute)".into()))?;
        self.total_minted = self
            .total_minted
            .checked_add(distributed)
            .ok_or_else(|| InvariantError("overflow: total_minted (redistribute)".into()))?;
        self.dust_accumulator = r;

        self.check_invariant()
    }

    fn apply_neg_entropy(&mut self) -> Result<(), InvariantError> {
        if self.paused {
            return Ok(());
        }
        let p_i128 = self.participants_count as i128;
        if p_i128 > NEG_E_MAX_P {
            return Ok(());
        }
        let delta = p_i128
            .checked_mul(NEG_E)
            .ok_or_else(|| InvariantError("overflow: delta (neg_entropy)".into()))?;

        self.global_field = self
            .global_field
            .checked_add(NEG_E)
            .ok_or_else(|| InvariantError("overflow: global_field (neg_entropy)".into()))?;
        self.total_base_sum = self
            .total_base_sum
            .checked_sub(delta)
            .ok_or_else(|| InvariantError("overflow: total_base_sum (neg_entropy)".into()))?;

        self.check_invariant()
    }
}

impl RiftTokenState {
    fn new(fee_bps: u16) -> Self {
        RiftTokenState {
            total_shares: 0,
            rift_multiplier: 1_000_000_000_000_000u128,
            fee_bps: fee_bps.min(MAX_FEE_BPS),
        }
    }

    fn issue_rift(&mut self, core: &CoreState, base_amount: u64) -> Result<(), InvariantError> {
        if core.paused || base_amount == 0 {
            return Ok(());
        }

        let fee_amount_u128 = (base_amount as u128 * self.fee_bps as u128) / 10_000;
        let fee_amount: u64 = match fee_amount_u128.try_into() {
            Ok(v) => v,
            Err(_) => return Err(InvariantError("overflow: fee_amount downcast".into())),
        };

        if self.fee_bps > 0 && fee_amount == 0 {
            return Ok(());
        }

        let amount_after_fee = match base_amount.checked_sub(fee_amount) {
            Some(v) => v,
            None => return Err(InvariantError("underflow: amount_after_fee".into())),
        };

        let field_pressure = core.global_field.unsigned_abs().max(MIN_FIELD_PRESSURE);
        let mint_multiplier = 1_000_000_000_000_000u128
            .checked_div(field_pressure)
            .unwrap_or(1_000_000_000_000u128);

        if mint_multiplier > 1_000_000_000u128 {
            return Err(InvariantError(format!(
                "mint_multiplier {} exceeds ceiling 1e9 (field_pressure={})",
                mint_multiplier, field_pressure
            )));
        }

        let shares_to_mint_u128 = (amount_after_fee as u128)
            .checked_mul(mint_multiplier)
            .ok_or_else(|| InvariantError("overflow: shares_to_mint mult".into()))?
            / 1_000_000_000_000u128;

        if shares_to_mint_u128 == 0 {
            return Ok(());
        }

        let shares_to_mint: u64 = match shares_to_mint_u128.try_into() {
            Ok(v) => v,
            Err(_) => return Err(InvariantError("overflow: shares_to_mint downcast".into())),
        };

        self.total_shares = self
            .total_shares
            .checked_add(shares_to_mint)
            .ok_or_else(|| InvariantError("overflow: total_shares".into()))?;

        Ok(())
    }

    fn rebase(&mut self, core: &CoreState) {
        let field_pressure = core.global_field.unsigned_abs().max(MIN_FIELD_PRESSURE);
        self.rift_multiplier = 1_000_000_000_000_000u128
            .checked_div(field_pressure)
            .unwrap_or(1_000_000_000_000u128);
    }

    fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0;
        h = h.wrapping_mul(31).wrapping_add(self.total_shares as u64);
        h = h.wrapping_mul(31).wrapping_add((self.rift_multiplier >> 32) as u64);
        h = h.wrapping_mul(31).wrapping_add(self.fee_bps as u64);
        h
    }
}

fn gen_op(rng: &mut impl Rng) -> Op {
    match rng.gen_range(0..9) {
        0 => Op::Register,
        1 => Op::Unregister { idx: rng.gen_range(0..NUM_ACCOUNTS) },
        2 => Op::Transfer {
            from: rng.gen_range(0..NUM_ACCOUNTS),
            to: rng.gen_range(0..NUM_ACCOUNTS),
            amount: rng.gen_range(0..1_000_000_000u64),
            edge_cost: 0,
        },
        3 => Op::Transfer {
            from: rng.gen_range(0..NUM_ACCOUNTS),
            to: rng.gen_range(0..NUM_ACCOUNTS),
            amount: rng.gen_range(0..1_000_000_000u64),
            edge_cost: rng.gen_range(0..=MAX_EDGE_COST),

        },
        4 => Op::Redistribute { amount: rng.gen_range(0..10_000_000_000u64) },
        5 => Op::ApplyNegEntropy,
        6 => Op::SetPaused { paused: rng.gen_bool(0.05) },
        7 => Op::IssueRift { amount: rng.gen_range(0..1_000_000_000u64) },
        _ => Op::Rebase,
    }
}

#[derive(Clone)]
struct OperationRecord {
    op: Op,
    core_fingerprint: u64,
    token_fingerprint: u64,
}

fn run_case(seed: u64, show_history: bool) -> Result<u64, (usize, Op, String, Vec<OperationRecord>)> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut core = CoreState::new();
    let fee_bps = rng.gen_range(0..=MAX_FEE_BPS);
    let mut token = RiftTokenState::new(fee_bps);
    let mut accounts = [Account::default(); NUM_ACCOUNTS];

    let initial_supply: u64 = rng.gen();
    let founder_share = ((initial_supply as u128 * FOUNDER_SHARE_BPS as u128) / 10_000) as u64;
    if founder_share > 0 {
        token.total_shares = founder_share;
    }

    let mut history: Vec<OperationRecord> = Vec::new();
    let mut ops_executed: u64 = 0;

    for i in 0..OPS_PER_CASE {
        let op = gen_op(&mut rng);

        let result: Result<(), InvariantError> = (|| {
            match &op {
                Op::Register => core.register(),
                Op::Unregister { idx } => {
                    let bal = accounts[*idx % NUM_ACCOUNTS].base_balance;
                    core.unregister(bal).map(|_| ())
                }
                Op::Transfer { from, to, amount, edge_cost } => {
                    let f = *from % NUM_ACCOUNTS;
                    let t = *to % NUM_ACCOUNTS;
                    if f == t {
                        return Ok(());
                    }
                    let (a, b) = if f < t {
                        let (left, right) = accounts.split_at_mut(t);
                        (&mut left[f], &mut right[0])
                    } else {
                        let (left, right) = accounts.split_at_mut(f);
                        (&mut right[0], &mut left[t])
                    };
                    if f < t {
                        core.transfer(a, b, *amount, *edge_cost)
                    } else {
                        core.transfer(b, a, *amount, *edge_cost)
                    }
                }
                Op::Redistribute { amount } => core.redistribute(*amount),
                Op::ApplyNegEntropy => core.apply_neg_entropy(),
                Op::SetPaused { paused } => {
                    core.paused = *paused;
                    Ok(())
                }
                Op::IssueRift { amount } => token.issue_rift(&core, *amount),
                Op::Rebase => {
                    token.rebase(&core);
                    Ok(())
                }
            }
        })();

        if let Err(e) = result {
            if show_history {
                println!("\n=== OPERATION HISTORY (last {} ops) ===", history.len().min(100));
                for (j, rec) in history.iter().rev().take(100).enumerate() {
                    println!("[{}] {:?}", i - j, rec.op);
                    println!("    core_fp={:016x}, token_fp={:016x}", rec.core_fingerprint, rec.token_fingerprint);
                }
                println!("[{}] {:?} <- FAILED HERE", i, op);
            }
            return Err((i, op, e.0, history));
        }

        if token.rift_multiplier > 1_000_000_000_000_000u128 {
            return Err((
                i,
                op,
                format!("rift_multiplier {} exceeds ceiling 1e15", token.rift_multiplier),
                history,
            ));
        }

        history.push(OperationRecord {
            op: op.clone(),
            core_fingerprint: core.fingerprint(),
            token_fingerprint: token.fingerprint(),
        });

        if history.len() > HISTORY_CAPACITY {
            history.remove(0);
        }

        ops_executed += 1;
    }

    if let Err(e) = core.check_invariant() {
        return Err((OPS_PER_CASE, Op::Rebase, format!("final check: {}", e.0), history));
    }

    Ok(ops_executed)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if let Some(pos) = args.iter().position(|a| a == "--replay-seed") {
        let seed: u64 = args[pos + 1].parse().expect("seed must be u64");
        println!("Replaying seed {} (with history)...\n", seed);
        match run_case(seed, true) {
            Ok(n) => println!("Seed {} completed {} ops with NO failure.", seed, n),
            Err((idx, op, msg, _)) => {
                println!("\nSeed {} FAILED at op #{}: {:?}", seed, idx, op);
                println!("Reason: {}\n", msg);
                std::process::exit(1);
            }
        }
        return;
    }

    let duration_secs: u64 = args
        .iter()
        .position(|a| a == "--seconds")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(18_000);

    let base_seed: u64 = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
        });

    let num_threads: usize = args
        .iter()
        .position(|a| a == "--threads")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));

    println!("================================================================");
    println!(" RT / UltraCore-RFT + RiftToken integrated stress-fuzzer v2");
    println!(" (with operation history + state fingerprinting)");
    println!("================================================================");
    println!(" duration:      {} sec ({:.2} hours)", duration_secs, duration_secs as f64 / 3600.0);
    println!(" base_seed:     {}", base_seed);
    println!(" threads:       {}", num_threads);
    println!(" ops per case:  {}", OPS_PER_CASE);
    println!("================================================================\n");

    let stop = Arc::new(AtomicBool::new(false));
    let total_cases = Arc::new(AtomicU64::new(0));
    let total_ops = Arc::new(AtomicU64::new(0));
    let failure_seed: Arc<std::sync::Mutex<Option<u64>>> = Arc::new(std::sync::Mutex::new(None));

    let start = Instant::now();
    let deadline = Duration::from_secs(duration_secs);

    let mut handles = Vec::new();
    for t in 0..num_threads {
        let stop = Arc::clone(&stop);
        let total_cases = Arc::clone(&total_cases);
        let total_ops = Arc::clone(&total_ops);
        let failure_seed = Arc::clone(&failure_seed);

        handles.push(std::thread::spawn(move || {
            let mut case_seed = base_seed.wrapping_add(t as u64 * 0x9E3779B97F4A7C15);
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }

                case_seed = case_seed.wrapping_add(0x9E3779B97F4A7C15);

                match run_case(case_seed, false) {
                    Ok(n) => {
                        total_ops.fetch_add(n, Ordering::Relaxed);
                        total_cases.fetch_add(1, Ordering::Relaxed);
                    }
                    Err((_, _, _, _)) => {
                        let mut f = failure_seed.lock().unwrap();
                        if f.is_none() {
                            *f = Some(case_seed);
                        }
                        stop.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }));
    }

    loop {
        std::thread::sleep(Duration::from_secs(5));
        let elapsed = start.elapsed();
        let cases = total_cases.load(Ordering::Relaxed);
        let ops = total_ops.load(Ordering::Relaxed);
        let ops_per_sec = ops as f64 / elapsed.as_secs_f64().max(0.001);

        println!(
            "[{:>6.0}s] cases={:>10}  ops={:>14}  ops/sec={:>10.0}  elapsed={:.1}h / {:.1}h",
            elapsed.as_secs_f64(),
            cases,
            ops,
            ops_per_sec,
            elapsed.as_secs_f64() / 3600.0,
            duration_secs as f64 / 3600.0
        );

        if failure_seed.lock().unwrap().is_some() || elapsed >= deadline {
            stop.store(true, Ordering::Relaxed);
            break;
        }
    }

    for h in handles {
        let _ = h.join();
    }

    println!("\n================================================================");
    let failure_opt = failure_seed.lock().unwrap().clone();
    if let Some(seed) = failure_opt {
        println!(" ❌ FAILURE FOUND");
        println!(" Reproduce with:");
        println!("   cargo run --release --bin fuzz_integrated -- --replay-seed {}", seed);
        println!("================================================================");
        
        // Auto-replay with history for convenience
        eprintln!("\n>>> Auto-replaying to show operation history:\n");
        match run_case(seed, true) {
            Ok(_) => {},
            Err((_, _, _, _)) => {},
        }
        
        std::process::exit(1);
    } else {
        let total = total_ops.load(Ordering::Relaxed);
        let cases = total_cases.load(Ordering::Relaxed);
        println!(" ✅ NO FAILURES");
        println!(" Total cases:      {}", cases);
        println!(" Total operations: {}", total);
        println!(" Elapsed:          {:.2} hours", start.elapsed().as_secs_f64() / 3600.0);
        println!("================================================================");
    }
}
