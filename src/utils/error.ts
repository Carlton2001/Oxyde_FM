
export interface CommandError {
    IoError?: string;
    PathError?: string;
    SystemError?: string;
    ArchiveError?: string;
    TrashError?: string;
    Other?: string;
    [key: string]: string | undefined;
}


export function formatCommandError(error: unknown): string {
    if (typeof error === 'string') {
        try {
            const parsed = JSON.parse(error);
            if (typeof parsed === 'object' && parsed !== null) {
                return formatCommandError(parsed);
            }
        } catch {
            // Not a JSON string
        }
        return error;
    }

    if (error === null || error === undefined) {
        return 'Unknown Error';
    }

    if (typeof error === 'object') {
        // Cast to any to access properties safely
        const err = error as any;

        // Check for specific backend error variants
        if (err.IoError) return `I/O Error: ${err.IoError}`;
        if (err.PathError) return `Path Error: ${err.PathError}`;
        if (err.SystemError) return `System Error: ${err.SystemError}`;
        if (err.ArchiveError) return `Archive Error: ${err.ArchiveError}`;
        if (err.TrashError) return `Trash Error: ${err.TrashError}`;
        if (err.Other) return `Error: ${err.Other}`;

        // Standard JS Error or Tauri Error with 'message'
        if (typeof err.message === 'string') {
            return err.message;
        }

        // Check for single key object (enum variant)
        const keys = Object.keys(err);
        if (keys.length === 1) {
            const key = keys[0];
            const value = err[key];
            if (typeof value === 'string') {
                return `${key}: ${value}`;
            }
        }

        // Try to stringify
        try {
            return JSON.stringify(error, null, 2);
        } catch {
            return 'Non-serializable Error Object';
        }
    }

    return String(error);
}
