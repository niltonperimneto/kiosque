#include <QtTest>
#include <QAbstractItemModelTester>
#include <QQmlComponent>
#include <QScopedPointer>
#include "test_setup.h"

class Models : public QObject {
    Q_OBJECT
    Setup setup;
private slots:
    void initTestCase() { setup.applicationAvailable(); }
    void modelContracts_data() {
        QTest::addColumn<QString>("type");
        QTest::addColumn<QByteArray>("method");
        QTest::newRow("apps") << QStringLiteral("AppListModel") << QByteArray("loadPopular");
        QTest::newRow("installed") << QStringLiteral("InstalledModel") << QByteArray("refresh");
        QTest::newRow("repositories") << QStringLiteral("RepoModel") << QByteArray("refresh");
    }
    void modelContracts() {
        QFETCH(QString, type); QFETCH(QByteArray, method);
        QQmlEngine engine; setup.qmlEngineAvailable(&engine);
        QQmlComponent component(&engine);
        const auto source = type == QStringLiteral("RepoModel")
            ? QStringLiteral("import QtQml\nimport com.kiosque\nQtObject { property var model: RepoModel }")
            : "import com.kiosque\n" + type + " {}";
        component.setData(source.toUtf8(), QUrl());
        QScopedPointer<QObject> object(component.create());
        QVERIFY2(object, qPrintable(component.errorString()));
        auto* model = qobject_cast<QAbstractItemModel*>(type == QStringLiteral("RepoModel")
            ? object->property("model").value<QObject*>() : object.data());
        QVERIFY(model);
        QAbstractItemModelTester tester(model, QAbstractItemModelTester::FailureReportingMode::QtTest);
        const auto property = model->metaObject()->property(model->metaObject()->indexOfProperty("loading"));
        QSignalSpy loading(model, property.notifySignal());
        QSignalSpy reset(model, &QAbstractItemModel::modelReset);
        QVERIFY(QMetaObject::invokeMethod(model, method.constData()));
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("loading").toBool(), 10000);
        QVERIFY(loading.count() >= 2);
        QVERIFY(reset.count() >= 1);
        QCOMPARE(model->thread(), QThread::currentThread());
        QVERIFY(!model->data(QModelIndex(), Qt::UserRole).isValid());
        if (type != QStringLiteral("RepoModel")) {
            QCOMPARE(model->rowCount(), 1);
            QVERIFY(model->roleNames().values().contains("appId"));
        }
    }
    void installedSizesAndUpdateChoices() {
        QQmlEngine engine; setup.qmlEngineAvailable(&engine);
        QQmlComponent component(&engine);
        component.setData("import com.kiosque\nInstalledModel {}", QUrl());
        QScopedPointer<QObject> object(component.create());
        QVERIFY2(object, qPrintable(component.errorString()));
        auto* model = qobject_cast<QAbstractItemModel*>(object.data());
        QVERIFY(model);
        const auto roles = model->roleNames();
        const int sizeRole = roles.key("size");
        const int checkedRole = roles.key("isChecked");
        const int updateRole = roles.key("hasUpdate");
        QVERIFY(QMetaObject::invokeMethod(model, "refresh"));
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("loading").toBool(), 10000);
        QCOMPARE(model->rowCount(), 1);
        // Bytes are shown with units, as the flatpak CLI does.
        QCOMPARE(model->index(0, 0).data(sizeRole).toString(), QStringLiteral("565.4 MB"));

        QVERIFY(kiosque_test_set_updates("org.kiosque.Fixture"));
        QVERIFY(QMetaObject::invokeMethod(model, "checkForUpdates"));
        QTRY_COMPARE_WITH_TIMEOUT(model->property("updates_available_count").toInt(), 1, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("checking_updates").toBool(), 10000);
        QVERIFY(model->index(0, 0).data(updateRole).toBool());
        QVERIFY(model->index(0, 0).data(checkedRole).toBool());

        // The user's choice survives later registry events and checks.
        QVERIFY(QMetaObject::invokeMethod(model, "toggleUpdateChecked", Q_ARG(int, 0)));
        QCOMPARE(model->property("updates_checked_count").toInt(), 0);
        QVERIFY(QMetaObject::invokeMethod(model, "checkForUpdates"));
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("checking_updates").toBool(), 10000);
        QVERIFY(QMetaObject::invokeMethod(model, "refresh"));
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("loading").toBool(), 10000);
        QCOMPARE(model->property("updates_available_count").toInt(), 1);
        QCOMPARE(model->property("updates_checked_count").toInt(), 0);
        QVERIFY(!model->index(0, 0).data(checkedRole).toBool());

        QVERIFY(kiosque_test_set_updates(""));
        QVERIFY(QMetaObject::invokeMethod(model, "checkForUpdates"));
        QTRY_COMPARE_WITH_TIMEOUT(model->property("updates_available_count").toInt(), 0, 10000);
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("checking_updates").toBool(), 10000);
    }
    void imageProviderDecodesOwnedPixels() {
        QTemporaryDir root;
        QImage fixture(4, 4, QImage::Format_RGBA8888); fixture.fill(Qt::red);
        const auto path = root.filePath(QStringLiteral("fixture.png"));
        QVERIFY(fixture.save(path));
        const auto id = QStringLiteral("u/") + QString::fromLatin1(QUrl::fromLocalFile(path).toEncoded().toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals));
        KiosqueImageProvider provider;
        QSize size;
        const auto decoded = provider.requestImage(id, &size, QSize(2, 2));
        QVERIFY(!decoded.isNull());
        QCOMPARE(size, QSize(2, 2));
        QCOMPARE(decoded.pixelColor(0, 0), QColor(Qt::red));
    }
    void featuredSeparatesDailyAndAvoidsUnchangedResets() {
        QQmlEngine engine; setup.qmlEngineAvailable(&engine);
        QQmlComponent component(&engine);
        component.setData("import com.kiosque\nFeaturedModel {}", QUrl());
        QScopedPointer<QObject> object(component.create());
        QVERIFY2(object, qPrintable(component.errorString()));
        auto *model = qobject_cast<QAbstractItemModel *>(object.data());
        QVERIFY(model);
        QSignalSpy resets(model, &QAbstractItemModel::modelReset);
        QVERIFY(QMetaObject::invokeMethod(model, "refresh"));
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("loading").toBool(), 10000);
        const auto daily = QJsonDocument::fromJson(model->property("aotd_json").toString().toUtf8()).object();
        QCOMPARE(daily.value("appId").toString(), QStringLiteral("org.kiosque.Daily"));
        QVERIFY(model->roleNames().values().contains("screenshotUrl"));
        QVERIFY(model->roleNames().values().contains("brandColorLight"));
        const auto idRole = model->roleNames().key("appId");
        for (int row = 0; row < model->rowCount(); ++row)
            QVERIFY(model->data(model->index(row, 0), idRole).toString() != daily.value("appId").toString());
        const int before = resets.count();
        QVERIFY(QMetaObject::invokeMethod(model, "refresh"));
        QTRY_VERIFY_WITH_TIMEOUT(!model->property("loading").toBool(), 10000);
        QCOMPARE(resets.count(), before);
    }
    void cleanupTestCase() { setup.cleanupTestCase(); }
};
QTEST_MAIN(Models)
#include "models.moc"
