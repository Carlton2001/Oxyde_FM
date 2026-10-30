# Packages the files that scripts/build-linux-packages.sh staged.
%global debug_package %{nil}
%global source_date_epoch_from_changelog 0

Name:           oxyde
Version:        %{oxyde_version}
Release:        1%{?dist}
Summary:        Modern and fast file manager
License:        GPL-3.0-only
URL:            https://github.com/Carlton2001/Oxyde_FM
ExclusiveArch:  x86_64

# Loaded at run time, so RPM does not find them in the executables. Without bubblewrap
# and libseccomp, decoders would run unconfined.
Requires:       bubblewrap
Requires:       glib2
Requires:       libseccomp
Requires:       vulkan-loader
Requires:       xdg-utils
Recommends:     gstreamer1-plugins-base
Recommends:     gstreamer1-plugins-good
Recommends:     gvfs
Recommends:     gvfs-fuse
Recommends:     gvfs-smb
Recommends:     mesa-vulkan-drivers
Recommends:     poppler-glib
Recommends:     udisks2

%description
Oxyde is a native file manager with resizable panes, tabs, integrated previews,
advanced search, and archive browsing and editing.

%install
cp -a %{oxyde_stage}/. %{buildroot}/

%files
%license %{_datadir}/licenses/oxyde/*
%dir %{_datadir}/licenses/oxyde
%{_bindir}/oxyde
%{_bindir}/oxyde-archive-io-worker
%{_bindir}/oxyde-archive-list-worker
%{_datadir}/applications/io.github.Carlton2001.Oxyde.desktop
%{_datadir}/icons/hicolor/*/apps/io.github.Carlton2001.Oxyde.*
%{_datadir}/metainfo/io.github.Carlton2001.Oxyde.metainfo.xml
