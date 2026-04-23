// src/coin/marisselle_coin.rs
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarisselleCoin {
    pub name: String,
    pub symbol: String,
    pub total_supply: u64,
    pub decimals: u8,
}

impl MarisselleCoin {
    pub fn new() -> Self {
        Self {
            name: "MarisselleCoin".to_string(),
            symbol: "MRL".to_string(),
            total_supply: 21_000_000,
            decimals: 8,
        }
    }
}

impl Default for MarisselleCoin {
    fn default() -> Self {
        Self::new()
    }
}