import React from 'react';
import {
    Folder, File, FileText, FileImage, FileVideo, Music,
    Package, Archive, Disc, FileCode, Link, ExternalLink,
    FileSpreadsheet, FileStack, Database, Terminal, Key,
    ShieldCheck, Box, HardDrive, Network, Globe
} from 'lucide-react';
import { AsyncFileIcon } from '../components/ui/AsyncFileIcon';

// Extension mappings for file type detection
export const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico', 'tiff', 'tif'];
export const PREVIEWABLE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico'];
export const PREVIEWABLE_VIDEO_EXTENSIONS = ['mp4', 'webm', 'ogv', 'mov', 'm4v', '3gp', 'mp4v'];

const AUDIO_EXTENSIONS = ['mp3', 'wav', 'ogg', 'flac', 'aac', 'm4a', 'wma', 'mid', 'midi'];
const VIDEO_EXTENSIONS = [...PREVIEWABLE_VIDEO_EXTENSIONS, 'mkv', 'avi', 'wmv', 'flv', 'mpg', 'mpeg'];
const EXECUTABLE_EXTENSIONS = ['exe', 'msi', 'msix', 'appx', 'bat', 'cmd', 'ps1', 'sh', 'jar', 'com'];
const ARCHIVE_EXTENSIONS = ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'cab', 'tgz', 'zst', 'tbz2', 'tzst', 'txz'];
export const PDF_EXTENSIONS = ['pdf'];
export const PREVIEWABLE_PDF_EXTENSIONS = [...PDF_EXTENSIONS];
export const PREVIEWABLE_OFFICE_EXTENSIONS = ['docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp'];
const DISK_EXTENSIONS = ['iso', 'img', 'vmdk', 'vhd', 'vhdx'];

// O(1) lookup Sets for icon resolution (called per-file on render)
const IMAGE_SET = new Set(IMAGE_EXTENSIONS);
const AUDIO_SET = new Set(AUDIO_EXTENSIONS);
const VIDEO_SET = new Set(VIDEO_EXTENSIONS);
const EXECUTABLE_SET = new Set(EXECUTABLE_EXTENSIONS);
const ARCHIVE_SET = new Set(ARCHIVE_EXTENSIONS);
const PDF_SET = new Set(PDF_EXTENSIONS);
const DISK_SET = new Set(DISK_EXTENSIONS);

export const CODE_EXTENSIONS = [
    // Web
    'js', 'ts', 'tsx', 'jsx', 'html', 'css', 'scss', 'sass', 'less', 'wasm',
    // Languages
    'rs', 'py', 'java', 'cpp', 'c', 'h', 'hpp', 'cs', 'go', 'php', 'rb', 'swift', 'kt', 'dart',
    'sql', 'sh', 'bat', 'ps1', 'cmd', 'vbs', 'pl', 'lua', 'r', 'm', 'f90', 'f', 'jl', 'scala',
    'elm', 'erl', 'ex', 'exs', 'clj', 'cljs', 'lisp', 'scm', 'ml', 'mli', 'asm', 's',
    // Data & Config
    'json', 'xml', 'yaml', 'yml', 'md', 'toml', 'ini', 'cfg', 'conf', 'config', 'env',
    'graphql', 'gql', 'proto', 'dockerfile', 'makefile', 'cmake', 'sln', 'csproj', 'vcxproj'
];

const TEXT_BASE_EXTENSIONS = ['txt', 'log', 'nfo', 'inf', 'ini', 'cfg', 'conf', 'config', 'bak', 'tmp', 'csv', 'tsv'];
export const PREVIEWABLE_TEXT_EXTENSIONS = [...TEXT_BASE_EXTENSIONS, ...CODE_EXTENSIONS];

const WORD_EXTENSIONS = ['doc', 'docx', 'odt', 'rtf', 'pages', 'dot', 'dotx'];
const EXCEL_EXTENSIONS = ['xls', 'xlsx', 'ods', 'csv', 'tsv', 'numbers', 'xlsm'];
const POWERPOINT_EXTENSIONS = ['ppt', 'pptx', 'odp', 'key', 'pps', 'ppsx'];
const DATABASE_EXTENSIONS = ['accdb', 'mdb', 'db', 'sqlite', 'sqlite3'];
const CERT_EXTENSIONS = ['pem', 'crt', 'cer', 'key', 'p12', 'pfx', 'pub'];
const SYSTEM_EXTENSIONS = ['dll', 'sys', 'ocx', 'drv', 'cpl', 'scr'];

// O(1) lookup Sets (continued)
const CODE_SET = new Set(CODE_EXTENSIONS);
const TEXT_SET = new Set(TEXT_BASE_EXTENSIONS);
const WORD_SET = new Set(WORD_EXTENSIONS);
const EXCEL_SET = new Set(EXCEL_EXTENSIONS);
const POWERPOINT_SET = new Set(POWERPOINT_EXTENSIONS);
const DATABASE_SET = new Set(DATABASE_EXTENSIONS);
const CERT_SET = new Set(CERT_EXTENSIONS);
const SYSTEM_SET = new Set(SYSTEM_EXTENSIONS);

interface IconOptions {
    size?: number;
    strokeWidth?: number;
}

/**
 * Returns an appropriate icon component for a file based on its extension
 * @param name - File name (used to extract extension)
 * @param isDir - Whether the file is a directory
 * @param options - Icon size and stroke width options
 * @param useSystemIcons - Whether to use the system icon protocol
 * @param path - Full path of the file (for specific icons)
 */
export const getFileIcon = (
    name: string,
    isDir: boolean,
    options: IconOptions = {},
    useSystemIcons: boolean = false,
    path?: string
): React.ReactNode => {
    const { size: pixelSize, strokeWidth = 1.5 } = options;
    const size = pixelSize ? `${pixelSize / 16}rem` : undefined;
    const iconProps = { size, strokeWidth };

    if (useSystemIcons) {
        return (
            <AsyncFileIcon
                path={path || ''}
                isDir={isDir}
                name={name}
                size={pixelSize}
                className="system-icon-img"
            />
        );
    }

    if (isDir) {
        if (path === '__network_vincinity__') {
            return <Globe className="file-icon network-root" {...iconProps} />;
        }
        if (path?.startsWith('\\\\')) {
            const parts = path.split('\\').filter(Boolean);
            if (parts.length <= 2) {
                return <Network className="file-icon network" {...iconProps} />;
            }
        }
        const isDrive = path && /^[a-zA-Z]:\\?$/.test(path);
        if (isDrive) {
            return <HardDrive className="file-icon drive" {...iconProps} />;
        }
        return <Folder className="file-icon folder" fill="currentColor" fillOpacity={0.2} {...iconProps} />;
    }

    const ext = name.split('.').pop()?.toLowerCase() || '';

    if (IMAGE_SET.has(ext)) {
        return <FileImage className="file-icon image" {...iconProps} />;
    }
    if (AUDIO_SET.has(ext)) {
        return <Music className="file-icon audio" {...iconProps} />;
    }
    if (VIDEO_SET.has(ext)) {
        return <FileVideo className="file-icon video" {...iconProps} />;
    }

    // Microsoft Office & Productivity
    if (WORD_SET.has(ext)) {
        return <FileText className="file-icon word" {...iconProps} />;
    }
    if (EXCEL_SET.has(ext)) {
        return <FileSpreadsheet className="file-icon excel" {...iconProps} />;
    }
    if (POWERPOINT_SET.has(ext)) {
        return <FileStack className="file-icon powerpoint" {...iconProps} />;
    }

    // Specialized types
    if (DATABASE_SET.has(ext)) {
        return <Database className="file-icon database" {...iconProps} />;
    }
    if (CERT_SET.has(ext)) {
        return <Key className="file-icon cert" {...iconProps} />;
    }
    if (SYSTEM_SET.has(ext)) {
        return <ShieldCheck className="file-icon system" {...iconProps} />;
    }

    // Executables and Installers
    if (ext === 'msix' || ext === 'appx') {
        return <Box className="file-icon msix" {...iconProps} />;
    }
    if (EXECUTABLE_SET.has(ext)) {
        return <Package className="file-icon executable" {...iconProps} />;
    }
    if (ext === 'bat' || ext === 'cmd' || ext === 'ps1' || ext === 'sh') {
        return <Terminal className="file-icon terminal" {...iconProps} />;
    }

    if (ARCHIVE_SET.has(ext)) {
        return <Archive className="file-icon archive" {...iconProps} />;
    }
    if (PDF_SET.has(ext)) {
        return <FileText className="file-icon pdf" {...iconProps} />;
    }
    if (DISK_SET.has(ext)) {
        return <Disc className="file-icon disk" {...iconProps} />;
    }
    if (ext === 'vhd' || ext === 'vhdx' || ext === 'vmdk') {
        return <HardDrive className="file-icon vhd" {...iconProps} />;
    }
    if (CODE_SET.has(ext)) {
        return <FileCode className="file-icon code" {...iconProps} />;
    }
    if (TEXT_SET.has(ext)) {
        return <FileText className="file-icon text" {...iconProps} />;
    }

    if (ext === 'lnk') {
        return <ExternalLink className="file-icon shortcut" {...iconProps} />;
    }
    if (['url', 'website', 'webloc'].includes(ext)) {
        return <Link className="file-icon link" {...iconProps} />;
    }

    return <File className="file-icon" {...iconProps} />;
};

/**
 * Returns an appropriate icon component for a FileEntry
 */
export const getFileEntryIcon = (
    entry: { name: string; is_dir: boolean; path?: string },
    options?: IconOptions,
    useSystemIcons: boolean = false
): React.ReactNode => {
    return getFileIcon(entry.name, entry.is_dir, options, useSystemIcons, entry.path);
};

