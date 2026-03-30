import React from 'react';
import { Search, X, Loader2 } from 'lucide-react';
import cx from 'classnames';
import './SearchBox.css';

interface SearchBoxProps {
    query: string;
    placeholder: string;
    isSearching: boolean;
    onChange: (query: string) => void;
    onSubmit: () => void;
    onClear: () => void;
    clearTitle?: string;
    searchTitle?: string;
    stopTitle?: string;
    onCancel?: () => void;
    autoExpand?: boolean;
    className?: string;
}

export const SearchBox: React.FC<SearchBoxProps> = ({
    query,
    placeholder,
    isSearching,
    onChange,
    onSubmit,
    onClear,
    clearTitle = 'Clear',
    searchTitle = 'Search',
    stopTitle = 'Stop',
    onCancel,
    autoExpand = false,
    className
}) => {
    const [isFocused, setIsFocused] = React.useState(false);
    const inputRef = React.useRef<HTMLInputElement>(null);
    const debounceTimer = React.useRef<any>(null);

    // Auto-search effect
    React.useEffect(() => {
        if (debounceTimer.current) {
            clearTimeout(debounceTimer.current);
        }
        
        // Only auto-submit if there's a query and the box is focused 
        // (to prevent infinite loops on mount or external resets)
        if (query && isFocused) {
            debounceTimer.current = setTimeout(() => {
                if (query.trim().length > 0) {
                    onSubmit();
                }
            }, 350); // 350ms debounce
        }
        
        return () => {
            if (debounceTimer.current) clearTimeout(debounceTimer.current);
        };
    }, [query, isFocused]);

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Escape') {
            onClear();
        }
    };

    return (
        <div 
            className={cx("search-box", className, { 
                "auto-expand": autoExpand, 
                "focused": isFocused,
                "has-text": !!query
            })}
            onMouseEnter={() => {
                if (autoExpand && !isFocused) {
                    inputRef.current?.focus();
                }
            }}
        >
            <input
                ref={inputRef}
                type="text"
                placeholder={placeholder}
                value={query}
                onChange={(e) => onChange(e.target.value)}
                onKeyDown={handleKeyDown}
                onFocus={() => setIsFocused(true)}
                onBlur={() => setIsFocused(false)}
            />
            <div className="search-actions">
                {isSearching ? (
                    <button
                        className="search-stop-btn"
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={(e) => {
                            e.stopPropagation();
                            onCancel?.();
                        }}
                        data-tooltip={stopTitle}
                    >
                        <Loader2 size={14} className="spinner" />
                        <X size={14} className="stop-icon" />
                    </button>
                ) : (
                    <>
                        {query ? (
                            <button
                                className="clear-search-btn"
                                onMouseDown={(e) => e.preventDefault()}
                                onClick={onClear}
                                data-tooltip={clearTitle}
                            >
                                <X size={14} />
                            </button>
                        ) : (
                            <button
                                className="search-submit-btn"
                                onMouseDown={(e) => e.preventDefault()}
                                onClick={(e) => {
                                    e.stopPropagation();
                                    if (autoExpand) {
                                        if (!isFocused) {
                                            inputRef.current?.focus();
                                        } else {
                                            inputRef.current?.blur();
                                        }
                                    }
                                }}
                                data-tooltip={searchTitle}
                            >
                                <Search size={14} />
                            </button>
                        )}
                    </>
                )}
            </div>
        </div>
    );
};

