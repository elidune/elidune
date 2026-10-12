import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { deskScanAlertMessage, deskScanNoticeAction } from './deskScanAlert.ts';
import {
  ITEM_ALREADY_ON_LOAN_TO_BORROWER,
  ITEM_ON_LOAN_TO_OTHER_BORROWER,
  checkoutForceFlag,
  checkoutMayForce,
  checkoutNoticeForConflict,
} from './loanConflict.ts';

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

/** English `message` strings from `POST /loans` 409 (`error` is always `"Conflict"`). */
const SERVER_MESSAGE: Record<string, string> = {
  [ITEM_ON_LOAN_TO_OTHER_BORROWER]: 'This item is on loan to another borrower.',
  [ITEM_ALREADY_ON_LOAN_TO_BORROWER]:
    'This item is already on loan to this borrower. Renew the existing loan instead of creating a new one.',
};

/** Mocked POST /loans response. `force` is the flag that was sent, if any. The body has no loan id. */
function mockLoanResponse(
  status: number,
  code: string,
  extra: Record<string, unknown> = {},
  force?: boolean,
) {
  return {
    isAxiosError: true,
    message: `Request failed with status code ${status}`,
    config: {
      method: 'post',
      url: '/loans',
      data: {
        userId: '1',
        itemIdentification: 'BC-100',
        ...(force === undefined ? {} : { force }),
      },
    },
    response: {
      status,
      statusText: status === 409 ? 'Conflict' : 'Unprocessable Entity',
      data: {
        code,
        error: status === 409 ? 'Conflict' : 'Unprocessable Entity',
        message: SERVER_MESSAGE[code] ?? 'Some other rule failed',
        ...extra,
      },
    },
  };
}

describe('checkout 409 when the copy is already on loan', () => {
  it('asks for the return when the copy is on loan to another reader, with or without force', () => {
    for (const force of [undefined, true, false] as const) {
      const error = mockLoanResponse(409, ITEM_ON_LOAN_TO_OTHER_BORROWER, {}, force);
      const notice = checkoutNoticeForConflict(error, {
        patronId: '1',
        snapshot: {
          availability: 'onLoan',
          loanId: 'loan-other',
          loanUserId: '2',
          loanUserName: 'Ada Lovelace',
        },
      });
      assert.deepEqual(notice, {
        kind: 'onLoan',
        patronName: null,
        otherBorrower: { userId: '2', cardNumber: null, initials: 'AL' },
        loanId: null,
      });
      assert.equal(JSON.stringify(notice).includes('Ada Lovelace'), false);
      assert.equal(deskScanNoticeAction(notice!), 'return');
      assert.equal(checkoutMayForce(error), false);
    }
    assert.equal(checkoutForceFlag('onLoan', true), undefined);
    const body = mockLoanResponse(409, ITEM_ON_LOAN_TO_OTHER_BORROWER).response.data;
    assert.equal(body.error, 'Conflict');
    assert.equal(body.message, SERVER_MESSAGE[ITEM_ON_LOAN_TO_OTHER_BORROWER]);
    assert.equal('loanId' in body, false);
  });

  it('asks to renew the same reader’s loan and never sends force', () => {
    const withId = mockLoanResponse(409, ITEM_ALREADY_ON_LOAN_TO_BORROWER, { loanId: 'loan-42' }, true);
    assert.deepEqual(
      checkoutNoticeForConflict(withId, { patronId: '1', snapshot: null }),
      { kind: 'alreadyOnLoan', patronName: null, loanId: 'loan-42' },
    );
    assert.equal(checkoutMayForce(withId), false);

    const fromSnapshot = mockLoanResponse(409, ITEM_ALREADY_ON_LOAN_TO_BORROWER);
    assert.deepEqual(
      checkoutNoticeForConflict(fromSnapshot, {
        patronId: '1',
        snapshot: {
          availability: 'onLoan',
          loanId: 'loan-snapshot',
          loanUserId: '1',
          loanUserName: 'Ada Lovelace',
        },
      }),
      { kind: 'alreadyOnLoan', patronName: null, loanId: 'loan-snapshot' },
    );

    const fromList = mockLoanResponse(409, ITEM_ALREADY_ON_LOAN_TO_BORROWER);
    assert.equal(
      checkoutNoticeForConflict(fromList, {
        patronId: '1',
        snapshot: { availability: 'onLoan', loanId: 'loan-other', loanUserId: '9' },
        listedLoanId: 'loan-listed',
      })?.loanId,
      'loan-listed',
    );

    const notice = checkoutNoticeForConflict(fromSnapshot, {
      patronId: '1',
      snapshot: { availability: 'onLoan', loanId: 'loan-snapshot', loanUserId: '1' },
    });
    assert.equal(deskScanNoticeAction(notice!), 'renew');
    assert.equal(checkoutForceFlag('onLoan', true), undefined);
    assert.equal(checkoutMayForce(mockLoanResponse(422, 'business_rule_violation')), true);
    assert.equal(checkoutNoticeForConflict(mockLoanResponse(409, 'conflict'), { patronId: '1', snapshot: null }), null);
    const sameBody = mockLoanResponse(409, ITEM_ALREADY_ON_LOAN_TO_BORROWER).response.data;
    assert.equal(sameBody.error, 'Conflict');
    assert.equal(sameBody.message, SERVER_MESSAGE[ITEM_ALREADY_ON_LOAN_TO_BORROWER]);
    assert.equal('loanId' in sameBody, false);
    assert.equal(
      checkoutNoticeForConflict(
        {
          barcode: 'BC-100',
          success: false,
          error: SERVER_MESSAGE[ITEM_ON_LOAN_TO_OTHER_BORROWER],
        },
        { patronId: '1', snapshot: null },
      ),
      null,
    );
  });

  it('stores both sentences in fr, en, de, and es', () => {
    assert.equal(
      fr.errors.apiCode.item_on_loan_to_other_borrower,
      "Cet exemplaire est encore en prêt à un autre lecteur. Faites d'abord le retour.",
    );
    assert.equal(
      fr.errors.apiCode.item_already_on_loan_to_borrower,
      'Cet exemplaire est déjà en prêt à ce lecteur. Vous pouvez le renouveler.',
    );
    assert.equal(
      en.errors.apiCode.item_on_loan_to_other_borrower,
      'This copy is still on loan to another reader. Process the return first.',
    );
    assert.equal(
      en.errors.apiCode.item_already_on_loan_to_borrower,
      'This copy is already on loan to this reader. You can renew it.',
    );
    assert.equal(
      de.errors.apiCode.item_on_loan_to_other_borrower,
      'Dieses Exemplar ist noch an einen anderen Leser ausgeliehen. Erfassen Sie zuerst die Rückgabe.',
    );
    assert.equal(
      de.errors.apiCode.item_already_on_loan_to_borrower,
      'Dieses Exemplar ist bereits an diesen Leser ausgeliehen. Sie können es verlängern.',
    );
    assert.equal(
      es.errors.apiCode.item_on_loan_to_other_borrower,
      'Este ejemplar sigue en préstamo a otro lector. Registre primero la devolución.',
    );
    assert.equal(
      es.errors.apiCode.item_already_on_loan_to_borrower,
      'Este ejemplar ya está en préstamo a este lector. Puede renovarlo.',
    );

    assert.equal(fr.loans.scanAlertRenew, 'Renouveler');
    assert.equal(en.loans.scanAlertRenew, 'Renew');
    assert.equal(de.loans.scanAlertRenew, 'Verlängern');
    assert.equal(es.loans.scanAlertRenew, 'Renovar');

    const renewNotice = { kind: 'alreadyOnLoan' as const, patronName: null };
    assert.equal(
      deskScanAlertMessage(renewNotice, translate('fr')),
      fr.errors.apiCode.item_already_on_loan_to_borrower,
    );
    assert.equal(
      deskScanAlertMessage({ kind: 'onLoan', patronName: null }, translate('fr')),
      fr.errors.apiCode.item_on_loan_to_other_borrower,
    );

    for (const catalog of [fr, en, de, es]) {
      const other = catalog.errors.apiCode.item_on_loan_to_other_borrower;
      const same = catalog.errors.apiCode.item_already_on_loan_to_borrower;
      const renew = catalog.loans.scanAlertRenew;
      for (const sentence of [other, same, renew]) {
        assert.equal(sentence.toLowerCase().includes('spécimen'), false);
        assert.equal(sentence.toLowerCase().includes('specimen'), false);
        assert.equal(sentence.includes('{{'), false);
      }
      assert.equal(deskScanNoticeAction(renewNotice), 'renew');
      assert.equal(
        deskScanAlertMessage(renewNotice, translate(
          catalog === fr ? 'fr' : catalog === en ? 'en' : catalog === de ? 'de' : 'es',
        )),
        same,
      );
    }
  });
});
