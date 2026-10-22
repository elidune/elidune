import { Link, useLocation } from 'react-router-dom';
import type { LegalModalKind } from './legalModalState';

interface LegalModalLinkProps {
  modal: LegalModalKind;
  className?: string;
  children: React.ReactNode;
}

export default function LegalModalLink({ modal, className, children }: LegalModalLinkProps) {
  const location = useLocation();

  return (
    <Link
      to={{ pathname: location.pathname, search: location.search }}
      state={{ legalModal: modal }}
      className={className}
    >
      {children}
    </Link>
  );
}
