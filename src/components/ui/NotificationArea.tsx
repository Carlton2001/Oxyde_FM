import React from 'react';
import { X, CheckCircle, XCircle, AlertCircle, Info, Loader2 } from 'lucide-react';
import cx from 'classnames';

export interface Notification {
    id: string;
    type: 'success' | 'error' | 'info' | 'warning' | 'loading';
    message: string;
}

interface NotificationAreaProps {
    notifications: Notification[];
    onDismiss: (id: string) => void;
}

export const NotificationArea: React.FC<NotificationAreaProps> = ({ notifications, onDismiss }) => {
    return (
        <div className="notification-area">
            {notifications.map(note => (
                <div key={note.id} className={cx("notification-toast", note.type)}>
                    <div className="notification-toast-content">
                        <div className="notification-icon">
                            {note.type === 'error' && <XCircle size={18} />}
                            {note.type === 'success' && <CheckCircle size={18} />}
                            {note.type === 'warning' && <AlertCircle size={18} />}
                            {note.type === 'info' && <Info size={18} />}
                            {note.type === 'loading' && <Loader2 size={18} className="animate-spin" />}
                        </div>
                        <div className="notification-message">{note.message}</div>
                        {note.type !== 'loading' && (
                            <button className="notification-close" onClick={() => onDismiss(note.id)}>
                                <X size={14} />
                            </button>
                        )}
                    </div>
                    {note.type === 'loading' && (
                        <div className="notification-progress-infinite">
                            <div className="notification-progress-bar-fill" />
                        </div>
                    )}
                </div>
            ))}
        </div>
    );
};

