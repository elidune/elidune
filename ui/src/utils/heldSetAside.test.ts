import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  commitDismissHeldSetAside,
  commitHeldSetAside,
  heldForOtherHoldId,
  heldSetAsideServerMessage,
  heldSetAsideSuccessMessage,
} from './heldSetAside.ts';

const NAME = 'Grace Hopper';
const CARD = 'C-88421';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { fr, en, de, es };

const READY = {
  fr: 'Exemplaire mis de côté, à retirer',
  en: 'Copy set aside, ready to pull',
  de: 'Exemplar zurückgelegt, zum Entnehmen',
  es: 'Ejemplar apartado, para retirar',
} as const;

const TRANSIT = {
  fr: 'Transit vers Nord lancé',
  en: 'Transit to Nord started',
  de: 'Transit nach Nord gestartet',
  es: 'Tránsito hacia Nord iniciado',
} as const;

const CODES = {
  set_aside_item_mismatch: {
    fr: 'Cet exemplaire ne correspond pas à la réservation.',
    en: 'This copy does not match the hold.',
    de: 'Dieses Exemplar entspricht nicht der Vormerkung.',
    es: 'Este ejemplar no corresponde a la reserva.',
  },
  set_aside_wrong_title: {
    fr: "Cet exemplaire n'appartient pas au titre réservé.",
    en: 'This copy is not a copy of the reserved title.',
    de: 'Dieses Exemplar gehört nicht zum vorgemerkten Titel.',
    es: 'Este ejemplar no pertenece al título reservado.',
  },
  set_aside_not_oldest: {
    fr: "Une réservation plus ancienne est en attente. Rescannez l'exemplaire.",
    en: 'An older hold is waiting. Scan the copy again.',
    de: 'Eine ältere Vormerkung wartet. Scannen Sie das Exemplar erneut.',
    es: 'Hay una reserva más antigua en espera. Vuelva a escanear el ejemplar.',
  },
} as const;

async function tFor(lng: Lang): Promise<TFunction> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      fr: { translation: fr },
      en: { translation: en },
      de: { translation: de },
      es: { translation: es },
    },
    interpolation: { escapeValue: false },
  });
  return i18n.t.bind(i18n);
}

function refusal(code: string, extra: Record<string, unknown> = {}) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status: 422,
      data: {
        code,
        error: 'Unprocessable Entity',
        message: `held for ${NAME}`,
        userName: NAME,
        firstname: 'Grace',
        lastname: 'Hopper',
        cardNumber: CARD,
        initials: 'GH',
        ...extra,
      },
    },
  };
}

function poisonedReady(extra: Record<string, unknown> = {}) {
  return {
    status: 'ready',
    holdStatus: 'ready',
    transitId: null,
    pickupSiteId: '3',
    userName: NAME,
    patronName: NAME,
    holdFor: { firstname: 'Grace', lastname: 'Hopper', cardNumber: CARD, initials: 'GH' },
    hold: { user: { firstname: 'Grace', lastname: 'Hopper' } },
    ...extra,
  };
}

function assertNoIdentity(text: string) {
  assert.equal(text.includes(NAME), false);
  assert.equal(text.includes('Grace'), false);
  assert.equal(text.includes('Hopper'), false);
  assert.equal(text.includes(CARD), false);
  assert.equal(text.includes('GH'), false);
}

describe('item_held_for_other set-aside', () => {
  it('reads only details.holdId from the checkout refusal', () => {
    const error = refusal('item_held_for_other', {
      details: {
        holdId: '55',
        userId: '9',
        cardNumber: CARD,
        initials: 'GH',
        firstname: 'Grace',
        lastname: 'Hopper',
        userName: NAME,
      },
      holdId: 'not-this',
    });
    assert.equal(heldForOtherHoldId(error), '55');
    assert.equal(JSON.stringify(heldForOtherHoldId(error)).includes(NAME), false);
    assert.equal(heldForOtherHoldId(refusal('item_to_pick_for_other', { details: { holdId: '55' } })), null);
    assert.equal(heldForOtherHoldId(refusal('item_held_for_other', { details: { userName: NAME } })), null);
    assert.equal(
      heldForOtherHoldId(refusal('item_held_for_other', { details: { holdId: 88001 } })),
      '88001',
    );
  });

  it('posts holdId and itemId and says the copy is set aside when there is no transit', async () => {
    const t = await tFor('fr');
    const calls: Array<{ code: string; body: { holdId: string; itemId: string } }> = [];
    const toasts: Array<{ variant: string; message: string }> = [];
    let dialogOpen = true;
    let barcode = 'BC-1';
    let focused = false;
    let lookups = 0;

    await commitHeldSetAside(' BC-1 ', {
      holdId: '55',
      itemId: '77',
      resolveItemId: async () => {
        lookups += 1;
        return 'other';
      },
      returnSetAside: async (code, body) => {
        calls.push({ code, body });
        return poisonedReady();
      },
      successMessage: (result) => heldSetAsideSuccessMessage(result, t),
      errorMessage: (error) => heldSetAsideServerMessage(error, t),
      showToast: (toast) => {
        toasts.push(toast);
      },
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => {
        barcode = '';
      },
      refocusBarcode: () => {
        focused = true;
      },
    });

    assert.deepEqual(calls, [{ code: 'BC-1', body: { holdId: '55', itemId: '77' } }]);
    assert.equal(typeof calls[0]?.body.holdId, 'string');
    assert.equal(typeof calls[0]?.body.itemId, 'string');
    assert.equal(lookups, 0);
    assert.equal(dialogOpen, false);
    assert.equal(barcode, '');
    assert.equal(focused, true);
    assert.deepEqual(toasts, [{ variant: 'success', message: READY.fr }]);
    assertNoIdentity(JSON.stringify(toasts));
  });

  it('says a transit started when the same transitId comes back', async () => {
    const t = await tFor('fr');
    const calls: string[] = [];
    const toasts: string[] = [];
    const body = {
      status: 'in_transit',
      holdStatus: 'pending',
      transitId: '9001',
      pickupSiteId: '4',
      destinationSiteName: 'Nord',
      userName: NAME,
      holdFor: { firstname: 'Grace', lastname: 'Hopper' },
    };

    const deps = {
      holdId: '55',
      itemId: '77',
      returnSetAside: async () => {
        calls.push('9001');
        return body;
      },
      successMessage: (result: unknown) => heldSetAsideSuccessMessage(result, t),
      errorMessage: (error: unknown) => heldSetAsideServerMessage(error, t),
      showToast: (toast: { message: string }) => {
        toasts.push(toast.message);
      },
      closeDialog: () => undefined,
      clearBarcode: () => undefined,
      refocusBarcode: () => undefined,
    };

    await commitHeldSetAside('BC-1', deps);
    await commitHeldSetAside('BC-1', deps);

    assert.deepEqual(calls, ['9001', '9001']);
    assert.deepEqual(toasts, [TRANSIT.fr, TRANSIT.fr]);
    assertNoIdentity(toasts.join(' '));
  });

  it('does not post again while the first set-aside is still pending', async () => {
    let release: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const calls: number[] = [];
    const first = commitHeldSetAside('BC-LOCK', {
      holdId: '55',
      itemId: '77',
      returnSetAside: async () => {
        calls.push(1);
        await gate;
        return poisonedReady({ transitId: '9001', destinationSiteName: 'Nord', status: 'in_transit' });
      },
      successMessage: () => 'ok',
      errorMessage: () => 'err',
      showToast: () => undefined,
      closeDialog: () => undefined,
      clearBarcode: () => undefined,
      refocusBarcode: () => undefined,
    });
    const second = commitHeldSetAside('BC-LOCK', {
      holdId: '55',
      itemId: '77',
      returnSetAside: async () => {
        calls.push(2);
        return poisonedReady();
      },
      successMessage: () => 'again',
      errorMessage: () => 'err',
      showToast: () => undefined,
      closeDialog: () => undefined,
      clearBarcode: () => undefined,
      refocusBarcode: () => undefined,
    });
    await Promise.resolve();
    assert.deepEqual(calls, [1]);
    release?.();
    await first;
    await second;
    assert.deepEqual(calls, [1]);
  });

  it('looks up the copy id when the scan did not keep one', async () => {
    const calls: Array<{ holdId: string; itemId: string }> = [];
    await commitHeldSetAside('BC-2', {
      holdId: ' 55 ',
      itemId: null,
      resolveItemId: async (code) => (code === 'BC-2' ? '77' : null),
      returnSetAside: async (_code, body) => {
        calls.push(body);
        return poisonedReady();
      },
      successMessage: () => 'ok',
      errorMessage: () => 'err',
      showToast: () => undefined,
      closeDialog: () => undefined,
      clearBarcode: () => undefined,
      refocusBarcode: () => undefined,
    });
    assert.deepEqual(calls, [{ holdId: '55', itemId: '77' }]);
  });

  it('does not call the return when holdId or itemId is missing', async () => {
    const calls: string[] = [];
    const toasts: string[] = [];
    const t = await tFor('fr');
    let dialogOpen = true;

    await commitHeldSetAside('BC-3', {
      holdId: null,
      itemId: '77',
      returnSetAside: async () => {
        calls.push('posted');
        return poisonedReady();
      },
      successMessage: () => 'ok',
      errorMessage: (error) => heldSetAsideServerMessage(error, t),
      showToast: (toast) => {
        toasts.push(toast.message);
      },
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => undefined,
      refocusBarcode: () => undefined,
    });

    assert.deepEqual(calls, []);
    assert.equal(dialogOpen, true);
    assert.deepEqual(toasts, [catalogs.fr.errors.generic]);
    assertNoIdentity(toasts.join(' '));
  });

  it('maps the three 422 codes and hides any other reader', async () => {
    for (const lang of Object.keys(CODES.set_aside_not_oldest) as Lang[]) {
      const t = await tFor(lang);
      for (const code of Object.keys(CODES) as (keyof typeof CODES)[]) {
        const message = heldSetAsideServerMessage(refusal(code), t);
        assert.equal(message, CODES[code][lang], `${lang} ${code}`);
        assertNoIdentity(message);
        assert.equal(message.toLowerCase().includes('spécimen'), false);
        assert.equal(message.toLowerCase().includes('specimen'), false);
        assert.equal(message.toLowerCase().includes('usager'), false);
        assert.equal(message.toLowerCase().includes('patron'), false);
      }
      const unknown = heldSetAsideServerMessage(
        refusal('set_aside_mystery', { message: `held for ${NAME}` }),
        t,
      );
      assert.equal(unknown, catalogs[lang].errors.generic);
      assertNoIdentity(unknown);
    }
    assert.match(CODES.set_aside_not_oldest.fr, /Rescannez/);
    assert.match(CODES.set_aside_not_oldest.en, /Scan/);
  });

  it('stays on the dialog when the return is refused', async () => {
    const t = await tFor('en');
    const toasts: string[] = [];
    let dialogOpen = true;
    let barcode = 'BC-9';

    await commitHeldSetAside('BC-9', {
      holdId: '55',
      itemId: '77',
      returnSetAside: async () => {
        throw refusal('set_aside_not_oldest');
      },
      successMessage: (result) => heldSetAsideSuccessMessage(result, t),
      errorMessage: (error) => heldSetAsideServerMessage(error, t),
      showToast: (toast) => {
        toasts.push(toast.message);
      },
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => {
        barcode = '';
      },
      refocusBarcode: () => undefined,
    });

    assert.equal(dialogOpen, true);
    assert.equal(barcode, 'BC-9');
    assert.deepEqual(toasts, [CODES.set_aside_not_oldest.en]);
    assertNoIdentity(toasts.join(' '));
  });

  it('cancel closes and refocuses without calling the API or clearing the barcode', () => {
    const calls: string[] = [];
    let dialogOpen = true;
    let barcode = 'BC-1';
    let focused = false;

    commitDismissHeldSetAside({
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => {
        barcode = '';
      },
      refocusBarcode: () => {
        focused = true;
      },
    });

    assert.deepEqual(calls, []);
    assert.equal(dialogOpen, false);
    assert.equal(barcode, 'BC-1');
    assert.equal(focused, true);
  });

  it('uses the ready and transit sentences in every language', async () => {
    for (const lang of Object.keys(READY) as Lang[]) {
      const t = await tFor(lang);
      const ready = heldSetAsideSuccessMessage(poisonedReady(), t);
      const transit = heldSetAsideSuccessMessage(
        poisonedReady({
          status: 'in_transit',
          holdStatus: 'pending',
          transitId: '9001',
          destinationSiteName: 'Nord',
        }),
        t,
      );
      const unnamed = heldSetAsideSuccessMessage(
        poisonedReady({ transitId: '9001', destinationSiteName: '   ' }),
        t,
      );
      assert.equal(ready, READY[lang]);
      assert.equal(transit, TRANSIT[lang]);
      assert.equal(unnamed.includes('Nord'), false);
      assert.equal(unnamed.includes(catalogs[lang].loans.pickSiteUnknown), true);
      for (const sentence of [ready, transit, unnamed]) {
        assertNoIdentity(sentence);
        assert.equal(sentence.includes('{{'), false);
        assert.equal(sentence.toLowerCase().includes('spécimen'), false);
        assert.equal(sentence.toLowerCase().includes('specimen'), false);
        assert.equal(sentence.toLowerCase().includes('usager'), false);
        assert.equal(sentence.toLowerCase().includes('patron'), false);
      }
    }
  });

  it('desk set-aside posts the return body', () => {
    const loans = readFileSync(fileURLToPath(new URL('../pages/LoansPage.tsx', import.meta.url)), 'utf8');
    const user = readFileSync(fileURLToPath(new URL('../pages/UserDetailPage.tsx', import.meta.url)), 'utf8');
    const batch = readFileSync(
      fileURLToPath(new URL('../components/loans/BatchCreateItemRow.tsx', import.meta.url)),
      'utf8',
    );
    const posted = /returnLoanByBarcode\([^)]+,\s*\{ holdId, itemId \}\)/;
    assert.match(loans, posted);
    assert.match(user, posted);
    assert.match(batch, posted);
    assert.match(loans, /heldForOtherHoldId\(/);
    assert.match(user, /heldForOtherHoldId\(/);
    assert.match(loans, /isLoading=\{toPickAsideLoading\}/);
    assert.match(user, /isLoading=\{toPickForceLoading\}/);
    assert.match(batch, /isLoading=\{asidePending\}/);
  });
});
