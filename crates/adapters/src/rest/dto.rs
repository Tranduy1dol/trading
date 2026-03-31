use domain::trade::Trade;
use serde::{Deserialize, Serialize};

pub struct AddOrderRequest {
    pub id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub quantity: u64,
    pub price: u64,
    pub side: String,
    pub r#type: String,
}

#[derive(Deserialize, Serialize)]
pub struct OrderResponse {
    pub success: bool,
    pub trades: Vec<Trade>,
    pub error: Option<String>,
}
