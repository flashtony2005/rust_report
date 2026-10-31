import React, { createContext, useContext, useState, useCallback } from 'react';

interface ToastItem {
  id: number;
  message: string;
  isError?: boolean;
}

interface ToastContextType {
  toast: (message: string, isError?: boolean) => void;
}

const ToastContext = createContext<ToastContextType>({ toast: () => {} });

export const useToast = () => useContext(ToastContext);

export const ToastProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const toast = useCallback((message: string, isError: boolean = false) => {
    const id = Date.now() + Math.random();
    setToasts((prev) => [...prev, { id, message, isError }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, isError ? 5200 : 2800);
  }, []);

  return (
    <ToastContext.Provider value={{ toast }}>
      {children}
      <div className="toast" id="toast">
        {toasts.map((t) => (
          <div key={t.id} className={t.isError ? 'err' : ''}>
            {t.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
};
