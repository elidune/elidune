import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18n } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import PatronLoansOutcome from './PatronLoansOutcome.tsx';

async function i18nFor(lng: 'en' | 'fr'): Promise<I18n> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      en: { translation: en },
      fr: { translation: fr },
      de: { translation: de },
      es: { translation: es },
    },
  });
  return i18n;
}

function visibleText(html: string): string {
  return html
    .replace(/<[^>]+>/g, ' ')
    .replace(/&#x27;|&#39;|&apos;/g, "'")
    .replace(/\s+/g, ' ')
    .trim();
}

function renderOutcome(
  i18n: I18n,
  denied: boolean,
  itemCount: number,
): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
        createElement(PatronLoansOutcome, {
          denied,
          itemCount,
          emptyMessage: i18n.t('loans.noLoans'),
          children: createElement('p', null, 'Principia'),
        }),
    ) as ReactElement,
  );
}

describe('PatronLoansOutcome', () => {
  it('shows an access sentence for a 403 and hides loan data and the empty-list wording', async () => {
    const enI18n = await i18nFor('en');
    const enHtml = renderOutcome(enI18n, true, 1);
    const enText = visibleText(enHtml);
    assert.equal(enText.includes('You do not have access to this information.'), true);
    assert.equal(enText.includes('No active loans'), false);
    assert.equal(enText.includes('Principia'), false);
    assert.equal(enHtml.includes('role="status"'), true);
    assert.equal(enHtml.includes('role="alert"'), false);
    assert.equal(enHtml.includes('<table'), false);

    const frI18n = await i18nFor('fr');
    const frText = visibleText(renderOutcome(frI18n, true, 1));
    assert.equal(frText.includes("Vous n'avez pas accès à cette information."), true);
    assert.equal(frText.includes('Aucun emprunt en cours'), false);
    assert.equal(frText.includes('Principia'), false);
  });

  it('shows the empty-list wording only for a successful list with zero items', async () => {
    const enI18n = await i18nFor('en');
    const emptyText = visibleText(renderOutcome(enI18n, false, 0));
    assert.equal(emptyText.includes('No active loans'), true);
    assert.equal(emptyText.includes('You do not have access to this information.'), false);
    assert.equal(emptyText.includes('Principia'), false);

    const rowsText = visibleText(renderOutcome(enI18n, false, 1));
    assert.equal(rowsText.includes('Principia'), true);
    assert.equal(rowsText.includes('No active loans'), false);
    assert.equal(rowsText.includes('You do not have access to this information.'), false);
  });
});
