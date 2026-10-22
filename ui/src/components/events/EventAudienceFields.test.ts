import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import EventAudienceFields from './EventAudienceFields.tsx';

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

function renderFields(
  i18n: I18nInstance,
  props: Parameters<typeof EventAudienceFields>[0],
): string {
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement(EventAudienceFields, props)) as ReactElement,
  );
}

const options = [
  { value: 'child', label: 'Child' },
  { value: 'adult', label: 'Adult' },
];

describe('EventAudienceFields', () => {
  it('shows the All option in the multiselect when all audiences is selected', async () => {
    const html = renderFields(await i18nFor('en'), {
      id: 'event-audience',
      selection: { allAudiences: true, publicTypes: ['child'] },
      options,
      onSelectionChange: () => undefined,
    });
    assert.equal(html.includes('type="checkbox"'), false);
    assert.match(html, />All</);
    assert.equal(html.includes('>Child<'), false);
    assert.equal(/id="event-audience"[^>]*disabled/.test(html), false);
  });

  it('keeps specific public types in the multiselect', async () => {
    const html = renderFields(await i18nFor('fr'), {
      id: 'event-audience',
      selection: { allAudiences: false, publicTypes: ['child'] },
      options,
      error: fr.events.targetPublicRequired,
      onSelectionChange: () => undefined,
    });
    assert.match(html, />Child</);
    assert.match(html, new RegExp(fr.events.targetPublicRequired));
    assert.equal(html.includes('type="checkbox"'), false);
  });

  it('renders localized All option and hint in German and Spanish', async () => {
    const deHtml = renderFields(await i18nFor('de'), {
      id: 'audience-de',
      selection: { allAudiences: false, publicTypes: [] },
      options,
      onSelectionChange: () => undefined,
    });
    const esHtml = renderFields(await i18nFor('es'), {
      id: 'audience-es',
      selection: { allAudiences: false, publicTypes: [] },
      options,
      onSelectionChange: () => undefined,
    });
    assert.match(deHtml, new RegExp(de.events.targetPublic.selectAll));
    assert.match(deHtml, new RegExp(de.events.targetPublicHint));
    assert.match(esHtml, new RegExp(es.events.targetPublic.selectAll));
    assert.match(esHtml, new RegExp(es.events.targetPublicHint));
  });
});
