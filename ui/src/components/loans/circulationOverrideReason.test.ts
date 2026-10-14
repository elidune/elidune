import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import axios, { AxiosHeaders } from 'axios';
import { createInstance, type i18n as I18nInstance, type TFunction } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import BatchCreateItemRow from './BatchCreateItemRow.tsx';
import ForceCirculationDialog from './ForceCirculationDialog.tsx';
import RenewalRefusalNotice from './RenewalRefusalNotice.tsx';
import ToPickForceLoanDialog from './ToPickForceLoanDialog.tsx';
import {
  forceConfirmEnabled,
  overrideReasonRefusalMessage,
  refusalReasonRequired,
} from '../../utils/circulationForce.ts';
import { presentCheckoutRefusal } from '../../utils/deskCheckoutError.ts';
import { IDEMPOTENCY_HEADER } from '../../utils/idempotency.ts';
import { presentRenewalRefusal } from '../../utils/renewalDesk.ts';

const SERVER_REASON = 'A reason is required to force this circulation action';
const OTHER_READER = 'Grace Hopper';
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
  code: string,
  message: string,
  extra: Record<string, unknown> = {},
) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status: 422,
      data: {
        code,
        error: 'Business Rule Violation',
        message,
        userName: OTHER_READER,
        cardNumber: OTHER_CARD,
        initials: 'GH',
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

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, node) as ReactElement,
  );
}

function buttonDisabled(html: string, label: string): boolean {
  const match = html.match(new RegExp(`<button\\b([^>]*)>${label}</button>`));
  assert.ok(match, label);
  return /(?:^|\s)disabled(?:\s*=|\s|$)/.test(match[1]);
}

interface CapturedRequest {
  url?: string;
  baseURL?: string;
  params?: unknown;
  data?: unknown;
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

function requestBody(data: unknown): Record<string, unknown> {
  if (typeof data === 'string') return JSON.parse(data) as Record<string, unknown>;
  return (data ?? {}) as Record<string, unknown>;
}

describe('circulation override reason', () => {
  it('checkout force with reasonRequired sends overrideReason in the body', async () => {
    installBrowserShims();
    const { i18n, t } = await i18nFr();
    const error = refusal('max_loans', 'quota', {
      forceable: true,
      reasonRequired: true,
    });
    assert.equal(refusalReasonRequired(error), true);
    assert.equal(refusalReasonRequired(refusal('subscription_expired', 'expired', { forceable: true })), false);
    const view = presentCheckoutRefusal(error, { canForce: true, alreadyForced: false, t });
    assert.equal(view.kind, 'force');
    if (view.kind !== 'force') return;
    assert.equal(view.reasonRequired, true);

    const html = render(
      i18n,
      createElement(ForceCirculationDialog, {
        isOpen: true,
        message: view.message,
        confirmLabel: fr.loans.forceLoan,
        reasonRequired: view.reasonRequired,
        onCancel: () => undefined,
        onConfirm: () => undefined,
      }),
    );
    assert.equal(html.includes(fr.loans.forceOverrideReason), true);
    assert.equal(buttonDisabled(html, fr.loans.forceLoan), true);

    const { default: api } = await import('../../services/api.ts');
    await withAxios(
      () => ({ data: { id: 'loan-1', expiryAt: '2026-12-01T00:00:00Z', message: 'ok' }, status: 200 }),
      async (calls) => {
        await api.createLoan({
          userId: '42',
          itemIdentification: 'ITEM-1',
          force: true,
          overrideReason: '  reader is in the room  ',
        }, 'idem-checkout-1');
        assert.equal(calls.length, 1);
        assert.equal(calls[0].method?.toLowerCase(), 'post');
        assert.match(String(calls[0].url), /\/loans$/);
        assert.deepEqual(requestBody(calls[0].data), {
          userId: '42',
          itemIdentification: 'ITEM-1',
          force: true,
          overrideReason: 'reader is in the room',
        });
        assert.equal(headerValue(calls[0].headers, IDEMPOTENCY_HEADER), 'idem-checkout-1');
      },
    );

    const loans = readFileSync(fileURLToPath(new URL('../../pages/LoansPage.tsx', import.meta.url)), 'utf8');
    const reader = readFileSync(fileURLToPath(new URL('../../pages/UserDetailPage.tsx', import.meta.url)), 'utf8');
    assert.match(loans, /overrideReason: reason/);
    assert.match(reader, /overrideReason: reason/);
  });

  it('renewal force sends overrideReason as a query parameter', async () => {
    installBrowserShims();
    const { i18n, t } = await i18nFr();
    const error = refusal('subscription_expired', 'User subscription expired', {
      forceable: true,
      reasonRequired: true,
    });
    const view = presentRenewalRefusal(error, { alreadyForced: false, t });
    assert.equal(view.kind, 'force');
    if (view.kind !== 'force') return;
    assert.equal(view.reasonRequired, true);
    const html = render(
      i18n,
      createElement(RenewalRefusalNotice, {
        view,
        onForce: () => undefined,
        onDismiss: () => undefined,
        onSetAside: () => undefined,
      }),
    );
    assert.equal(html.includes(fr.loans.forceOverrideReason), true);
    assert.equal(buttonDisabled(html, fr.loans.forceRenew), true);
    assert.equal(html.includes(fr.loans.forceLoan), false);

    const { default: api } = await import('../../services/api.ts');
    await withAxios(
      () => ({ data: { id: 'loan-9', expiryAt: '2026-12-01T00:00:00Z', message: 'ok' }, status: 200 }),
      async (calls) => {
        await api.renewLoan('loan-9', 'idem-renew-9', {
          force: true,
          overrideReason: 'still enrolled',
        });
        const uri = axios.getUri({
          url: calls[0].url,
          baseURL: calls[0].baseURL,
          params: calls[0].params as Record<string, string | boolean> | undefined,
        });
        const parsed = new URL(uri, 'http://desk.local');
        assert.match(parsed.pathname, /\/loans\/loan-9\/renew$/);
        assert.equal(parsed.searchParams.get('force'), 'true');
        assert.equal(parsed.searchParams.get('overrideReason'), 'still enrolled');
        assert.equal(headerValue(calls[0].headers, IDEMPOTENCY_HEADER), 'idem-renew-9');

        await api.renewLoanByBarcode('BC-9', 'idem-renew-barcode', {
          force: true,
          overrideReason: 'still enrolled',
        });
        const barcodeUri = axios.getUri({
          url: calls[1].url,
          baseURL: calls[1].baseURL,
          params: calls[1].params as Record<string, string | boolean> | undefined,
        });
        const barcode = new URL(barcodeUri, 'http://desk.local');
        assert.match(barcode.pathname, /\/loans\/items\/BC-9\/renew$/);
        assert.equal(barcode.searchParams.get('force'), 'true');
        assert.equal(barcode.searchParams.get('overrideReason'), 'still enrolled');
      },
    );
  });

  it('disables confirm while the reason is blank', async () => {
    const { i18n, t } = await i18nFr();
    assert.equal(forceConfirmEnabled(true, ''), false);
    assert.equal(forceConfirmEnabled(true, '   '), false);
    assert.equal(forceConfirmEnabled(true, ' desk note '), true);
    assert.equal(forceConfirmEnabled(false, ''), true);

    const checkout = render(
      i18n,
      createElement(ForceCirculationDialog, {
        isOpen: true,
        message: 'Quota',
        confirmLabel: fr.loans.forceLoan,
        reasonRequired: true,
        onCancel: () => undefined,
        onConfirm: () => undefined,
      }),
    );
    assert.equal(buttonDisabled(checkout, fr.loans.forceLoan), true);
    assert.equal(checkout.includes('<textarea'), true);

    const held = presentCheckoutRefusal(
      refusal('item_held_for_other', 'held for Grace Hopper', {
        forceable: true,
        reasonRequired: true,
        details: { holdId: '55', cardNumber: OTHER_CARD, initials: 'GH' },
      }),
      { canForce: true, alreadyForced: false, t },
    );
    assert.equal(held.kind, 'setAside');
    if (held.kind !== 'setAside') return;
    const popup = render(
      i18n,
      createElement(ToPickForceLoanDialog, {
        isOpen: true,
        message: held.message,
        reasonRequired: held.reasonRequired,
        onCancel: () => undefined,
        onSetAside: () => undefined,
        onForce: () => undefined,
      }),
    );
    assert.equal(popup.includes(fr.loans.forceOverrideReason), true);
    assert.equal(buttonDisabled(popup, fr.loans.forceLoan), true);
    assert.equal(buttonDisabled(popup, fr.loans.toPickSetAside), false);
    assert.equal(popup.includes(OTHER_READER), false);
    assert.equal(popup.includes(OTHER_CARD), false);
    assert.equal(popup.includes('Grace'), false);
    assert.equal(popup.includes('55'), false);
  });

  it('shows the server message for 422 override_reason_required and keeps the dialog open', async () => {
    const { i18n } = await i18nFr();
    const missing = refusal('override_reason_required', SERVER_REASON);
    assert.equal(overrideReasonRefusalMessage(missing), SERVER_REASON);
    assert.equal(overrideReasonRefusalMessage(refusal('max_loans', 'quota', { forceable: true })), null);
    assert.equal(refusalReasonRequired({ reasonRequired: 'true' }), false);
    assert.equal(refusalReasonRequired({ reasonRequired: false }), false);

    const html = render(
      i18n,
      createElement(ForceCirculationDialog, {
        isOpen: true,
        message: 'Abonnement expiré',
        confirmLabel: fr.loans.forceLoan,
        reasonRequired: true,
        error: overrideReasonRefusalMessage(missing),
        onCancel: () => undefined,
        onConfirm: () => undefined,
      }),
    );
    assert.equal(html.includes(SERVER_REASON), true);
    assert.match(html, /role="alert"/);
    assert.match(html, /aria-invalid="true"/);
    const described = html.match(/aria-describedby="([^"]+)"/);
    assert.ok(described);
    assert.match(html, new RegExp(`id="${described[1]}"`));
    assert.equal(html.includes(fr.loans.forceLoan), true);
    assert.equal(html.includes(fr.loans.forceOverrideReason), true);

    const loans = readFileSync(fileURLToPath(new URL('../../pages/LoansPage.tsx', import.meta.url)), 'utf8');
    const reader = readFileSync(fileURLToPath(new URL('../../pages/UserDetailPage.tsx', import.meta.url)), 'utf8');
    assert.equal(loans.includes('overrideReasonRefusalMessage'), true);
    assert.equal(reader.includes('overrideReasonRefusalMessage'), true);
  });

  it('omits the reason field when reasonRequired is absent or false', async () => {
    const { i18n, t } = await i18nFr();
    for (const extra of [{ forceable: true }, { forceable: true, reasonRequired: false }] as const) {
      const view = presentCheckoutRefusal(
        refusal('max_loans', 'quota', extra),
        { canForce: true, alreadyForced: false, t },
      );
      assert.equal(view.kind, 'force');
      if (view.kind !== 'force') continue;
      assert.equal(view.reasonRequired, false);
      const html = render(
        i18n,
        createElement(ForceCirculationDialog, {
          isOpen: true,
          message: view.message,
          confirmLabel: fr.loans.forceLoan,
          reasonRequired: view.reasonRequired,
          onCancel: () => undefined,
          onConfirm: () => undefined,
        }),
      );
      assert.equal(html.includes(fr.loans.forceOverrideReason), false);
      assert.equal(html.includes('<textarea'), false);
      assert.equal(buttonDisabled(html, fr.loans.forceLoan), false);
    }

    const renewal = presentRenewalRefusal(
      refusal('max_renewals', 'Maximum renewals reached', { forceable: true }),
      { alreadyForced: false, t },
    );
    assert.equal(renewal.kind, 'force');
    if (renewal.kind !== 'force') return;
    assert.equal(renewal.reasonRequired, false);
    const renewed = render(
      i18n,
      createElement(RenewalRefusalNotice, {
        view: renewal,
        onForce: () => undefined,
        onDismiss: () => undefined,
        onSetAside: () => undefined,
      }),
    );
    assert.equal(renewed.includes(fr.loans.forceOverrideReason), false);
    assert.equal(buttonDisabled(renewed, fr.loans.forceRenew), false);

    const picked = presentCheckoutRefusal(
      refusal('item_to_pick_for_other', 'on the pick list', {
        forceable: true,
        reasonRequired: true,
        details: { holdId: '9', userId: '3', cardNumber: OTHER_CARD, initials: 'GH' },
      }),
      { canForce: true, alreadyForced: false, t },
    );
    assert.equal(picked.kind, 'sentence');
  });

  it('batch checkout shows no force', async () => {
    const { i18n } = await i18nFr();
    const held = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: {
          barcode: 'B-1',
          success: false,
          code: 'item_held_for_other',
          error: 'held',
          forceable: true,
          reasonRequired: true,
        },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.match(held, /Mettre de côté/);
    assert.match(held, /Annuler/);
    assert.equal(held.includes('Forcer'), false);
    assert.equal(held.includes(fr.loans.forceOverrideReason), false);
    assert.equal(held.includes('<textarea'), false);

    const quota = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: {
          barcode: 'B-2',
          success: false,
          code: 'subscription_expired',
          error: 'expired',
          forceable: true,
          reasonRequired: true,
        },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.equal(quota.includes('Forcer'), false);
    assert.equal(quota.includes(fr.loans.forceOverrideReason), false);
    assert.equal(quota.includes('<textarea'), false);
  });
});
