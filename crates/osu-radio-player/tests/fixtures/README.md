These original test tones were generated with FFmpeg (no third-party audio).
All are 1.5-second, 44.1 kHz stereo sine waves. `cbr.mp3` uses 128 kbps CBR at
440 Hz; `vbr.mp3` uses VBR quality 4 at 660 Hz. `vorbis.ogg` uses Ogg/Vorbis
quality 4 at 550 Hz; `pcm.wav` uses signed 16-bit PCM at 880 Hz. Generate them with:

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=44100:duration=1.5' -ac 2 -codec:a libmp3lame -b:a 128k cbr.mp3
ffmpeg -f lavfi -i 'sine=frequency=660:sample_rate=44100:duration=1.5' -ac 2 -codec:a libmp3lame -q:a 4 vbr.mp3
ffmpeg -f lavfi -i 'sine=frequency=550:sample_rate=44100:duration=1.5' -ac 2 -codec:a libvorbis -q:a 4 vorbis.ogg
ffmpeg -f lavfi -i 'sine=frequency=880:sample_rate=44100:duration=1.5' -ac 2 -codec:a pcm_s16le pcm.wav
```

Tests cover MP3, Ogg/Vorbis and PCM WAV, including extensionless copies, seek,
EOF/replay and replacement across formats. Tests decode the committed fixtures
and consume the actual Rodio mixer without opening a physical audio output. FFmpeg is not required to run tests.
