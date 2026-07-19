//! CoreState structure

#[derive(Clone, Copy, Debug, Default)]
pub struct Account {
    pub base_balance: i128,
}

#[derive(Clone, Debug)]
pub struct CoreState {
    pub global_field: i128,
    pub total_base_sum: i128,
    pub total_supply: u128,
    pub total_minted: u128,
    pub total_burned: u128,
    pub participants_count: u64,
    pub dust_accumulator: u128,
    pub paused: bool,
}

impl CoreState {
    pub fn new() -> Self {
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

    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0;
        h = h.wrapping_mul(31).wrapping_add(self.global_field as u64);
        h = h.wrapping_mul(31).wrapping_add(self.total_base_sum as u64);
        h = h.wrapping_mul(31).wrapping_add(self.total_supply as u64);
        h = h.wrapping_mul(31).wrapping_add(self.participants_count);
        h
    }
}
