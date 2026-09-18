# Bundled Qt GUI assets

Copied byte-for-byte from `apps/osu-radio-gui-vizia/assets/` on 2026-09-17.
All runtime asset references use compiled Qt resources; launching either mode
requires no downloads or files in the working directory.

- `fonts/Poppins-{Regular,Medium,SemiBold,Bold}.ttf`: original static fonts from
  [Google Fonts / Poppins](https://github.com/google/fonts/tree/main/ofl/poppins).
  See [Poppins-OFL.txt](fonts/Poppins-OFL.txt) for the SIL Open Font License.
- `fonts/Nunito-Variable.ttf`: existing fallback font; see [OFL.txt](fonts/OFL.txt).
- `icons/*.svg`: original, unmodified 24px SVGs from
  [Lucide](https://github.com/lucide-icons/lucide/tree/main/icons).
  [LUCIDE-LICENSE](icons/LUCIDE-LICENSE) includes the Lucide and Feather notices.
  QML applies tint at rendering time; the source SVG bytes are unchanged.
- `covers/karakara.jpg`, `alice.jpg`, `rabbit.jpg`, and `bbbb.png`: existing
  repository reference artwork, copied for the bundled mock Songs view.
  Original artist/source/license attribution is not recorded in the existing
  asset directory; no additional ownership or redistribution license is claimed.
