import React, { createContext, useContext, useState, useCallback, ReactNode, useRef } from 'react';
import { useApp } from './AppContext';
import { ConflictEntry } from '../types';

export type DialogType = 'alert' | 'confirm' | 'prompt' | 'properties' | 'conflict' | 'about' | 'delete' | 'search' | 'duplicates' | 'mapNetworkDrive' | 'disconnectNetworkDrive' | 'trashSettings';

export interface DialogRequest {
    id: string;
    type: DialogType;
    props: any;
    zIndex: number;
    resolve?: (value: any) => void;
}

export interface DialogContextType {
    dialogs: DialogRequest[];
    openDialog: <T = any>(type: DialogType, props: any) => Promise<T>;
    closeDialog: (id: string, result?: any) => void;
    focusDialog: (id: string) => void;

    // Quick helpers
    alert: (message: string, title?: string) => Promise<void>;
    confirm: (message: string, title?: string, isDanger?: boolean, confirmLabel?: string, sources?: string[], destination?: string, subMessage?: string) => Promise<boolean>;
    prompt: (message: string, title?: string, defaultValue?: string) => Promise<string | null>;
    // Semantic Helpers
    openPropertiesDialog: (paths: string[]) => void;
    openAboutDialog: () => void;
    openRenameDialog: (path: string) => Promise<string | null>;
    openNewFolderDialog: (props: { parentPath: string, onCreate: (name: string) => void, existingNames?: string[] }) => void;
    openDeleteDialog: (props: { paths: string[], onConfirm: () => void, isPermanent?: boolean }) => void;
    openConflictDialog: (props: { conflicts: any[], onResolve: (resolutions: any) => void, operation?: string, totalCount?: number }) => void;
    openSearchDialog: (props: { initialRoot: string, initialOptions?: any }) => Promise<any>;
    openDuplicateSearch: (props: { initialRoot: string }) => void;
    closeAllDialogs: () => void;
    propertiesPaths: string[]; // Expose currently open properties paths for UI highlighting

    openMapNetworkDriveDialog: () => Promise<void>;
    openDisconnectNetworkDriveDialog: () => Promise<void>;
    openTrashSettingsDialog: () => void;
}

const DialogContext = createContext<DialogContextType | null>(null);

export const DialogProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [dialogs, setDialogs] = useState<DialogRequest[]>([]);
    const zIndexCounter = useRef(1000); // Start high to stay above base UI
    const { t } = useApp();

    const getNextZIndex = useCallback(() => {
        zIndexCounter.current += 1;
        return zIndexCounter.current;
    }, []);

    const openDialog = useCallback(<T = any>(type: DialogType, props: any): Promise<T> => {
        return new Promise((resolve) => {
            setDialogs(prev => {
                // Check for duplicates
                let existingIndex = -1;

                if (type === 'properties' && props.paths) {
                    const paths = props.paths as string[];
                    existingIndex = prev.findIndex(d =>
                        d.type === 'properties' &&
                        Array.isArray(d.props.paths) &&
                        d.props.paths.length === paths.length &&
                        d.props.paths.every((p: string, i: number) => p === paths[i])
                    );
                } else if (['about', 'search', 'duplicates', 'mapNetworkDrive', 'disconnectNetworkDrive'].includes(type)) {
                    existingIndex = prev.findIndex(d => d.type === type);
                }

                if (existingIndex !== -1) {
                    // Update z-index of existing dialog without reordering
                    const updated = [...prev];
                    updated[existingIndex] = {
                        ...updated[existingIndex],
                        zIndex: getNextZIndex()
                    };
                    resolve(undefined as any);
                    return updated;
                }

                const id = Math.random().toString(36).substring(7);
                return [...prev, { id, type, props, zIndex: getNextZIndex(), resolve }];
            });
        });
    }, [getNextZIndex]);

    const closeDialog = useCallback((id: string, result?: any) => {
        setDialogs(prev => {
            const dialog = prev.find(d => d.id === id);
            if (dialog && dialog.resolve) {
                dialog.resolve(result);
            }
            return prev.filter(d => d.id !== id);
        });
    }, []);

    const focusDialog = useCallback((id: string) => {
        setDialogs(prev => {
            const index = prev.findIndex(d => d.id === id);
            if (index === -1) return prev;
            const updated = [...prev];
            updated[index] = {
                ...updated[index],
                zIndex: getNextZIndex()
            };
            return updated;
        });
    }, [getNextZIndex]);

    // Quick helpers implementation
    const alert = useCallback((message: string, title?: string) => {
        return openDialog<void>('alert', { message, title });
    }, [openDialog]);

    const confirm = useCallback((message: string, title?: string, isDanger?: boolean, confirmLabel?: string, sources?: string[], destination?: string, subMessage?: string) => {
        return openDialog<boolean>('confirm', { message, title, isDanger, confirmLabel, sources, destination, subMessage });
    }, [openDialog]);

    const prompt = useCallback((message: string, title?: string, defaultValue?: string, icon?: 'rename' | 'new_folder' | 'default') => {
        return openDialog<string | null>('prompt', { message, title, defaultValue, icon });
    }, [openDialog]);

    const openPropertiesDialog = useCallback((paths: string[]) => {
        openDialog('properties', { paths });
    }, [openDialog]);

    const openAboutDialog = useCallback(() => {
        openDialog('about', {});
    }, [openDialog]);

    const openRenameDialog = useCallback((path: string) => {
        const currentName = path.split(/[/\\]/).pop() || '';
        return prompt(t('rename_label'), t('rename'), currentName, 'rename');
    }, [prompt, t]);

    const openNewFolderDialog = useCallback(({ onCreate, existingNames }: { onCreate: (name: string) => void, existingNames?: string[] }) => {
        let defaultName = t('new_folder');

        if (existingNames && existingNames.length > 0) {
            const namesLower = new Set(existingNames.map(n => n.toLowerCase()));
            if (namesLower.has(defaultName.toLowerCase())) {
                let counter = 2;
                while (namesLower.has(`${defaultName} (${counter})`.toLowerCase())) {
                    counter++;
                }
                defaultName = `${defaultName} (${counter})`;
            }
        }

        prompt(t('enter_folder_name'), t('new_folder'), defaultName, 'new_folder' as const).then(name => {
            if (name) onCreate(name);
        });
    }, [prompt, t]);

    const openDeleteDialog = useCallback(({ paths, onConfirm, isPermanent }: { paths: string[], onConfirm: () => void, isPermanent?: boolean }) => {
        const count = paths.length;
        const title = t('confirm_delete_title' as any);

        let mainMessage = '';
        let subMessage = '';

        if (isPermanent) {
            mainMessage = count > 1
                ? t('perm_delete_confirm_multiple' as any, { count })
                : t('perm_delete_confirm_single' as any);
            // Optionally add a scary subMessage or keep it grouped
        } else {
            mainMessage = count > 1
                ? t('recycle_confirm_multiple' as any, { count })
                : t('recycle_confirm_single' as any);
        }

        // We pass subMessage if we want to visually split the text. In this case, just styling the main message handles the size issue.
        confirm(mainMessage, title, true, undefined, paths, undefined, subMessage).then(confirmed => {
            if (confirmed) onConfirm();
        });
    }, [confirm, t]);

    const openConflictDialog = useCallback(({ conflicts, onResolve, operation, totalCount }: { conflicts: ConflictEntry[], onResolve: (resolutions: any) => void, operation?: string, totalCount?: number }) => {
        openDialog('conflict', { conflicts, operation, totalCount }).then(onResolve);
    }, [openDialog]);

    const openSearchDialog = useCallback((props: { initialRoot: string, initialOptions?: any }) => {
        return openDialog('search', props);
    }, [openDialog]);

    const openDuplicateSearch = useCallback((props: { initialRoot: string }) => {
        openDialog('duplicates', props);
    }, [openDialog]);

    const closeAllDialogs = useCallback(() => {
        setDialogs([]);
    }, []);

    const openMapNetworkDriveDialog = useCallback(() => {
        return openDialog<void>('mapNetworkDrive', {});
    }, [openDialog]);

    const openDisconnectNetworkDriveDialog = useCallback(() => {
        return openDialog<void>('disconnectNetworkDrive', {});
    }, [openDialog]);
    
    const openTrashSettingsDialog = useCallback(() => {
        openDialog('trashSettings', {});
    }, [openDialog]);

    const propertiesPaths = dialogs
        .filter(d => d.type === 'properties')
        .reduce((acc, d) => [...acc, ...(d.props.paths || [])], [] as string[]);

    return (
        <DialogContext.Provider value={{
            dialogs,
            openDialog,
            closeDialog,
            focusDialog,
            alert,
            confirm,
            prompt,
            openPropertiesDialog,
            openAboutDialog,
            openRenameDialog,
            openNewFolderDialog,
            openDeleteDialog,
            openConflictDialog,
            openSearchDialog,
            openDuplicateSearch,
            closeAllDialogs,
            propertiesPaths,
            openMapNetworkDriveDialog,
            openDisconnectNetworkDriveDialog,
            openTrashSettingsDialog
        }}>
            {children}
        </DialogContext.Provider>
    );
};

export const useDialogs = () => {
    const context = useContext(DialogContext);
    if (!context) {
        throw new Error('useDialogs must be used within a DialogProvider');
    }
    return context;
};

