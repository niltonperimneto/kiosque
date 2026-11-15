# QML

- Preserve `com.kiosque` public types/properties and translated text through the existing localization context.
- Prefer state-based Qt Quick Test assertions and SignalSpy/tryCompare to delays or screenshot equality.
- Exercise loading, empty, error and success states plus keyboard access for changed controls.
- Run `python3 scripts/test.py qml`; it uses the actual registered bridge types and isolated services.
