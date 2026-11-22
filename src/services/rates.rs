//! Currency rates from Frankfurter (ECB), fetched at most daily and cached.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{http, spawn, Bus, Event};
use crate::util::{load_json, save_json, unix_secs};

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Rates {
    pub fetched: i64,
    pub rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct Resp {
    rates: HashMap<String, f64>,
}

pub fn load_cached() -> HashMap<String, f64> {
    load_json::<Rates>("cache/rates.json").rates
}

/// Fetches if the cache is older than a day.
pub fn refresh(bus: Bus) {
    let cached: Rates = load_json("cache/rates.json");
    if unix_secs() - cached.fetched < 86400 && !cached.rates.is_empty() {
        return;
    }
    spawn("rates", move || {
        super::com_mta();
        if let Ok(r) = http::get("https://api.frankfurter.app/latest", &[]) {
            if let Ok(p) = serde_json::from_str::<Resp>(&r.body) {
                let rates = Rates {
                    fetched: unix_secs(),
                    rates: p.rates,
                };
                let _ = crate::util::sub_dir("cache");
                save_json("cache/rates.json", &rates);
                bus.send(Event::Rates(rates));
            }
        }
    });
}
