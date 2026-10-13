import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  isAccountRenewalRefusal,
  renewalRefusalText,
  type AccountRenewalRefusal,
} from '@/utils/apiError';

interface AccountRenewalBannerProps {
  code: AccountRenewalRefusal | null;
}

/** One account-level renewal refusal for every loan on « Mes prêts ». */
export default function AccountRenewalBanner({ code }: AccountRenewalBannerProps) {
  const { t, i18n } = useTranslation();
  if (!isAccountRenewalRefusal(code)) return null;
  const text = renewalRefusalText(t, code, null, i18n.language);
  if (!text) return null;
  return (
    <div
      data-account-renewal={code}
      className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-xl"
    >
      <AlertTriangle className="h-5 w-5 text-amber-600 dark:text-amber-500 flex-shrink-0 mt-0.5" aria-hidden />
      <p className="font-medium text-amber-900 dark:text-amber-100">{text}</p>
    </div>
  );
}
