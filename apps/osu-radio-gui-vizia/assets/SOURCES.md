# Bundled GUI assets

- `fonts/Poppins-{Regular,Medium,SemiBold,Bold}.ttf`: original static fonts from
  [Google Fonts / Poppins](https://github.com/google/fonts/tree/main/ofl/poppins),
  downloaded 2026-09-17. SIL Open Font License: [Poppins-OFL.txt](fonts/Poppins-OFL.txt).
- `fonts/Nunito-Variable.ttf`: existing fallback font; [OFL.txt](fonts/OFL.txt).
- `icons/*.svg`: original, unmodified 24px SVG files from
  [Lucide](https://github.com/lucide-icons/lucide/tree/main/icons), downloaded
  2026-09-17. [LUCIDE-LICENSE](icons/LUCIDE-LICENSE) includes Lucide and Feather notices.
  The shared icon component uses Vizia's CSS `fill` tint pass to colour the SVG
  silhouette; source paths and `stroke="currentColor"` remain intact.

Fonts and icons are embedded at build time; neither GUI launch mode downloads assets.
Reference cover images are existing decoding fixtures, not gallery/library records.
