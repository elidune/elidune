import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Modal } from '@/components/common';
import { formLabelClass, formTextareaClass } from '@/utils/formControl';
import type { AcquisitionVendor, CreateVendor, UpdateVendor } from '@/types';

interface VendorFormModalProps {
  vendor: AcquisitionVendor | null;
  isOpen: boolean;
  isSaving: boolean;
  error: string | null;
  onClose: () => void;
  onSubmit: (data: CreateVendor | UpdateVendor) => void;
}

export default function VendorFormModal({
  vendor,
  isOpen,
  isSaving,
  error,
  onClose,
  onSubmit,
}: VendorFormModalProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(vendor?.name ?? '');
  const [code, setCode] = useState(vendor?.code ?? '');
  const [email, setEmail] = useState(vendor?.email ?? '');
  const [phone, setPhone] = useState(vendor?.phone ?? '');
  const [address, setAddress] = useState(vendor?.address ?? '');
  const [notes, setNotes] = useState(vendor?.notes ?? '');
  const [active, setActive] = useState(vendor?.active ?? true);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed || isSaving) return;
    onSubmit({
      name: trimmed,
      code: code.trim() || null,
      email: email.trim() || null,
      phone: phone.trim() || null,
      address: address.trim() || null,
      notes: notes.trim() || null,
      active,
    });
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={vendor ? t('acquisitions.vendors.editTitle') : t('acquisitions.vendors.createTitle')}
      size="md"
      footer={
        <div className="flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isSaving}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" form="vendor-form" isLoading={isSaving} disabled={!name.trim()}>
            {t('common.save')}
          </Button>
        </div>
      }
    >
      <form id="vendor-form" className="space-y-3" onSubmit={handleSubmit}>
        <Input
          label={t('acquisitions.vendors.name')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          autoFocus
        />
        <Input
          label={t('acquisitions.vendors.code')}
          value={code}
          onChange={(e) => setCode(e.target.value)}
        />
        <Input
          label={t('acquisitions.vendors.email')}
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        <Input
          label={t('acquisitions.vendors.phone')}
          value={phone}
          onChange={(e) => setPhone(e.target.value)}
        />
        <div>
          <label htmlFor="vendor-address" className={formLabelClass()}>
            {t('acquisitions.vendors.address')}
          </label>
          <textarea
            id="vendor-address"
            className={formTextareaClass()}
            rows={2}
            value={address}
            onChange={(e) => setAddress(e.target.value)}
          />
        </div>
        <div>
          <label htmlFor="vendor-notes" className={formLabelClass()}>
            {t('acquisitions.vendors.notes')}
          </label>
          <textarea
            id="vendor-notes"
            className={formTextareaClass()}
            rows={2}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </div>
        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <input
            type="checkbox"
            checked={active}
            onChange={(e) => setActive(e.target.checked)}
          />
          {t('acquisitions.vendors.active')}
        </label>
        {error ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        ) : null}
      </form>
    </Modal>
  );
}
