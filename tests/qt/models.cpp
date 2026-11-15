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
