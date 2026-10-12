import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import OverdueDueDate from './OverdueDueDate.tsx';
import { OVERDUE_FACTS_CLASS, formatOverdueDueDate } from '../../utils/overdueDueDate.ts';

describe('overdue due date', () => {
  it('keeps the full localized date in a row that does not clip it', () => {
    const iso = '2026-10-02T12:00:00Z';
    const full = formatOverdueDueDate(iso, 'fr');
    assert.ok(full);
    assert.equal(full.includes('2026'), true);
    assert.equal(full.includes('t. 2026') && full.length <= 't. 2026'.length, false);
    assert.ok(full.length > 't. 2026'.length);

    const html = renderToStaticMarkup(
      createElement(
        'div',
        { className: OVERDUE_FACTS_CLASS },
        createElement(OverdueDueDate, { iso, locale: 'fr' }),
      ),
    );
    assert.equal(html.includes(full), true);
    assert.equal(html.split(full).length - 1, 1);
    assert.equal(OVERDUE_FACTS_CLASS.includes('flex-wrap'), true);
    assert.equal(OVERDUE_FACTS_CLASS.includes('overflow-x-auto'), false);
    assert.equal(OVERDUE_FACTS_CLASS.includes('flex-nowrap'), false);
    assert.equal(html.includes('whitespace-nowrap'), true);
  });
});
