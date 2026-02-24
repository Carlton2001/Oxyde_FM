import React from 'react';
import {
    Shield, Printer, Settings, Play, Search,
    Code, Terminal, GitBranch, Share2, ShieldCheck, Cast, Pin,
    HardDrive, ArchiveRestore, Archive, RefreshCw, CloudOff,
    CheckCircle, ExternalLink, Pencil
} from 'lucide-react';

export function getNativeIcon(verb?: string, label?: string): React.ComponentType<{ className?: string, size?: number | string }> | null {
    if (!verb && !label) return null;

    const v = verb?.toLowerCase() || '';
    const l = label?.toLowerCase() || '';

    // 🔨 System & Execution
    if (v === 'runas' || l.includes('administrateur') || l.includes('administrator')) return Shield;
    if (v === 'open' || l.includes('ouvrir') || l.includes('open')) return ExternalLink;
    if (v === 'print' || l.includes('imprimer') || l.includes('print')) return Printer;
    if (v === 'properties' || l.includes('propriétés') || l.includes('properties')) return Settings;
    if (v === 'play' || l.includes('jouer') || l.includes('lecture') || l.includes('play')) return Play;
    if (v === 'edit' || l.includes('modifier') || l.includes('edit')) return Pencil;
    if (v === 'find' || l.includes('rechercher') || l.includes('find')) return Search;

    // 💻 Code & Dev
    if (l.includes('code') || l.includes('visual studio')) return Code;
    if (l.includes('terminal') || l.includes('powershell') || l.includes('cmd')) return Terminal;
    if (l.includes('git ') || l.includes('github') || l.includes('commit') || l.includes('tortoisegit')) return GitBranch;

    // 📦 Common Utilities
    if (v === 'share' || l.includes('partager') || l.includes('share')) return Share2;
    if (l.includes('scan') || l.includes('defender') || l.includes('antivirus') || l.includes('malware') || l.includes('eset') || l.includes('kaspersky') || l.includes('norton') || l.includes('bitdefender')) return ShieldCheck;
    if (v === 'cast' || l.includes('diffuser') || l.includes('cast')) return Cast;
    if (v === 'pin' || l.includes('épingler') || l.includes('pin') || l.includes('accès rapide')) return Pin;
    if (v === 'format' || l.includes('formater') || l.includes('format')) return HardDrive;

    // Archiving (7-Zip, WinRAR, NanaZip)
    if (l.includes('extraire') || l.includes('extract') || l.includes('unzip')) return ArchiveRestore;
    if (l.includes('compresser') || l.includes('compress') || l.includes('zip') || l.includes('rar') || l.includes('7z') || l.includes('ajouter à')) return Archive;

    // ☁️ Cloud (OneDrive / DropBox / GDrive)
    if (l.includes('synchroniser') || l.includes('sync')) return RefreshCw;
    if (l.includes("libérer de l'espace") || l.includes('free up space')) return CloudOff;
    if (l.includes('toujours conserver') || l.includes('always keep')) return CheckCircle;

    // Default Fallback
    return null;
}
