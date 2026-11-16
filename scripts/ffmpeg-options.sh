# Shared codec capabilities for both statically linked media bundles.
FFMPEG_OPTIONS=(
  --disable-network --disable-doc --disable-debug --disable-shared --enable-static
  --disable-ffplay --disable-everything
  --enable-protocol=file,pipe
  --enable-demuxer=mov,mp3,aac,flac,wav,ogg,concat,ffmetadata,image2,image2pipe,matroska,aiff,asf,ape,wv
  --enable-decoder=aac,aac_fixed,ac3,alac,ape,flac,mp3,mp3float,opus,vorbis,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be,pcm_u8,pcm_f32le,pcm_f64le,wavpack,wmav1,wmav2,mjpeg,png,bmp,ppm
  --enable-parser=aac,aac_latm,ac3,flac,mpegaudio,opus,vorbis,png,mjpeg
  --enable-encoder=aac,pcm_s16le,mjpeg,png
  --enable-muxer=ipod,mp4,mov,wav,null,pcm_s16le,image2
  --enable-filter=aresample,anull,anullsrc,sine,format,aformat,scale,transpose
  --enable-indev=lavfi --enable-swscale --enable-swresample
)
