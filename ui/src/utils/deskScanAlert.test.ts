import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  deskScanAlert,
  deskScanAlertFromCheckIn,
  deskScanAlertFromCheckoutMessage,
  deskScanAlertMessage,
} from './deskScanAlert.ts';

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

  it('names the other reader when the lookup has one', () => {
    const t = (key: string, values?: Record<string, unknown>) => {
      const parts = key.split('.');
      let cursor: unknown = fr;
      for (const part of parts) {
        if (!cursor || typeof cursor !== 'object' || !(part in cursor)) return key;
        cursor = (cursor as Record<string, unknown>)[part];
      }
      const template = typeof cursor === 'string' ? cursor : key;
      return template.replace(/\{\{(\w+)\}\}/g, (_, name: string) => String(values?.[name] ?? ''));
    };
    assert.equal(
      deskScanAlertMessage({ kind: 'onLoan', patronName: 'Ada Lovelace' }, t),
      'Cet exemplaire est en prêt à Ada Lovelace.',
    );
    assert.equal(
      deskScanAlertMessage({ kind: 'onHold', patronName: null }, t),
      'Cet exemplaire est réservé pour un autre lecteur.',
    );
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
