# Bundled fonts

The primitive storefront components (`StorefrontAppCard.qml`, `HeroBanner.qml`)
get their typography from the `Style` singleton (`qml/style/Style.qml`), which
loads a custom variable font via `FontLoader`.

Until a face is dropped in here, `Style.fontFamily` **falls back to the Kirigami
default system font**, so the UI renders correctly either way.

## To enable the custom font (recommended: Inter, SIL OFL 1.1)

1. Download Inter and place the variable face here as:

   ```
   qml/fonts/Inter-Variable.ttf
   ```

   (Also drop the upstream `OFL.txt` license alongside it.)

2. Register it in `resources.qrc` under the `<qresource prefix="/">` block:

   ```xml
   <file>qml/fonts/Inter-Variable.ttf</file>
   ```

3. Set `Style.fontSource` to `"qrc:/qml/fonts/Inter-Variable.ttf"` in
   `qml/style/Style.qml`, then rebuild. `Style._loader` will report
   `FontLoader.Ready` and
   `Style.fontFamily` will switch to the loaded family automatically.

If you prefer a different face, update `fontSource` in
`qml/style/Style.qml` to match the filename you register here.
