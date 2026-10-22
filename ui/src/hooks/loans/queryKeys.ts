/** Patron self-service loans (`GET /loans/me`). */
export const MY_LOANS_QUERY_ROOT = ['my-loans'] as const;

export function myLoansQueryKey(
  userId: string | undefined,
  params: { page: number; perPage: number; archived?: boolean },
) {
  return [...MY_LOANS_QUERY_ROOT, userId, params] as const;
}

export const OVERDUE_LOANS_QUERY_ROOT = ['overdue-loans'] as const;

export function overdueLoansQueryKey(page: number, perPage: number) {
  return [...OVERDUE_LOANS_QUERY_ROOT, page, perPage] as const;
}

export const DESK_PATRON_LOANS_QUERY_ROOT = ['desk-patron-loans'] as const;

export function deskPatronLoansQueryKey(userId: string, page: number, perPage: number) {
  return [...DESK_PATRON_LOANS_QUERY_ROOT, userId, page, perPage] as const;
}
