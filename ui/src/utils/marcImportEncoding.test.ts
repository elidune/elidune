import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  DEFAULT_MARC_FILE_ENCODING,
  isMarcFileEncoding,
  marcFileEncodingLabelKey,
} from './marcImportEncoding.ts';

describe('marcFileEncoding', () => {
  it('defaults to utf8', () => {
    assert.equal(DEFAULT_MARC_FILE_ENCODING, 'utf8');
  });

  it('maps label keys', () => {
    assert.equal(marcFileEncodingLabelKey('iso8859-2'), 'importMarc.encoding.iso8859_2');
  });

  it('validates known values', () => {
    assert.equal(isMarcFileEncoding('marc8'), true);
    assert.equal(isMarcFileEncoding('latin1'), false);
  });
});
