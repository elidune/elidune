import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  commitDismissHeldSetAside,
  commitHeldSetAside,
  heldSetAsideServerMessage,
} from './heldSetAside.ts';

const FR_TOAST =
  "Exemplaire mis de côté pour la réservation : placez-le sur l'étagère des réservations.";
const NAME = 'Grace Hopper';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { fr, en, de, es };

async function tFor(lng: Lang): Promise<TFunction> {
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
  return i18n.t.bind(i18n);
}

describe('item_held_for_other set-aside stays on checkout', () => {
  it('calls the check-in API, shows the toast, and does not leave checkout', async () => {
    const calls: string[] = [];
    const toasts: Array<{ variant: string; message: string }> = [];
    const tab = 'borrow';
    let dialogOpen = true;
    let barcode = 'BC-1';
    let focused = false;

    await commitHeldSetAside(' BC-1 ', {
      returnByBarcode: async (code) => {
        calls.push(code);
        return {
          status: 'ready',
          userName: NAME,
          holdFor: { firstname: 'Grace', lastname: 'Hopper' },
        };
      },
      successMessage: FR_TOAST,
      errorMessage: (error) => heldSetAsideServerMessage(error, ((key: string) => key) as TFunction),
      showToast: (toast) => {
        toasts.push(toast);
      },
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => {
        barcode = '';
      },
      refocusBarcode: () => {
        focused = true;
      },
    });

    assert.deepEqual(calls, ['BC-1']);
    assert.equal(tab, 'borrow');
    assert.equal(dialogOpen, false);
    assert.equal(barcode, '');
    assert.equal(focused, true);
    assert.deepEqual(toasts, [{ variant: 'success', message: FR_TOAST }]);
    assert.equal(JSON.stringify(toasts).includes(NAME), false);
    assert.equal(JSON.stringify(toasts).includes('Grace'), false);
    assert.equal(JSON.stringify(toasts).includes('Hopper'), false);
  });

  it('shows the server message and stays on the dialog when the API fails', async () => {
    const calls: string[] = [];
    const toasts: Array<{ variant: string; message: string }> = [];
    const tab = 'borrow';
    let dialogOpen = true;
    let barcode = 'BC-9';
    const t = await tFor('fr');

    await commitHeldSetAside('BC-9', {
      returnByBarcode: async (code) => {
        calls.push(code);
        throw {
          response: {
            data: {
              message: 'Cannot move this copy to the holds shelf',
              userName: NAME,
              firstname: 'Grace',
              lastname: 'Hopper',
            },
          },
        };
      },
      successMessage: FR_TOAST,
      errorMessage: (error) => heldSetAsideServerMessage(error, t),
      showToast: (toast) => {
        toasts.push(toast);
      },
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => {
        barcode = '';
      },
      refocusBarcode: () => undefined,
    });

    assert.deepEqual(calls, ['BC-9']);
    assert.equal(tab, 'borrow');
    assert.equal(dialogOpen, true);
    assert.equal(barcode, 'BC-9');
    assert.deepEqual(toasts, [
      { variant: 'error', message: 'Cannot move this copy to the holds shelf' },
    ]);
    assert.equal(toasts[0]?.message.includes(NAME), false);
    assert.equal(toasts[0]?.message.includes('Grace'), false);
  });

  it('cancel closes and refocuses without calling the API or clearing the barcode', () => {
    const calls: string[] = [];
    const tab = 'borrow';
    let dialogOpen = true;
    let barcode = 'BC-1';
    let focused = false;

    commitDismissHeldSetAside({
      closeDialog: () => {
        dialogOpen = false;
      },
      clearBarcode: () => {
        barcode = '';
      },
      refocusBarcode: () => {
        focused = true;
      },
    });

    assert.deepEqual(calls, []);
    assert.equal(tab, 'borrow');
    assert.equal(dialogOpen, false);
    assert.equal(barcode, 'BC-1');
    assert.equal(focused, true);
  });

  it('uses the set-aside sentence in every language, with no other reader', () => {
    const expected = {
      fr: FR_TOAST,
      en: 'Copy set aside for the hold: place it on the holds shelf.',
      de: 'Exemplar für die Vormerkung zurückgelegt: legen Sie es ins Abholregal.',
      es: 'Ejemplar apartado para la reserva: colóquelo en el estante de reservas.',
    } as const;
    for (const lang of Object.keys(expected) as Lang[]) {
      const sentence = catalogs[lang].loans.heldSetAside;
      assert.equal(sentence, expected[lang]);
      assert.equal(sentence.includes('{{'), false, lang);
      assert.equal(sentence.includes(NAME), false, lang);
      assert.equal(sentence.toLowerCase().includes('force'), false, lang);
      assert.equal(sentence.toLowerCase().includes('usager'), false, lang);
      assert.equal(sentence.toLowerCase().includes('patron'), false, lang);
    }
    assert.equal(catalogs.fr.loans.heldSetAside.includes('Exemplaire'), true);
    assert.equal(catalogs.fr.loans.heldSetAside.includes('étagère des réservations'), true);
  });
});
