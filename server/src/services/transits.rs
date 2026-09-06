//! Inter-site item transit for hold fulfillment.

use std::sync::Arc;

use crate::{
    error::AppResult,
    hold_email,
    models::{
        hold::Hold,
        transit::{CreateTransit, ItemTransit, TransitStatus, TransitWithHold},
    },
    repository::Repository,
    services::{
        audit::{self, AuditLogMeta, AuditService},
        email::EmailService,
        event_bus::EventBus,
    },
};

#[derive(Clone)]
pub struct TransitsService {
    repository: Arc<Repository>,
    audit: AuditService,
    events: EventBus,
    email: EmailService,
}

impl TransitsService {
    pub fn new(
        repository: Arc<Repository>,
        audit: AuditService,
        events: EventBus,
        email: EmailService,
    ) -> Self {
        Self {
            repository,
            audit,
            events,
            email,
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_by_id(&self, id: i64) -> AppResult<TransitWithHold> {
        let transit = self.repository.transits_get_by_id(id).await?;
        let hold = self.load_hold(transit.hold_id).await?;
        Ok(TransitWithHold { transit, hold })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn active_for_hold(&self, hold_id: i64) -> AppResult<Option<ItemTransit>> {
        self.repository.holds_get_by_id(hold_id).await?;
        self.repository.transits_get_active_for_hold(hold_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn active_for_item(&self, item_id: i64) -> AppResult<Option<ItemTransit>> {
        self.repository.transits_get_active_for_item(item_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn list(
        &self,
        status: Option<TransitStatus>,
        item_id: Option<i64>,
        hold_id: Option<i64>,
        to_source_id: Option<i64>,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<ItemTransit>, i64)> {
        self.repository
            .transits_list(status, item_id, hold_id, to_source_id, page, per_page)
            .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn request(
        &self,
        hold_id: i64,
        data: CreateTransit,
        actor_id: i64,
        client_ip: Option<String>,
    ) -> AppResult<TransitWithHold> {
        let (transit, hold) = self
            .repository
            .transits_request(hold_id, &data, actor_id)
            .await?;
        let event = if transit.status == TransitStatus::InTransit {
            audit::event::TRANSIT_SHIPPED
        } else {
            audit::event::TRANSIT_REQUESTED
        };
        self.audit.log(
            event,
            Some(actor_id),
            Some("item_transit"),
            Some(transit.id),
            client_ip,
            Some(serde_json::json!({
                "holdId": hold.id.to_string(),
                "itemId": transit.item_id.to_string(),
                "fromSourceId": transit.from_source_id.to_string(),
                "toSourceId": transit.to_source_id.to_string(),
                "status": transit.status.as_str(),
            })),
            AuditLogMeta::success(),
        );
        let sse_event = if transit.status == TransitStatus::InTransit {
            "transit.shipped"
        } else {
            "transit.requested"
        };
        self.events
            .transit_event(sse_event, transit.id, transit.item_id, transit.hold_id);
        Ok(TransitWithHold {
            transit,
            hold: Some(hold),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn ship(
        &self,
        transit_id: i64,
        actor_id: i64,
        notes: Option<String>,
        client_ip: Option<String>,
    ) -> AppResult<ItemTransit> {
        let transit = self
            .repository
            .transits_ship(transit_id, actor_id, notes.as_deref())
            .await?;
        self.audit.log(
            audit::event::TRANSIT_SHIPPED,
            Some(actor_id),
            Some("item_transit"),
            Some(transit.id),
            client_ip,
            Some(serde_json::json!({
                "holdId": transit.hold_id.map(|id| id.to_string()),
                "itemId": transit.item_id.to_string(),
                "fromSourceId": transit.from_source_id.to_string(),
                "toSourceId": transit.to_source_id.to_string(),
            })),
            AuditLogMeta::success(),
        );
        self.events.transit_event(
            "transit.shipped",
            transit.id,
            transit.item_id,
            transit.hold_id,
        );
        Ok(transit)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn receive(
        &self,
        transit_id: i64,
        actor_id: i64,
        notes: Option<String>,
        client_ip: Option<String>,
    ) -> AppResult<TransitWithHold> {
        let (transit, hold) = self
            .repository
            .transits_receive(transit_id, actor_id, notes.as_deref())
            .await?;
        self.audit.log(
            audit::event::TRANSIT_RECEIVED,
            Some(actor_id),
            Some("item_transit"),
            Some(transit.id),
            client_ip.clone(),
            Some(serde_json::json!({
                "holdId": transit.hold_id.map(|id| id.to_string()),
                "itemId": transit.item_id.to_string(),
                "fromSourceId": transit.from_source_id.to_string(),
                "toSourceId": transit.to_source_id.to_string(),
            })),
            AuditLogMeta::success(),
        );
        self.events.transit_event(
            "transit.received",
            transit.id,
            transit.item_id,
            transit.hold_id,
        );

        if let Some(ref hold) = hold {
            if hold.status == crate::models::hold::HoldStatus::Ready {
                self.audit.log(
                    audit::event::HOLD_READY,
                    Some(actor_id),
                    Some("hold"),
                    Some(hold.id),
                    client_ip,
                    Some(serde_json::json!({
                        "user_id": hold.user_id,
                        "item_id": hold.item_id,
                        "expires_at": hold.expires_at,
                        "trigger": "transit_receive",
                    })),
                    AuditLogMeta::success(),
                );
                if let Some(item_id) = hold.item_id {
                    self.events.hold_ready(hold.id, hold.user_id, item_id);
                }
                self.send_ready_email(hold, transit.item_id).await;
            }
        }

        Ok(TransitWithHold { transit, hold })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cancel(
        &self,
        transit_id: i64,
        actor_id: i64,
        reverse: Option<bool>,
        notes: Option<String>,
        client_ip: Option<String>,
    ) -> AppResult<TransitWithHold> {
        let current = self.repository.transits_get_by_id(transit_id).await?;
        let reverse = reverse.unwrap_or(current.status == TransitStatus::InTransit);
        let (transit, reverse_row) = self
            .repository
            .transits_cancel(transit_id, actor_id, reverse, notes.as_deref())
            .await?;
        self.audit.log(
            audit::event::TRANSIT_CANCELLED,
            Some(actor_id),
            Some("item_transit"),
            Some(transit.id),
            client_ip,
            Some(serde_json::json!({
                "holdId": transit.hold_id.map(|id| id.to_string()),
                "itemId": transit.item_id.to_string(),
                "reversed": reverse_row.as_ref().map(|r| r.id.to_string()),
            })),
            AuditLogMeta::success(),
        );
        self.events.transit_event(
            "transit.cancelled",
            transit.id,
            transit.item_id,
            transit.hold_id,
        );
        if let Some(ref reverse_row) = reverse_row {
            self.events.transit_event(
                "transit.requested",
                reverse_row.id,
                reverse_row.item_id,
                reverse_row.hold_id,
            );
        }
        let hold = self.load_hold(transit.hold_id).await?;
        Ok(TransitWithHold { transit, hold })
    }

    async fn load_hold(&self, hold_id: Option<i64>) -> AppResult<Option<Hold>> {
        let Some(hold_id) = hold_id else {
            return Ok(None);
        };
        match self.repository.holds_get_by_id(hold_id).await {
            Ok(hold) => Ok(Some(hold)),
            Err(crate::error::AppError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn send_ready_email(&self, hold: &Hold, item_id: i64) {
        let contact = self
            .repository
            .users_hold_ready_contact(hold.user_id)
            .await
            .ok()
            .flatten();
        let ctx = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            r#"
            SELECT b.title, it.barcode
            FROM items it
            JOIN biblios b ON b.id = it.biblio_id
            WHERE it.id = $1
            "#,
        )
        .bind(item_id)
        .fetch_optional(self.repository.pool())
        .await
        .ok()
        .flatten();
        let title = ctx
            .as_ref()
            .and_then(|(t, _)| t.as_deref())
            .unwrap_or("(unknown title)");
        let barcode = ctx.as_ref().and_then(|(_, b)| b.as_deref());
        if let Err(e) =
            hold_email::send_hold_ready_simple(&self.email, contact, hold, title, barcode).await
        {
            tracing::warn!(
                target: "transits",
                error = %e,
                hold_id = hold.id,
                "Failed to queue hold ready email after transit receive"
            );
        }
    }
}
