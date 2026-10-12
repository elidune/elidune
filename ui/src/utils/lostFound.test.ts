import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  LOST_FOUND_DISPOSITIONS,
  classifyLostFoundError,
  formatCentsAsEuros,
  isLostFoundAlreadyProcessed,
  isLostFoundInvalidAction,
  isLostReturn,
  itemIdFromCopies,
  itemIdFromLostReturn,
  lostFeeKind,
  lostFoundChoiceHint,
  lostFoundErrorMessage,
  lostFoundFeeMessage,
  lostFoundPanelFee,
  lostFoundResolutionMessage,
  type Translate,
} from './lostFound.ts';

const catalogs = { en, fr, de, es } as const;
type Lang = keyof typeof catalogs;

function keysOf(value: unknown, prefix = ''): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return prefix ? [prefix] : [];
  return Object.entries(value).flatMap(([key, child]) => keysOf(child, prefix ? `${prefix}.${key}` : key));
}

function lookup(catalog: unknown, key: string): string | null {
  const parts = key.split('.');
  let cursor: unknown = catalog;
  for (const part of parts) {
    if (!cursor || typeof cursor !== 'object' || !(part in cursor)) return null;
    cursor = (cursor as Record<string, unknown>)[part];
  }
  return typeof cursor === 'string' ? cursor : null;
}

/** Same CLDR categories i18next uses (`_one`, `_other`, …). */
function translate(lang: Lang): Translate {
  const catalog = catalogs[lang];
  const rules = new Intl.PluralRules(lang);
  return (key, values) => {
    const count = values?.count;
    const resolved =
      typeof count === 'number' ? lookup(catalog, `${key}_${rules.select(count)}`) : null;
    const template = resolved ?? lookup(catalog, key) ?? key;
    return template.replace(/\{\{(\w+)\}\}/g, (_, name: string) => {
      const value = values?.[name];
      return value == null ? '' : String(value);
    });
  };
}

describe('lost return notice', () => {
  it('opens only when wasLost is true', () => {
    assert.equal(isLostReturn({ wasLost: true }), true);
    assert.equal(isLostReturn({ wasLost: false }), false);
    assert.equal(isLostReturn({}), false);
    assert.equal(isLostReturn(null), false);
  });

  it('reads the item id from the loan, then the desk fallback, then the barcode lookup', () => {
    assert.equal(itemIdFromLostReturn({ loan: { itemId: '42' } }, '9'), '42');
    assert.equal(itemIdFromLostReturn({ loan: null }, '9'), '9');
    assert.equal(
      itemIdFromCopies(
        [
          { id: '1', barcode: 'AAA' },
          { id: '2', barcode: 'BBB' },
        ],
        'BBB',
      ),
      '2',
    );
  });
});

describe('replacement fee wording', () => {
  it('selects cancel, refund, partial, or none from the return fields', () => {
    assert.equal(lostFeeKind({ fineStatus: 'unpaid', fineAmount: 1500, paidAmount: 0 }), 'unpaid');
    assert.equal(lostFeeKind({ fineStatus: 'paid', fineAmount: 1500, paidAmount: 1500 }), 'paid');
    assert.equal(lostFeeKind({ fineStatus: 'partial', fineAmount: 1500, paidAmount: 500 }), 'partial');
    assert.equal(lostFeeKind({ wasLost: true }), 'none');
  });

  it('formats cents as euros in each UI locale', () => {
    assert.match(formatCentsAsEuros(1500, 'fr'), /15,00/);
    assert.match(formatCentsAsEuros(1500, 'fr'), /€/);
    assert.match(formatCentsAsEuros(1, 'fr'), /0,01/);
    assert.match(formatCentsAsEuros(1500, 'en'), /15\.00/);
    assert.match(formatCentsAsEuros(1500, 'de'), /15,00/);
    assert.match(formatCentsAsEuros(1500, 'es'), /15,00/);
  });

  it('states the fee outcome in French without a raw status code', () => {
    const t = translate('fr');
    assert.match(
      lostFoundFeeMessage({ fineStatus: 'unpaid', fineAmount: 1500, paidAmount: 0 }, 'fr', t),
      /annulés/,
    );
    assert.match(
      lostFoundFeeMessage({ fineStatus: 'paid', fineAmount: 1500, paidAmount: 1500 }, 'fr', t),
      /remboursé/,
    );
    const partial = lostFoundFeeMessage(
      { fineStatus: 'partial', fineAmount: 1500, paidAmount: 500 },
      'fr',
      t,
    );
    assert.match(partial, /remboursé/);
    assert.match(partial, /annulé/);
    assert.match(lostFoundFeeMessage({ wasLost: true }, 'fr', t), /Aucun frais/);
    assert.equal(lostFoundFeeMessage({ wasLost: true }, 'fr', t).includes('unpaid'), false);
  });

  it('uses one unchanged-fees label, and only when the copy is marked damaged', () => {
    const t = translate('fr');
    const unchanged = 'Les frais ne sont pas modifiés';
    assert.equal(t('loans.lostFound.feesUnchanged'), unchanged);
    const fields = { fineStatus: 'unpaid' as const, fineAmount: 1500, paidAmount: 0 };
    const damaged = lostFoundChoiceHint('damaged', fields, 'fr', t);
    assert.match(damaged, /Les frais ne sont pas modifiés/);
    assert.equal(damaged.includes('annulés'), false);
    assert.equal(damaged.includes('rembours'), false);

    const circulate = lostFoundChoiceHint('circulate', fields, 'fr', t);
    const leave = lostFoundChoiceHint('leave', fields, 'fr', t);
    assert.equal(circulate.includes(unchanged), false);
    assert.equal(leave.includes(unchanged), false);
    assert.match(circulate, /Les frais non payés sont annulés \(15,00/);
    assert.match(leave, /Les frais non payés sont annulés \(15,00/);
    assert.match(circulate, /€/);

    const paid = lostFoundChoiceHint(
      'circulate',
      { fineStatus: 'paid', fineAmount: 1500, paidAmount: 1500 },
      'fr',
      t,
    );
    assert.match(paid, /Les frais déjà payés seront remboursés \(15,00/);
    const partial = lostFoundChoiceHint(
      'leave',
      { fineStatus: 'partial', fineAmount: 1500, paidAmount: 500 },
      'fr',
      t,
    );
    assert.match(partial, /5,00/);
    assert.match(partial, /remboursés, le reste \(10,00/);
    assert.match(partial, /est annulé/);

    assert.equal(
      lostFoundFeeMessage({ fineStatus: 'unpaid' }, 'fr', t),
      'Les frais non payés sont annulés',
    );
    assert.equal(lostFoundPanelFee('damaged', fields, null, 'fr', t), unchanged);
    assert.equal(
      lostFoundPanelFee(
        'circulate',
        fields,
        {
          disposition: 'circulate',
          fineAction: 'refunded',
          refundedAmount: 1500,
          cancelledAmount: 0,
        },
        'fr',
        t,
      ).includes(unchanged),
      false,
    );
    assert.match(
      lostFoundResolutionMessage(
        {
          disposition: 'damaged',
          fineAction: 'refunded',
          refundedAmount: 1500,
          cancelledAmount: 0,
        },
        'fr',
        t,
      ),
      /Les frais ne sont pas modifiés/,
    );
    assert.equal(
      lostFoundResolutionMessage(
        { disposition: 'leave', fineAction: 'none', refundedAmount: 0, cancelledAmount: 0 },
        'fr',
        t,
      ).includes(unchanged),
      false,
    );

    for (const lang of ['en', 'de', 'es'] as const) {
      const label = translate(lang)('loans.lostFound.feesUnchanged');
      assert.equal(label.includes('{{'), false);
      assert.equal(
        lostFoundChoiceHint('damaged', fields, lang, translate(lang)).includes(label),
        true,
      );
      assert.equal(
        lostFoundChoiceHint('circulate', fields, lang, translate(lang)).includes(label),
        false,
      );
      assert.equal(
        lostFoundChoiceHint('leave', fields, lang, translate(lang)).includes(label),
        false,
      );
    }
  });
});

describe('lost-found errors', () => {
  it('treats a second resolve as already processed and hides the raw 422 text', () => {
    const input = {
      status: 422,
      code: 'business_rule_violation',
      message: 'Item is not marked lost',
    };
    assert.equal(isLostFoundAlreadyProcessed(input), true);
    assert.equal(isLostFoundInvalidAction(input), false);
    assert.deepEqual(classifyLostFoundError(input), { kind: 'alreadyProcessed' });
    assert.equal(lostFoundErrorMessage(input, translate('fr')), 'Ce retour a déjà été traité.');
    assert.equal(
      lostFoundErrorMessage(input, translate('en')),
      'This return has already been processed.',
    );
    assert.equal(lostFoundErrorMessage(input, translate('fr')).includes('Item is not marked lost'), false);
  });

  it('uses the same localized message for an invalid action on 400 and 422', () => {
    const badRequest = {
      status: 400,
      code: 'validation_error',
      message: 'unknown variant `shelf`, expected `circulate`',
    };
    const unprocessable = {
      status: 422,
      code: null,
      message: 'Failed to deserialize the JSON body into the target type',
    };
    assert.equal(isLostFoundInvalidAction(badRequest), true);
    assert.equal(isLostFoundInvalidAction(unprocessable), true);
    assert.equal(
      lostFoundErrorMessage(badRequest, translate('fr')),
      lostFoundErrorMessage(unprocessable, translate('fr')),
    );
    assert.equal(lostFoundErrorMessage(badRequest, translate('fr')), "Cette action n'est pas valide.");
    assert.equal(lostFoundErrorMessage(badRequest, translate('en')), 'This action is not valid.');
    assert.equal(lostFoundErrorMessage(unprocessable, translate('en')), 'This action is not valid.');
    assert.equal(lostFoundErrorMessage(badRequest, translate('fr')).includes('unknown variant'), false);
    assert.equal(
      lostFoundErrorMessage(unprocessable, translate('en')).includes('Failed to deserialize'),
      false,
    );
  });

  it('keeps a permission denial distinct from an invalid action', () => {
    const input = {
      status: 403,
      code: 'authorization_failed',
      message: 'Insufficient rights to write loans',
    };
    const message = lostFoundErrorMessage(input, translate('fr'));
    assert.equal(message, "Vous n'avez pas le droit de modifier les emprunts.");
    assert.equal(message.includes('Insufficient rights'), false);
  });
});

describe('lost-found locales', () => {
  it('keeps the same keys in fr, en, de, and es, including plural forms', () => {
    const expected = keysOf(fr.loans.lostFound).sort();
    assert.ok(expected.includes('alreadyProcessed'));
    assert.ok(expected.includes('invalidAction'));
    assert.ok(expected.includes('holdReady_one'));
    assert.ok(expected.includes('holdReady_other'));
    for (const catalog of [en, de, es]) {
      assert.deepEqual(keysOf(catalog.loans.lostFound).sort(), expected);
    }
  });

  it('resolves hold plurals for each language', () => {
    assert.equal(translate('fr')('loans.lostFound.holdReady', { count: 0 }), '0 réservation en attente est servie.');
    assert.equal(translate('fr')('loans.lostFound.holdReady', { count: 1 }), '1 réservation en attente est servie.');
    assert.equal(translate('fr')('loans.lostFound.holdReady', { count: 2 }), '2 réservations en attente sont servies.');
    assert.equal(translate('en')('loans.lostFound.holdReady', { count: 0 }), '0 waiting holds are served.');
    assert.equal(translate('en')('loans.lostFound.holdReady', { count: 1 }), '1 waiting hold is served.');
    assert.equal(translate('de')('loans.lostFound.holdReady', { count: 1 }), '1 wartende Vormerkung wird bedient.');
    assert.equal(translate('de')('loans.lostFound.holdReady', { count: 2 }), '2 wartende Vormerkungen werden bedient.');
    assert.equal(translate('es')('loans.lostFound.holdReady', { count: 1 }), 'Se sirve 1 reserva en espera.');
    assert.equal(translate('es')('loans.lostFound.holdReady', { count: 2 }), 'Se sirven 2 reservas en espera.');
  });

  it('posts the three dispositions and confirms a circulate refund', () => {
    assert.deepEqual([...LOST_FOUND_DISPOSITIONS], ['circulate', 'damaged', 'leave']);
    const message = lostFoundResolutionMessage(
      {
        disposition: 'circulate',
        fineAction: 'refunded',
        refundedAmount: 1500,
        cancelledAmount: 0,
        hold: { status: 'ready' },
      },
      'en',
      translate('en'),
    );
    assert.match(message, /returned to circulation/);
    assert.match(message, /refunded/);
    assert.match(message, /1 waiting hold is served/);
  });
});
