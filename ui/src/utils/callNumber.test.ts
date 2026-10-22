import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  authorMarkFromAuthors,
  buildCallNumberOptions,
  buildSuggestedCallNumber,
  buildSuggestedCallNumberFromBiblio,
  genreShelfCode,
} from './callNumber.ts';

/** Seed-aligned shelf map (mirrors GET /genres defaults). */
const SEED_SHELF: Record<string, string> = {
  litteratureFiction: 'ROM',
  litteratureComic: 'BD',
  litteratureTheatre: 'THE',
  litteraturePoem: 'POE',
};

describe('authorMarkFromAuthors', () => {
  it('uses lastname of the preferred author function', () => {
    assert.equal(
      authorMarkFromAuthors([
        { lastname: 'Illustrator', function: 'illustrator' },
        { lastname: 'Balzac', function: 'author' },
      ]),
      'BAL',
    );
  });

  it('falls back to first author when no author function', () => {
    assert.equal(authorMarkFromAuthors([{ lastname: 'Hugo' }]), 'HUG');
  });
});

describe('genreShelfCode', () => {
  it('maps fiction to ROM from injected catalog', () => {
    assert.equal(genreShelfCode('litteratureFiction', SEED_SHELF), 'ROM');
  });

  it('returns null without map or unknown key', () => {
    assert.equal(genreShelfCode('litteratureFiction'), null);
    assert.equal(genreShelfCode('unknown', SEED_SHELF), null);
    assert.equal(genreShelfCode(null, SEED_SHELF), null);
  });
});

describe('buildCallNumberOptions', () => {
  it('returns genre, dewey, and legacy when all inputs present (cote before Dewey)', () => {
    const options = buildCallNumberOptions({
      dewey: '843.914',
      genre: 'litteratureFiction',
      authors: [{ lastname: 'Balzac', function: 'author' }],
      mediaType: 'printedText',
      publicationDate: '2024',
      genreShelfCodes: SEED_SHELF,
    });
    assert.deepEqual(
      options.map((o) => o.kind),
      ['genre', 'dewey', 'legacy'],
    );
    assert.equal(options[0]!.value, 'ROM BAL');
    assert.equal(options[1]!.value, '843.914 BAL');
    assert.equal(options[2]!.value, 'PRIN-2024-BAL');
  });

  it('omits dewey when absent', () => {
    const options = buildCallNumberOptions({
      dewey: '',
      genre: 'litteratureFiction',
      authors: [{ lastname: 'Balzac' }],
      mediaType: 'printedText',
      publicationDate: '2024',
      genreShelfCodes: SEED_SHELF,
    });
    assert.deepEqual(
      options.map((o) => o.kind),
      ['genre', 'legacy'],
    );
    assert.equal(options[0]!.value, 'ROM BAL');
  });

  it('omits genre without shelf code and keeps legacy', () => {
    const options = buildCallNumberOptions({
      genre: 'unknown',
      mediaType: 'printedText',
      publicationDate: '2020',
      authors: [{ lastname: 'Dupont' }],
      genreShelfCodes: SEED_SHELF,
    });
    assert.deepEqual(
      options.map((o) => o.kind),
      ['legacy'],
    );
    assert.equal(options[0]!.value, 'PRIN-2020-DUP');
  });
});

describe('buildSuggestedCallNumberFromBiblio', () => {
  it('prefers genre shelf (cote) over Dewey when both present', () => {
    assert.equal(
      buildSuggestedCallNumberFromBiblio({
        dewey: '843.914',
        genre: 'litteratureFiction',
        authors: [{ lastname: 'Balzac', function: 'author' }],
        genreShelfCodes: SEED_SHELF,
      }),
      'ROM BAL',
    );
  });

  it('uses Dewey when genre shelf code is absent', () => {
    assert.equal(
      buildSuggestedCallNumberFromBiblio({
        dewey: '843.914',
        genre: '',
        authors: [{ lastname: 'Balzac' }],
        genreShelfCodes: SEED_SHELF,
      }),
      '843.914 BAL',
    );
  });

  it('uses genre shelf code when Dewey is absent', () => {
    assert.equal(
      buildSuggestedCallNumberFromBiblio({
        dewey: '',
        genre: 'litteratureFiction',
        authors: [{ lastname: 'Balzac' }],
        genreShelfCodes: SEED_SHELF,
      }),
      'ROM BAL',
    );
  });

  it('falls back to legacy CATEGORY-YEAR-XXX without dewey or genre', () => {
    const result = buildSuggestedCallNumberFromBiblio({
      mediaType: 'printedText',
      publicationDate: '2024',
      authors: [{ lastname: 'Smith' }],
      genreShelfCodes: SEED_SHELF,
    });
    assert.equal(result, buildSuggestedCallNumber({
      categoryCode: 'PRIN',
      year: 2024,
      authorOrCollectorName: 'Smith',
    }));
    assert.equal(result, 'PRIN-2024-SMI');
  });

  it('falls back to legacy when genre has no shelf code', () => {
    const result = buildSuggestedCallNumberFromBiblio({
      genre: 'unknown',
      mediaType: 'printedText',
      publicationDate: '2020',
      authors: [{ lastname: 'Dupont' }],
      genreShelfCodes: SEED_SHELF,
    });
    assert.equal(result, 'PRIN-2020-DUP');
  });
});
