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
import { formatUserShortName, patronOptionSecondary } from '@/utils/userDisplay';
import PatronOptionLabel from './PatronOptionLabel.tsx';

type Lang = 'fr' | 'en' | 'de' | 'es';

async function i18nFor(lng: Lang): Promise<I18nInstance> {
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

function render(i18n: I18nInstance, user: { firstname: string; lastname: string; login: string }): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(PatronOptionLabel, { user }),
    ) as ReactElement,
  );
}

function nameSlot(html: string): string {
  const match = /data-patron-name[^>]*>([^<]*)</.exec(html);
  return match?.[1] ?? '';
}

describe('patron picker label', () => {
  it('keeps the sign-in off the name', () => {
    const paul = { firstname: 'Paul', lastname: 'Lecteur', login: 'lecteur3', id: '3' };
    const gilles = { firstname: 'Gilles', lastname: 'Tuteur', login: 'tuteur5', id: '5' };
    assert.equal(formatUserShortName(paul), 'Paul Lecteur');
    assert.equal(patronOptionSecondary(paul), 'lecteur3');
    assert.equal(formatUserShortName(gilles), 'Gilles Tuteur');
    assert.equal(patronOptionSecondary(gilles), 'tuteur5');
    assert.equal(`${formatUserShortName(paul)}${paul.id}`, 'Paul Lecteur3');
    assert.notEqual(formatUserShortName(paul), 'Paul Lecteur3');
  });

  it('renders the name and the login in separate slots with a gap', async () => {
    const labels = {
      fr: 'Identifiant',
      en: 'Username',
      de: 'Benutzername',
      es: 'Nombre de usuario',
    } as const;

    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const html = render(await i18nFor(lang), {
        firstname: 'Paul',
        lastname: 'Lecteur',
        login: 'lecteur3',
      });
      const name = nameSlot(html);
      assert.equal(name, 'Paul Lecteur');
      assert.equal(html.includes('data-patron-id'), true);
      assert.equal(html.includes('lecteur3'), true);
      assert.equal(html.includes(labels[lang]), true);
      assert.equal(html.includes('gap-x-2'), true);
      assert.equal(html.includes('sr-only'), true);
      assert.equal(html.includes('Lecteur3'), false);
      assert.equal(html.includes('Lecteurlecteur'), false);
      assert.equal(html.includes('usager'), false);
    }
  });
});
