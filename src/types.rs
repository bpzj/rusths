use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub volume: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KLineData {
    pub time: DateTime<Local>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockData {
    pub code: String,
    pub name: String,
    pub change: f64,
    pub volume: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub time: DateTime<Local>,
    pub price: f64,
    pub volume: i64,
    pub bs_flag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookData {
    pub price: f64,
    pub volume: i64,
    pub order_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpoData {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub pe: f64,
    pub shares: i64,
    pub date: String,
} 