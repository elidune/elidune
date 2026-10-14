import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import axios, { AxiosHeaders } from 'axios';
import { createInstance, type i18n as I18nInstance, type TFunction } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import RenewalRefusalNotice from '../components/loans/RenewalRefusalNotice.tsx';
import { IDEMPOTENCY_HEADER } from './idempotency.ts';
import { presentRenewalRefusal, type RenewalDeskView } from './renewalDesk.ts';

const OTHER_READER = 'Ada Lovelace';
const OTHER_CARD = 'C-88421';

function installBrowserShims(): void {
  const store = new Map<string, string>();
  const storage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => {
      store.set(key, String(value));
    },
    removeItem: (key: string) => {
      store.delete(key);
    },
    clear: () => store.clear(),
    key: () => null,
    length: 0,
  };
  Object.defineProperty(globalThis, 'localStorage', { value: storage, configurable: true });
  if (typeof globalThis.window === 'undefined') {
    Object.defineProperty(globalThis, 'window', { value: globalThis, configurable: true });
  }
}

function refusal(
  status: number,
  code: string,
  message: string,
  extra: Record<string, unknown> = {},
) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status,
      data: {
        code,
        error: status === 422 ? 'Business Rule Violation' : 'Forbidden',
        message,
        userName: OTHER_READER,
        cardNumber: OTHER_CARD,
        initials: 'AL',
        details: { firstname: 'Ada', lastname: 'Lovelace', cardNumber: OTHER_CARD },
        ...extra,
      },
    },
  };
}

async function i18nFr(): Promise<{ i18n: I18nInstance; t: TFunction }> {
  const i18n = createInstance();
  await i18n.init({
    lng: 'fr',
    resources: {
      fr: { translation: fr },
      en: { translation: en },
      de: { translation: de },
      es: { translation: es },
    },
    interpolation: { escapeValue: false },
  });
  return { i18n, t: i18n.t.bind(i18n) };
}

function renderNotice(i18n: I18nInstance, view: RenewalDeskView, onForce: () => void): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(RenewalRefusalNotice, {
        view,
        onForce,
        onDismiss: () => undefined,
        onSetAside: () => undefined,
      }),
    ) as ReactElement,
  ).replace(/&#x27;|&apos;/g, "'");
}

function assertNoOtherReader(html: string): void {
  assert.equal(html.includes(OTHER_READER), false);
  assert.equal(html.includes(OTHER_CARD), false);
  assert.equal(html.includes('Ada'), false);
  assert.equal(html.includes('Lovelace'), false);
}

interface CapturedRequest {
  url?: string;
  baseURL?: string;
  params?: unknown;
  headers?: unknown;
  method?: string;
}

async function withAxios(
  respond: (config: CapturedRequest) => { status: number; data: unknown },
  run: (calls: CapturedRequest[]) => Promise<void>,
): Promise<void> {
  const calls: CapturedRequest[] = [];
  const proto = axios.Axios.prototype as unknown as {
    request: (config: CapturedRequest) => Promise<unknown>;
  };
  const original = proto.request;
  proto.request = async function request(config: CapturedRequest) {
    calls.push(config);
    const outcome = respond(config);
    if (outcome.status >= 400) {
      const error = new axios.AxiosError('Request failed');
      error.response = {
        status: outcome.status,
        statusText: 'Error',
        data: outcome.data,
        headers: {},
        config: config as never,
      };
      throw error;
    }
    return {
      data: outcome.data,
      status: outcome.status,
      statusText: 'OK',
      headers: {},
      config,
    };
  };
  try {
    await run(calls);
  } finally {
    proto.request = original;
  }
}

function headerValue(headers: unknown, name: string): string | undefined {
  if (!headers || typeof headers !== 'object') return undefined;
  const fromAxios = headers instanceof AxiosHeaders ? headers.get(name) : undefined;
  if (typeof fromAxios === 'string') return fromAxios;
  const record = headers as Record<string, unknown>;
  const direct = record[name] ?? record[name.toLowerCase()];
  return typeof direct === 'string' ? direct : undefined;
}

describe('staff renewal refusal', () => {
  it('shows Forcer le renouvellement for a forceable 422 and sends ?force=true', async () => {
    installBrowserShims();
    const { i18n, t } = await i18nFr();
    const error = refusal(422, 'not_a_listed_code', 'Maximum renewals reached (2/2)', {
      forceable: true,
    });
    const view = presentRenewalRefusal(error, { alreadyForced: false, t });
    assert.equal(view.kind, 'force');
    if (view.kind !== 'force') return;

    let forced = false;
    const html = renderNotice(i18n, view, () => {
      forced = true;
    });
    assert.match(html, /<button[^>]*>Forcer le renouvellement<\/button>/);
    assert.equal(html.includes(fr.loans.renewalRefusal.max_renewals) || html.includes('Maximum renewals reached (2/2)'), true);
    assert.equal(html.includes('Forcer le prêt'), false);
    assertNoOtherReader(html);
    assert.equal(forced, false);

    const { default: api } = await import('../services/api.ts');
    await withAxios(
      () => ({ data: { id: 'loan-1', expiryAt: '2026-12-01T00:00:00Z', message: 'ok' }, status: 200 }),
      async (calls) => {
        const renewed = await api.renewLoan('loan-1', 'idem-renew-1', { force: true });
        assert.equal(renewed.expiryAt, '2026-12-01T00:00:00Z');
        assert.equal(calls.length, 1);
        const uri = axios.getUri({
          url: calls[0].url,
          baseURL: calls[0].baseURL,
          params: calls[0].params as Record<string, string | boolean> | undefined,
        });
        assert.match(uri, /\/loans\/loan-1\/renew\?force=true$/);
        assert.equal(headerValue(calls[0].headers, IDEMPOTENCY_HEADER), 'idem-renew-1');
        assert.equal(calls[0].method?.toLowerCase(), 'post');
      },
    );
  });

  it('shows the refusal and no force button when forceable is false or missing', async () => {
    const { i18n, t } = await i18nFr();
    for (const extra of [{ forceable: false }, {}] as const) {
      const error = refusal(422, 'subscription_expired', 'User subscription expired', extra);
      const view = presentRenewalRefusal(error, { alreadyForced: false, t });
      assert.equal(view.kind, 'sentence');
      if (view.kind !== 'sentence') continue;
      assert.equal(view.message, fr.loans.renewalRefusal.subscription_expired);
      const html = renderNotice(i18n, view, () => undefined);
      assert.match(html, /role="alert"/);
      assert.equal(html.includes(fr.loans.renewalRefusal.subscription_expired), true);
      assert.equal(html.includes('Forcer'), false);
      assert.equal(html.includes('<button'), false);
      assertNoOtherReader(html);
    }
    const waiting = refusal(422, 'hold_waiting', 'Cannot renew: another patron is waiting', {
      forceable: false,
    });
    const heldView = presentRenewalRefusal(waiting, { alreadyForced: false, t });
    assert.equal(heldView.kind, 'sentence');
    if (heldView.kind === 'sentence') {
      assert.equal(heldView.message, fr.loans.renewalRefusal.hold_waiting);
      assert.equal(heldView.message.includes(OTHER_READER), false);
    }
  });

  it('shows the server message and no force button when the forced call returns 403', async () => {
    const { i18n, t } = await i18nFr();
    const denied = refusal(403, 'authorization_failed', 'Insufficient rights to write loans');
    const view = presentRenewalRefusal(denied, { alreadyForced: true, t });
    assert.equal(view.kind, 'sentence');
    if (view.kind !== 'sentence') return;
    assert.equal(view.message, fr.errors.apiCode.authorization_failed);
    const html = renderNotice(i18n, view, () => undefined);
    assert.equal(html.includes(fr.errors.apiCode.authorization_failed), true);
    assert.equal(html.includes('Insufficient rights to write loans'), false);
    assert.equal(html.includes('Forcer'), false);
    assertNoOtherReader(html);

    const unmapped = refusal(403, 'rights_missing', 'Renewal force is not allowed for this desk');
    const raw = presentRenewalRefusal(unmapped, { alreadyForced: true, t });
    assert.equal(raw.kind, 'sentence');
    if (raw.kind === 'sentence') {
      assert.equal(raw.message, 'Renewal force is not allowed for this desk');
    }
  });

  it('reuses the held-for-other popup and offers force only from the flag', async () => {
    const { i18n, t } = await i18nFr();
    const held = refusal(422, 'item_held_for_other', 'held for Grace Hopper', { forceable: true });
    const view = presentRenewalRefusal(held, { alreadyForced: false, t });
    assert.equal(view.kind, 'setAside');
    if (view.kind !== 'setAside') return;
    assert.equal(view.force, true);
    assert.equal(view.message, fr.errors.apiCode.item_held_for_other);
    const html = renderNotice(i18n, view, () => undefined);
    assert.equal(html.includes(fr.loans.toPickForOtherTitle), true);
    assert.equal(html.includes(fr.loans.toPickSetAside), true);
    assert.match(html, /<button[^>]*>Forcer le renouvellement<\/button>/);
    assert.equal(html.includes('Forcer le prêt'), false);
    assert.equal(html.includes('Grace Hopper'), false);
    assertNoOtherReader(html);

    const locked = presentRenewalRefusal(
      refusal(422, 'item_held_for_other', 'held', { forceable: false }),
      { alreadyForced: false, t },
    );
    assert.equal(locked.kind, 'setAside');
    if (locked.kind === 'setAside') {
      assert.equal(locked.force, false);
      const quiet = renderNotice(i18n, locked, () => undefined);
      assert.equal(quiet.includes('Forcer'), false);
      assert.equal(quiet.includes(fr.loans.toPickSetAside), true);
    }
  });

  it('wires the loans desk and the reader file to the renewal refusal, not a reason-code list', () => {
    const loans = readFileSync(fileURLToPath(new URL('../pages/LoansPage.tsx', import.meta.url)), 'utf8');
    const reader = readFileSync(fileURLToPath(new URL('../pages/UserDetailPage.tsx', import.meta.url)), 'utf8');
    for (const source of [loans, reader]) {
      assert.equal(source.includes('presentRenewalRefusal'), true);
      assert.equal(source.includes('RenewalRefusalNotice'), true);
      assert.equal(source.includes('renewalMayForce'), false);
    }
    assert.equal(loans.includes('handleOverdueRenew'), true);
    assert.match(loans, /renewLoan\(loanId, newIdempotencyKey\(\), \{ force: force \|\| undefined \}\)/);
    assert.match(reader, /renewLoan\(loanId, newIdempotencyKey\(\), \{ force: force \|\| undefined \}\)/);
  });
});
