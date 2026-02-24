import { useState, useRef, useCallback } from 'react';
import { NotificationType, AppNotification } from '../types';

interface UseNotificationsReturn {
    notifications: AppNotification[];
    notify: (message: string, type?: NotificationType, duration?: number) => void;
    dismissNotification: (id: string) => void;
}

/**
 * Hook for managing application notifications
 * Includes deduplication logic to prevent duplicate notifications
 */
export const useNotifications = (): UseNotificationsReturn => {
    const [notifications, setNotifications] = useState<AppNotification[]>([]);
    const lastNotifyRef = useRef<{ message: string; time: number } | null>(null);

    const dismissNotification = useCallback((id: string) => {
        setNotifications(prev => prev.filter(n => n.id !== id));
    }, []);

    const notify = useCallback((message: string, type: NotificationType = 'error', duration = 5000) => {
        const now = Date.now();

        // Ignore duplicate messages within 1 second
        if (lastNotifyRef.current &&
            lastNotifyRef.current.message === message &&
            now - lastNotifyRef.current.time < 1000) {
            return;
        }
        lastNotifyRef.current = { message, time: now };

        const id = Math.random().toString(36).substring(2, 9);
        setNotifications(prev => [...prev, { id, type, message, duration }]);

        if (duration > 0) {
            setTimeout(() => dismissNotification(id), duration);
        }
    }, [dismissNotification]);

    return { notifications, notify, dismissNotification };
};
