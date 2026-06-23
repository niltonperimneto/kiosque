Name:           kiosque
Version:        0.8
Release:        1%{?dist}
Summary:        A modern, lightweight Flatpak software center for KDE Plasma

License:        GPLv3+
URL:            https://github.com/Kiosque/kiosque
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  meson
BuildRequires:  gcc-c++
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  cmake
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtdeclarative-devel
BuildRequires:  kf6-kirigami-devel
BuildRequires:  kf6-ki18n-devel
BuildRequires:  dbus-devel

Requires:       qt6-qtdeclarative
Requires:       kf6-kirigami

%description
A modern, lightweight Flatpak software center for KDE Plasma.
Built with Rust and Kirigami.

%prep
%autosetup

%build
%meson
%meson_build

%install
%meson_install
%find_lang %{name}

%files -f %{name}.lang
%{_bindir}/kiosque
%{_bindir}/kiosque-update
%{_datadir}/applications/org.kiosque.Kiosque.desktop
%{_datadir}/icons/hicolor/scalable/apps/org.kiosque.Kiosque.svg

%changelog
* Fri Jun 19 2026 Kiosque Contributors <contributors@kiosque.org> - 0.1.0-1
- Initial RPM release
