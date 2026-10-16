# Dépendances système de tri : LibRaw (RAW), libheif (HEIC/HEIF, feature `heif`),
# libjpeg-turbo (JPEG rapide, feature `turbojpeg`, compilée depuis ses sources : CMake et
# NASM requis). Voir doc/build.md pour le détail.

# Linux Mint 22 / Ubuntu 24.04 et plus récents (libheif >= 1.17 requis)
# sudo apt install libraw-dev libheif-dev libheif-plugin-libde265 zlib1g-dev cmake nasm pkg-config build-essential

# macOS
# brew install libraw libheif cmake pkg-config

# Windows (non testé), avec vcpkg
# vcpkg install libraw:x64-windows libheif:x64-windows   (+ CMake et NASM)
