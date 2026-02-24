import React from 'react';
import { X, CheckCircle, XCircle, AlertCircle, Info } from 'lucide-react';
import cx from 'classnames';

export interface Notification {
    id: string;
    type: 'success' | 'error' | 'info' | 'warning';
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
                    <div className="notification-icon">
                        {note.type === 'error' && <XCircle size={18} />}
                        {note.type === 'success' && <CheckCircle size={18} />}
                        {note.type === 'warning' && <AlertCircle size={18} />}
                        {note.type === 'info' && <Info size={18} />}
                    </div>
                    <div className="notification-message">{note.message}</div>
                    <button className="notification-close" onClick={() => onDismiss(note.id)}>
                        <X size={14} />
                    </button>
                </div>
            ))}
        </div>
    );
};

