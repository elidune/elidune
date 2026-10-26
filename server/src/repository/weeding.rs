//! Weeding candidates (`items.weeding_status = candidate`).

use sqlx::{postgres::PgArguments, Arguments};

use crate::{
    error::AppResult,
    models::dto::weeding::{WeedingCandidate, WeedingCandidateList, WeedingCandidateQuery},
};

use super::Repository;

enum Param {
    Text(String),
    I64(i64),
}

struct PreparedFilter {
    where_sql: String,
    params: Vec<Param>,
}

fn like_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn prepare_filter(query: &WeedingCandidateQuery) -> PreparedFilter {
    let mut parts = vec![
        "i.archived_at IS NULL".to_string(),
        "i.weeding_status = 'candidate'".to_string(),
        "b.archived_at IS NULL".to_string(),
        crate::models::deposit::own_stock_sql("i"),
    ];
    let mut params = Vec::new();

    if let Some(site_id) = query.site_id {
        params.push(Param::I64(site_id));
        parts.push(format!("i.site_id = ${}", params.len()));
    }
    if let Some(collection_id) = query.collection_id {
        params.push(Param::I64(collection_id));
        parts.push(format!(
            "EXISTS (SELECT 1 FROM biblio_collections bc WHERE bc.biblio_id = i.biblio_id AND bc.collection_id = ${})",
            params.len()
        ));
    }
    if let Some(call_number) = query
        .call_number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        params.push(Param::Text(format!("%{}%", like_escape(call_number))));
        parts.push(format!(
            "unaccent(lower(i.call_number)) LIKE unaccent(lower(${}))",
            params.len()
        ));
    }

    PreparedFilter {
        where_sql: parts.join(" AND "),
        params,
    }
}

fn bind_params(params: &[Param]) -> PgArguments {
    let mut args = PgArguments::default();
    for param in params {
        match param {
            Param::Text(value) => {
                args.add(value);
            }
            Param::I64(value) => {
                args.add(*value);
            }
        }
    }
    args
}

const SELECT_CANDIDATE: &str = r#"
    SELECT
        i.id AS item_id,
        i.biblio_id,
        i.barcode,
        i.call_number,
        b.title AS biblio_title,
        i.site_id,
        so.name AS site_name,
        i.weeding_reason,
        i.updated_at AS marked_at
    FROM items i
    JOIN biblios b ON b.id = i.biblio_id
    LEFT JOIN sites so ON so.id = i.site_id
"#;

impl Repository {
    pub async fn weeding_candidates_list(
        &self,
        query: &WeedingCandidateQuery,
    ) -> AppResult<WeedingCandidateList> {
        let prepared = prepare_filter(query);
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * per_page;

        let count_sql = format!(
            r#"
            SELECT COUNT(*)::bigint
            FROM items i
            JOIN biblios b ON b.id = i.biblio_id
            WHERE {}
            "#,
            prepared.where_sql
        );
        let total: i64 = sqlx::query_scalar_with(&count_sql, bind_params(&prepared.params))
            .fetch_one(&self.pool)
            .await?;

        let mut page_params = Vec::new();
        page_params.extend(prepared.params.iter().map(|param| match param {
            Param::Text(value) => Param::Text(value.clone()),
            Param::I64(value) => Param::I64(*value),
        }));
        page_params.push(Param::I64(per_page));
        let limit_idx = page_params.len();
        page_params.push(Param::I64(offset));
        let offset_idx = page_params.len();

        let page_sql = format!(
            "{SELECT_CANDIDATE} WHERE {} ORDER BY i.updated_at DESC, i.id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
            prepared.where_sql
        );
        let items =
            sqlx::query_as_with::<_, WeedingCandidate, _>(&page_sql, bind_params(&page_params))
                .fetch_all(&self.pool)
                .await?;

        Ok(WeedingCandidateList {
            items,
            total,
            page,
            per_page,
        })
    }
}
