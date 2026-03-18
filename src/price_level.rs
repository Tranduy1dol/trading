use crate::order_queue::OrderQueue;

const CHUNK: usize = 64;
pub const MAX_LEVEL: usize = 1001;
const B: usize = MAX_LEVEL.div_ceil(CHUNK);

pub struct PriceLevel {
    pub levels: [OrderQueue; MAX_LEVEL],
    bitmap: [u64; B],
    totals: [u64; MAX_LEVEL],
}

impl PriceLevel {
    const fn bit_index_to_level(&self, chunk: usize, bit: usize) -> usize {
        chunk * CHUNK + bit
    }

    const fn total_at(&self, index: usize) -> u64 {
        self.totals[index]
    }

    fn set_bit(&mut self, index: usize) {
        self.bitmap[index / CHUNK] |= 1 << (index % CHUNK);
    }

    fn clear_bit(&mut self, index: usize) {
        self.bitmap[index / CHUNK] &= !(1 << (index % CHUNK));
    }

    pub fn add_qty_at(&mut self, index: usize, delta: u64) {
        self.totals[index] += delta;
        if self.totals[index] > 0 {
            self.set_bit(index);
        } else {
            self.totals[index] = 0;
            self.clear_bit(index);
        }
    }

    pub fn find_next_non_empty_from(&self, start: usize) -> Option<usize> {
        if start >= MAX_LEVEL {
            return None;
        }

        let chunk = start / CHUNK;
        let offset = start % CHUNK;

        let w = self.bitmap[chunk] & (!0u64 << offset);
        if w != 0 {
            return Some(chunk * CHUNK + w.trailing_zeros() as usize);
        }

        for c in (chunk + 1)..B {
            if self.bitmap[c] != 0 {
                return Some(c * CHUNK + self.bitmap[c].trailing_zeros() as usize);
            }
        }

        None
    }

    pub fn sub_qty_at(&mut self, index: usize, delta: u64) {
        if self.totals[index] > delta {
            self.totals[index] -= delta;
        } else {
            self.totals[index] = 0;
            self.clear_bit(index);
        }
    }

    pub fn find_prev_non_empty_from(&self, start: usize) -> Option<usize> {
        let start = if start >= MAX_LEVEL {
            MAX_LEVEL - 1
        } else {
            start
        };

        let chunk = start / CHUNK;
        let offset = start % CHUNK;

        let mask = !0u64 >> (63 - offset);
        let w = self.bitmap[chunk] & mask;
        if w != 0 {
            return Some(chunk * CHUNK + (63 - w.leading_zeros() as usize));
        }

        for c in (0..chunk).rev() {
            if self.bitmap[c] != 0 {
                return Some(c * CHUNK + (63 - self.bitmap[c].leading_zeros() as usize));
            }
        }

        None
    }
}
