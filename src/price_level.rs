use crate::{order_pool::OrderPool, order_queue::OrderQueue};

const CHUNK: usize = 64;
const MAX_LEVEL: usize = 1001;
const B: usize = (MAX_LEVEL + CHUNK - 1) / CHUNK;

pub struct PriceLevel {
    levels: [OrderQueue; MAX_LEVEL],
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
        self.bitmap[index / CHUNK] &= 1 << (index % CHUNK);
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

    pub fn set_pool(&mut self, pool: &OrderPool) {
        todo!()
    }

    fn find_next_non_empty_from(&self, start: usize) -> Option<usize> {
        if start >= MAX_LEVEL {
            return None;
        }

        let chunk = start / CHUNK;
        let offset = start % CHUNK;

        let mut w = self.bitmap[chunk] & (!0u64 << offset);
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
}
