import React, { createContext, useContext, useRef, useCallback } from 'react';
import { PanelInitialConfig } from '../types';

interface SplitConfigContextType {
    captureSplitConfig: (config: PanelInitialConfig) => void;
    consumeSplitConfig: () => PanelInitialConfig | null;
}

const SplitConfigContext = createContext<SplitConfigContextType | undefined>(undefined);

export const SplitConfigProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const lastConfigRef = useRef<PanelInitialConfig | null>(null);

    const captureSplitConfig = useCallback((config: PanelInitialConfig) => {
        lastConfigRef.current = config;
    }, []);

    const consumeSplitConfig = useCallback(() => {
        const config = lastConfigRef.current;
        lastConfigRef.current = null;
        return config;
    }, []);

    return (
        <SplitConfigContext.Provider value={{ captureSplitConfig, consumeSplitConfig }}>
            {children}
        </SplitConfigContext.Provider>
    );
};

export const useSplitConfig = () => {
    const context = useContext(SplitConfigContext);
    if (!context) {
        throw new Error('useSplitConfig must be used within a SplitConfigProvider');
    }
    return context;
};
