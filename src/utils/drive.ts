import { DriveInfo } from '../types';

/**
 * Gets the display name for a drive.
 * For network drives, shows the last directory of the remote path plus the drive letter.
 */
export const getDriveDisplayName = (drive: DriveInfo, t: any): string => {
    if (drive.path === 'trash://') return t('recycle_bin');
    if (drive.path === '__network_vincinity__') return t('network_vincinity');

    const pathClean = drive.path.replace(/[/\\]+$/, '');

    if (drive.drive_type === 'remote') {
        const sourcePath = drive.remote_path || drive.path;
        const parts = sourcePath.split(/[/\\]/).filter(Boolean);
        if (parts.length > 0) {
            return `${parts[parts.length - 1]} (${pathClean})`;
        }
    }

    return drive.label ? `${drive.label} (${pathClean})` : pathClean;
};

/**
 * Gets the tooltip for a drive.
 * For network drives, shows ONLY the source network path.
 */
export const getDriveTooltip = (drive: DriveInfo, t: any): string => {
    if (drive.path === 'trash://') return t('recycle_bin');
    if (drive.path === '__network_vincinity__') return t('network_vincinity');

    if (drive.drive_type === 'remote') {
        return drive.remote_path || drive.path;
    }

    return drive.label ? `${drive.label} (${drive.path})` : drive.path;
};

/**
 * Gets just the clean drive letter/path (e.g. 'C:', 'D:', or 'trash://')
 */
export const getDriveLetter = (drive: { path: string }): string => {
    if (drive.path === 'trash://') return '';
    if (drive.path === '__network_vincinity__') return '';
    return drive.path.replace(/[/\\]+$/, '');
};

/**
 * Gets just the name part without the letter
 */
export const getDriveNameOnly = (drive: DriveInfo, t: any): string => {
    if (drive.path === 'trash://') return t('recycle_bin');
    if (drive.path === '__network_vincinity__') return t('network_vincinity');

    const displayName = getDriveDisplayName(drive, t);
    const lastParen = displayName.lastIndexOf(' (');
    if (lastParen === -1) return displayName;
    return displayName.substring(0, lastParen);
};

/**
 * Returns true if the drive should show standard capacity info in the tooltip.
 * Network drives usually don't show this in the same way or format.
 */
export const shouldShowDriveCapacity = (drive: DriveInfo): boolean => {
    return drive.drive_type !== 'remote' && drive.path !== 'trash://' && drive.path !== '__network_vincinity__';
};
