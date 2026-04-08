import { Language } from './types';

import { en } from './i18n/locales/en';
import { fr } from './i18n/locales/fr';
import { es } from './i18n/locales/es';
import { de } from './i18n/locales/de';
import { it } from './i18n/locales/it';

export const translations = {
    en,
    fr,
    es,
    de,
    it,
};


export type TFunc = (key: keyof typeof translations.en, params?: Record<string, string | number>) => string;

export const getT = (lang: Language): TFunc => {
    return (key, params) => {
        let text = translations[lang][key] || translations.en[key] || key;
        if (params) {
            Object.entries(params).forEach(([k, v]) => {
                text = text.replace(`{${k}}`, String(v));
            });
        }
        return text;
    };
};
