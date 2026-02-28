import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { useApp } from './AppContext';
import { ConflictEntry } from '../types';

export type DialogType = 'alert' | 'confirm' | 'prompt' | 'properties' | 'conflict' | 'about' | 'delete' | 'search' | 'duplicates' | 'mapNetworkDrive' | 'disconnectNetworkDrive';

export interface DialogRequest {
    id: string;
    type: DialogType;
    props: any;
    resolve?: (value: any) => void;
}

export interface DialogContextType {
    dialogs: DialogRequest[];
    openDialog: <T = any>(type: DialogType, props: any) => Promise<T>;
    closeDialog: (id: string, result?: any) => void;

    // Quick helpers
    alert: (message: string, title?: string) => Promise<void>;
    confirm: (message: string, title?: string, isDanger?: boolean, confirmLabel?: string, sources?: string[], destination?: string, subMessage?: string) => Promise<boolean>;
    prompt: (message: string, title?: string, defaultValue?: string) => Promise<string | null>;
    // Semantic Helpers
    openPropertiesDialog: (paths: string[]) => void;
    openAboutDialog: () => void;
    openRenameDialog: (path: string) => Promise<string | null>;
    openNewFolderDialog: (props: { parentPath: string, onCreate: (name: string) => void }) => void;
    openDeleteDialog: (props: { paths: string[], onConfirm: () => void, isPermanent?: boolean }) => void;
    openConflictDialog: (props: { conflicts: any[], onResolve: (resolutions: any) => void, operation?: string, totalCount?: number }) => void;
    openSearchDialog: (props: { initialRoot: string, initialOptions?: any }) => Promise<any>;
    openDuplicateSearch: (props: { initialRoot: string }) => void;
    closeAllDialogs: () => void;
    propertiesPaths: string[]; // Expose currently open properties paths for UI highlighting

    openMapNetworkDriveDialog: () => Promise<void>;
    openDisconnectNetworkDriveDialog: () => Promise<void>;
}

const DialogContext = createContext<DialogContextType | null>(null);

export const DialogProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [dialogs, setDialogs] = useState<DialogRequest[]>([]);
    const { t } = useApp();

    const openDialog = useCallback(<T = any>(type: DialogType, props: any): Promise<T> => {
        return new Promise((resolve) => {
            const id = Math.random().toString(36).substring(7);
            setDialogs(prev => [...prev, { id, type, props, resolve }]);
        });
    }, []);

    const closeDialog = useCallback((id: string, result?: any) => {
        setDialogs(prev => {
            const dialog = prev.find(d => d.id === id);
            if (dialog && dialog.resolve) {
                dialog.resolve(result);
            }
            return prev.filter(d => d.id !== id);
        });
    }, []);

    // Quick helpers implementation
    const alert = useCallback((message: string, title?: string) => {
        return openDialog<void>('alert', { message, title });
    }, [openDialog]);

    const confirm = useCallback((message: string, title?: string, isDanger?: boolean, confirmLabel?: string, sources?: string[], destination?: string, subMessage?: string) => {
        return openDialog<boolean>('confirm', { message, title, isDanger, confirmLabel, sources, destination, subMessage });
    }, [openDialog]);

    const prompt = useCallback((message: string, title?: string, defaultValue?: string) => {
        return openDialog<string | null>('prompt', { message, title, defaultValue });
    }, [openDialog]);

    const openPropertiesDialog = useCallback((paths: string[]) => {
        openDialog('properties', { paths });
    }, [openDialog]);

    const openAboutDialog = useCallback(() => {
        openDialog('about', {});
    }, [openDialog]);

    const openRenameDialog = useCallback((path: string) => {
        // Rename usually uses a prompt, but we might want a specific 'rename' type if we have complex logic
        // For now, let's use 'prompt' but wrap it nicely
        // OR we can simple use 'prompt' and return the promise
        const currentName = path.split(/[/\\]/).pop() || ''; // Handles both / and \
        // Translations: rename_label or generic?
        // i18n has 'rename' (title), 'rename_label' (New name)
        return prompt(t('rename_label'), t('rename'), currentName);
    }, [prompt, t]);

    const openNewFolderDialog = useCallback(({ onCreate }: { onCreate: (name: string) => void }) => {
        // i18n: enter_folder_name, new_folder, new_folder_placeholder
        prompt(t('enter_folder_name'), t('new_folder'), t('new_folder_placeholder')).then(name => {
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

    const propertiesPaths = dialogs.find(d => d.type === 'properties')?.props.paths || [];

    return (
        <DialogContext.Provider value={{
            dialogs,
            openDialog,
            closeDialog,
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
            openDisconnectNetworkDriveDialog
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

