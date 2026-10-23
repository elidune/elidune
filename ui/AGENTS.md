# AGENTS.md — elidune-ui

Library management SPA (Elidune) built with React 19 + TypeScript + Vite.

## Stack

| Layer | Technology |
|-------|------------|
| Framework | React 19, TypeScript ~5.9 |
| Build | Vite 7, `tsc -b && vite build` |
| Styling | Tailwind CSS v4 (via `@tailwindcss/vite`) |
| Routing | React Router v7 |
| Server state | TanStack React Query v5 (`staleTime: 5 min, retry: 1`) |
| HTTP | Axios — single `ApiService` class in `src/services/api.ts` |
| i18n | i18next + react-i18next — `en` / `fr` / `de` / `es` in `src/locales/{lang}/translation.json` |
| Icons | lucide-react |
| Charts | Recharts |
| Lint | ESLint 9 (`rtk lint`) |

## Project structure

```
src/
  App.tsx               # Root — providers + routes
  services/api.ts       # Single ApiService class (all API calls)
  types/index.ts        # All shared types + permission helpers
  contexts/             # AuthContext, ThemeContext, LanguageContext, LibraryContext
  pages/                # One file per page (large files are normal here)
  components/           # Organized by domain: auth/ common/ items/ loans/ stats/ users/ specimen/
  hooks/                # Organized by domain (same structure as components)
  utils/                # apiError.ts · callNumber.ts · codeLabels.ts
  locales/{en,fr,de,es}/  # translation.json — keep keys in sync across locales
```

## Key conventions

### API calls
- All calls go through the `ApiService` singleton in `src/services/api.ts`. Never use `axios` or `fetch` directly in components or hooks.
- The base URL is `/api/v1` (proxied by Vite in dev).

### Types
- All shared types live in `src/types/index.ts`. Add new types there.
- Permission helpers (`isAdmin`, `isLibrarian`, `canManage*`) are defined in `types/index.ts` — use them, don't inline role checks.

### Route guards
Three wrappers in `App.tsx`:
- `ProtectedRoute` — requires authentication
- `LibrarianRoute` — requires `isLibrarian(account_type)`
- `AdminRoute` — requires `isAdmin(account_type)`

Role hierarchy: `Guest < Reader < Librarian < Administrator`

### i18n
- All user-visible strings must go through `useTranslation()` / `t('key')`.
- Translation keys live in `src/locales/{en,fr,de,es}/translation.json`. Keep the four files in sync (same keys).
- When adding a key, add it to **en, fr, de, and es** in the same turn.
- Format dates and date-times with `i18n.language` (`toLocaleDateString(i18n.language)`, `toLocaleString(i18n.language)`). Do not hardcode `'fr-FR'` or rely on the browser default.

### Contexts
| Context | Purpose |
|---------|---------|
| `AuthContext` | JWT token, current user, login/logout |
| `ThemeContext` | dark / light mode |
| `LanguageContext` | active locale |
| `LibraryContext` | library info (name, address…) |

### Components
- Functional components only; no class components.
- Co-locate domain logic in the matching `hooks/<domain>/` folder.
- Common UI building blocks go in `components/common/`.

## UI layout & interaction norms

These rules apply to **new pages and refactors**. Prefer aligning existing screens when you touch them.

### Page chrome (`PageHeading`)

- Every staff/patron **top-level page** uses `PageHeading` in the page body (not in the sticky mobile app bar).
- **Subtitle is required** — one line explaining what the page is for.
- **Icon**: pass a lucide icon; default tile is `rounded-xl`, `w-14`, stretches to the title + subtitle block height, glyph `h-8 w-8`, `bg-amber-100 dark:bg-amber-900/40` and amber colors (overridable via `iconTileClassName` / `iconClassName`).
- **Actions** (create, help, export…): `actions` prop; layout is column on narrow viewports, row on `sm+` to the right of the title block.
- **Mobile app header** (`Layout`): menu + **logo only** (link to `/home`). Do **not** duplicate the page title in the sticky header; the title lives in `PageHeading` below.

### Tabs & section navigation (`SectionTabBar`)

- **Default pattern for switching views or filters on a page** (e.g. Circulation borrow/return, catalogue tabs, inventory Tous/Ouverte/Clôturée, deposit sub-pages). Do **not** use pill/chip button groups for primary page modes.
- Use `SectionTabBar` from `components/common/` — horizontal scroll, amber bottom border on active tab, optional lucide icon per tab, `role="tablist"`.
- **Page lead block**: wrap `PageHeading` + primary `SectionTabBar` in `PageTabLeadGroup` so title → tabs spacing matches site-wide (tighter on mobile).
- Wire **accessibility**: when the tab switches main content, wrap the panel in `role="tabpanel"` with `id={`section-panel-${id}`}` and `aria-labelledby={`section-tab-${id}`}` (ids match `SectionTabBar` buttons).
- **`size="compact"`** for secondary tab rows **inside** a card (e.g. scan vs batch, scans vs missing list), not for the main page mode tabs.
- Provide `ariaLabel` on each bar (dedicated i18n key, all four locales).

### Lists & scrolling

- Long lists: `ScrollableListRegion` + infinite scroll or pagination as already done on peer pages.
- **Viewport-filling list pages** (`UsersPage`, `BibliosPage`, …): wrap in `ListPageLayout`; list `Card` uses `flex min-h-0 flex-1 flex-col`; `ScrollableListRegion` with **`fill`**. Chrome (heading, filters) uses `shrink-0`. On lg+ only the list band scrolls — no double scroll with the shell.
- **List totals:** show the result count in `ListResultCountBar` directly above the scrollable table/list (right-aligned). Do not duplicate the same count in `PageHeading` / `CardHeader` subtitles.
- List scroll root: `getListScrollRoot()` / `APP_LIST_SCROLL_ROOT_CLASS` where the page participates in the app shell scroll model.

### Single-field actions (search, barcode, identifier)

- **No adjacent “Validate / Search / Submit” button** next to a lone text field at any breakpoint. Users confirm with **Enter** (or native `<form onSubmit>`).
- Set **`enterKeyHint`**: `search` for search fields, `go` for scan/action fields (default on `BarcodeScanField`).
- **`SearchInput`**: with `submitMode`, call `onSubmit` on Enter only; no separate submit control.
- **`BarcodeScanField`**: camera control lives **inside** the field via `Input` `rightIcon`; do not add a trailing validate column.
- **Do not reintroduce** `ValidatedFieldRow` / `ResponsiveSubmitButton` (removed). Multi-field forms keep their **primary submit at the bottom** of the form; non-submit actions (e.g. “Withdraw selection”, print) may stay as separate buttons where they are not redundant with Enter.
- Optional short hint text under the field is fine for a11y; prefer that over a duplicate submit control.

### Search vs live filter

- **Explicit search** (catalog OPAC hero, reader catalog, list screens that hit the server on submit): `SearchInput` `submitMode` + Enter, or `<form onSubmit>` around a search `Input`.
- **Live filter** (debounced table/list filtering): `SearchInput` without `submitMode` (debounced `onChange`).

### Help & density

- Long instructional copy on list pages: **Help** button in `PageHeading` actions → **Modal**, not a permanent essay above the list.

### Navigation copy (sidebar / sections)

- Keep labels consistent with product vocabulary in i18n (e.g. loans desk page title **Circulation**, section **Service aux lecteurs** / `nav.sectionMySpace` for patron “Mon espace”).
- **Events**: single route `/events` (public share link + in-app calendar). Staff with `eventsWrite` manage create/edit/delete in-page on `PublicEventsPage`; no separate `/events/manage`.
- Patron-only chrome: avoid redundant “Lecteur” section headers when the whole view is reader-scoped.

### Reference implementations

| Pattern | Example |
|---------|---------|
| Heading + primary tabs | `LoansPage`, `BibliosPage`, `DepositsPage`, `SettingsPage`, `InventoryPage` (session list) |
| In-card compact tabs | `InventoryPage` (scan/batch, scans/missing) |
| Search on Enter | `LoginPage` OPAC search, `ReaderCatalogPage`, `SearchInput` `submitMode` |
| Scan on Enter | `BarcodeScanField` inside `<form onSubmit>` on circulation, holds, deposits |

## Coding Conventions
- factorize as much as possible, reuse existing code and create new factorisations on changes
- avoid multi call to same rest apis, reuse data


### Data fetching
- Use TanStack Query (`useQuery` / `useMutation`) for all server state.
- Pass the `ApiService` method directly as `queryFn`; don't duplicate fetch logic.

## Commands

```bash
pnpm dev          # Start dev server (Vite)
pnpm build        # tsc -b && vite build
rtk lint          # ESLint (token-optimised)
rtk tsc           # Type-check only (token-optimised)
```

## Important domain notes

- An **Item** is a bibliographic record; a **Specimen** is a physical copy of an item.
- `ItemShort.specimens` replaces the deprecated `nb_specimens` / `nb_available` fields.
- `MediaType` values are camelCase strings matching the server enum (e.g. `printedText`, `videoDvd`).
- `day_of_week` in schedule slots: `0 = Monday`, `6 = Sunday`.
- Loan overdue status is provided directly by the API (`is_overdue`); don't recompute it client-side.
