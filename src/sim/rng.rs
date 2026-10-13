//! Small deterministic RNG shared by the simulation (seedable for tests).

use std::cell::Cell;

thread_local! {
    static STATE: Cell<u64> = const { Cell::new(0x2545_F491_4F6C_DD1D) };
}

pub fn srand(seed: u64) {
    STATE.with(|s| s.set(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1));
}

fn next_u64() -> u64 {
    STATE.with(|s| {
        let mut x = s.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        s.set(x);
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    })
}

pub trait Sample: Sized {
    fn sample(lo: Self, hi: Self) -> Self;
}

impl Sample for f32 {
    fn sample(lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((next_u64() >> 40) as f32 / (1u64 << 24) as f32)
    }
}

impl Sample for usize {
    fn sample(lo: usize, hi: usize) -> usize {
        if hi <= lo { lo } else { lo + (next_u64() % (hi - lo) as u64) as usize }
    }
}

impl Sample for i32 {
    fn sample(lo: i32, hi: i32) -> i32 {
        if hi <= lo { lo } else { lo + (next_u64() % (hi - lo) as u64) as i32 }
    }
}

/// Uniform sample in `[lo, hi)`.
pub fn gen_range<T: Sample>(lo: T, hi: T) -> T {
    T::sample(lo, hi)
}
