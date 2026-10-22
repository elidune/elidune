import { Navigate, useLocation, useNavigate } from 'react-router-dom';
import { useAuth } from '@/contexts/AuthContext';
import AboutModal from './AboutModal';
import PrivacyModal from './PrivacyModal';
import { readLegalModal, type LegalModalKind } from './legalModalState';

function LegalModalRedirect({ modal }: { modal: LegalModalKind }) {
  const { isAuthenticated } = useAuth();
  const fallback = isAuthenticated ? '/home' : '/';

  return <Navigate to={fallback} replace state={{ legalModal: modal }} />;
}

export function LegalModalAboutRedirect() {
  return <LegalModalRedirect modal="about" />;
}

export function LegalModalPrivacyRedirect() {
  return <LegalModalRedirect modal="privacy" />;
}

export default function LegalInfoModalHost() {
  const location = useLocation();
  const navigate = useNavigate();
  const modal = readLegalModal(location.state);

  const close = () => {
    navigate(
      { pathname: location.pathname, search: location.search },
      { replace: true, state: null }
    );
  };

  return (
    <>
      <AboutModal isOpen={modal === 'about'} onClose={close} />
      <PrivacyModal isOpen={modal === 'privacy'} onClose={close} />
    </>
  );
}
