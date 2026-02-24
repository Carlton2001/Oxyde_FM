import React, { useState, useRef } from 'react';
import { X, Search, Folder, Calendar, Target, ChevronDown, Regex, Code, ExternalLink, Check } from 'lucide-react';
import { useDraggable } from '../../hooks/useDraggable';
import { openUrl } from '@tauri-apps/plugin-opener';
import { SearchOptions } from '../../types';
import './SearchDialog.css';

interface SearchDialogProps {
    initialRoot: string;
    initialOptions?: Partial<SearchOptions>;
    initialActiveTab?: 'general' | 'advanced' | 'help';
    onSearch: (options: SearchOptions) => void;
    onClose: () => void;
    t: any;
}

export const SearchDialog: React.FC<SearchDialogProps> = ({
    initialRoot,
    initialOptions = {},
    initialActiveTab = 'general',
    onSearch,
    onClose,
    t
}) => {
    const [query, setQuery] = useState(initialOptions.query || '');
    const [root, setRoot] = useState(initialOptions.root || initialRoot);
    const [isRegex, setIsRegex] = useState(initialOptions.regex || false);
    const [contentIsRegex, setContentIsRegex] = useState(initialOptions.contentRegex || false);
    const [isCaseSensitive, setIsCaseSensitive] = useState(initialOptions.caseSensitive || false);
    const [ignoreAccents, setIgnoreAccents] = useState(initialOptions.ignoreAccents || false);
    const [isRecursive, setIsRecursive] = useState(initialOptions.recursive !== false);
    const [searchInArchives, setSearchInArchives] = useState(initialOptions.searchInArchives || false);
    const [activeTab, setActiveTab] = useState<'general' | 'advanced' | 'help'>(initialActiveTab as any || 'general');

    const [minSize, setMinSize] = useState<number | undefined>(initialOptions.minSize !== undefined && initialOptions.sizeUnit ? Math.floor(initialOptions.minSize / {
        bytes: 1, kb: 1024, mb: 1024 * 1024, gb: 1024 * 1024 * 1024, tb: 1024 * 1024 * 1024 * 1024
    }[initialOptions.sizeUnit as 'kb']) : (initialOptions.minSize ? Math.floor(initialOptions.minSize / 1024) : undefined));

    const [maxSize, setMaxSize] = useState<number | undefined>(initialOptions.maxSize !== undefined && initialOptions.sizeUnit ? Math.floor(initialOptions.maxSize / {
        bytes: 1, kb: 1024, mb: 1024 * 1024, gb: 1024 * 1024 * 1024, tb: 1024 * 1024 * 1024 * 1024
    }[initialOptions.sizeUnit as 'kb']) : (initialOptions.maxSize ? Math.floor(initialOptions.maxSize / 1024) : undefined));

    const [sizeUnit, setSizeUnit] = useState<'bytes' | 'kb' | 'mb' | 'gb' | 'tb'>(initialOptions.sizeUnit as any || 'kb');
    const [minDate, setMinDate] = useState<string>(initialOptions.minDate ? new Date(initialOptions.minDate).toISOString().split('T')[0] : '');
    const [maxDate, setMaxDate] = useState<string>(initialOptions.maxDate ? new Date(initialOptions.maxDate).toISOString().split('T')[0] : '');
    const [contentQuery, setContentQuery] = useState(initialOptions.contentQuery || '');
    const [isUnitDropdownOpen, setIsUnitDropdownOpen] = useState(false);
    const unitDropdownRef = useRef<HTMLDivElement>(null);

    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });

    React.useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (unitDropdownRef.current && !unitDropdownRef.current.contains(event.target as Node)) {
                setIsUnitDropdownOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const handleReset = () => {
        setQuery('');
        setIsRegex(false);
        setIsCaseSensitive(false);
        setIsRecursive(true);
        setMinSize(undefined);
        setMaxSize(undefined);
        setSizeUnit('kb');
        setMinDate('');
        setMaxDate('');
        setContentQuery('');
        setContentIsRegex(false);
        setIgnoreAccents(false);
        setSearchInArchives(false);
    };

    const handleExecuteSearch = (e?: React.FormEvent) => {
        if (e) e.preventDefault();
        const unitMult = { bytes: 1, kb: 1024, mb: 1024 * 1024, gb: 1024 * 1024 * 1024, tb: 1024 * 1024 * 1024 * 1024 }[sizeUnit];
        const options: SearchOptions = {
            query, root, regex: isRegex, caseSensitive: isCaseSensitive, recursive: isRecursive,
            minSize: minSize !== undefined ? minSize * unitMult : undefined,
            maxSize: maxSize !== undefined ? maxSize * unitMult : undefined,
            minDate: minDate ? new Date(minDate).getTime() : undefined,
            maxDate: maxDate ? new Date(maxDate).getTime() : undefined,
            contentQuery: contentQuery.trim() || undefined,
            contentRegex: contentIsRegex, ignoreAccents, searchInArchives, sizeUnit
        };
        onSearch(options);
        onClose();
    };

    return (
        <div className="modal-overlay" style={{ background: 'transparent', pointerEvents: 'none' }}>
            <div
                ref={dragRef}
                className="modal search-dialog"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none',
                    pointerEvents: 'auto'
                }}
            >
                <div className="modal-header" onMouseDown={handleMouseDown}>
                    <div className="modal-title">
                        <Search size={16} />
                        <span>{t('search') || 'Advanced Search'}</span>
                    </div>
                    <button className="btn-icon" onClick={onClose}>
                        <X size={16} />
                    </button>
                </div>

                <div className="search-tabs">
                    <button
                        className={`search-tab ${activeTab === 'general' ? 'active' : ''}`}
                        onClick={() => setActiveTab('general')}
                    >
                        <Target size={14} />
                        {t('settings') || 'Settings'}
                    </button>
                    <button
                        className={`search-tab ${activeTab === 'help' ? 'active' : ''}`}
                        onClick={() => setActiveTab('help')}
                    >
                        <Regex size={14} />
                        {t('help') || 'Help'}
                    </button>
                </div>

                <form className="modal-content search-form" onSubmit={handleExecuteSearch}>
                    {activeTab === 'general' ? (
                        <div className="search-tab-content">
                            <div className="input-group">
                                <label>{t('search_for')}</label>
                                <div className="input-with-icon action-left">
                                    <div className="input-actions-hint left">
                                        <button
                                            type="button"
                                            className={`regex-badge-btn ${isRegex ? 'active' : ''}`}
                                            onClick={() => setIsRegex(!isRegex)}
                                            data-tooltip={t('use_regex' as any) || 'Use Regular Expression'}
                                            data-tooltip-pos="bottom"
                                        >
                                            <Regex size={14} />
                                        </button>
                                    </div>
                                    <input
                                        type="text"
                                        value={query}
                                        onChange={(e) => setQuery(e.target.value)}
                                        placeholder={t('search_for_placeholder')}
                                        autoFocus
                                        style={{ fontFamily: isRegex ? "'JetBrains Mono', monospace" : undefined }}
                                    />
                                </div>
                            </div>

                            <div className="input-group">
                                <label>{t('search_in')}</label>
                                <div className="input-with-icon icon-left">
                                    <Folder size={14} className="input-icon" />
                                    <input
                                        type="text"
                                        value={root}
                                        onChange={(e) => setRoot(e.target.value)}
                                    />
                                </div>
                            </div>

                            <div className="input-group" style={{ marginTop: '-0.5rem', marginBottom: '0.5rem' }}>
                                <label></label>
                                <div className="checkbox-row">
                                    <label className="prop-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={isRecursive}
                                            onChange={(e) => setIsRecursive(e.target.checked)}
                                        />
                                        <div className="checkbox-visual">
                                            {isRecursive && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('search_subfolders')}</span>
                                    </label>

                                    <label className="prop-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={searchInArchives}
                                            onChange={(e) => setSearchInArchives(e.target.checked)}
                                        />
                                        <div className="checkbox-visual">
                                            {searchInArchives && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('search_in_archives')}</span>
                                    </label>
                                </div>
                            </div>

                            <div className="input-group">
                                <label>{t('find_text')}</label>
                                <div className="input-with-icon action-left">
                                    <div className="input-actions-hint left">
                                        <button
                                            type="button"
                                            className={`regex-badge-btn ${contentIsRegex ? 'active' : ''}`}
                                            onClick={() => setContentIsRegex(!contentIsRegex)}
                                            data-tooltip={t('use_regex' as any) || 'Use Regular Expression'}
                                            data-tooltip-pos="bottom"
                                        >
                                            <Regex size={14} />
                                        </button>
                                    </div>
                                    <input
                                        type="text"
                                        value={contentQuery}
                                        onChange={(e) => setContentQuery(e.target.value)}
                                        placeholder={t('find_text_placeholder')}
                                        style={{ fontFamily: contentIsRegex ? "'JetBrains Mono', monospace" : undefined }}
                                    />
                                </div>
                            </div>

                            <div className="input-group">
                                <label>{t('size_range')}</label>
                                <div className="range-inputs-row">
                                    <input
                                        type="number"
                                        value={minSize || ''}
                                        onChange={(e) => setMinSize(e.target.value ? parseInt(e.target.value) : undefined)}
                                        placeholder={t('min')}
                                    />
                                    <span className="range-separator">—</span>
                                    <input
                                        type="number"
                                        value={maxSize || ''}
                                        onChange={(e) => setMaxSize(e.target.value ? parseInt(e.target.value) : undefined)}
                                        placeholder={t('max')}
                                    />
                                    <div className="custom-unit-selector" ref={unitDropdownRef}>
                                        <div
                                            className="unit-selected-value"
                                            onClick={() => setIsUnitDropdownOpen(!isUnitDropdownOpen)}
                                        >
                                            {t(`unit_${sizeUnit}` as any)}
                                            <ChevronDown size={12} className={`arrow ${isUnitDropdownOpen ? 'open' : ''}`} />
                                        </div>
                                        {isUnitDropdownOpen && (
                                            <div className="unit-dropdown-list">
                                                {(['bytes', 'kb', 'mb', 'gb', 'tb'] as const).map((unit) => (
                                                    <div
                                                        key={unit}
                                                        className={`unit-option ${sizeUnit === unit ? 'active' : ''}`}
                                                        onClick={() => {
                                                            setSizeUnit(unit);
                                                            setIsUnitDropdownOpen(false);
                                                        }}
                                                    >
                                                        {t(`unit_${unit}` as any)}
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            </div>

                            <div className="input-group">
                                <label>{t('date_range')}</label>
                                <div className="range-inputs-row">
                                    <div className="input-with-icon action-right">
                                        <input
                                            type="date"
                                            value={minDate}
                                            onChange={(e) => setMinDate(e.target.value)}
                                        />
                                        <div className="input-actions-hint right">
                                            <button
                                                type="button"
                                                className="regex-badge-btn"
                                                onClick={(e) => (e.currentTarget.parentElement?.previousElementSibling as HTMLInputElement).showPicker()}
                                            >
                                                <Calendar size={14} />
                                            </button>
                                        </div>
                                    </div>
                                    <span className="range-separator">—</span>
                                    <div className="input-with-icon action-right">
                                        <input
                                            type="date"
                                            value={maxDate}
                                            onChange={(e) => setMaxDate(e.target.value)}
                                        />
                                        <div className="input-actions-hint right">
                                            <button
                                                type="button"
                                                className="regex-badge-btn"
                                                onClick={(e) => (e.currentTarget.parentElement?.previousElementSibling as HTMLInputElement).showPicker()}
                                            >
                                                <Calendar size={14} />
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div className="separator-row"></div>

                            <div className="input-group">
                                <label>{t('global_options')}</label>
                                <div className="checkbox-row">
                                    <label className="prop-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={isCaseSensitive}
                                            onChange={(e) => setIsCaseSensitive(e.target.checked)}
                                        />
                                        <div className="checkbox-visual">
                                            {isCaseSensitive && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('case_sensitive')}</span>
                                    </label>

                                    <label className="prop-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={ignoreAccents}
                                            onChange={(e) => setIgnoreAccents(e.target.checked)}
                                        />
                                        <div className="checkbox-visual">
                                            {ignoreAccents && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('ignore_accents')}</span>
                                    </label>
                                </div>
                            </div>
                        </div>
                    ) : (
                        <div className="search-tab-content help-tab-content">
                            <div className="help-section">
                                <div className="help-section-title">
                                    <div className="help-title-left">
                                        <Code size={14} />
                                        <span>{t('regex_help_title')}</span>
                                    </div>
                                    <a
                                        href="#"
                                        className="regex-link"
                                        onClick={(e) => {
                                            e.preventDefault();
                                            openUrl('https://regex101.com');
                                        }}
                                    >
                                        regex101.com
                                        <ExternalLink size={10} style={{ marginLeft: '0.25rem' }} />
                                    </a>
                                </div>
                                <div className="regex-help-list">
                                    {[
                                        { pattern: '.', label: 'regex_any_char' },
                                        { pattern: '\\d', label: 'regex_digit' },
                                        { pattern: '\\w', label: 'regex_word_char' },
                                        { pattern: '\\s', label: 'regex_whitespace' },
                                        { pattern: '^', label: 'regex_start_line' },
                                        { pattern: '$', label: 'regex_end_line' },
                                        { pattern: '+', label: 'regex_one_or_more' },
                                        { pattern: '*', label: 'regex_zero_or_more' },
                                        { pattern: '?', label: 'regex_optional' },
                                        { pattern: '|', label: 'regex_either_or' },
                                        { pattern: '()', label: 'regex_group' },
                                    ].map((item, idx) => (
                                        <div key={idx} className="regex-help-item">
                                            <code className="regex-code">{item.pattern}</code>
                                            <div className="regex-label">{t(item.label as any)}</div>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </div>
                    )}
                </form>

                <div className="modal-footer" style={{ justifyContent: 'space-between' }}>
                    <div className="footer-left">
                        <button className="btn secondary" onClick={handleReset}>
                            {t('reset') || 'Reset'}
                        </button>
                    </div>
                    <div className="footer-right" style={{ display: 'flex', gap: '0.75rem' }}>
                        <button className="btn secondary" onClick={onClose}>
                            {t('cancel') || 'Cancel'}
                        </button>
                        <button className="btn primary" onClick={() => handleExecuteSearch()}>
                            <Search size={14} style={{ marginRight: '0.5rem' }} />
                            {t('start_search') || 'Start Search'}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};
