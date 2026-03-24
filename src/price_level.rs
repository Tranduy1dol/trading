use crate::{order_queue::OrderQueue, price::Price};

pub const MAX_LEVEL: usize = 1001;
const CHUNK: usize = 64;
const B: usize = MAX_LEVEL.div_ceil(CHUNK);
const PRICE_OFFSET: u64 = 10000;

pub struct PriceLevel {
    pub levels: [OrderQueue; MAX_LEVEL],
    bitmap: [u64; B],
    pub totals: [u64; MAX_LEVEL],
}

impl Default for PriceLevel {
    fn default() -> Self {
        Self {
            levels: std::array::from_fn(|_| OrderQueue::new()),
            bitmap: [0u64; B],
            totals: [0u64; MAX_LEVEL],
        }
    }
}

impl PriceLevel {
    pub fn new() -> Self {
        Self::default()
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

    pub fn sub_qty_at(&mut self, index: usize, delta: u64) {
        if self.totals[index] > delta {
            self.totals[index] -= delta;
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

    pub fn get_price_from_index(index: usize) -> Price {
        Price(index as u64 + PRICE_OFFSET)
    }

    pub fn get_index_from_price(price: Price) -> Option<usize> {
        if price.0 < PRICE_OFFSET {
            return None;
        }

        let index = (price.0 - PRICE_OFFSET) as usize;

        if index >= MAX_LEVEL {
            None
        } else {
            Some(index)
        }
    }

    fn set_bit(&mut self, index: usize) {
        self.bitmap[index / CHUNK] |= 1 << (index % CHUNK);
    }

    fn clear_bit(&mut self, index: usize) {
        self.bitmap[index / CHUNK] &= !(1 << (index % CHUNK));
    }
}
