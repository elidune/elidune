import { NavLink } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

const LINKS = [
  { to: '/acquisitions/orders', key: 'nav.orders' },
  { to: '/acquisitions/vendors', key: 'nav.vendors' },
  { to: '/acquisitions/funds', key: 'nav.funds' },
] as const;

export default function AcquisitionsSubnav() {
  const { t } = useTranslation();

  return (
    <nav aria-label={t('acquisitions.sectionNav')} className="flex flex-wrap gap-2">
      {LINKS.map((link) => (
        <NavLink
          key={link.to}
          to={link.to}
          className={({ isActive }) =>
            `rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
              isActive
                ? 'bg-amber-50 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
                : 'text-gray-600 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800'
            }`
          }
        >
          {t(link.key)}
        </NavLink>
      ))}
    </nav>
  );
}
