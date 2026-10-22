import { useTranslation } from 'react-i18next';
import { Shield, Mail, Phone, MapPin, Building2 } from 'lucide-react';
import { Card, Modal } from '@/components/common';
import type { LibraryInfo } from '@/types';
import { formatCountryName } from '@/utils/countryTimezone';
import { useLibraryInfoQuery } from '@/hooks/common/useLibraryInfoQuery';

function SectionCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="border-l-2 border-indigo-300 dark:border-indigo-600 pl-4">
      <h3 className="text-base font-semibold text-gray-900 dark:text-white mb-2">{title}</h3>
      <div className="text-sm text-gray-600 dark:text-gray-400 space-y-1">{children}</div>
    </div>
  );
}

interface PrivacyModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function PrivacyModal({ isOpen, onClose }: PrivacyModalProps) {
  const { t, i18n } = useTranslation();
  const { data: libraryInfo, isLoading: loading } = useLibraryInfoQuery();

  const formatAddress = (info: LibraryInfo) => {
    const parts = [
      info.addrLine1,
      info.addrLine2,
      [info.addrPostcode, info.addrCity].filter(Boolean).join(' '),
      formatCountryName(info.addrCountry, i18n.language),
    ].filter(Boolean);
    return parts;
  };

  const formattedDate = libraryInfo?.updatedAt
    ? new Date(libraryInfo.updatedAt).toLocaleDateString(i18n.language, {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
      })
    : null;

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={t('privacy.title')} size="xl">
      <div className="space-y-4">
        <div className="flex items-start gap-3">
          <div className="p-2 bg-indigo-100 dark:bg-indigo-900/40 rounded-lg shrink-0">
            <Shield className="h-5 w-5 text-indigo-600 dark:text-indigo-400" />
          </div>
          <div>
            <p className="text-sm text-gray-500 dark:text-gray-400">{t('privacy.subtitle')}</p>
            {formattedDate && (
              <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
                {t('privacy.lastUpdate')} : {formattedDate}
              </p>
            )}
          </div>
        </div>

        <Card>
          <div className="p-4 sm:p-6 space-y-6">
            <SectionCard title={t('privacy.dataController.title')}>
              <p>{t('privacy.dataController.description')}</p>
              {loading ? (
                <p className="text-gray-400 dark:text-gray-500 italic">{t('common.loading')}</p>
              ) : libraryInfo ? (
                <div className="mt-3 p-3 bg-gray-50 dark:bg-gray-800 rounded-lg space-y-1.5">
                  {libraryInfo.name && (
                    <div className="flex items-center gap-2">
                      <Building2 className="h-4 w-4 text-gray-400 flex-shrink-0" />
                      <span className="font-medium text-gray-800 dark:text-gray-200">
                        {libraryInfo.name}
                      </span>
                    </div>
                  )}
                  {formatAddress(libraryInfo).length > 0 && (
                    <div className="flex items-start gap-2">
                      <MapPin className="h-4 w-4 text-gray-400 flex-shrink-0 mt-0.5" />
                      <div>
                        {formatAddress(libraryInfo).map((line, i) => (
                          <div key={i} className="text-gray-700 dark:text-gray-300">
                            {line}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                  {libraryInfo.email && (
                    <div className="flex items-center gap-2">
                      <Mail className="h-4 w-4 text-gray-400 flex-shrink-0" />
                      <a
                        href={`mailto:${libraryInfo.email}`}
                        className="text-indigo-600 dark:text-indigo-400 hover:underline"
                      >
                        {libraryInfo.email}
                      </a>
                    </div>
                  )}
                  {libraryInfo.phones?.length > 0 && (
                    <div className="flex items-start gap-2">
                      <Phone className="h-4 w-4 text-gray-400 flex-shrink-0 mt-0.5" />
                      <div>
                        {libraryInfo.phones.map((phone, i) => (
                          <div key={i} className="text-gray-700 dark:text-gray-300">
                            {phone}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ) : (
                <p className="mt-2 text-gray-400 dark:text-gray-500 italic">—</p>
              )}
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.dataCollected.title')}>
              <p>{t('privacy.dataCollected.description')}</p>
              <p className="mt-1">{t('privacy.dataCollected.items')}</p>
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.purpose.title')}>
              <p>{t('privacy.purpose.description')}</p>
              <p className="mt-1">{t('privacy.purpose.items')}</p>
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.legalBasis.title')}>
              <p>{t('privacy.legalBasis.description')}</p>
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.retention.title')}>
              <p>{t('privacy.retention.description')}</p>
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.rights.title')}>
              <p>{t('privacy.rights.description')}</p>
              <p className="mt-1">{t('privacy.rights.items')}</p>
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.contact.title')}>
              <p>{t('privacy.contact.description')}</p>
              {!loading && libraryInfo?.email && (
                <a
                  href={`mailto:${libraryInfo.email}`}
                  className="inline-flex items-center gap-1.5 mt-2 text-indigo-600 dark:text-indigo-400 hover:underline"
                >
                  <Mail className="h-4 w-4" />
                  {libraryInfo.email}
                </a>
              )}
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.security.title')}>
              <p>{t('privacy.security.description')}</p>
            </SectionCard>

            <div className="border-t border-gray-100 dark:border-gray-800" />

            <SectionCard title={t('privacy.noThirdParty.title')}>
              <p>{t('privacy.noThirdParty.description')}</p>
            </SectionCard>
          </div>
        </Card>
      </div>
    </Modal>
  );
}
