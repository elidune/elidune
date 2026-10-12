/**
 * Read `rights.holdsRights` (and legacy `borrowsRights`) from an access token payload.
 * No signature verification — display / routing only; the API remains authoritative.
 */
function decodeAccessTokenPayload(token: string): Record<string, unknown> | null {
  const parts = token.trim().split('.');
  if (parts.length < 2) return null;
  try {
    let b64 = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const pad = (4 - (b64.length % 4)) % 4;
    if (pad) b64 += '='.repeat(pad);
    const json = atob(b64);
    const data: unknown = JSON.parse(json);
    return data && typeof data === 'object' ? (data as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function normalizeHoldsRightsValue(raw: unknown): string | null {
  const s = raw != null ? String(raw).trim().toLowerCase() : '';
  return s || null;
}

export function readHoldsRightsFromJwt(accessToken: string | null | undefined): string | null {
  if (!accessToken?.trim()) return null;
  const payload = decodeAccessTokenPayload(accessToken);
  if (!payload) return null;
  const rights = payload.rights;
  if (rights && typeof rights === 'object') {
    const r = rights as Record<string, unknown>;
    const fromNested = normalizeHoldsRightsValue(r.holdsRights ?? r.borrowsRights);
    if (fromNested) return fromNested;
  }
  return normalizeHoldsRightsValue(payload.holdsRights ?? payload.borrowsRights);
}

function normalizeAcquisitionsRightsValue(raw: unknown): 'n' | 'r' | 'w' | null {
  const s = raw != null ? String(raw).trim().toLowerCase() : '';
  if (s === 'n' || s === 'none') return 'n';
  if (s === 'r' || s === 'read') return 'r';
  if (s === 'w' || s === 'write') return 'w';
  return null;
}

/**
 * Minimal user from an access token when `/auth/me` fails transiently.
 * No signature verification — enough to keep the shell signed in.
 */
export function readSessionUserFromAccessToken(accessToken: string | null | undefined): {
  id: string;
  username: string;
  login?: string;
  accountType?: string;
} | null {
  if (!accessToken?.trim()) return null;
  const payload = decodeAccessTokenPayload(accessToken);
  if (!payload) return null;
  const idRaw = payload.user_id ?? payload.userId ?? payload.sub;
  const id = idRaw != null && String(idRaw).trim() !== '' ? String(idRaw) : '';
  if (!id) return null;
  const accountRaw = payload.account_type ?? payload.accountType;
  const login = typeof payload.sub === 'string' && payload.sub.trim() ? payload.sub : undefined;
  return {
    id,
    username: login ?? id,
    login,
    accountType: accountRaw != null && String(accountRaw).trim() ? String(accountRaw) : undefined,
  };
}

export function readAcquisitionsRightsFromJwt(accessToken: string | null | undefined): 'n' | 'r' | 'w' | null {
  return readRightsLevelFromJwt(accessToken, 'acquisitionsRights');
}

function readRightsLevelFromJwt(
  accessToken: string | null | undefined,
  field: string,
): 'n' | 'r' | 'w' | null {
  if (!accessToken?.trim()) return null;
  const payload = decodeAccessTokenPayload(accessToken);
  if (!payload) return null;
  const rights = payload.rights;
  if (rights && typeof rights === 'object') {
    const nested = normalizeAcquisitionsRightsValue((rights as Record<string, unknown>)[field]);
    if (nested) return nested;
  }
  return normalizeAcquisitionsRightsValue(payload[field]);
}

export function readWeedingRightsFromJwt(accessToken: string | null | undefined): 'n' | 'r' | 'w' | null {
  return readRightsLevelFromJwt(accessToken, 'weedingRights');
}

export function readWeedingCancelRightsFromJwt(
  accessToken: string | null | undefined,
): 'n' | 'r' | 'w' | null {
  return readRightsLevelFromJwt(accessToken, 'weedingCancelRights');
}

export function readItemsArchiveRightsFromJwt(
  accessToken: string | null | undefined,
): 'n' | 'r' | 'w' | null {
  return readRightsLevelFromJwt(accessToken, 'itemsArchiveRights');
}

export function readItemsRightsFromJwt(accessToken: string | null | undefined): 'n' | 'r' | 'w' | null {
  return readRightsLevelFromJwt(accessToken, 'itemsRights');
}
