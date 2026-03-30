use rustc_hash::FxHashMap;

use crate::{error::OrderError, order::Order, order_book::OrderBook, price::Price, trade::Trade};

pub struct Exchange {
    books: FxHashMap<u64, OrderBook>,
    capacity: usize,
}

impl Exchange {
    pub fn new(capacity: usize) -> Self {
        Self {
            books: FxHashMap::default(),
            capacity,
        }
    }

    pub fn get_book(&self, asset_id: u64) -> Option<&OrderBook> {
        self.books.get(&asset_id)
    }

    pub fn add_order(&mut self, order: Order) -> Result<&[Trade], OrderError> {
        let book = self
            .books
            .entry(order.asset_id)
            .or_insert_with(|| OrderBook::new(self.capacity));
        book.add_order(order)
    }

    pub fn cancel_order(&mut self, asset_id: u64, order_id: u64) {
        if let Some(book) = self.books.get_mut(&asset_id) {
            book.cancel_order(order_id);
        }
    }

    pub fn modify_order(&mut self, asset_id: u64, order_id: u64, new_price: Price, new_qty: u64) {
        if let Some(book) = self.books.get_mut(&asset_id) {
            book.modify_order(order_id, new_price, new_qty);
        }
    }
}
