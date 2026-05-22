# Water — application icons

Source-of-truth: `stream-mark.svg` (the React `StreamMark` component
rasterized at icon scale, with the deep-sea palette + rounded square
plate).

To regenerate the platform icons after editing `stream-mark.svg`,
use Tauri's bundled icon generator from the `app/src-tauri/`
directory:

```bash
pnpm --filter @water/app tauri icon icons/stream-mark.svg
```

That emits the full Windows/macOS/Linux set Tauri expects
(`icon.ico`, `icon.icns`, `32x32.png`, `128x128.png`, etc.) into
this directory.

Manual fallback (no Tauri CLI installed):

```bash
# Windows .ico:
magick convert -density 384 -background none stream-mark.svg \
  -define icon:auto-resize=256,128,64,48,32,16 icon.ico

# macOS .icns:
iconutil -c icns stream-mark.iconset   # build the .iconset first

# PNG sizes:
for s in 16 24 32 48 64 128 256 512; do
  rsvg-convert -w $s -h $s stream-mark.svg -o ${s}x${s}.png
done
```

Once regenerated, `tauri.conf.json`'s `bundle.icon` array picks up
the new files automatically — no config change needed unless the
filenames change.
