//! Domain schema memo shared by the chat system prompt and MCP `schema_overview`.

/// Short Elidune LMS schema map for LLM agents (French labels, English table names).
pub fn overview() -> &'static str {
    r#"Modèle Elidune (PostgreSQL, schéma public ; vues sûres sous mcp.*) :
- Emprunts en cours → table `loans` (user_id, item_id, date, expiry_at, returned_at ; actif si returned_at IS NULL).
- Historique emprunts → `loans_archives`.
- Réservations / holds → `holds` (user_id, item_id, status pending|ready|cancelled|expired).
- Exemplaire physique → `items` (barcode, biblio_id, call_number, borrowable).
- Notice bibliographique → `biblios` (title, isbn, media_type, …) ; auteurs via `biblio_authors` + `authors`.
- Jointure typique emprunt : loans.item_id → items.id → biblios.id.
- Usager (sans secrets) → vue `mcp.users` (id, login, firstname, lastname, email, barcode).
« Mes » emprunts/réservations = toujours l'utilisateur JWT courant (RLS appliquée)."#
}

/// Tool-selection policy injected into the chat system prompt.
pub fn tool_policy() -> &'static str {
    r#"Stratégie outils (dans cet ordre) :
1. Intents courants → outils métier : `list_my_loans`, `list_my_holds`, `search_biblios`.
2. Staff / cas avancés → `query` (SELECT explicite, jamais SELECT *, LIMIT raisonnable).
3. Exploration schéma → `schema_overview`, puis `describe_table` si besoin ; `list_tables` en dernier recours.
Interdit : commencer par list_tables ; SELECT * ; interroger users pour « mes emprunts »."#
}
