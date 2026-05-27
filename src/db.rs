use crate::event::cloud_event::NormalizedEvent;
use anyhow::{anyhow, Result};
use serde_json::Value;
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn max_aggregate_version(&self, aggregate_id: &str) -> Result<i64> {
        let row = sqlx::query("SELECT COALESCE(MAX(aggregate_version), 0) AS aggregate_version FROM event_store_t WHERE aggregate_id = $1")
            .bind(aggregate_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("aggregate_version")?)
    }

    pub async fn insert_events(&self, events: &mut [NormalizedEvent]) -> Result<InsertResult> {
        let mut tx = self.pool.begin().await?;
        match insert_events_tx(&mut tx, events).await {
            Ok(()) => {
                tx.commit().await?;
                Ok(InsertResult::Inserted(events.len()))
            }
            Err(err) if is_unique_violation(&err) && events.len() == 1 => {
                tx.rollback().await?;
                if self.is_exact_duplicate(&events[0]).await? {
                    Ok(InsertResult::SkippedExactDuplicate)
                } else {
                    Err(err)
                }
            }
            Err(err) => {
                tx.rollback().await?;
                Err(err)
            }
        }
    }

    async fn is_exact_duplicate(&self, event: &NormalizedEvent) -> Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT aggregate_id, aggregate_version, aggregate_type, event_type, payload, metadata
            FROM event_store_t
            WHERE id = $1
            "#,
        )
        .bind(event.id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(false);
        };

        let aggregate_id: String = row.try_get("aggregate_id")?;
        let aggregate_version: i64 = row.try_get("aggregate_version")?;
        let aggregate_type: String = row.try_get("aggregate_type")?;
        let event_type: String = row.try_get("event_type")?;
        let payload: Value = row.try_get("payload")?;
        let metadata: Value = row.try_get("metadata")?;

        Ok(aggregate_id == event.aggregate_id
            && aggregate_version == event.aggregate_version
            && aggregate_type == event.aggregate_type
            && event_type == event.event_type
            && payload == event.payload()
            && metadata == event.metadata())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertResult {
    Inserted(usize),
    SkippedExactDuplicate,
}

async fn insert_events_tx(
    tx: &mut Transaction<'_, Postgres>,
    events: &mut [NormalizedEvent],
) -> Result<()> {
    let mut current_offset = reserve_offsets(tx, events.len()).await?;
    let transaction_id = Uuid::new_v4();

    for event in events {
        let nonce = reserve_nonce(tx, event.user_id).await?;
        event.set_nonce(nonce);
        let payload = event.payload();
        let metadata = event.metadata();

        sqlx::query(
            r#"
            INSERT INTO event_store_t
              (id, host_id, user_id, nonce, aggregate_id, aggregate_version,
               aggregate_type, event_type, event_ts, payload, metadata)
            VALUES
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb, $11::jsonb)
            "#,
        )
        .bind(event.id)
        .bind(event.host_id)
        .bind(event.user_id)
        .bind(nonce)
        .bind(&event.aggregate_id)
        .bind(event.aggregate_version)
        .bind(&event.aggregate_type)
        .bind(&event.event_type)
        .bind(event.event_ts)
        .bind(&payload)
        .bind(&metadata)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO outbox_message_t
              (id, host_id, user_id, nonce, aggregate_id, aggregate_version,
               aggregate_type, event_type, event_ts, payload, metadata, c_offset,
               transaction_id)
            VALUES
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb, $11::jsonb, $12, $13)
            "#,
        )
        .bind(event.id)
        .bind(event.host_id)
        .bind(event.user_id)
        .bind(nonce)
        .bind(&event.aggregate_id)
        .bind(event.aggregate_version)
        .bind(&event.aggregate_type)
        .bind(&event.event_type)
        .bind(event.event_ts)
        .bind(&payload)
        .bind(&metadata)
        .bind(current_offset)
        .bind(transaction_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO notification_t
              (id, host_id, user_id, nonce, event_class, event_json, event_ts, process_ts,
               status, error, aggregate_id, aggregate_type, aggregate_version,
               event_partition, event_offset, transaction_id)
            VALUES
              ($1, $2, $3, $4, $5, $6, $7, now(), 'PENDING', NULL,
               $8, $9, $10, NULL, NULL, $11)
            ON CONFLICT (host_id, id) DO NOTHING
            "#,
        )
        .bind(event.id)
        .bind(event.host_id)
        .bind(event.user_id)
        .bind(nonce)
        .bind(&event.event_type)
        .bind(serde_json::to_string(&payload)?)
        .bind(event.event_ts)
        .bind(&event.aggregate_id)
        .bind(&event.aggregate_type)
        .bind(event.aggregate_version)
        .bind(transaction_id)
        .execute(&mut **tx)
        .await?;

        current_offset += 1;
    }

    Ok(())
}

async fn reserve_nonce(tx: &mut Transaction<'_, Postgres>, user_id: Uuid) -> Result<i64> {
    let row = sqlx::query(
        r#"
        UPDATE user_t
        SET nonce = nonce + 1
        WHERE user_id = $1
        RETURNING nonce
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(|row| row.try_get::<i64, _>("nonce"))
        .transpose()?
        .ok_or_else(|| anyhow!("unable to reserve nonce for user_id {user_id}"))
}

async fn reserve_offsets(tx: &mut Transaction<'_, Postgres>, batch_size: usize) -> Result<i64> {
    let batch_size = i64::try_from(batch_size)?;
    let row = sqlx::query(
        r#"
        UPDATE log_counter
        SET next_offset = next_offset + $1
        WHERE id = 1
        RETURNING next_offset - $1 AS c_offset
        "#,
    )
    .bind(batch_size)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(|row| row.try_get::<i64, _>("c_offset"))
        .transpose()?
        .ok_or_else(|| anyhow!("failed to reserve offsets from log_counter"))
}

pub fn is_unique_violation(err: &anyhow::Error) -> bool {
    err.chain().any(|source| {
        source
            .downcast_ref::<sqlx::Error>()
            .and_then(|sqlx_err| match sqlx_err {
                sqlx::Error::Database(db_err) => db_err.try_downcast_ref::<PgDatabaseError>(),
                _ => None,
            })
            .map(|db_err| db_err.code() == "23505")
            .unwrap_or(false)
    })
}
