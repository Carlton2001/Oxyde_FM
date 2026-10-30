# Packages the files that scripts/build-linux-packages.sh staged.
%global debug_package %{nil}

Name:           oxyde
Version:        %{oxyde_version}
Release:        1%{?dist}
Summary:        Modern and fast file manager
# Oxyde, the Rust crates linked on Linux and the embedded assets.
License:        %{oxyde_license}
URL:            https://github.com/Carlton2001/Oxyde_FM
ExclusiveArch:  x86_64

# Loaded at run time, so RPM does not find them in the executables. Without bubblewrap
# and libseccomp, decoders would run unconfined.
Requires:       bubblewrap
Requires:       libgio-2.0.so.0()(64bit)
Requires:       libfontconfig.so.1()(64bit)
Requires:       libseccomp.so.2()(64bit)
Requires:       libwayland-client.so.0()(64bit)
Requires:       vulkan-loader
Requires:       xdg-utils
# Mesa also provides lavapipe, the software fallback of GPUI's Vulkan rendering.
Requires:       mesa-vulkan-drivers
Requires:       hicolor-icon-theme
# Fallback for characters missing from the embedded Inter and JetBrains Mono.
Requires:       default-fonts-core-sans
Recommends:     gstreamer1-plugins-base
Recommends:     gstreamer1-plugins-good
Recommends:     gvfs
Recommends:     gvfs-fuse
Recommends:     gvfs-smb
Recommends:     poppler-glib
Recommends:     udisks2

%description
Oxyde is a native file manager with multiple panes, tabs, integrated previews,
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

%changelog
* %{oxyde_date} Carlton2001 - %{version}-1
- Oxyde %{version}; release notes in the AppStream metadata.
