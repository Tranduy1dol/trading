use actix_web::{HttpResponse, Responder, web};
use crossbeam_channel::Sender;
use tokio::sync::oneshot;

use application::{command::Command, engine_thread::EngineMessage, response::Response};

use domain::order::{Order, OrderSide, OrderType};
use domain::price::Price;

use super::dto::{AddOrderRequest, OrderResponse};

pub async fn add_order(
    body: web::Json<AddOrderRequest>,
    engine: web::Data<Sender<EngineMessage>>,
) -> impl Responder {
    let side = match body.side.as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => {
            return HttpResponse::BadRequest().json(OrderResponse {
                success: false,
                trades: vec![],
                error: Some("invalid side".into()),
            });
        }
    };

    let order_type = match body.r#type.as_str() {
        "gtc" => OrderType::GTC,
        "ioc" => OrderType::IOC,
        "fok" => OrderType::FOK,
        _ => {
            return HttpResponse::BadRequest().json(OrderResponse {
                success: false,
                trades: vec![],
                error: Some("invalid order_type".into()),
            });
        }
    };

    let order = Order::new(
        body.id,
        body.user_id,
        body.asset_id,
        body.quantity,
        Price(body.price),
        side,
        order_type,
    );

    let (tx, rx) = oneshot::channel();
    let _ = engine.send((Command::AddOrder(order), tx));

    match rx.await {
        Ok(Response::Trades(trades)) => HttpResponse::Ok().json(OrderResponse {
            success: true,
            trades,
            error: None,
        }),
        Ok(Response::Error(e)) => HttpResponse::BadRequest().json(OrderResponse {
            success: false,
            trades: vec![],
            error: Some(format!("{:?}", e)),
        }),
        _ => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn cancel_order(
    path: web::Path<(u64, u64)>,
    engine: web::Data<Sender<EngineMessage>>,
) -> impl Responder {
    let (asset_id, order_id) = path.into_inner();
    let (tx, rx) = oneshot::channel();
    let _ = engine.send((Command::CancelOrder { asset_id, order_id }, tx));

    match rx.await {
        Ok(Response::Ack) => HttpResponse::Ok().json(serde_json::json!({"success": true})),
        _ => HttpResponse::InternalServerError().finish(),
    }
}
