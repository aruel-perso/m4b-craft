#!/usr/bin/env bash
# Build a small self-contained Windows media backend from unmodified FFmpeg source.
# Developer prerequisites: make, x86_64-w64-mingw32-gcc, tar, xz, POSIX shell.
# End users need none of these: package ffmpeg.exe/ffprobe.exe beside the app.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source_archive="${1:?Usage: build-ffmpeg-windows.sh ffmpeg-8.0.1.tar.xz zlib-1.3.1.tar.gz}"
zlib_archive="${2:?Pass the zlib-1.3.1.tar.gz source archive}"
# Verify pinned source before extraction or execution of configure/make.
printf '%s  %s\n' '05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41' "$source_archive" | sha256sum -c -
printf '%s  %s\n' '9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23' "$zlib_archive" | sha256sum -c -
out="$root/vendor/windows-media"
mkdir -p "$out"
build="${TMPDIR:-$HOME/.hermes/cache/scratch}/m4b-ffmpeg-build"
mkdir -p "$build"
tar -xf "$source_archive" -C "$build"
tar -xf "$zlib_archive" -C "$build"
cd "$build/zlib-1.3.1"
make -j1 -f win32/Makefile.gcc PREFIX=x86_64-w64-mingw32- libz.a
zlib="$build/zlib-1.3.1"
cd "$build/ffmpeg-8.0.1"
./configure \
  --prefix="$out" --enable-cross-compile --target-os=mingw32 --arch=x86_64 \
  --cross-prefix=x86_64-w64-mingw32- --disable-x86asm --disable-autodetect \
  --disable-network --disable-doc --disable-debug --disable-shared --enable-static \
  --disable-ffplay --disable-everything \
  --enable-protocol=file,pipe \
  --enable-demuxer=mov,mp3,aac,flac,wav,ogg,concat,ffmetadata,image2,image2pipe,matroska,aiff,asf,ape,wv \
  --enable-decoder=aac,aac_fixed,ac3,alac,ape,flac,mp3,mp3float,opus,vorbis,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be,pcm_u8,pcm_f32le,pcm_f64le,wavpack,wmav1,wmav2,mjpeg,png,bmp,ppm \
  --enable-parser=aac,aac_latm,ac3,flac,mpegaudio,opus,vorbis,png,mjpeg \
  --enable-encoder=aac,pcm_s16le,mjpeg,png \
  --enable-muxer=ipod,mp4,mov,wav,null,pcm_s16le,image2 \
  --enable-filter=aresample,anull,anullsrc,sine,format,aformat,scale,transpose \
  --enable-indev=lavfi --enable-swscale --enable-swresample \
  --enable-zlib --extra-cflags="-I$zlib" --extra-ldflags="-static -L$zlib"
make -j1 ffmpeg.exe ffprobe.exe
cp ffmpeg.exe ffprobe.exe "$out/"
cp config.h config_components.h ffbuild/config.mak "$out/"
cp COPYING.LGPLv2.1 LICENSE.md "$out/"
if [ "$(readlink -f "$source_archive")" != "$(readlink -f "$out/ffmpeg-8.0.1.tar.xz")" ]; then cp "$source_archive" "$out/ffmpeg-8.0.1.tar.xz"; fi
if [ "$(readlink -f "$zlib_archive")" != "$(readlink -f "$out/zlib-1.3.1.tar.gz")" ]; then cp "$zlib_archive" "$out/zlib-1.3.1.tar.gz"; fi
cp "$zlib/LICENSE" "$out/LICENSE.zlib"
sha256sum "$out/ffmpeg.exe" "$out/ffprobe.exe" "$out/ffmpeg-8.0.1.tar.xz" > "$out/SHA256SUMS"
printf 'Built bundled Windows media tools in %s\n' "$out"
