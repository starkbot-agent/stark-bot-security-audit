import { useState, useEffect, useCallback, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { validateToken, logout as apiLogout } from '@/lib/api';

interface UseAuthReturn {
  token: string | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (token: string) => void;
  logout: () => Promise<void>;
}

export function useAuth(): UseAuthReturn {
  const [token, setToken] = useState<string | null>(() =>
    localStorage.getItem('stark_token')
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const navigate = useNavigate();
  const hasChecked = useRef(false);

  useEffect(() => {
    // Only run auth check once on mount
    if (hasChecked.current) return;
    hasChecked.current = true;

    async function checkAuth() {
      const storedToken = localStorage.getItem('stark_token');

      if (!storedToken) {
        setIsLoading(false);
        setIsAuthenticated(false);
        return;
      }

      try {
        const result = await validateToken();
        if (result.valid) {
          setToken(storedToken);
          setIsAuthenticated(true);
        } else {
          localStorage.removeItem('stark_token');
          setToken(null);
          setIsAuthenticated(false);
        }
      } catch {
        // Network error - keep token and assume valid (will fail on next API call if not)
        // This prevents logout on temporary network issues
        if (storedToken) {
          setToken(storedToken);
          setIsAuthenticated(true);
        }
      } finally {
        setIsLoading(false);
      }
    }

    checkAuth();
  }, []);

  const login = useCallback((newToken: string) => {
    localStorage.setItem('stark_token', newToken);
    setToken(newToken);
    setIsAuthenticated(true);
    navigate('/dashboard');
  }, [navigate]);

  const logout = useCallback(async () => {
    try {
      await apiLogout();
    } catch {
      // Ignore logout errors
    }
    localStorage.removeItem('stark_token');
    setToken(null);
    setIsAuthenticated(false);
    navigate('/');
  }, [navigate]);

  return {
    token,
    isLoading,
    isAuthenticated,
    login,
    logout,
  };
}
