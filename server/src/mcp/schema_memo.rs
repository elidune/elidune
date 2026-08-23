//! Domain schema memo shared by the chat system prompt, MCP tools, and `schema_overview`.

/// Short Elidune LMS schema map for LLM agents (tables, key columns, joins).
pub fn overview() -> &'static str {
    r#"Elidune model (PostgreSQL, public schema; safe views under mcp.*):

Tables & key columns:
- loans — active circulation: user_id, item_id, date, expiry_at, returned_at (NULL while open).
- loans_archives — returned loan history: user_id, item_id, date, returned_at. Use list_my_loan_history, not query, for patron history.
- holds — reservations: user_id, item_id, status (pending|ready|cancelled|expired).
- items — physical copies: id, barcode, biblio_id, call_number, borrowable.
- biblios — catalog records: id, title, isbn, media_type, subject, keywords, abstract, notes, publication_date.
  IMPORTANT: summary text is column abstract (NOT summary). In SQL use b.abstract or b."abstract".
- biblio_authors + authors — authors linked to biblios (position, function).
- mcp.users — safe user view (id, login, firstname, lastname, email, barcode; no secrets).

Joins:
- loan → biblio: loans.item_id → items.id → biblios.id (same for loans_archives).
- "My" loans/holds/history = current JWT user_id (RLS enforced)."#
}

/// Compact schema digest for the `query` tool description (chat-relevant tables only).
pub fn query_digest() -> &'static str {
    r#"SQL digest (explicit columns only; never SELECT *; always LIMIT):
- loans(user_id, item_id, date, expiry_at, returned_at) — open loans when returned_at IS NULL.
- loans_archives(user_id, item_id, date, returned_at) — returned loans; prefer list_my_loan_history for patrons.
- holds(user_id, item_id, status).
- items(id, barcode, biblio_id).
- biblios(id, title, isbn, media_type, subject, keywords, abstract, notes) — use abstract not summary.
- Join: … JOIN items i ON …item_id = i.id JOIN biblios b ON i.biblio_id = b.id
Call describe_table before querying unfamiliar columns."#
}

/// Tool-selection policy injected into the chat system prompt.
pub fn tool_policy() -> &'static str {
    r#"Tool strategy (in this order):
1. Patron intents → domain tools:
   - list_my_loans — active loans only
   - list_my_loan_history — past/returned loans (dernier emprunt, historique, recommandation)
   - list_my_holds — reservations
   - search_biblios — catalog text search
   - get_biblio — full bibliographic record by id (abstract, subject, keywords, authors)
2. Staff / advanced → query (explicit SELECT, never SELECT *, reasonable LIMIT; see schema digest).
3. Schema exploration → schema_overview, then describe_table; list_tables last resort.
Forbidden: inventing column names (e.g. summary); starting with list_tables; SELECT *; query for "my loans/history" when domain tools fit."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_documents_abstract_not_summary() {
        let text = overview();
        assert!(text.contains("abstract"));
        assert!(text.contains("NOT summary"));
        assert!(text.contains("loans_archives"));
    }

    #[test]
    fn tool_policy_lists_new_domain_tools() {
        let text = tool_policy();
        assert!(text.contains("list_my_loan_history"));
        assert!(text.contains("get_biblio"));
    }

    #[test]
    fn query_digest_covers_biblios_columns() {
        let text = query_digest();
        assert!(text.contains("biblios"));
        assert!(text.contains("abstract"));
    }
}
