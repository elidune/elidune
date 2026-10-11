import { createContext, useContext, useRef, useState, useEffect, ReactNode } from 'react';
import api from '@/services/api';
import type { User, LoginRequest, LoginResponse, TwoFactorMethod, FirstSetupResponse } from '@/types';
import {
  getHttpStatus,
  isInvalidSessionError,
  isRateLimitedError,
  retryAfterDelayMs,
  getRetryAfterSeconds,
} from '@/utils/apiError';
import { readSessionUserFromAccessToken } from '@/utils/jwtRights';

/** Extra tries after the first 429 on `/auth/me`, each waiting on `Retry-After`. */
const PROFILE_RATE_LIMIT_RETRIES = 3;

export type ProfileWarning = {
  messageKey: 'errors.tooManyRequests' | 'errors.tooManyRequestsWait' | 'errors.network' | 'errors.sessionUnavailable';
  seconds?: number;
};

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function warningForKeptSession(error: unknown): ProfileWarning {
  if (isRateLimitedError(error)) {
    const seconds = getRetryAfterSeconds(error);
    if (seconds != null) return { messageKey: 'errors.tooManyRequestsWait', seconds };
    return { messageKey: 'errors.tooManyRequests' };
  }
  if (getHttpStatus(error) == null) return { messageKey: 'errors.network' };
  return { messageKey: 'errors.sessionUnavailable' };
}

const MUST_CHANGE_PASSWORD_KEY = 'elidune_must_change_password';

function readMustChangePasswordFlag(): boolean {
  return localStorage.getItem(MUST_CHANGE_PASSWORD_KEY) === '1';
}

function setMustChangePasswordFlag(on: boolean) {
  if (on) {
    localStorage.setItem(MUST_CHANGE_PASSWORD_KEY, '1');
  } else {
    localStorage.removeItem(MUST_CHANGE_PASSWORD_KEY);
  }
}

// Pending 2FA state when login requires verification
interface Pending2FA {
  userId: string;
  method: TwoFactorMethod;
  user: LoginResponse['user'];
}

interface AuthContextType {
  user: User | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  mustChangePassword: boolean;
  pending2FA: Pending2FA | null;
  login: (credentials: LoginRequest) => Promise<{ requires2FA: boolean; mustChangePassword?: boolean }>;
  verify2FA: (code: string, trustDevice?: boolean) => Promise<{ mustChangePassword?: boolean }>;
  verifyRecovery: (code: string) => Promise<{ mustChangePassword?: boolean }>;
  cancel2FA: () => void;
  logout: () => void;
  refreshProfile: () => Promise<void>;
  /** Non-blocking notice when `/auth/me` fails but the session is kept. */
  profileWarning: ProfileWarning | null;
  dismissProfileWarning: () => void;
  /** After successful POST /first_setup — same as login without 2FA. */
  completeFirstSetup: (response: FirstSetupResponse) => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [pending2FA, setPending2FA] = useState<Pending2FA | null>(null);
  const [mustChangePassword, setMustChangePassword] = useState(false);
  const [profileWarning, setProfileWarning] = useState<ProfileWarning | null>(null);
  const refreshLock = useRef<Promise<void> | null>(null);

  const refreshProfile = async () => {
    if (refreshLock.current) return refreshLock.current;

    const run = async () => {
      let rateLimitRetries = 0;
      for (;;) {
        try {
          const profile = await api.getProfile();
          setUser(profile);
          setProfileWarning(null);
          return;
        } catch (error) {
          if (isInvalidSessionError(error)) {
            setProfileWarning(null);
            setUser(null);
            api.logout();
            return;
          }
          if (isRateLimitedError(error) && rateLimitRetries < PROFILE_RATE_LIMIT_RETRIES) {
            rateLimitRetries += 1;
            await delay(retryAfterDelayMs(error));
            continue;
          }
          setUser((current) => current ?? readSessionUserFromAccessToken(api.getToken()));
          setProfileWarning(warningForKeptSession(error));
          return;
        }
      }
    };

    const pending = run();
    refreshLock.current = pending;
    try {
      await pending;
    } finally {
      if (refreshLock.current === pending) refreshLock.current = null;
    }
  };

  useEffect(() => {
    const initAuth = async () => {
      if (api.isAuthenticated()) {
        if (readMustChangePasswordFlag()) {
          setMustChangePassword(true);
        } else {
          await refreshProfile();
        }
      }
      setIsLoading(false);
    };
    initAuth();
  }, []);

  const login = async (
    credentials: LoginRequest
  ): Promise<{ requires2FA: boolean; mustChangePassword?: boolean }> => {
    const response = await api.login(credentials);

    // If 2FA is required, store pending state
    if (response.requires2fa) {
      setPending2FA({
        userId: response.user.id,
        method: (response.twoFactorMethod || 'totp') as TwoFactorMethod,
        user: response.user,
      });
      return { requires2FA: true };
    }

    if (response.mustChangePassword) {
      setMustChangePassword(true);
      setMustChangePasswordFlag(true);
      return { requires2FA: false, mustChangePassword: true };
    }

    setMustChangePassword(false);
    setMustChangePasswordFlag(false);

    // No 2FA required, set user directly
    setUser({
      id: response.user.id,
      username: response.user.username || response.user.login,
      login: response.user.login,
      firstname: response.user.firstname,
      lastname: response.user.lastname,
      accountType: response.user.accountType,
      language: response.user.language,
      publicType: response.user.publicType,
      guardianId: response.user.guardianId,
    });
    // Refresh profile to get complete user data
    await refreshProfile();
    return { requires2FA: false };
  };

  const verify2FA = async (code: string, trustDevice = false) => {
    if (!pending2FA) throw new Error('No pending 2FA verification');
    const pending = pending2FA;

    const res = await api.verify2FA({
      userId: pending.userId,
      code,
      trustDevice,
      deviceId: api.getDeviceId() ?? undefined,
    });

    setPending2FA(null);

    if (res.mustChangePassword) {
      setMustChangePassword(true);
      setMustChangePasswordFlag(true);
      return { mustChangePassword: true };
    }

    setMustChangePassword(false);
    setMustChangePasswordFlag(false);

    // Set user from pending state
    setUser({
      id: pending.user.id,
      username: pending.user.username || pending.user.login,
      login: pending.user.login,
      firstname: pending.user.firstname,
      lastname: pending.user.lastname,
      accountType: pending.user.accountType,
      language: pending.user.language,
      publicType: pending.user.publicType,
      guardianId: pending.user.guardianId,
    });

    // Refresh profile to get complete user data
    await refreshProfile();
    return { mustChangePassword: false };
  };

  const verifyRecovery = async (code: string) => {
    if (!pending2FA) throw new Error('No pending 2FA verification');
    const pending = pending2FA;

    const res = await api.verifyRecovery({
      userId: pending.userId,
      code,
    });

    setPending2FA(null);

    if (res.mustChangePassword) {
      setMustChangePassword(true);
      setMustChangePasswordFlag(true);
      return { mustChangePassword: true };
    }

    setMustChangePassword(false);
    setMustChangePasswordFlag(false);

    // Set user from pending state
    setUser({
      id: pending.user.id,
      username: pending.user.username || pending.user.login,
      login: pending.user.login,
      firstname: pending.user.firstname,
      lastname: pending.user.lastname,
      accountType: pending.user.accountType,
      language: pending.user.language,
      publicType: pending.user.publicType,
      guardianId: pending.user.guardianId,
    });

    // Refresh profile to get complete user data
    await refreshProfile();
    return { mustChangePassword: false };
  };

  const cancel2FA = () => {
    setPending2FA(null);
  };

  const logout = () => {
    api.logout();
    setUser(null);
    setPending2FA(null);
    setMustChangePassword(false);
    setMustChangePasswordFlag(false);
    setProfileWarning(null);
  };

  const dismissProfileWarning = () => {
    setProfileWarning(null);
  };

  const completeFirstSetup = async (response: FirstSetupResponse) => {
    api.setToken(response.token);
    setMustChangePassword(false);
    setMustChangePasswordFlag(false);
    setPending2FA(null);
    setUser({
      id: response.user.id,
      username: response.user.username || response.user.login,
      login: response.user.login,
      firstname: response.user.firstname,
      lastname: response.user.lastname,
      accountType: response.user.accountType,
      language: response.user.language,
      publicType: response.user.publicType,
      guardianId: response.user.guardianId,
    });
    await refreshProfile();
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        isLoading,
        isAuthenticated: !!user,
        mustChangePassword,
        pending2FA,
        login,
        verify2FA,
        verifyRecovery,
        cancel2FA,
        logout,
        refreshProfile,
        profileWarning,
        dismissProfileWarning,
        completeFirstSetup,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}


