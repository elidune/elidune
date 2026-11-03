//! Persistence for one-off periodical gifts (no subscription).

use super::serials::next_id;
use super::Repository;
use crate::{
    error::AppResult,
    models::serial::{SerialGift, SerialGiftQuery},
};

const GIFT_SELECT: &str = "\
g.id, g.item_id, g.biblio_id, b.title AS biblio_title, b.issn, \
g.site_id, s.name AS site_name, g.volume_label, g.cover_date, \
it.barcode, it.call_number, g.created_at";

const GIFT_FROM: &str = "\
FROM serial_gifts g \
JOIN biblios b ON b.id = g.biblio_id \
JOIN sites s ON s.id = g.site_id \
JOIN items it ON it.id = g.item_id AND it.archived_at IS NULL";

#[derive(sqlx::FromRow)]
struct GiftRow {
    id: i64,
    item_id: i64,
    biblio_id: i64,
    biblio_title: Option<String>,
    issn: Option<String>,
    site_id: i64,
    site_name: Option<String>,
    volume_label: String,
    cover_date: Option<chrono::NaiveDate>,
    barcode: Option<String>,
    call_number: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

fn map_gift(row: GiftRow) -> SerialGift {
    SerialGift {
        id: row.id,
        item_id: row.item_id,
        biblio_id: row.biblio_id,
        biblio_title: row.biblio_title,
        issn: row.issn,
        site_id: row.site_id,
        site_name: row.site_name,
        volume_label: row.volume_label,
        cover_date: row.cover_date,
        barcode: row.barcode,
        call_number: row.call_number,
        created_at: row.created_at,
    }
}

impl Repository {
    pub async fn gifts_insert(
        &self,
        item_id: i64,
        biblio_id: i64,
        site_id: i64,
        volume_label: &str,
        cover_date: Option<chrono::NaiveDate>,
    ) -> AppResult<i64> {
        let id = next_id();
        sqlx::query(
            "INSERT INTO serial_gifts (id, item_id, biblio_id, site_id, volume_label, cover_date) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(item_id)
        .bind(biblio_id)
        .bind(site_id)
        .bind(volume_label)
        .bind(cover_date)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn gifts_get(&self, id: i64) -> AppResult<SerialGift> {
        let sql = format!("SELECT {GIFT_SELECT} {GIFT_FROM} WHERE g.id = $1");
        let row: GiftRow = sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| crate::error::AppError::NotFound(format!("Serial gift {id} not found")))?;
        Ok(map_gift(row))
    }

    pub async fn gifts_list(&self, query: &SerialGiftQuery) -> AppResult<(Vec<SerialGift>, i64)> {
        let q = query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{s}%"));
        let site_id = query.site_id;

        let total: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*)::bigint {GIFT_FROM} \
             WHERE ($1::text IS NULL OR b.title ILIKE $1 OR COALESCE(b.issn, '') ILIKE $1 \
                OR COALESCE(it.barcode, '') ILIKE $1 OR g.volume_label ILIKE $1) \
               AND ($2::bigint IS NULL OR g.site_id = $2)"
        ))
        .bind(&q)
        .bind(site_id)
        .fetch_one(&self.pool)
        .await?;

        let sql = format!(
            "SELECT {GIFT_SELECT} {GIFT_FROM} \
             WHERE ($1::text IS NULL OR b.title ILIKE $1 OR COALESCE(b.issn, '') ILIKE $1 \
                OR COALESCE(it.barcode, '') ILIKE $1 OR g.volume_label ILIKE $1) \
               AND ($2::bigint IS NULL OR g.site_id = $2) \
             ORDER BY g.created_at DESC, g.id DESC \
             LIMIT $3 OFFSET $4"
        );
        let rows: Vec<GiftRow> = sqlx::query_as(&sql)
            .bind(&q)
            .bind(site_id)
            .bind(query.per_page())
            .bind(query.offset())
            .fetch_all(&self.pool)
            .await?;
        Ok((rows.into_iter().map(map_gift).collect(), total))
    }
}
