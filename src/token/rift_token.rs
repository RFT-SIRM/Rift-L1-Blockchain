//! RiftTokenState структура

#[derive(Clone, Debug)]
pub struct RiftTokenState {
    pub total_shares: u64,
    pub rift_multiplier: u128,
    pub fee_bps: u16,
}

impl RiftTokenState {
    pub fn new(fee_bps: u16) -> Self {
        RiftTokenState {
            total_shares: 0,
            rift_multiplier: 1_000_000_000_000_000u128,
            fee_bps: fee_bps.min(10),
        }
    }

    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0;
        h = h.wrapping_mul(31).wrapping_add(self.total_shares as u64);
        h = h.wrapping_mul(31).wrapping_add((self.rift_multiplier >> 32) as u64);
        h = h.wrapping_mul(31).wrapping_add(self.fee_bps as u64);
        h
    }
}
