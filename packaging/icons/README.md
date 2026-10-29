# Package icons

Derived from the unmodified
[lazer.png](../../apps/osu-radio-qt/assets/logos/lazer.png) (1024×1024 RGBA).
No new artwork: the images are plain downscales of that logo. Its provenance and
the maintainer's redistribution permission are in
[SOURCES.md](../../apps/osu-radio-qt/assets/SOURCES.md) and the
[distribution notices](../arch/licenses/README.md); the derived files carry the
same status, not the project's MIT license and not a separate image license.

- `osu-radio.ico` — Windows: 16, 24, 32, 48, 64, 128 (32-bit BMP frames) and 256 (PNG frame).
- `hicolor/<N>x<N>/apps/osu-radio.png` — Linux, N = 16, 24, 32, 48, 64, 128, 256, 512;
  installable as-is under `/usr/share/icons/hicolor` and usable for AppImage.

Regenerate with Pillow (tested with 13.0.0; Lanczos resampling):

```sh
python3 -m venv /tmp/icons-venv && /tmp/icons-venv/bin/pip install pillow
/tmp/icons-venv/bin/python packaging/icons/generate.py
```

The source is 1024px, so the largest output (512px) is not upscaled.
