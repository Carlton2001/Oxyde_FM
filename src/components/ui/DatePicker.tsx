import React, { useState, useRef, useEffect } from 'react';
import { Calendar as CalendarIcon, ChevronLeft, ChevronRight, X } from 'lucide-react';
import './DatePicker.css';

interface DatePickerProps {
    value: string; // ISO format YYYY-MM-DD
    onChange: (date: string) => void;
    placeholder?: string;
    language?: 'en' | 'fr';
}

export const DatePicker: React.FC<DatePickerProps> = ({ value, onChange, placeholder, language = 'fr' }) => {
    const [isOpen, setIsOpen] = useState(false);
    const [openUpward, setOpenUpward] = useState(false);

    const toggleOpen = () => {
        if (!isOpen && containerRef.current) {
            const rect = containerRef.current.getBoundingClientRect();
            const spaceBelow = window.innerHeight - rect.bottom;
            // Popover is about 320px high, adding margin
            setOpenUpward(spaceBelow < 350);
        }
        setIsOpen(!isOpen);
    };

    // Parse initial date or default to today
    const parseInitialDate = (v: string) => {
        if (!v) return new Date();
        const [y, m, d] = v.split('-').map(Number);
        return new Date(y, m - 1, d);
    };

    const [viewDate, setViewDate] = useState(parseInitialDate(value));
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    // Sync viewDate when value changes externally (if open)
    useEffect(() => {
        if (value && isOpen) {
            setViewDate(parseInitialDate(value));
        }
    }, [value, isOpen]);

    const formatDateISO = (date: Date) => {
        const y = date.getFullYear();
        const m = String(date.getMonth() + 1).padStart(2, '0');
        const d = String(date.getDate()).padStart(2, '0');
        return `${y}-${m}-${d}`;
    };

    const formatDisplay = (v: string) => {
        if (!v) return '';
        const [y, m, d] = v.split('-');
        return `${d}/${m}/${y}`;
    };

    const handleDateSelect = (date: Date) => {
        onChange(formatDateISO(date));
        setIsOpen(false);
    };

    const nextMonth = () => setViewDate(new Date(viewDate.getFullYear(), viewDate.getMonth() + 1, 1));
    const prevMonth = () => setViewDate(new Date(viewDate.getFullYear(), viewDate.getMonth() - 1, 1));

    const getMonthName = (date: Date) => {
        return date.toLocaleString(language === 'fr' ? 'fr-FR' : 'en-US', { month: 'long', year: 'numeric' });
    };

    const renderHeader = () => {
        return (
            <div className="calendar-header">
                <div className="calendar-month-year" style={{ textTransform: 'capitalize' }}>
                    {getMonthName(viewDate)}
                </div>
                <div className="calendar-nav">
                    <button type="button" className="calendar-nav-btn" onClick={prevMonth}><ChevronLeft size={16} /></button>
                    <button type="button" className="calendar-nav-btn" onClick={nextMonth}><ChevronRight size={16} /></button>
                </div>
            </div>
        );
    };

    const renderDays = () => {
        const year = viewDate.getFullYear();
        const month = viewDate.getMonth();

        const firstDayOfMonth = new Date(year, month, 1);
        const lastDayOfMonth = new Date(year, month + 1, 0);

        // Day of week for first day (0=Sun, 1=Mon, ..., 6=Sat)
        // Adjust to make Monday index 0
        let firstDayIdx = firstDayOfMonth.getDay();
        firstDayIdx = firstDayIdx === 0 ? 6 : firstDayIdx - 1;

        const daysInMonth = lastDayOfMonth.getDate();
        const days = [];

        // Weekday labels
        const weekdayLabels = language === 'fr'
            ? ['lu', 'ma', 'me', 'je', 've', 'sa', 'di']
            : ['mo', 'tu', 'we', 'th', 'fr', 'sa', 'su'];

        const labels = weekdayLabels.map((l, i) => (
            <div key={`label-${i}`} className="calendar-weekday">{l}</div>
        ));

        // Empty slots for previous month
        for (let i = 0; i < firstDayIdx; i++) {
            days.push(<div key={`empty-${i}`} className="calendar-day empty"></div>);
        }

        const selectedDateStr = value;
        const today = new Date();
        const todayStr = formatDateISO(today);

        for (let d = 1; d <= daysInMonth; d++) {
            const date = new Date(year, month, d);
            const dateStr = formatDateISO(date);
            const isSelected = dateStr === selectedDateStr;
            const isToday = dateStr === todayStr;

            days.push(
                <div
                    key={`day-${d}`}
                    className={`calendar-day ${isSelected ? 'selected' : ''} ${isToday ? 'today' : ''}`}
                    onClick={() => handleDateSelect(date)}
                >
                    {d}
                </div>
            );
        }

        return (
            <div className="calendar-body">
                <div className="calendar-grid">
                    {labels}
                    {days}
                </div>
            </div>
        );
    };

    return (
        <div className="custom-date-picker" ref={containerRef}>
            <div
                className={`date-input-display ${isOpen ? 'active' : ''}`}
                onClick={toggleOpen}
            >
                {value ? (
                    <span>{formatDisplay(value)}</span>
                ) : (
                    <span className="date-placeholder">{placeholder || (language === 'fr' ? 'Sélectionner...' : 'Select...')}</span>
                )}
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                    {value && (
                        <button
                            type="button"
                            className="btn-icon xs"
                            style={{ padding: 0, width: '1.25rem', height: '1.25rem' }}
                            onClick={(e) => { e.stopPropagation(); onChange(''); }}
                        >
                            <X size={12} />
                        </button>
                    )}
                    <CalendarIcon size={14} style={{ color: 'var(--text-muted)' }} />
                </div>
            </div>

            {isOpen && (
                <div className={`calendar-popover ${openUpward ? 'upward' : ''}`}>
                    {renderHeader()}
                    {renderDays()}
                    <div className="calendar-footer">
                        <button
                            type="button"
                            className="calendar-footer-btn"
                            onClick={() => handleDateSelect(new Date())}
                        >
                            {language === 'fr' ? "Aujourd'hui" : "Today"}
                        </button>
                        <button
                            type="button"
                            className="calendar-footer-btn"
                            onClick={() => { onChange(''); setIsOpen(false); }}
                        >
                            {language === 'fr' ? "Effacer" : "Clear"}
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
};
