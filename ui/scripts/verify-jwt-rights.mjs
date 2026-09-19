#!/usr/bin/env node
/**
 * Light verification for rights-ranking / JWT claim reading (no vitest in ui/).
 * Run from ui/: node scripts/verify-jwt-rights.mjs
 */

function rightsRank(level) {
  switch ((level ?? '').trim().toLowerCase()) {
    case 'none':
    case 'n':
      return 0;
    case 'own':
    case 'o':
      return 1;
    case 'read':
    case 'r':
      return 2;
    case 'write':
    case 'w':
      return 3;
    default:
      return -1;
  }
}

function normalizeRightsLevel(raw) {
  const s = raw != null ? String(raw).trim().toLowerCase() : '';
  if (s === 'n' || s === 'none') return 'none';
  if (s === 'o' || s === 'own') return 'own';
  if (s === 'r' || s === 'read') return 'read';
  if (s === 'w' || s === 'write') return 'write';
  return null;
}

function hasMinRightsLevel(actual, required) {
  const a = rightsRank(actual);
  const r = rightsRank(required);
  return a >= 0 && r >= 0 && a >= r;
}

function b64url(obj) {
  return Buffer.from(JSON.stringify(obj), 'utf8')
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

function fakeJwt(payload) {
  return `${b64url({ alg: 'none' })}.${b64url(payload)}.sig`;
}

function decodeAccessTokenPayload(token) {
  const parts = token.trim().split('.');
  if (parts.length < 2) return null;
  let b64 = parts[1].replace(/-/g, '+').replace(/_/g, '/');
  const pad = (4 - (b64.length % 4)) % 4;
  if (pad) b64 += '='.repeat(pad);
  return JSON.parse(Buffer.from(b64, 'base64').toString('utf8'));
}

function readDomainRightFromJwt(accessToken, domain) {
  const payload = decodeAccessTokenPayload(accessToken);
  if (!payload) return null;
  const rights = payload.rights && typeof payload.rights === 'object' ? payload.rights : null;
  const keys = {
    items: ['itemsRights'],
    holds: ['holdsRights', 'borrowsRights'],
    loans: ['loansRights'],
  }[domain];
  for (const key of keys) {
    const nested = rights?.[key];
    const n = normalizeRightsLevel(nested);
    if (n) return n;
  }
  return null;
}

let failed = 0;
function check(name, cond) {
  if (!cond) {
    console.error('FAIL', name);
    failed += 1;
  } else {
    console.log('ok', name);
  }
}

check('own < read', rightsRank('own') < rightsRank('read'));
check('write >= write', hasMinRightsLevel('write', 'write'));
check('read insufficient for write', !hasMinRightsLevel('read', 'write'));
check('own insufficient for staff holds read', !hasMinRightsLevel('own', 'read'));
check('normalize letter w', normalizeRightsLevel('w') === 'write');

const token = fakeJwt({
  rights: {
    itemsRights: 'none',
    loansRights: 'write',
    holdsRights: 'own',
    borrowsRights: 'write',
  },
});
check('items none from jwt', readDomainRightFromJwt(token, 'items') === 'none');
check('loans write from jwt', readDomainRightFromJwt(token, 'loans') === 'write');
check('holds prefers holdsRights over borrows', readDomainRightFromJwt(token, 'holds') === 'own');

if (failed) {
  console.error(`${failed} check(s) failed`);
  process.exit(1);
}
console.log('All jwt rights checks passed');
