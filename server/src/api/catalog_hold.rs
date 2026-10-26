//! Attach `holdFor` on staff catalog copies.

use std::collections::HashMap;

use crate::error::AppResult;
use crate::models::biblio::{Biblio, BiblioShort};
use crate::models::catalog_view::{
    project_hold_for, BiblioShortStaff, BiblioStaffView, HoldPatronSource, ItemStaffView,
};
use crate::models::item::{Item, ItemShort};
use crate::models::user::{Rights, UserClaims, UserShort};

/// Staff notice. `holdFor` follows [`project_hold_for`].
pub(crate) async fn staff_biblio_view(
    state: &crate::AppState,
    claims: &UserClaims,
    biblio: Biblio,
) -> AppResult<BiblioStaffView> {
    let sources: Vec<Option<HoldPatronSource>> =
        biblio.items.iter().map(Item::hold_patron_source).collect();
    let full = patrons_by_id(state, claims, sources.iter().flatten()).await?;
    let mut view = BiblioStaffView::from(biblio);
    attach_full(&mut view.items, sources, claims, &full);
    Ok(view)
}

/// Staff copy list for one biblio.
pub(crate) async fn staff_item_views(
    state: &crate::AppState,
    claims: &UserClaims,
    items: Vec<Item>,
) -> AppResult<Vec<ItemStaffView>> {
    let sources: Vec<Option<HoldPatronSource>> =
        items.iter().map(Item::hold_patron_source).collect();
    let full = patrons_by_id(state, claims, sources.iter().flatten()).await?;
    Ok(items
        .into_iter()
        .zip(sources)
        .map(|(item, source)| {
            let mut view = ItemStaffView::from(item);
            if let Some(source) = source {
                view.hold_for = project_hold_for(source, claims, &full);
            }
            view
        })
        .collect())
}

/// Staff search, series, and collection hits.
pub(crate) async fn staff_biblio_shorts(
    state: &crate::AppState,
    claims: &UserClaims,
    biblios: Vec<BiblioShort>,
) -> AppResult<Vec<BiblioShortStaff>> {
    let sources: Vec<Vec<Option<HoldPatronSource>>> = biblios
        .iter()
        .map(|biblio| {
            biblio
                .items
                .iter()
                .map(ItemShort::hold_patron_source)
                .collect()
        })
        .collect();
    let full = patrons_by_id(state, claims, sources.iter().flatten().flatten()).await?;
    Ok(biblios
        .into_iter()
        .zip(sources)
        .map(|(biblio, copy_sources)| {
            let mut view = BiblioShortStaff::from(biblio);
            for (item, source) in view.items.iter_mut().zip(copy_sources) {
                if let Some(source) = source {
                    item.hold_for = project_hold_for(source, claims, &full);
                }
            }
            view
        })
        .collect())
}

fn attach_full(
    items: &mut [ItemStaffView],
    sources: Vec<Option<HoldPatronSource>>,
    claims: &UserClaims,
    full: &HashMap<i64, UserShort>,
) {
    for (item, source) in items.iter_mut().zip(sources) {
        if let Some(source) = source {
            item.hold_for = project_hold_for(source, claims, full);
        }
    }
}

async fn patrons_by_id<'a>(
    state: &crate::AppState,
    claims: &UserClaims,
    sources: impl Iterator<Item = &'a HoldPatronSource>,
) -> AppResult<HashMap<i64, UserShort>> {
    if claims.rights.users_rights.rank() < Rights::Read.rank() {
        return Ok(HashMap::new());
    }
    let mut ids: Vec<i64> = sources.map(|source| source.id).collect();
    ids.sort_unstable();
    ids.dedup();
    state.services.repository.users_get_short_by_ids(&ids).await
}
