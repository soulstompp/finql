///! Implementation for quote handler with Sqlite3 database as backend
use std::str::FromStr;
use chrono::{DateTime, Local};
use async_trait::async_trait;
use std::sync::Arc;

use finql_data::currency::Currency;
use finql_data::{DataError, QuoteHandler, AssetHandler};
use finql_data::quote::{Quote, Ticker};

use super::PostgresDB;

/// PostgreSQL implementation of quote handler
#[async_trait]
impl QuoteHandler for PostgresDB {
    fn into_arc_dispatch(self: Arc<Self>) -> Arc<dyn AssetHandler + Send + Sync> {
        self
    }

    // insert, get, update and delete for market data sources
    async fn insert_ticker(&self, ticker: &Ticker) -> Result<usize, DataError> {
        let row = sqlx::query!(
                "INSERT INTO ticker (name, asset_id, source, priority, currency, factor, tz, cal) 
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
                ticker.name,
                (ticker.asset as i32),
                (ticker.source.to_string()),
                ticker.priority,
                (ticker.currency.to_string()),
                ticker.factor,
                ticker.tz,
                ticker.cal
            ).fetch_one(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        let id: i32 = row.id;
        Ok(id as usize)
    }

    async fn get_ticker_id(&self, ticker: &str) -> Option<usize> {
        let row = sqlx::query!("SELECT id FROM ticker WHERE name=$1", ticker)
            .fetch_one(&self.pool).await;
        match row {
            Ok(row) => {
                let id: i32 = row.id;
                Some(id as usize)
            }
            _ => None,
        }
    }

    async fn insert_if_new_ticker(&self, ticker: &Ticker) -> Result<usize, DataError> {
        match self.get_ticker_id(&ticker.name).await {
            Some(id) => Ok(id),
            None => self.insert_ticker(ticker).await,
        }
    }

    async fn get_ticker_by_id(&self, id: usize) -> Result<Ticker, DataError> {
        let row = sqlx::query!(
                "SELECT name, asset_id, source, priority, currency, factor, tz, cal FROM ticker WHERE id=$1",
                (id as i32),
            ).fetch_one(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?;
        let name = row.name;
        let asset = row.asset_id;
        let source = row.source;
        let currency = row.currency;
        let currency =
            Currency::from_str(&currency).map_err(|e| DataError::NotFound(e.to_string()))?;
        Ok(Ticker {
            id: Some(id),
            name,
            asset: asset as usize,
            source,
            priority: row.priority,
            currency,
            factor: row.factor,
            tz: row.tz,
            cal: row.cal,
        })
    }

    async fn get_all_ticker(&self) -> Result<Vec<Ticker>, DataError> {
        let mut all_ticker = Vec::new();
        for row in sqlx::query!(
                "SELECT id, name, asset_id, priority, source, currency, factor, tz, cal FROM ticker",
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let id = row.id;
            let asset = row.asset_id;
            let source = row.source;
            let currency = row.currency;
            let currency =
                Currency::from_str(&currency).map_err(|e| DataError::NotFound(e.to_string()))?;
            let factor = row.factor;
            all_ticker.push(Ticker {
                id: Some(id as usize),
                name: row.name,
                asset: asset as usize,
                source,
                priority: row.priority,
                currency,
                factor,
                tz: row.tz,
                cal: row.cal,
            });
        }
        Ok(all_ticker)
    }

    async fn get_all_ticker_for_source(
        &self,
        source: &str,
    ) -> Result<Vec<Ticker>, DataError> {
        let mut all_ticker = Vec::new();
        for row in sqlx::query!(
                "SELECT id, name, asset_id, priority, currency, factor, tz, cal FROM ticker WHERE source=$1",
                (source.to_string()),
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let id = row.id;
            let asset = row.asset_id;
            let currency = row.currency;
            let currency =
                Currency::from_str(&currency).map_err(|e| DataError::NotFound(e.to_string()))?;
            let factor = row.factor;
            all_ticker.push(Ticker {
                id: Some(id as usize),
                name: row.name,
                asset: asset as usize,
                source: source.to_string(),
                priority: row.priority,
                currency,
                factor,
                tz: row.tz,
                cal: row.cal,
            });
        }
        Ok(all_ticker)
    }

    async fn get_all_ticker_for_asset(
        &self,
        asset_id: usize,
    ) -> Result<Vec<Ticker>, DataError> {
        let mut all_ticker = Vec::new();
        for row in sqlx::query!(
                "SELECT id, name, source, priority, currency, factor, tz, cal FROM ticker WHERE asset_id=$1",
                (asset_id as i32),
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let id = row.id;
            let source = row.source;
            let currency = row.currency;
            let currency =
                Currency::from_str(&currency).map_err(|e| DataError::NotFound(e.to_string()))?;
            let factor: f64 = row.factor;
            all_ticker.push(Ticker {
                id: Some(id as usize),
                name: row.name,
                asset: asset_id,
                source,
                priority: row.priority,
                currency,
                factor,
                tz: row.tz,
                cal: row.cal,
            });
        }
        Ok(all_ticker)
    }


    async fn update_ticker(&self, ticker: &Ticker) -> Result<(), DataError> {
        if ticker.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = ticker.id.unwrap() as i32;
        sqlx::query!(
                "UPDATE ticker SET name=$2, asset_id=$3, source=$4, priority=$5, currency=$6, factor=$7, tz=$8, cal=$9
                WHERE id=$1",
                id,
                ticker.name,
                (ticker.asset as i32),
                ticker.source.to_string(),
                ticker.priority,
                ticker.currency.to_string(),
                ticker.factor,
                ticker.tz,
                ticker.cal
            )
            .execute(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    async fn delete_ticker(&self, id: usize) -> Result<(), DataError> {
        sqlx::query!("DELETE FROM ticker WHERE id=$1;", (id as i32))
            .execute(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    // insert, get, update and delete for market data sources
    async fn insert_quote(&self, quote: &Quote) -> Result<usize, DataError> {
        let row = sqlx::query!(
                "INSERT INTO quotes (ticker_id, price, time, volume) 
                VALUES ($1, $2, $3, $4) RETURNING id",
                (quote.ticker as i32),
                quote.price,
                quote.time,
                quote.volume,
            ).fetch_one(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        let id = row.id;
        Ok(id as usize)
    }

    async fn get_last_quote_before(
        &self,
        asset_name: &str,
        time: DateTime<Local>,
    ) -> Result<(Quote, Currency), DataError> {
        let row = sqlx::query!(
                "SELECT q.id, q.ticker_id, q.price, q.time, q.volume, t.currency, t.priority
                FROM quotes q, ticker t, assets a 
                WHERE a.name=$1 AND t.asset_id=a.id AND t.id=q.ticker_id AND q.time<= $2
                ORDER BY q.time DESC, t.priority ASC LIMIT 1",
                asset_name, time,
            ).fetch_one(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?;

        let id = row.id;
        let ticker = row.ticker_id;
        let price = row.price;
        let time: DateTime<Local> = row.time.into();
        let volume = row.volume;
        let currency = row.currency;
        let currency =
            Currency::from_str(&currency).map_err(|e| DataError::NotFound(e.to_string()))?;
        Ok((
            Quote {
                id: Some(id as usize),
                ticker: ticker as usize,
                price,
                time,
                volume,
            },
            currency,
        ))
    }

    async fn get_last_quote_before_by_id(
        &self,
        asset_id: usize,
        time: DateTime<Local>,
    ) -> Result<(Quote, Currency), DataError> {
        let row = sqlx::query!(
                "SELECT q.id, q.ticker_id, q.price, q.time, q.volume, t.currency, t.priority
                FROM quotes q, ticker t
                WHERE t.asset_id=$1 AND t.id=q.ticker_id AND q.time<= $2
                ORDER BY q.time DESC, t.priority ASC LIMIT 1",
                (asset_id as i32), time,
            ).fetch_one(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?;

        let id = row.id;
        let ticker = row.ticker_id;
        let price = row.price;
        let time: DateTime<Local> = row.time.into();
        let volume = row.volume;
        let currency = row.currency;
        let currency =
            Currency::from_str(&currency).map_err(|e| DataError::NotFound(e.to_string()))?;
        Ok((
            Quote {
                id: Some(id as usize),
                ticker: ticker as usize,
                price,
                time,
                volume,
            },
            currency,
        ))
    }

    async fn get_all_quotes_for_ticker(&self, ticker_id: usize) -> Result<Vec<Quote>, DataError> {
        let mut quotes = Vec::new();
        for row in sqlx::query!(
                "SELECT id, price, time, volume FROM quotes 
                WHERE ticker_id=$1 ORDER BY time ASC;",
                (ticker_id as i32),
            ).fetch_all(&self.pool).await
            .map_err(|e| DataError::NotFound(e.to_string()))?
        {
            let id = row.id;
            let time = row.time.into();
            quotes.push(Quote {
                id: Some(id as usize),
                ticker: ticker_id,
                price: row.price,
                time,
                volume: row.volume,
            });
        }
        Ok(quotes)
    }

    async fn update_quote(&self, quote: &Quote) -> Result<(), DataError> {
        if quote.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = quote.id.unwrap() as i32;
        sqlx::query!(
                "UPDATE quotes SET ticker_id=$2, price=$3, time=$4, volume=$5
                WHERE id=$1",
                id,
                (quote.ticker as i32),
                quote.price,
                quote.time,
                quote.volume,
            )
            .execute(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    async fn delete_quote(&self, id: usize) -> Result<(), DataError> {
        sqlx::query!("DELETE FROM quotes WHERE id=$1;", (id as i32))
            .execute(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    async fn remove_duplicates(&self) -> Result<(), DataError> {
        sqlx::query!("
            delete from quotes q 
            where q.id in
            (select q2.id
            from 
                quotes q1,
                quotes q2
            where 
                q1.id < q2.id
            and q1.ticker_id = q2.ticker_id 
            and q1.time = q2.time
            and q1.price = q2.price) 
            ")
            .execute(&self.pool).await
            .map_err(|e| DataError::DeleteFailed(e.to_string()))?;
        Ok(())
    }

    async fn get_rounding_digits(&self, currency: Currency) -> i32 {
        let rows = sqlx::query!(
            "SELECT digits FROM rounding_digits WHERE currency=$1;",
            currency.to_string(),
        ).fetch_all(&self.pool).await;
        match rows {
            Ok(row_vec) => {
                if row_vec.is_empty() {
                    let digits: i32 = row_vec[0].digits;
                    digits
                } else {
                    2
                }
            }
            Err(_) => 2,
        }
    }

    async fn set_rounding_digits(&self, currency: Currency, digits: i32) -> Result<(), DataError> {
        let _row = sqlx::query!(
                "INSERT INTO rounding_digits (currency, digits) VALUES ($1, $2)",
                currency.to_string(), digits,
            ).execute(&self.pool).await
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }
}
