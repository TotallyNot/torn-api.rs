use serde::Deserialize;
use torn_api_macros::ApiCategory;

#[derive(Debug, Clone, Copy, ApiCategory)]
#[api(category = "market")]
pub enum MarketSelection {
    #[api(type = "Vec<BazaarItem>", field = "bazaar")]
    Bazaar,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BazaarItem {
    pub cost: u64,
    pub quantity: u32,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::{async_test, setup, Client, ClientTrait};

    #[async_test]
    async fn market_bazaar() {
        let key = setup();

        let response = Client::default()
            .torn_api(key)
            .market(|b| b.id(1).selections([MarketSelection::Bazaar]))
            .await
            .unwrap();

        _ = response.bazaar().unwrap();
    }
}
