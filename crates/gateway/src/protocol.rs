#[repr(C, packed)]
pub struct FrameHeader {
    pub len: u32,
    pub msg_type: u8,
}

pub const HEADER_SIZE: usize = size_of::<FrameHeader>();

pub const MSG_NEW_ORDER: u8 = 0x01;
pub const MSG_CANCEL_ORDER: u8 = 0x02;
pub const MSG_MODIFY_ORDER: u8 = 0x03;
pub const MSG_ACK: u8 = 0x10;
pub const MSG_FILL: u8 = 0x11;
pub const MSG_REJECT: u8 = 0x12;

#[repr(C, packed)]
pub struct NewOrderMsg {
    pub client_seq: u64,
    pub order_id: u64,
    pub user_id: u64,
    pub asset_id: u64,
    pub price: u64,
    pub quantity: u64,
    pub side: u8,
    pub order_type: u8,
}

#[repr(C, packed)]
pub struct CancelOrderMsg {
    pub client_seq: u64,
    pub order_id: u64,
    pub asset_id: u64,
}

#[repr(C, packed)]
pub struct ModifyOrderMsg {
    pub client_seq: u64,
    pub order_id: u64,
    pub asset_id: u64,
    pub new_price: u64,
    pub new_qty: u64,
}

#[repr(C, packed)]
pub struct AckMsg {
    pub client_seq: u64,
    pub engine_seq: u64,
}

#[repr(C, packed)]
pub struct FillMsg {
    pub engine_seq: u64,
    pub taker_order_id: u64,
    pub maker_order_id: u64,
    pub price: u64,
    pub quantity: u64,
    pub taker_side: u8,
    pub timestamp: u64,
}

#[repr(C, packed)]
pub struct RejectMsg {
    pub client_seq: u64,
    pub engine_seq: u64,
    pub reason: u8,
}
