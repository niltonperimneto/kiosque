Name:           kiosque
Version:        %{kiosque_version}
Release:        1%{?dist}
Summary:        A modern, lightweight Flatpak software center for KDE Plasma

License:        GPLv3+
URL:            https://github.com/Kiosque/kiosque
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  meson
BuildRequires:  desktop-file-utils
BuildRequires:  gcc-c++
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  cmake
BuildRequires:  mold
BuildRequires:  qt6-qtbase-private-devel
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtdeclarative-devel
BuildRequires:  kf6-kirigami-devel
BuildRequires:  kf6-ki18n-devel
BuildRequires:  dbus-devel
BuildRequires:  flatpak-devel

Requires:       %{name}-companion%{?_isa} = %{version}-%{release}
Requires:       qt6-qtdeclarative
Requires:       kf6-kirigami
Requires:       kf6-kirigami-addons
Requires:       kf6-qqc2-desktop-style

%description
A modern, lightweight Flatpak software center for KDE Plasma.
Built with Rust and Kirigami.

%package companion
Summary:        Background updates and host services for Kiosque

%description companion
The Kiosque host companion: automatic and scheduled Flatpak updates, update
notifications and a tray icon, KRunner search, and the D-Bus service through
which the Kiosque Flatpak manages applications on the host.

Install this package to use the Kiosque Flatpak from Flathub. The native
storefront (package kiosque) requires it as well.

%prep
%autosetup

%build
%meson
%meson_build

%install
%meson_install
%find_lang %{name}

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/org.kiosque.Kiosque.desktop
test -f %{buildroot}%{_datadir}/dbus-1/services/org.kiosque.Kiosque.Daemon.service
test -f %{buildroot}%{_datadir}/krunner/dbusplugins/org.kiosque.Kiosque.desktop

%files
%{_bindir}/kiosque
%{_datadir}/applications/org.kiosque.Kiosque.desktop
%{_datadir}/metainfo/org.kiosque.Kiosque.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/org.kiosque.Kiosque.svg

# No desktop file: with the Kiosque Flatpak installed the companion must not
# add a second "Kiosque" launcher. Translations are shared by both packages.
%files companion -f %{name}.lang
%{_bindir}/kiosque-update
%{_datadir}/dbus-1/services/org.kiosque.Kiosque.Daemon.service
%dir %{_datadir}/krunner
%dir %{_datadir}/krunner/dbusplugins
%{_datadir}/krunner/dbusplugins/org.kiosque.Kiosque.desktop
%{_datadir}/icons/hicolor/symbolic/apps/org.kiosque.Kiosque-symbolic.svg

%changelog
* Fri Jun 19 2026 Kiosque Contributors <contributors@kiosque.org> - 0.1.0-1
- Initial RPM release
