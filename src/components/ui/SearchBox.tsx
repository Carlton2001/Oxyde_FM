import React from 'react';
import { Search, X, Loader2 } from 'lucide-react';
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
}

export const SearchBox: React.FC<SearchBoxProps> = ({
    query,
    placeholder,
    isSearching,
    onChange,
    onSubmit,
    onClear,
    clearTitle = 'Clear',
    searchTitle = 'Search'
}) => {
    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            onSubmit();
        }
        if (e.key === 'Escape') {
            onClear();
        }
    };

    return (
        <div className="search-box">
            <input
                type="text"
                placeholder={placeholder}
                value={query}
                onChange={(e) => onChange(e.target.value)}
                onKeyDown={handleKeyDown}
            />
            <div className="search-actions">
                {isSearching ? (
                    <Loader2 size={14} className="spinner" />
                ) : (
                    <>
                        {query && (
                            <button
                                className="clear-search-btn"
                                onClick={onClear}
                                data-tooltip={clearTitle}
                            >
                                <X size={14} />
                            </button>
                        )}
                        <button
                            className="search-submit-btn"
                            onClick={onSubmit}
                            data-tooltip={searchTitle}
                        >
                            <Search size={14} />
                        </button>
                    </>
                )}
            </div>
        </div>
    );
};

