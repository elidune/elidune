import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { patronId } from '@/utils/patronLink';

interface PatronLinkProps {
  userId?: string | number | null;
  /** Second id, typically `user.id` when `userId` is absent. */
  fallbackId?: string | number | null;
  className?: string;
  children: ReactNode;
  onClick?: (event: React.MouseEvent<HTMLAnchorElement>) => void;
}

/** Patron file link. Plain text when no usable id is available. */
export default function PatronLink({
  userId,
  fallbackId,
  className,
  children,
  onClick,
}: PatronLinkProps) {
  const id = patronId(userId, fallbackId);
  if (!id) {
    return <span className={className}>{children}</span>;
  }
  return (
    <Link className={className} to={`/users/${id}`} onClick={onClick}>
      {children}
    </Link>
  );
}
