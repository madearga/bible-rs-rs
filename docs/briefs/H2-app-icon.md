GOAL
Give the application a real icon in the Dock and in the Finder.

SCOPE
This unit may edit:
  assets/icons/**              the render script, the iconset, the icns
  scripts/bundle-macos.sh      the Info.plist key and the Resources copy step

CONTEXT
The bundle shipped with the generic executable icon. The design is drawn from
the reader's own language: the light theme's warm paper as a rounded square
with a subtle gradient, a serif capital B set in the bundled Literata face at
about 57 percent cap height, and the accent rule under it. A Python script
renders the whole set from one 1024 master with the font rasterised directly,
so the icon is reproducible from the repository, and iconutil packs the icns.
The bundle script generates its Info.plist inline, so the CFBundleIconFile key
goes there and the icns is copied into Contents/Resources.

ACCEPTANCE
- iconutil round-trips the icns into ten PNGs, plutil accepts the generated
  plist, and the built bundle carries AppIcon.icns in Resources.
- The letter is centered, unclipped, and legible at 16 points.
- verify-distribution.sh --launch still passes every check on the rebuilt
  bundle.
