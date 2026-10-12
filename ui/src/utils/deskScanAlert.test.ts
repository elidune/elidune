import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  CheckoutScanNoticeShown,
  checkoutScanNoticeFromRefusal,
  deskScanAlert,
  deskScanAlertFromCheckIn,
  deskScanAlertFromCheckoutMessage,
  deskScanAlertMessage,
  deskScanNoticeAction,
  isCheckoutScanNoticeShown,
  otherBorrowerFromPayload,
  type OtherBorrowerRef,
} from './deskScanAlert.ts';

const catalogs = { fr, en, de, es } as const;

function lookup(catalog: unknown, key: string): string | null {
  const parts = key.split('.');
  let cursor: unknown = catalog;
  for (const part of parts) {
    if (!cursor || typeof cursor !== 'object' || !(part in cursor)) return null;
    cursor = (cursor as Record<string, unknown>)[part];
  }
  return typeof cursor === 'string' ? cursor : null;
}

function translate(lang: keyof typeof catalogs) {
  const catalog = catalogs[lang];
  return (key: string, values?: Record<string, unknown>) => {
    const template = lookup(catalog, key) ?? key;
    return template.replace(/\{\{(\w+)\}\}/g, (_, name: string) => String(values?.[name] ?? ''));
  };
}

describe('desk scan alert', () => {
  it('warns when the copy is on loan to someone else or on hold', () => {
    assert.deepEqual(
      deskScanAlert(
        { availability: 'onLoan', loanUserId: '2', loanUserName: 'Ada Lovelace' },
        { patronId: '1' },
      ),
      { kind: 'onLoan', patronName: 'Ada Lovelace' },
    );
    assert.equal(
      deskScanAlert(
        { availability: 'onLoan', loanUserId: '1', loanUserName: 'Ada Lovelace' },
        { patronId: '1' },
      ),
      null,
    );
    assert.deepEqual(
      deskScanAlert({ availability: 'onHold', holdUserId: '9', holdUserName: 'Grace Hopper' }, {}),
      { kind: 'onHold', patronName: 'Grace Hopper' },
    );
    assert.equal(
      deskScanAlert(
        { availability: 'onHold', holdUserId: '1', holdUserName: 'Grace Hopper' },
        { patronId: '1' },
      ),
      null,
    );
    assert.equal(deskScanAlert({ availability: 'available' }, { patronId: '1' }), null);
    assert.equal(deskScanAlert(null, {}), null);
  });

  it('reads a check-in body and the checkout refusal text', () => {
    assert.deepEqual(
      deskScanAlertFromCheckIn({
        loan: { user: { id: '4', firstname: 'Marie', lastname: 'Curie' } },
      }),
      { kind: 'onLoan', patronName: 'Marie Curie' },
    );
    assert.deepEqual(
      deskScanAlertFromCheckIn({ hold: { userId: '5', user: { firstname: 'Alan', lastname: 'Turing' } } }),
      { kind: 'onHold', patronName: 'Alan Turing' },
    );
    assert.equal(deskScanAlertFromCheckIn({}), null);
    assert.deepEqual(deskScanAlertFromCheckoutMessage('Item is already borrowed'), {
      kind: 'onLoan',
      patronName: null,
    });
    assert.deepEqual(
      deskScanAlertFromCheckoutMessage(
        'This copy has an active hold for another patron — only the queued patron may borrow it, or use force=true to override',
      ),
      { kind: 'onHold', patronName: null },
    );
  });

  it('asks for the return first when the copy is on loan, and never offers a loan', () => {
    const onLoan = { kind: 'onLoan' as const, patronName: 'Ada Lovelace' };
    const onHold = { kind: 'onHold' as const, patronName: 'Grace Hopper' };
    assert.equal(deskScanNoticeAction(onLoan), 'return');
    assert.equal(deskScanNoticeAction(onLoan, 'return'), null);
    assert.equal(deskScanNoticeAction(onHold), null);
    assert.equal(deskScanNoticeAction(onHold, 'checkout'), null);

    assert.equal(
      deskScanAlertMessage(onLoan, translate('fr')),
      "Cet exemplaire est encore en prêt au lecteur AL. Faites d'abord le retour.",
    );
    assert.equal(deskScanAlertMessage(onLoan, translate('fr')).includes('Ada Lovelace'), false);
    assert.equal(
      deskScanAlertMessage({ kind: 'onLoan', patronName: null }, translate('fr')),
      "Cet exemplaire est encore en prêt à un autre lecteur. Faites d'abord le retour.",
    );
    assert.equal(
      deskScanAlertMessage(onLoan, translate('en')),
      'This copy is still on loan to the reader AL. Process the return first.',
    );
    assert.equal(
      deskScanAlertMessage(onLoan, translate('de')),
      'Dieses Exemplar ist noch an den Leser AL ausgeliehen. Erfassen Sie zuerst die Rückgabe.',
    );
    assert.equal(
      deskScanAlertMessage(onLoan, translate('es')),
      'Este ejemplar sigue en préstamo al lector AL. Registre primero la devolución.',
    );
    assert.equal(
      deskScanAlertMessage(onLoan, translate('fr'), 'return'),
      'Cet exemplaire est en prêt à Ada Lovelace.',
    );

    assert.equal(
      deskScanAlertMessage(onHold, translate('fr')),
      'Cet exemplaire est réservé pour Grace Hopper.',
    );
    assert.equal(
      deskScanAlertMessage({ kind: 'onHold', patronName: null }, translate('fr')),
      'Cet exemplaire est réservé pour un autre lecteur.',
    );

    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const loans = catalogs[lang].loans as Record<string, unknown>;
      assert.equal('scanAlertForce' in loans, false);
      const checkout = deskScanAlertMessage(onLoan, translate(lang));
      const hold = deskScanAlertMessage(onHold, translate(lang));
      assert.equal(checkout.includes('force'), false);
      assert.equal(hold.includes('force'), false);
      assert.equal(checkout.includes('{{'), false);
      assert.equal(lookup(catalogs[lang], 'loans.scanAlertGoToReturn')?.includes('{{'), false);
      assert.equal(lookup(catalogs[lang], 'errors.apiCode.item_to_pick_for_other')?.toLowerCase().includes('force'), false);
    }
  });

  it('keeps the generic business-rule error only when no specific notice applies', () => {
    const holdMessage =
      'This copy has an active hold for another patron — only the queued patron may borrow it, or use force=true to override';
    assert.deepEqual(
      checkoutScanNoticeFromRefusal({ code: 'business_rule_violation', message: holdMessage }),
      { kind: 'onHold', patronName: null },
    );
    assert.deepEqual(
      checkoutScanNoticeFromRefusal({
        code: 'item_to_pick_for_other',
        message: 'This copy is on the pick list for another patron — use force=true to override',
        patronName: 'Grace Hopper',
      }),
      { kind: 'onHold', patronName: 'Grace Hopper' },
    );
    assert.deepEqual(
      checkoutScanNoticeFromRefusal({
        code: 'business_rule_violation',
        message: 'Item is already borrowed',
      }),
      { kind: 'onLoan', patronName: null },
    );
    assert.equal(
      checkoutScanNoticeFromRefusal({
        code: 'business_rule_violation',
        message: 'User subscription expired on 2026-01-01 — use force=true to override',
      }),
      null,
    );
    assert.equal(
      checkoutScanNoticeFromRefusal({
        code: 'business_rule_violation',
        message: 'Maximum loans for this document type reached (3/3)',
      }),
      null,
    );
    const shown = new CheckoutScanNoticeShown();
    assert.equal(isCheckoutScanNoticeShown(shown), true);
    assert.equal(isCheckoutScanNoticeShown(new Error('Action refusée par une règle métier')), false);
  });
});

describe('loan borrower identifier shapes', () => {
  function checkoutSentence(ref: OtherBorrowerRef | null): string {
    return deskScanAlertMessage(
      { kind: 'onLoan', patronName: 'Julien Dubois', otherBorrower: ref },
      translate('en'),
    );
  }

  it('shows the card from the minimized borrower shape', () => {
    const ref = otherBorrowerFromPayload({
      id: '42',
      cardNumber: 'L-10042',
      barcode: 'BC-9',
      initials: 'JD',
    });
    assert.deepEqual(ref, { userId: '42', cardNumber: 'L-10042', initials: 'JD' });
    const sentence = checkoutSentence(ref);
    assert.equal(
      sentence,
      'This copy is still on loan to the reader (library card no. L-10042). Process the return first.',
    );
    assert.equal(sentence.includes('Julien Dubois'), false);
    assert.equal(sentence.includes('BC-9'), false);
  });

  it('uses server initials when cardNumber is null, without re-deriving or adding dots', () => {
    const ref = otherBorrowerFromPayload({
      id: '42',
      cardNumber: null,
      initials: 'JD',
      firstname: 'Ada',
      lastname: 'Lovelace',
    });
    assert.deepEqual(ref, { userId: '42', cardNumber: null, initials: 'JD' });
    const dotted = otherBorrowerFromPayload({ id: 42, cardNumber: null, initials: 'J.D.' });
    assert.equal(dotted?.initials, 'JD');
    assert.equal(dotted?.initials?.includes('.'), false);
    const sentence = checkoutSentence(ref);
    assert.equal(sentence, 'This copy is still on loan to the reader JD. Process the return first.');
    assert.equal(sentence.includes('Ada'), false);
    assert.equal(sentence.includes('Lovelace'), false);
    assert.equal(sentence.includes('J.D.'), false);
  });

  it('derives the card and initials from the legacy barcode and name fields', () => {
    const ref = otherBorrowerFromPayload({
      id: '42',
      barcode: 'BC-9',
      firstname: 'Julien',
      lastname: 'Dubois',
    });
    assert.deepEqual(ref, { userId: '42', cardNumber: 'BC-9', initials: 'JD' });
    const blob = JSON.stringify(ref);
    assert.equal(blob.includes('Julien'), false);
    assert.equal(blob.includes('Dubois'), false);
    const sentence = checkoutSentence(ref);
    assert.equal(
      sentence,
      'This copy is still on loan to the reader (library card no. BC-9). Process the return first.',
    );
    assert.equal(sentence.includes('Julien'), false);
    assert.equal(sentence.includes('Dubois'), false);
  });

  it('calls a reader card a card number in every locale, not a barcode', () => {
    const ref = otherBorrowerFromPayload({ id: '42', cardNumber: 'CARTE-0002', initials: 'JD' });
    const expected = {
      fr: "Cet exemplaire est encore en prêt au lecteur carte n° CARTE-0002. Faites d'abord le retour.",
      en: 'This copy is still on loan to the reader (library card no. CARTE-0002). Process the return first.',
      de: 'Dieses Exemplar ist noch an den Leser (Ausweis-Nr. CARTE-0002) ausgeliehen. Erfassen Sie zuerst die Rückgabe.',
      es: 'Este ejemplar sigue en préstamo al lector (carné n.º CARTE-0002). Registre primero la devolución.',
    } as const;
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const sentence = deskScanAlertMessage(
        { kind: 'onLoan', patronName: 'Julien Dubois', otherBorrower: ref },
        translate(lang),
      );
      assert.equal(sentence, expected[lang]);
      assert.equal(/code-barres|barcode|código de barras/i.test(sentence), false);
      assert.equal(sentence.includes('Julien Dubois'), false);
    }
  });
});

describe('item_not_on_loan', () => {
  it('stores the check-in 404 sentence under errors.apiCode in every locale', () => {
    assert.equal(fr.errors.apiCode.item_not_on_loan, "Cet exemplaire n'est pas en prêt");
    assert.equal(en.errors.apiCode.item_not_on_loan, 'This copy is not on loan');
    assert.equal(de.errors.apiCode.item_not_on_loan, 'Dieses Exemplar ist nicht ausgeliehen');
    assert.equal(es.errors.apiCode.item_not_on_loan, 'Este ejemplar no está en préstamo');
    for (const catalog of [fr, en, de, es]) {
      assert.equal(catalog.errors.apiCode.item_not_on_loan.includes('Item '), false);
    }
  });
});
