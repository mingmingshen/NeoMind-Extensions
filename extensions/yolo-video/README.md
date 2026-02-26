# YOLO Video Processor - Multi-Protocol Architecture

## Overview

This extension supports real-time object detection on video streams from multiple sources using a unified URL-based interface.

## Supported Protocols

| Protocol | URL Format | Example | Status |
|----------|-----------|---------|--------|
| **Camera** | `camera://<device>?width=...&height=...` | `camera://0` | ✅ Core |
| **RTSP** | `rtsp://[user:pass@]host[:port]/path?transport=...` | `rtsp://192.168.1.100/stream` | 🔶 Optional |
| **RTMP** | `rtmp://server/app/stream_key` | `rtmp://live.example.com/live/key` | 🔶 Optional |
| **HLS** | `hls://example.com/stream.m3u8?reload=...` | `hls://example.com/live/stream.m3u8` | 🔶 Optional |
| **HTTP-MPEGTS** | `http-ts://example.com/stream.ts` | `http-ts://example.com/stream` | 🔶 Optional |
| **File** | `file:///path/to/video.mp4?loop=true` | `file:///videos/test.mp4?loop=true` | 🔶 Optional |
| **Screen** | `screen://<display>?width=...&height=...` | `screen://0` | 🔶 Optional |
| **Window** | `window://<window_id>?width=...` | `window://chrome` | 🔶 Optional |

## URL Format

All video sources use a URL format: `<protocol>://[path][?params]`

### Parameters

| Param | Description | Example |
|-------|-------------|---------|
| `width` | Output width in pixels | `width=1920` |
| `height` | Output height in pixels | `height=1080` |
| `fps` | Target frames per second | `fps=30` |
| `transport` | RTSP transport (tcp/udp/auto) | `transport=tcp` |
| `timeout` | Connection timeout (seconds) | `timeout=10` |
| `loop` | Loop video file (true/false) | `loop=true` |
| `start` | Start time for videos (seconds) | `start=30.5` |
| `reload` | HLS playlist reload interval | `reload=5` |

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Video Source Layer                               │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │ Camera  │ │  RTSP   │ │  RTMP   │ │  HLS    │ │  File   │  ...      │
│  │Adapter  │ │Adapter  │ │Adapter  │ │Adapter  │ │Adapter  │           │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘           │
│       │           │           │           │           │                 │
│       └───────────┼───────────┼───────────┼───────────┘                 │
│                   │           │           │                             │
│            ┌──────▼───────────▼───────────▼──────┐                       │
│            │       VideoSource Trait            │                       │
│            │  - info() → SourceInfo             │                       │
│            │  - start() → Result                │                       │
│            │  - read_frame(timeout) → Frame      │                       │
│            │  - stop() → Result                 │                       │
│            │  - is_active() → bool              │                       │
│            └──────────────────────┬─────────────┘                       │
│                                   │                                      │
│            ┌──────────────────────▼─────────────┐                       │
│            │       StreamProcessor              │                       │
│            │  - Frame Queue (async)             │                       │
│            │  - YOLOv8 Detection               │                       │
│            │  - Draw Bounding Boxes            │                       │
│            │  - MJPEG Encoding                 │                       │
│            └──────────────────────┬─────────────┘                       │
│                                   │                                      │
│            ┌──────────────────────▼─────────────┐                       │
│            │     MJPEG Stream Output            │                       │
│            │  /api/extensions/yolo-video/       │                       │
│            │  stream/{stream_id}                │                       │
│            └────────────────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────────────┘
```

## Build Features

```toml
# Core (always enabled)
default = ["camera", "onnx"]

# Protocol support (optional)
rtsp = ["dep:ffmpeg-next"]
rtmp = ["dep:rtmp"]
hls = ["dep:m3u8-rs", "dep:reqwest"]
http = ["dep:reqwest"]
file = ["dep:ffmpeg-next"]
screen = ["dep:screenshots"]

# Enable all protocols
all-protocols = ["rtsp", "rtmp", "hls", "http", "file", "screen"]

# Full feature set
full = ["onnx", "camera", "all-protocols"]
```

## Building

```bash
# Minimal build (Camera only)
cargo build --release -p neomind-yolo-video

# With RTSP support
cargo build --release -p neomind-yolo-video --features rtsp

# With all protocols
cargo build --release -p neomind-yolo-video --features full
```

## Usage Examples

### Frontend Component

```tsx
<YoloVideoDisplay
  sourceUrl="rtsp://192.168.1.100:554/stream?transport=tcp"
  confidenceThreshold={0.6}
  fps={15}
  drawBoxes={true}
/>
```

### API Commands

```bash
# Start RTSP stream
curl -X POST http://localhost:9375/api/extensions/yolo-video/command \
  -H "Content-Type: application/json" \
  -d '{
    "command": "start_stream",
    "args": {
      "source_url": "rtsp://192.168.1.100/stream",
      "confidence_threshold": 0.5,
      "target_fps": 15,
      "draw_boxes": true
    }
  }'

# Response: { "success": true, "data": { "stream_id": "...", "stream_url": "..." } }

# Get stream stats
curl -X POST http://localhost:9375/api/extensions/yolo-video/command \
  -H "Content-Type: application/json" \
  -d '{
    "command": "get_stream_stats",
    "args": { "stream_id": "..." }
  }'

# Stop stream
curl -X POST http://localhost:9375/api/extensions/yolo-video/command \
  -H "Content-Type: application/json" \
  -d '{
    "command": "stop_stream",
    "args": { "stream_id": "..." }
  }'
```

## Adding New Protocols

1. **Create a new adapter module** (`src/video_source/your_protocol.rs`)

```rust
pub struct YourProtocolSource {
    info: SourceInfo,
    active: bool,
    // ... protocol-specific fields
}

#[async_trait]
impl VideoSource for YourProtocolSource {
    fn info(&self) -> &SourceInfo { &self.info }
    async fn start(&mut self) -> Result<(), String> { ... }
    async fn stop(&mut self) -> Result<(), String> { ... }
    async fn read_frame(&mut self, timeout: Duration) -> FrameResult { ... }
    fn is_active(&self) -> bool { self.active }
}
```

2. **Add to SourceType enum** in `src/video_source.rs`

```rust
pub enum SourceType {
    // ...
    YourProtocol {
        url: String,
        // ... params
    },
}
```

3. **Add factory implementation** in `SourceFactory::create()`

```rust
SourceType::YourProtocol { url, ... } => {
    your_protocol::YourProtocolSource::new(&url, ...)
        .await
        .map(|s| Box::new(s) as Box<dyn VideoSource>)
}
```

4. **Add Cargo feature** to `Cargo.toml`

```toml
[features]
your_protocol = ["dep:your-dependency"]
```

## Performance Considerations

| Source | CPU Usage | Memory | Latency |
|--------|-----------|--------|---------|
| Camera | Low | Low | ~50ms |
| RTSP | Medium | Medium | ~100ms |
| HLS | Medium | Medium | ~2-5s |
| File | Low | Low | Variable |
| Screen | High | High | ~100ms |

## Troubleshooting

### RTSP Connection Issues
- Try different transport: `?transport=udp`
- Increase timeout: `?timeout=30`
- Check firewall: TCP 554

### HLS Stream Lag
- Reduce reload interval: `?reload=2`
- Use HTTP-MPEGTS if available (lower latency)

### Camera Access Denied
- Check permissions
- Try different device index: `camera://1`
- Reduce resolution: `?width=1280&height=720`
