import { useState, type ComponentType } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ChevronDown } from 'lucide-react';
import { useMatchMedia } from '@/hooks/common/useMatchMedia';
import { SIDEBAR_NAV_COMPACT_MEDIA, isSidebarNavItemActive } from '@/utils/sidebarNav';

export type SidebarNavItem = {
  name: string;
  href: string;
  icon: ComponentType<{ className?: string }>;
};

export type SidebarNavSection = {
  id: string;
  label?: string;
  /** Accordion title on compact nav when there is no visible section `label`. */
  collapseLabel?: string;
  items: SidebarNavItem[];
};

type SidebarNavSectionsProps = {
  /** Always visible (e.g. Home), outside accordion sections. */
  pinnedItems?: SidebarNavItem[];
  sections: SidebarNavSection[];
  onNavigate?: () => void;
};

function NavItemLink({
  item,
  isActive,
  onNavigate,
}: {
  item: SidebarNavItem;
  isActive: boolean;
  onNavigate?: () => void;
}) {
  return (
    <Link
      to={item.href}
      onClick={onNavigate}
      className={`flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
        isActive
          ? 'bg-amber-50 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
          : 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800'
      }`}
    >
      <item.icon className="h-5 w-5 shrink-0" aria-hidden="true" />
      {item.name}
    </Link>
  );
}

function sectionWithActiveItem(
  sections: SidebarNavSection[],
  location: ReturnType<typeof useLocation>,
): SidebarNavSection | undefined {
  return sections.find((section) =>
    section.items.some((item) => isSidebarNavItemActive(item.href, location)),
  );
}

function defaultExpandedSectionId(
  sections: SidebarNavSection[],
  location: ReturnType<typeof useLocation>,
): string | null {
  return sectionWithActiveItem(sections, location)?.id ?? sections[0]?.id ?? null;
}

/** Accordion target after compact mode, route, or section list changes. */
function nextExpandedSectionId(
  compactNav: boolean,
  sections: SidebarNavSection[],
  location: ReturnType<typeof useLocation>,
  current: string | null,
): string | null {
  if (!compactNav) return null;
  const activeSection = sectionWithActiveItem(sections, location);
  if (activeSection) return activeSection.id;
  if (current && sections.some((section) => section.id === current)) return current;
  return sections[0]?.id ?? null;
}

function sectionsSyncKey(sections: SidebarNavSection[]): string {
  return sections
    .map((section) => `${section.id}:${section.items.map((item) => item.href).join(',')}`)
    .join('|');
}

function sectionAccordionTitle(section: SidebarNavSection, t: (key: string) => string): string {
  return section.label ?? section.collapseLabel ?? t('nav.sectionUntitled');
}

export default function SidebarNavSections({
  pinnedItems = [],
  sections,
  onNavigate,
}: SidebarNavSectionsProps) {
  const { t } = useTranslation();
  const location = useLocation();
  const compactNav = useMatchMedia(SIDEBAR_NAV_COMPACT_MEDIA);
  const [expandedSectionId, setExpandedSectionId] = useState<string | null>(() =>
    compactNav ? defaultExpandedSectionId(sections, location) : null,
  );
  const locationKey = `${location.pathname}${location.search}`;
  const sectionKey = sectionsSyncKey(sections);
  const [accordionSync, setAccordionSync] = useState({
    compactNav,
    locationKey,
    sectionKey,
  });
  if (
    accordionSync.compactNav !== compactNav ||
    accordionSync.locationKey !== locationKey ||
    accordionSync.sectionKey !== sectionKey
  ) {
    setAccordionSync({ compactNav, locationKey, sectionKey });
    const next = nextExpandedSectionId(compactNav, sections, location, expandedSectionId);
    if (next !== expandedSectionId) setExpandedSectionId(next);
  }

  const toggleSection = (sectionId: string) => {
    setExpandedSectionId((prev) => (prev === sectionId ? null : sectionId));
  };

  return (
    <>
      {pinnedItems.length > 0 && (
        <div className="space-y-1 pb-1">
          {pinnedItems.map((item) => (
            <NavItemLink
              key={item.href}
              item={item}
              isActive={isSidebarNavItemActive(item.href, location)}
              onNavigate={onNavigate}
            />
          ))}
        </div>
      )}
      {sections.map((section) => {
        const sectionExpanded = !compactNav || expandedSectionId === section.id;
        const accordionTitle = sectionAccordionTitle(section, t);
        const showStaticLabel = !compactNav && section.label;
        const showAccordionHeader = compactNav;

        return (
          <div key={section.id} className="space-y-1">
            {showAccordionHeader && (
              <button
                type="button"
                id={`nav-section-trigger-${section.id}`}
                aria-expanded={sectionExpanded}
                aria-controls={`nav-section-panel-${section.id}`}
                onClick={() => toggleSection(section.id)}
                aria-label={
                  sectionExpanded
                    ? `${t('common.collapseSection')}: ${accordionTitle}`
                    : `${t('common.expandSection')}: ${accordionTitle}`
                }
                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
              >
                <ChevronDown
                  className={`h-4 w-4 shrink-0 text-gray-500 transition-transform duration-200 dark:text-gray-400 ${
                    sectionExpanded ? 'rotate-0' : '-rotate-90'
                  }`}
                  aria-hidden="true"
                />
                <span className="flex-1 text-[11px] font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
                  {accordionTitle}
                </span>
              </button>
            )}
            {showStaticLabel && (
              <p className="px-3 pt-1 text-[11px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
                {section.label}
              </p>
            )}
            {sectionExpanded && (
              <div
                id={`nav-section-panel-${section.id}`}
                role={compactNav ? 'region' : undefined}
                aria-labelledby={compactNav ? `nav-section-trigger-${section.id}` : undefined}
                className={`space-y-1 ${compactNav ? 'pb-1 pl-1' : ''}`}
              >
                {section.items.map((item) => (
                  <NavItemLink
                    key={item.href}
                    item={item}
                    isActive={isSidebarNavItemActive(item.href, location)}
                    onNavigate={onNavigate}
                  />
                ))}
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}
