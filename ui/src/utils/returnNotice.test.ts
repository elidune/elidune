import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { ItemCheckIn } from '../types/index.ts';
import {
  returnCopyStatusLabel,
  returnDeskNotice,
  returnNoticeMessage,
  returnNoticeOffersLostFound,
  returnScanFeedback,
} from './returnNotice.ts';

async function i18nFor(lng: 'fr' | 'en' | 'de' | 'es'): Promise<I18nInstance> {
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
  return i18n;
}

function body(partial: Partial<ItemCheckIn>): ItemCheckIn {
  return { status: 'available', ...partial };
}

describe('return desk notice', () => {
  it('reads the informational and warning contracts', () => {
    assert.equal(returnDeskNotice(body({ status: 'returned', loan: {} as ItemCheckIn['loan'] })), null);
    assert.deepEqual(
      returnDeskNotice(
        body({
          status: 'available',
          notice: { code: 'item_not_on_loan', level: 'informational' },
        }),
      ),
      {
        level: 'informational',
        code: 'item_not_on_loan',
        circulationStatus: null,
        weedingStatus: null,
      },
    );
    assert.deepEqual(
      returnDeskNotice(
        body({
          status: 'unchanged',
          notice: { code: 'item_not_available_status', level: 'warning' },
          circulationStatus: 'lost',
          weedingStatus: 'onShelf',
        }),
      ),
      {
        level: 'warning',
        code: 'item_not_available_status',
        circulationStatus: 'lost',
        weedingStatus: 'onShelf',
      },
    );
    assert.equal(
      returnDeskNotice(
        body({
          notice: { code: 'item_not_on_loan', level: 'warning' },
          circulationStatus: 'damaged',
        }),
      )?.level,
      'warning',
    );
  });

  it('keeps a blocked scan off the success toast', () => {
    const blocked = body({
      status: 'unchanged',
      notice: { code: 'item_not_available_status', level: 'warning' },
      circulationStatus: 'lost',
      weedingStatus: 'onShelf',
    });
    assert.deepEqual(returnScanFeedback(blocked, { pick: false, hasLoan: false }), {
      toast: 'none',
      notice: {
        level: 'warning',
        code: 'item_not_available_status',
        circulationStatus: 'lost',
        weedingStatus: 'onShelf',
      },
      putBack: false,
    });
    const shelf = body({
      status: 'available',
      notice: { code: 'item_not_on_loan', level: 'informational' },
    });
    assert.equal(returnScanFeedback(shelf, { pick: false, hasLoan: false }).toast, 'info');
    assert.equal(returnScanFeedback(shelf, { pick: false, hasLoan: false }).putBack, true);
    const held = body({
      status: 'ready',
      notice: { code: 'item_not_on_loan', level: 'informational' },
    });
    assert.equal(returnScanFeedback(held, { pick: true, hasLoan: false }).toast, 'success');
    assert.equal(returnScanFeedback(held, { pick: true, hasLoan: false }).putBack, false);
    assert.equal(returnScanFeedback(body({ status: 'returned' }), { pick: false, hasLoan: true }).toast, 'success');
  });

  it('uses exemplaire and lecteur in French and the existing status label', async () => {
    const frI18n = await i18nFor('fr');
    const enI18n = await i18nFor('en');
    const tFr = frI18n.getFixedT('fr');
    const tEn = enI18n.getFixedT('en');
    const info = returnDeskNotice(
      body({ notice: { code: 'item_not_on_loan', level: 'informational' } }),
    );
    assert.ok(info);
    const shelf = returnNoticeMessage(info, tFr, { putBack: true });
    assert.equal(shelf, "Cet exemplaire n'est pas en prêt à un lecteur. Remettez-le en rayon.");
    assert.match(shelf, /exemplaire/);
    assert.match(shelf, /lecteur/);
    assert.doesNotMatch(shelf, /usager|spécimen|specimen|item/i);
    assert.equal(
      returnNoticeMessage(info, tEn, { putBack: true }),
      'This copy is not on loan to a reader. Put it back on the shelf.',
    );
    assert.equal(returnNoticeMessage(info, tFr, { putBack: false }).includes('rayon'), false);

    const lost = returnDeskNotice(
      body({
        status: 'unchanged',
        notice: { code: 'item_not_available_status', level: 'warning' },
        circulationStatus: 'lost',
        weedingStatus: 'onShelf',
      }),
    );
    assert.ok(lost);
    assert.equal(returnNoticeOffersLostFound(lost), true);
    const lostFr = returnNoticeMessage(lost, tFr);
    const lostEn = returnNoticeMessage(lost, tEn);
    assert.equal(
      lostFr,
      'Exemplaire déclaré perdu : utilisez « Retrouvé » pour le remettre en circulation.',
    );
    assert.equal(lostEn, 'Copy declared lost: use “Found” to put it back into circulation.');
    assert.equal(lostFr, fr.loans.returnNoticeLost);
    assert.equal(lostEn, en.loans.returnNoticeLost);
    assert.equal(lostFr.includes(fr.loans.returnNoticeFound), true);
    assert.equal(lostEn.includes(en.loans.returnNoticeFound), true);
    assert.equal(lostFr.includes(fr.loans.returnSuccess), false);
    assert.equal(lostEn.includes(en.loans.returnSuccess), false);
    assert.equal(lostFr.includes(fr.loans.returnNoticeBlocked.split('{{')[0] ?? ''), false);

    const deI18n = await i18nFor('de');
    const esI18n = await i18nFor('es');
    assert.equal(returnNoticeMessage(lost, deI18n.getFixedT('de')), de.loans.returnNoticeLost);
    assert.equal(returnNoticeMessage(lost, esI18n.getFixedT('es')), es.loans.returnNoticeLost);
    assert.equal(de.loans.returnNoticeLost.includes(de.loans.returnNoticeFound), true);
    assert.equal(es.loans.returnNoticeLost.includes(es.loans.returnNoticeFound), true);

    const cases = [
      ['damaged', 'onShelf', 'Abîmé', 'Damaged'],
      ['toSearch', 'onShelf', 'À rechercher', 'To search'],
      ['missing', 'onShelf', 'manquant', 'missing'],
      ['available', 'withdrawn', 'Sorti', 'Withdrawn'],
    ] as const;
    for (const [circulation, weeding, frStatus, enStatus] of cases) {
      const notice = returnDeskNotice(
        body({
          status: 'unchanged',
          notice: { level: 'warning', code: 'item_not_available_status' },
          circulationStatus: circulation,
          weedingStatus: weeding,
        }),
      );
      assert.ok(notice);
      assert.match(returnNoticeMessage(notice, tFr), new RegExp(frStatus));
      assert.match(returnNoticeMessage(notice, tEn), new RegExp(enStatus));
    }

    const lostAndWithdrawn = returnCopyStatusLabel('lost', 'withdrawn', tEn);
    assert.equal(lostAndWithdrawn, 'Lost');
    const notTheServerToken = {
      level: 'warning' as const,
      code: 'item_not_available_status' as const,
      circulationStatus: 'Lost',
      weedingStatus: 'onShelf',
    };
    assert.equal(returnNoticeOffersLostFound(notTheServerToken), false);
    assert.match(returnNoticeMessage(notTheServerToken, tEn), /Status: Unknown \(Lost\)\./);
    const shelfLost = returnDeskNotice(
      body({
        notice: { level: 'informational', code: 'item_not_on_loan' },
        circulationStatus: 'lost',
      }),
    );
    assert.ok(shelfLost);
    assert.equal(returnNoticeOffersLostFound(shelfLost), false);
    const damaged = returnDeskNotice(
      body({
        status: 'unchanged',
        notice: { level: 'warning', code: 'item_not_available_status' },
        circulationStatus: 'damaged',
      }),
    );
    assert.ok(damaged);
    assert.equal(returnNoticeOffersLostFound(damaged), false);
    const knownElsewhere = returnCopyStatusLabel('claimedReturned', 'onShelf', tEn);
    assert.equal(knownElsewhere, 'Claimed returned');
    const unknown = returnCopyStatusLabel('notAStatus', 'onShelf', tFr);
    assert.equal(unknown, 'Inconnu (notAStatus)');
    assert.equal(returnCopyStatusLabel(null, null, tEn), 'Unavailable');
  });
});
