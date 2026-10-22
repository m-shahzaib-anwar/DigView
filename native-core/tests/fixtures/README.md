# Generated video thumbnail fixture

`thumbnail-red.mp4` is a generated one-second, silent 160×90 red H.264/YUV420P
video. It contains no user files or third-party footage. Generated with:

```powershell
ffmpeg -hide_banner -loglevel error -f lavfi -i 'color=c=red:s=160x90:d=1' -c:v libx264 -pix_fmt yuv420p -movflags +faststart -y native-core/tests/fixtures/thumbnail-red.mp4
```

The Windows native test extracts a real frame through Shell, checks its bounds
and red centre pixel, and uses a filename containing quotes and shell characters.
FFmpeg is only a fixture-generation tool; the app and tests do not invoke it.
