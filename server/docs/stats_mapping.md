### Library statistics ↔ annual form (sections C, D, E, G, H)

This document describes how the annual form indicators map to the data model and statistics endpoints.

#### Section D – Collections (holdings, acquisitions, withdrawals)

- **D1 – Print materials (holdings, acquisitions, withdrawals)**  
  - **Tables / fields**  
    - `items`  
      - `media_type` = print → codes `b`, `bc`, `p` (books, comics, periodicals).  
      - `public_type` = 97 (adults), 106 (youth).  
      - `is_archive` = 0 / NULL → document active in the collection.  
      - `crea_date` → date of entry in the database (approx. acquisition).  
      - `archived_date` → date of removal (withdrawal / weeding).  
  - **Holdings as of 12/31 year N**  
    - Count of active `items` rows at the reference date (not deleted, not archived by that date), filtered by `media_type` (print) and optionally by `public_type` (adults / youth).  
  - **Acquisitions for year N**  
    - Count of `items` with `crea_date` in [January 1 N ; December 31 N], same `media_type` / `public_type` filters.  
  - **Withdrawals for year N**  
    - Count of `items` with `archived_date` in [January 1 N ; December 31 N], same filters.

- **D2 – Serials**  
  - Same as D1 but filtered on `media_type = "p"` (periodicals), optionally using `Collection` / `Edition` relations if specific handling is needed.

- **D3 – Other documents**  
  - Represented by other non-print `media_type` codes (audio, video, CD-ROM, images…).  
  - Same holdings / acquisitions / withdrawals logic as D1, with filters on the relevant `MediaType` codes.

- **D4 – Audiovisual and multimedia documents on physical carriers**  
  - Subset of non-print media: `v`, `vt`, `vd`, `a`, `am*`, `an*`, `c`, `m`, `i` per the chosen definition.  
  - Uses the same fields as D1 (holdings as of 12/31 via current status, acquisitions via `crea_date`, withdrawals via `archived_date`).

> Note: the distinction "own holdings / holdings deposited by the departmental library" is not represented by a dedicated field in the current model. D statistics may therefore be computed **on the full local collection**, without that split, unless a specific attribute is added later.

#### Section E1 – Usage and users (new registrations, active borrowers)

- **New registrations for year N (E111…E143)**  
  - **Table / fields**  
    - `users`  
      - `crea_date` → patron record creation date.  
      - `status` → exclude deleted users (`status = 2`).  
      - `public_type` → breakdown adults / youth.  
      - `sex` → breakdown male / female / unknown (70=F, 77=M, 85=Unknown).  
  - **Definition**  
    - User whose `crea_date` falls in [January 1 N ; December 31 N].  
    - Ventilation by `public_type` and `sex` available in aggregate response.

- **Active borrowers for year N (E121…E144)**  
  - **Tables / fields**  
    - `loans`: current loans.  
    - `loans_archives`: completed loans, with  
      - `date` (loan),  
      - `returned_date` (return),  
      - `borrower_public_type`, `account_type` for breakdowns.  
  - **Definition**  
    - User with a valid registration who had at least one loan (in `loans` or `loans_archives`) in the period [January 1 N ; December 31 N].  
    - In practice: `COUNT(DISTINCT user_id)` over the union of active and archived loans filtered by period.  
    - Ventilation by `public_type` available in aggregate response.

- **Group accounts / Collectivites (E143, E144)**  
  - Users with `account_type = 'group'`.  
  - Count available in aggregate response via `groups_total`.

- **Visitors / Frequentation (E147)**  
  - **Table**: `visitor_counts`  
    - `count_date`, `count`, `source` (manual / counter / estimate).  
  - **Definition**: Sum of all `count` values for dates in the year.  
  - **API**: `GET /visitor-counts` + `POST /visitor-counts` for external counter integration.

#### Section E2 – Loans

- **Total loans for year N (all media / adult collection / youth collection)**  
  - **Tables / fields**  
    - `loans` + `loans_archives`  
      - `date`: loan date.  
      - `user_id` / `borrower_public_type`: public type.  
      - `item_id` or join to `items` to access `media_type`, `public_type`.  
  - **Definition**  
    - Total number of loans whose `date` is in [January 1 N ; December 31 N].  
    - Breakdown by:  
      - **media type** → filters on `items.media_type`.  
      - **adult / youth collection** → filters on `items.public_type` (97 = adult, 106 = youth).  
    - Loans from items supplied by the departmental library are not distinguishable in the current model.

#### Section H – Cultural actions and education

- **Tables / fields**  
  - `events`  
    - `event_type`: 0=animation, 1=school_visit, 2=exhibition, 3=conference, 4=workshop, 5=show, 6=other.  
    - `event_date`: date of the event.  
    - `attendees_count`: total participants for the event.  
    - `target_public`: 97=adult, 106=children.  
    - `school_name`, `class_name`, `students_count`: school-specific fields.

- **School partnerships (H)**  
  - Distinct classes: `COUNT(DISTINCT class_name)` where `event_type = 1`.  
  - Total visits: `COUNT(*)` where `event_type = 1`.  
  - Total students: `SUM(students_count)` where `event_type = 1`.

- **Animations and events**  
  - Total actions: `COUNT(*)` for the year.  
  - Total attendees: `SUM(attendees_count)` for the year.  
  - Each occurrence (same event on different dates) counts separately.

- **API**: `GET /events`, `POST /events`, `PUT /events/:id`, `DELETE /events/:id`.

#### Sections C & G – Resources and equipment

- **Opening hours**  
  - **Tables**: `schedule_periods`, `schedule_slots`, `schedule_closures`.  
  - Weekly hours: sum of `(close_time - open_time)` for all slots in the relevant period.  
  - Annual opening days: computed from scheduled days per week × 52 − closures.  
  - **API**: `GET /schedules/periods`, slots, closures endpoints.

- **IT equipment**  
  - **Table**: `equipment`  
    - `equipment_type`: 0=computer, 1=tablet, 2=ereader, 3=other.  
    - `has_internet`, `is_public`, `quantity`, `status`.  
  - Public internet stations: `SUM(quantity)` where `is_public = TRUE AND has_internet = TRUE AND status = 0`.  
  - Public devices (tablets/ereaders): `SUM(quantity)` where `is_public = TRUE AND equipment_type IN (1,2) AND status = 0`.  
  - **API**: `GET /equipment`, `POST /equipment`, `PUT /equipment/:id`, `DELETE /equipment/:id`.

- **Personnel (ETPT)**  
  - **Table**: `users` (staff fields).  
    - `staff_type`: NULL=not staff, 0=employee, 1=volunteer.  
    - `hours_per_week`: contractual hours.  
    - `staff_start_date`, `staff_end_date`: employment period.  
  - ETPT = `hours_per_week / 35.0` (base 35h).  
  - Computed from active staff members (where `staff_start_date <= year_end AND (staff_end_date IS NULL OR staff_end_date >= year_start)`).

#### Endpoints ↔ form summary

- **Holdings as of 12/31 / acquisitions / withdrawals (Section D)**  
  - `GET /stats` with optional parameters `year`, `media_type`, `public_type`.  
  - Response now includes `acquisitions`, `acquisitions_by_media_type`, `withdrawals`, `withdrawals_by_media_type`.

- **Users (E1): new registrations, active borrowers**  
  - `GET /stats/users` with parameters `mode=aggregate`, `start_date`, `end_date`.  
  - Response includes `new_users_by_public_type`, `new_users_by_sex`, `active_borrowers_by_public_type`, `groups_total`.

- **Loans (E2): totals by collection type / media**  
  - `GET /stats/loans` with parameters `start_date`, `end_date`, `interval`, `media_type`, `public_type`.

- **Visitors (E147)**  
  - `GET /visitor-counts` with `start_date`, `end_date`.

- **Events (H)**  
  - `GET /events` with filters `start_date`, `end_date`, `event_type`.

- **Resources (C, G)**  
  - `GET /schedules/periods`, `GET /equipment`.
