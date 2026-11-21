#pragma once
// Shared by kiosque and kiosque-update: load the "kiosque" translation domain
// from the build tree when running uninstalled, otherwise from the install prefix.

#include <QCoreApplication>
#include <QDir>
#include <KLocalizedString>

inline void setupLocalization()
{
    const QDir localeDir(QCoreApplication::applicationDirPath() + QStringLiteral("/../po"));
    if (localeDir.exists()) {
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), localeDir.absolutePath());
    } else {
#ifdef LOCALE_INSTALL_DIR
        KLocalizedString::addDomainLocaleDir(QByteArrayLiteral("kiosque"), QStringLiteral(LOCALE_INSTALL_DIR));
#endif
    }
    KLocalizedString::setApplicationDomain("kiosque");
}
