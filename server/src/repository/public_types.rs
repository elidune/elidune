//! Public types domain methods on Repository

use async_trait::async_trait;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::public_type::{
        CreatePublicType, PublicType, PublicTypeLoanSettingInput, PublicTypeLoanSettings,
        UpdatePublicType,
    },
};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PublicTypesRepository: Send + Sync {
    async fn public_types_list(&self) -> AppResult<Vec<PublicType>>;
    async fn public_types_get_by_id(&self, id: i64) -> AppResult<PublicType>;
    async fn public_types_get_loan_settings(
        &self,
        public_type_id: i64,
    ) -> AppResult<Vec<PublicTypeLoanSettings>>;
    async fn public_types_create(&self, data: &CreatePublicType) -> AppResult<PublicType>;
    async fn public_types_update(
        &self,
        id: i64,
        data: &UpdatePublicType,
    ) -> AppResult<PublicType>;
    async fn public_types_delete(&self, id: i64) -> AppResult<()>;
    async fn public_types_replace_loan_settings(
        &self,
        public_type_id: i64,
        settings: &[PublicTypeLoanSettingInput],
    ) -> AppResult<Vec<PublicTypeLoanSettings>>;
    /// Resolve a `public_types.name` to its id, if it exists.
    async fn public_types_find_id_by_name(&self, name: &str) -> AppResult<Option<i64>>;
}

#[async_trait::async_trait]
impl PublicTypesRepository for super::Repository {
    async fn public_types_list(&self) -> crate::error::AppResult<Vec<crate::models::public_type::PublicType>> {
        super::Repository::public_types_list(self).await
    }
    async fn public_types_get_by_id(&self, id: i64) -> crate::error::AppResult<crate::models::public_type::PublicType> {
        super::Repository::public_types_get_by_id(self, id).await
    }
    async fn public_types_get_loan_settings(&self, public_type_id: i64) -> crate::error::AppResult<Vec<crate::models::public_type::PublicTypeLoanSettings>> {
        super::Repository::public_types_get_loan_settings(self, public_type_id).await
    }
    async fn public_types_create(&self, data: &crate::models::public_type::CreatePublicType) -> crate::error::AppResult<crate::models::public_type::PublicType> {
        super::Repository::public_types_create(self, data).await
    }
    async fn public_types_update(&self, id: i64, data: &crate::models::public_type::UpdatePublicType) -> crate::error::AppResult<crate::models::public_type::PublicType> {
        super::Repository::public_types_update(self, id, data).await
    }
    async fn public_types_delete(&self, id: i64) -> crate::error::AppResult<()> {
        super::Repository::public_types_delete(self, id).await
    }
    async fn public_types_replace_loan_settings(
        &self,
        public_type_id: i64,
        settings: &[PublicTypeLoanSettingInput],
    ) -> crate::error::AppResult<Vec<crate::models::public_type::PublicTypeLoanSettings>> {
        super::Repository::public_types_replace_loan_settings(self, public_type_id, settings).await
    }
    async fn public_types_find_id_by_name(&self, name: &str) -> crate::error::AppResult<Option<i64>> {
        super::Repository::public_types_find_id_by_name(self, name).await
    }
}


impl Repository {
    /// List all public types with their loan settings overrides
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_list(&self) -> AppResult<Vec<PublicType>> {
        Ok(sqlx::query_as::<_, PublicType>(
            "SELECT * FROM public_types ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Get public type by ID
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_get_by_id(&self, id: i64) -> AppResult<PublicType> {
        sqlx::query_as::<_, PublicType>("SELECT * FROM public_types WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Public type {} not found", id)))
    }

    /// Get loan settings overrides for a public type
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_get_loan_settings(&self, public_type_id: i64) -> AppResult<Vec<PublicTypeLoanSettings>> {
        Ok(sqlx::query_as::<_, PublicTypeLoanSettings>(
            r#"SELECT * FROM public_type_loan_settings WHERE public_type_id = $1 ORDER BY (media_type IS NOT NULL), media_type"#,
        )
        .bind(public_type_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Create a new public type
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_create(&self, data: &CreatePublicType) -> AppResult<PublicType> {
        let public_type = sqlx::query_as::<_, PublicType>(
            r#"
            INSERT INTO public_types (
                name, label, subscription_duration_days, age_min, age_max,
                subscription_price
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(&data.name)
        .bind(&data.label)
        .bind(data.subscription_duration_days)
        .bind(data.age_min)
        .bind(data.age_max)
        .bind(data.subscription_price)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO public_type_loan_settings (public_type_id, media_type, duration, nb_max, nb_renews, renew_at)
            SELECT $1, NULL, 21, 5, 2, 'now'
            WHERE NOT EXISTS (
                SELECT 1 FROM public_type_loan_settings x
                WHERE x.public_type_id = $1 AND x.media_type IS NULL
            )
            "#,
        )
        .bind(public_type.id)
        .execute(&self.pool)
        .await?;

        Ok(public_type)
    }

    /// Update a public type
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_update(&self, id: i64, data: &UpdatePublicType) -> AppResult<PublicType> {
        let existing = self.public_types_get_by_id(id).await?;

        let name = data.name.as_deref().unwrap_or(&existing.name);
        let label = data.label.as_deref().unwrap_or(&existing.label);
        let subscription_duration_days = data.subscription_duration_days.or(existing.subscription_duration_days);
        let age_min = data.age_min.or(existing.age_min);
        let age_max = data.age_max.or(existing.age_max);
        let subscription_price = data.subscription_price.or(existing.subscription_price);

        sqlx::query_as::<_, PublicType>(
            r#"
            UPDATE public_types SET
                name = $1, label = $2, subscription_duration_days = $3,
                age_min = $4, age_max = $5, subscription_price = $6
            WHERE id = $7
            RETURNING *
            "#,
        )
        .bind(name)
        .bind(label)
        .bind(subscription_duration_days)
        .bind(age_min)
        .bind(age_max)
        .bind(subscription_price)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Public type {} not found", id)))
    }

    /// Delete a public type (fails if users reference it)
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_delete(&self, id: i64) -> AppResult<()> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE public_type = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if count > 0 {
            return Err(AppError::BusinessRule(format!(
                "Cannot delete public type: {} user(s) still reference it",
                count
            )));
        }

        let result = sqlx::query("DELETE FROM public_types WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Public type {} not found", id)));
        }

        Ok(())
    }

    /// Replace all `public_type_loan_settings` rows for a public type with the given snapshot.
    #[tracing::instrument(skip(self, settings), err)]
    pub async fn public_types_replace_loan_settings(
        &self,
        public_type_id: i64,
        settings: &[PublicTypeLoanSettingInput],
    ) -> AppResult<Vec<PublicTypeLoanSettings>> {
        self.public_types_get_by_id(public_type_id).await?;

        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM public_type_loan_settings WHERE public_type_id = $1")
            .bind(public_type_id)
            .execute(&mut *tx)
            .await?;

        for row in settings {
            let mt: Option<&str> = match row.media_type.as_deref() {
                None => None,
                Some(s) => {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                }
            };

            sqlx::query(
                r#"
                INSERT INTO public_type_loan_settings (public_type_id, media_type, duration, nb_max, nb_renews, renew_at)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(public_type_id)
            .bind(mt)
            .bind(row.duration)
            .bind(row.nb_max)
            .bind(row.nb_renews)
            .bind(row.renew_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.public_types_get_loan_settings(public_type_id).await
    }

    /// Smallest `public_types.id` (seed default for first admin).
    pub async fn public_types_first_id(&self) -> AppResult<Option<i64>> {
        sqlx::query_scalar::<_, i64>("SELECT id FROM public_types ORDER BY id LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    /// Lookup `public_types.id` by stable `name` (e.g. `child`, `adult`).
    #[tracing::instrument(skip(self), err)]
    pub async fn public_types_find_id_by_name(&self, name: &str) -> AppResult<Option<i64>> {
        Ok(sqlx::query_scalar::<_, i64>("SELECT id FROM public_types WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?)
    }
}

