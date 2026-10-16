import { Monitor, Moon, Sun } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useTheme } from '@/contexts/ThemeContext';

type ThemeSwitcherProps = {
  /** When false, only icons are shown (for compact layouts). */
  showLabels?: boolean;
};

export default function ThemeSwitcher({ showLabels = true }: ThemeSwitcherProps) {
  const { t } = useTranslation();
  const { theme, setTheme } = useTheme();

  const themeOptions = [
    { value: 'light' as const, icon: Sun, label: t('theme.light') },
    { value: 'dark' as const, icon: Moon, label: t('theme.dark') },
    { value: 'system' as const, icon: Monitor, label: t('theme.system') },
  ];

  return (
    <div
      className="flex max-w-md items-center gap-1 p-1 bg-gray-100 dark:bg-gray-800 rounded-lg"
      role="group"
      aria-label={t('profile.theme')}
    >
      {themeOptions.map((option) => (
        <button
          key={option.value}
          type="button"
          onClick={() => setTheme(option.value)}
          aria-label={option.label}
          aria-pressed={theme === option.value}
          className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
            theme === option.value
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
              : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
          }`}
        >
          <option.icon className="h-4 w-4 shrink-0" aria-hidden="true" />
          {showLabels ? <span>{option.label}</span> : null}
        </button>
      ))}
    </div>
  );
}
