import assert from 'node:assert/strict';
import { before, describe, it } from 'node:test';
import i18next from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  copyAvailabilityLabel,
  formatCopyAvailability,
  staffCopyAvailabilityLabel,
} from './itemAvailability.ts';

describe('OPAC copy availability', () => {
  const i18n = i18next.createInstance();

  before(async () => {
    await i18n.init({
      lng: 'fr',
      fallbackLng: 'fr',
      resources: {
        en: { translation: en },
        fr: { translation: fr },
        de: { translation: de },
        es: { translation: es },
      },
      interpolation: { escapeValue: false },
    });
  });

  it('counts every copy, including ones that are not available', () => {
    const onLoan = { availability: 'onLoan' as const };
    const items = [onLoan, onLoan, { availability: 'available' as const }];
    assert.equal(copyAvailabilityLabel(i18n.getFixedT('fr'), items), '1 disponible sur 3 exemplaires');
    assert.equal(
      copyAvailabilityLabel(i18n.getFixedT('fr'), [onLoan, { availability: 'unavailable' }]),
      '0 disponible sur 2 exemplaires',
    );
    assert.notEqual(
      copyAvailabilityLabel(i18n.getFixedT('fr'), [onLoan, onLoan]),
      '0 exemplaire',
    );
    assert.equal(copyAvailabilityLabel(i18n.getFixedT('fr'), []), null);
    assert.equal(copyAvailabilityLabel(i18n.getFixedT('fr'), null), null);
  });

  it('pluralizes available-over-total in fr, en, de, and es', () => {
    const expected: Array<[string, number, number, string]> = [
      ['fr', 0, 3, '0 disponible sur 3 exemplaires'],
      ['fr', 1, 1, '1 disponible sur 1 exemplaire'],
      ['fr', 2, 5, '2 disponibles sur 5 exemplaires'],
      ['en', 0, 3, '0 available of 3 copies'],
      ['en', 1, 1, '1 available of 1 copy'],
      ['en', 2, 5, '2 available of 5 copies'],
      ['de', 0, 3, '0 verfügbar von 3 Exemplaren'],
      ['de', 1, 1, '1 verfügbar von 1 Exemplar'],
      ['de', 2, 5, '2 verfügbar von 5 Exemplaren'],
      ['es', 0, 3, '0 disponibles de 3 ejemplares'],
      ['es', 1, 1, '1 disponible de 1 ejemplar'],
      ['es', 2, 5, '2 disponibles de 5 ejemplares'],
    ];

    for (const [lng, available, total, text] of expected) {
      assert.equal(formatCopyAvailability(i18n.getFixedT(lng), available, total), text);
    }
  });

  it('does not report zero copies when the available count exceeds a missing total', () => {
    assert.equal(formatCopyAvailability(i18n.getFixedT('en'), 2, 0), '2 available of 2 copies');
    assert.equal(formatCopyAvailability(i18n.getFixedT('fr'), 2, 0), '2 disponibles sur 2 exemplaires');
  });

  it('keeps public unavailable copies in the total', () => {
    const items = [
      { availability: 'available', borrowable: true },
      { availability: 'unavailable', borrowable: false },
      { availability: 'onLoan', borrowable: true },
    ];
    assert.equal(copyAvailabilityLabel(i18n.getFixedT('fr'), items), '1 disponible sur 3 exemplaires');
    assert.equal(copyAvailabilityLabel(i18n.getFixedT('en'), items), '1 available of 3 copies');
  });

  it('breaks the staff total down by availability and weeding', () => {
    const items = [
      { availability: 'available', weedingStatus: 'onShelf' },
      { availability: 'onLoan', weedingStatus: 'candidate' },
      { availability: 'toSearch', weedingStatus: 'onShelf' },
      { availability: 'unavailable', weedingStatus: 'withdrawn' },
    ];
    assert.equal(
      staffCopyAvailabilityLabel(i18n.getFixedT('fr'), items),
      '4 exemplaires (1 disponible, 1 en prêt, 1 à rechercher, 1 indisponible), dont 1 candidat au désherbage, 1 sorti',
    );
    assert.equal(
      staffCopyAvailabilityLabel(i18n.getFixedT('en'), items),
      '4 copies (1 available, 1 on loan, 1 to search, 1 unavailable), including 1 weeding candidate, 1 withdrawn',
    );
    assert.equal(
      staffCopyAvailabilityLabel(i18n.getFixedT('de'), items),
      '4 Exemplare (1 verfügbar, 1 ausgeliehen, 1 zu suchen, 1 nicht verfügbar), davon 1 Aussonderungskandidat, 1 ausgeschieden',
    );
    assert.equal(
      staffCopyAvailabilityLabel(i18n.getFixedT('es'), items),
      '4 ejemplares (1 disponible, 1 en préstamo, 1 por buscar, 1 no disponible), de los cuales 1 candidato a expurgo, 1 retirado',
    );
    assert.equal(staffCopyAvailabilityLabel(i18n.getFixedT('fr'), []), null);
  });
});
