//! YOLO Video Processor Extension
//!
//! Real-time video stream processing with YOLOv11 object detection.
//! Supports ONNX runtime for real inference with automatic fallback to demo mode.

mod detector;

pub mod video_source;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use once_cell::sync::Lazy;
use uuid::Uuid;

use neomind_core::extension::system::{
    Extension, ExtensionMetadata, ExtensionError, MetricDefinition,
    ExtensionMetricValue, ParamMetricValue, MetricDataType, CommandDefinition,
    CExtensionMetadata, ABI_VERSION, Result,
};
use neomind_core::extension::{
    StreamCapability, StreamMode, StreamDirection, StreamDataType,
    StreamSession, SessionStats,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use semver::Version;

use detector::{Detection, YoloDetector};

// ============================================================================
// Embedded Model
// ============================================================================

/// Get the YOLOv11n model data
/// First tries to load from the models directory next to the extension binary,
/// otherwise returns a reference to embedded data (if available).
fn get_model_data() -> Option<Vec<u8>> {
    // Try loading from models directory relative to extension
    if let Ok(mut model_path) = std::env::current_exe() {
        model_path.pop(); // Remove binary name
        model_path.push("models");
        model_path.push("yolo11n.onnx");

        if let Ok(data) = std::fs::read(&model_path) {
            tracing::info!("Loaded YOLOv11n model from: {}", model_path.display());
            return Some(data);
        }
    }

    // Fallback: try to load from extension source directory (for development)
    #[cfg(debug_assertions)]
    {
        let dev_path = "../models/yolo11n.onnx";
        if let Ok(data) = std::fs::read(dev_path) {
            tracing::info!("Loaded YOLOv11n model from development path");
            return Some(data);
        }
    }

    tracing::warn!("YOLOv11n model not found, running in demo mode");
    None
}

/// Get model size (0 if not loaded)
fn get_model_size() -> usize {
    get_model_data().map(|d| d.len()).unwrap_or(0)
}

// ============================================================================
// Types
// ============================================================================

/// Object detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDetection {
    pub id: u32,
    pub label: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
    pub class_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Stream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub source_url: String,
    pub confidence_threshold: f32,
    pub max_objects: u32,
    pub target_fps: u32,
    pub draw_boxes: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            source_url: "camera://0".to_string(),
            confidence_threshold: 0.5,
            max_objects: 20,
            target_fps: 15,
            draw_boxes: true,
        }
    }
}

/// Stream information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub stream_id: String,
    pub stream_url: String,
    pub status: String,
    pub width: u32,
    pub height: u32,
}

/// Stream statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub stream_id: String,
    pub frame_count: u64,
    pub fps: f32,
    pub total_detections: u64,
    pub detected_objects: HashMap<String, u32>,
}

/// Active stream state
#[derive(Debug)]
struct ActiveStream {
    id: String,
    config: StreamConfig,
    started_at: Instant,
    frame_count: u64,
    total_detections: u64,
    last_frame: Option<Vec<u8>>,
    last_detections: Vec<ObjectDetection>,
    last_frame_time: Option<Instant>,
    fps: f32,
    running: bool,
    detected_objects: HashMap<String, u32>,
}

/// YOLOv11 class labels (COCO 80 classes)
const COCO_CLASSES: [&str; 80] = [
    "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck", "boat",
    "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat",
    "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack",
    "umbrella", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball",
    "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket",
    "bottle", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple",
    "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake",
    "chair", "couch", "potted plant", "bed", "dining table", "toilet", "tv", "laptop",
    "mouse", "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
    "refrigerator", "book", "clock", "vase", "scissors", "teddy bear", "hair drier", "toothbrush",
];

/// Model download URLs (mirrors for reliability)
const MODEL_URLS: &[&str] = &[
    // YOLOv11n (latest, recommended)
    "https://github.com/ultralytics/assets/releases/download/v0.0.0/yolo11n.onnx",
    // Hugging Face mirrors (as backup)
    "https://huggingface.co/Ultralytics/YOLOv11/resolve/main/yolo11n.onnx",
    "https://huggingface.co/FencerD/yolov11n-onnx/resolve/main/yolo11n.onnx",
];

/// Model configuration
#[derive(Debug, Clone, Copy)]
struct ModelConfig {
    pub name: &'static str,
    pub input_size: u32,
    pub num_classes: usize,
    pub num_boxes: usize,
}

const MODEL_CONFIG: ModelConfig = ModelConfig {
    name: "yolo11n",
    input_size: 640,
    num_classes: 80,
    num_boxes: 8400,
};

// ============================================================================
// YOLO Detector Wrapper
// ============================================================================

/// Wrapper that converts detector results to extension format
fn detections_to_object_detection(detections: Vec<Detection>) -> Vec<ObjectDetection> {
    detections
        .into_iter()
        .enumerate()
        .map(|(i, d)| ObjectDetection {
            id: i as u32,
            label: d.class_name,
            confidence: d.confidence,
            bbox: d.bbox,
            class_id: d.class_id,
        })
        .collect()
}

/// Generate mock detections (fallback when model is not loaded)
fn generate_fallback_detections(frame_count: u64, max_objects: u32) -> Vec<ObjectDetection> {
    let objects = [
        ("person", 0u32, 0.75f32),
        ("car", 2, 0.65),
        ("dog", 16, 0.70),
        ("bicycle", 1, 0.60),
        ("cat", 15, 0.68),
    ];

    let count = ((frame_count % 3) + 1) as usize;
    let offset = (frame_count as usize % objects.len()) as usize;

    objects.iter()
        .cycle()
        .skip(offset)
        .take(count.min(max_objects as usize))
        .enumerate()
        .map(|(i, (label, class_id, conf))| {
            let ox = (frame_count % 100) as f32 * 3.0 + i as f32 * 50.0;
            let oy = (frame_count % 80) as f32 * 2.0 + i as f32 * 30.0;
            ObjectDetection {
                id: i as u32,
                label: label.to_string(),
                confidence: *conf,
                bbox: BoundingBox {
                    x: 100.0 + ox,
                    y: 100.0 + oy,
                    width: 100.0 + i as f32 * 20.0,
                    height: 150.0,
                },
                class_id: *class_id,
            }
        })
        .collect()
}

/// Color palette
const BOX_COLORS: [(u8, u8, u8); 10] = [
    (239, 68, 68), (34, 197, 94), (59, 130, 246), (234, 179, 8), (6, 182, 212),
    (139, 92, 246), (236, 72, 153), (249, 115, 22), (132, 204, 22), (20, 184, 166),
];

// ============================================================================
// Metrics and Commands
// ============================================================================

static METRICS: Lazy<[MetricDefinition; 3]> = Lazy::new(|| [
    MetricDefinition {
        name: "active_streams".to_string(),
        display_name: "Active Streams".to_string(),
        data_type: MetricDataType::Integer,
        unit: "count".to_string(),
        min: Some(0.0),
        max: None,
        required: false,
    },
    MetricDefinition {
        name: "total_frames_processed".to_string(),
        display_name: "Total Frames Processed".to_string(),
        data_type: MetricDataType::Integer,
        unit: "frames".to_string(),
        min: Some(0.0),
        max: None,
        required: false,
    },
    MetricDefinition {
        name: "avg_fps".to_string(),
        display_name: "Average FPS".to_string(),
        data_type: MetricDataType::Float,
        unit: "fps".to_string(),
        min: Some(0.0),
        max: None,
        required: false,
    },
]);

static COMMANDS: Lazy<[CommandDefinition; 3]> = Lazy::new(|| [
    CommandDefinition {
        name: "start_stream".to_string(),
        display_name: "Start Video Stream".to_string(),
        payload_template: r#"{"source_url": "camera://0"}"#.to_string(),
        parameters: vec![],
        fixed_values: HashMap::new(),
        samples: vec![],
        llm_hints: "Start a video detection stream".to_string(),
        parameter_groups: vec![],
    },
    CommandDefinition {
        name: "stop_stream".to_string(),
        display_name: "Stop Video Stream".to_string(),
        payload_template: r#"{"stream_id": ""}"#.to_string(),
        parameters: vec![],
        fixed_values: HashMap::new(),
        samples: vec![],
        llm_hints: "Stop a video stream".to_string(),
        parameter_groups: vec![],
    },
    CommandDefinition {
        name: "get_stream_stats".to_string(),
        display_name: "Get Stream Statistics".to_string(),
        payload_template: r#"{"stream_id": ""}"#.to_string(),
        parameters: vec![],
        fixed_values: HashMap::new(),
        samples: vec![],
        llm_hints: "Get stream statistics".to_string(),
        parameter_groups: vec![],
    },
]);

// ============================================================================
// Global Stream Registry
// ============================================================================

struct StreamRegistry {
    streams: HashMap<String, Arc<Mutex<ActiveStream>>>,
    total_frames: u64,
    total_detections: u64,
}

impl StreamRegistry {
    fn new() -> Self {
        Self {
            streams: HashMap::new(),
            total_frames: 0,
            total_detections: 0,
        }
    }
}

static REGISTRY: Lazy<Mutex<StreamRegistry>> = Lazy::new(|| {
    Mutex::new(StreamRegistry::new())
});

// ============================================================================
// Frame Drawing Utilities
// ============================================================================

/// Draw detections on an image
pub fn draw_detections(image: &mut image::RgbImage, detections: &[ObjectDetection]) {
    use imageproc::drawing::{draw_hollow_rect_mut, draw_filled_rect_mut};
    use imageproc::rect::Rect;

    for (i, det) in detections.iter().enumerate() {
        let color = BOX_COLORS[i % BOX_COLORS.len()];
        let image_color = image::Rgb([color.0, color.1, color.2]);

        let x = det.bbox.x as i32;
        let y = det.bbox.y as i32;
        let w = det.bbox.width as u32;
        let h = det.bbox.height as u32;

        let x = x.max(0).min(image.width() as i32 - 2);
        let y = y.max(0).min(image.height() as i32 - 2);
        let w = w.min(image.width() - x as u32 - 1);
        let h = h.min(image.height() - y as u32 - 1);

        if w < 2 || h < 2 {
            continue;
        }

        draw_hollow_rect_mut(image, Rect::at(x, y).of_size(w, h), image_color);
        draw_hollow_rect_mut(image, Rect::at(x + 1, y + 1).of_size(w.saturating_sub(2), h.saturating_sub(2)), image_color);

        let label_height = 18u32;
        if y >= 0 && (y as u32) + label_height <= image.height() && w >= 30 {
            let label_width = ((det.label.len() * 7) + 25).min(w as usize) as u32;
            draw_filled_rect_mut(
                image,
                Rect::at(x, y).of_size(label_width, label_height),
                image_color,
            );
        }
    }
}

/// Encode image to JPEG
pub fn encode_jpeg(image: &image::RgbImage, quality: u8) -> Vec<u8> {
    let mut buffer = Vec::new();
    let dynamic = image::DynamicImage::ImageRgb8(image.clone());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
    let _ = dynamic.write_with_encoder(encoder);
    buffer
}

/// Generate mock detections
fn generate_mock_detections(frame_count: u64, max_objects: u32) -> Vec<ObjectDetection> {
    let objects = [
        ("person", 0u32, 0.85f32),
        ("car", 2, 0.92),
        ("dog", 16, 0.78),
        ("bicycle", 1, 0.65),
        ("cat", 15, 0.71),
    ];

    let count = ((frame_count % 3) + 1) as usize;
    let offset = (frame_count as usize % objects.len()) as usize;

    objects.iter()
        .cycle()
        .skip(offset)
        .take(count.min(max_objects as usize))
        .enumerate()
        .map(|(i, (label, class_id, conf))| {
            let ox = (frame_count % 100) as f32 * 3.0 + i as f32 * 50.0;
            let oy = (frame_count % 80) as f32 * 2.0 + i as f32 * 30.0;
            ObjectDetection {
                id: i as u32,
                label: label.to_string(),
                confidence: *conf,
                bbox: BoundingBox {
                    x: 100.0 + ox,
                    y: 100.0 + oy,
                    width: 100.0 + i as f32 * 20.0,
                    height: 150.0,
                },
                class_id: *class_id,
            }
        })
        .collect()
}

// ============================================================================
// Stream Processor
// ============================================================================

pub struct StreamProcessor {
    detector: Arc<Mutex<YoloDetector>>,
}

impl StreamProcessor {
    pub fn new() -> Self {
        // Initialize detector (will load model if available)
        let detector = YoloDetector::default();

        Self {
            detector: Arc::new(Mutex::new(detector)),
        }
    }

    fn has_model(&self) -> bool {
        self.detector.lock().is_loaded()
    }

    /// Start a new stream
    pub async fn start_stream(&self, config: StreamConfig) -> Result<StreamInfo> {
        let stream_id = Uuid::new_v4().to_string();

        // Parse dimensions from source URL
        let (width, height) = if config.source_url.contains("1920") || config.source_url.contains("rtsp") {
            (1920, 1080)
        } else {
            (640, 480)
        };

        let active_stream = Arc::new(Mutex::new(ActiveStream {
            id: stream_id.clone(),
            config: config.clone(),
            started_at: Instant::now(),
            frame_count: 0,
            total_detections: 0,
            last_frame: None,
            last_detections: vec![],
            last_frame_time: None,
            fps: 0.0,
            running: true,
            detected_objects: HashMap::new(),
        }));

        {
            let mut registry = REGISTRY.lock();
            registry.streams.insert(stream_id.clone(), active_stream.clone());
        }

        // Spawn processing task
        let stream_id_clone = stream_id.clone();
        let config_clone = config.clone();
        let detector_clone = Arc::clone(&self.detector);

        tokio::spawn(async move {
            Self::processing_loop(active_stream, stream_id_clone, config_clone, detector_clone).await;
        });

        Ok(StreamInfo {
            stream_id: stream_id.clone(),
            stream_url: format!("/api/extensions/yolo-video/stream/{}", stream_id),
            status: "starting".to_string(),
            width,
            height,
        })
    }

    /// Processing loop - generates demo frames with real inference when available
    async fn processing_loop(
        stream: Arc<Mutex<ActiveStream>>,
        stream_id: String,
        config: StreamConfig,
        detector: Arc<Mutex<YoloDetector>>,
    ) {
        tracing::info!("[Stream {}] Processing loop started", stream_id);

        let frame_interval = Duration::from_millis(1000 / config.target_fps.max(1) as u64);
        let mut ticker = tokio::time::interval(frame_interval);
        let mut frame_num = 0u64;

        while stream.lock().running {
            ticker.tick().await;

            // Generate demo frame with some visual content
            let mut demo_frame = image::RgbImage::from_pixel(640, 480, image::Rgb([40, 44, 52]));

            // Add some visual interest - create a more realistic scene
            let cx = ((frame_num * 3) % 500) as i32 + 70;
            let cy = ((frame_num * 2) % 300) as i32 + 90;

            // Draw a larger colored circle
            for y in (0.max(cy - 60))..480.min(cy + 60) {
                for x in (0.max(cx - 60))..640.min(cx + 60) {
                    let dx = (x - cx) as f32;
                    let dy = (y - cy) as f32;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 3600.0 {
                        let intensity = (1.0 - (dist_sq / 3600.0).sqrt()) * 100.0;
                        let base = 60 + (frame_num % 80) as u8;
                        let px = x as u32;
                        let py = y as u32;
                        if px < 640 && py < 480 {
                            demo_frame.put_pixel(
                                px,
                                py,
                                image::Rgb([
                                    (base as f32 + intensity).min(255.0) as u8,
                                    (base as f32 + intensity * 0.5).min(255.0) as u8,
                                    80,
                                ]),
                            );
                        }
                    }
                }
            }

            // Add some random "objects" (colored rectangles)
            for i in 0..3 {
                let ox = ((frame_num * 5 + i * 200) % 550) as i32;
                let oy = ((frame_num * 3 + i * 150) % 400) as i32;
                let ow = (40 + ((i * 20) % 40)) as i32;
                let oh = (30 + ((i * 15) % 30)) as i32;

                for y in oy.max(0)..(oy + oh).min(480) {
                    for x in ox.max(0)..(ox + ow).min(640) {
                        let px = x as u32;
                        let py = y as u32;
                        let pixel = demo_frame.get_pixel(px, py);
                        demo_frame.put_pixel(
                            px,
                            py,
                            image::Rgb([
                                (pixel[0] as f32 * 1.2).min(255.0) as u8,
                                pixel[1],
                                (pixel[2] as f32 * 1.3).min(255.0) as u8,
                            ]),
                        );
                    }
                }
            }

            // Run inference on the generated frame
            let detector_lock = detector.lock();
            let detections = if detector_lock.is_loaded() {
                tracing::trace!("[Stream {}] Running real inference", stream_id);
                let raw_detections = detector_lock.detect(
                    &demo_frame,
                    config.confidence_threshold,
                    config.max_objects,
                );
                detections_to_object_detection(raw_detections)
            } else {
                tracing::trace!("[Stream {}] Using fallback detections", stream_id);
                generate_fallback_detections(frame_num, config.max_objects)
            };
            drop(detector_lock);

            // Draw boxes if enabled
            let mut output_img = demo_frame;
            if config.draw_boxes {
                draw_detections(&mut output_img, &detections);
            }

            // Encode to JPEG
            let jpeg_data = encode_jpeg(&output_img, 85);

            // Update stream state
            {
                let mut s = stream.lock();
                s.frame_count += 1;
                s.total_detections += detections.len() as u64;
                s.last_frame = Some(jpeg_data);
                s.last_detections = detections.clone();
                s.last_frame_time = Some(Instant::now());

                let elapsed = s.started_at.elapsed().as_secs_f32();
                if elapsed > 0.0 {
                    s.fps = s.frame_count as f32 / elapsed;
                }

                for det in &detections {
                    *s.detected_objects.entry(det.label.clone()).or_insert(0) += 1;
                }
            }

            frame_num += 1;
        }

        {
            let mut s = stream.lock();
            s.running = false;
        }
        tracing::info!("[Stream {}] Processing loop stopped. Frames: {}", stream_id, frame_num);
    }

    /// Stop a stream
    pub fn stop_stream(&self, stream_id: &str) -> Result<()> {
        let mut registry = REGISTRY.lock();
        if let Some(stream) = registry.streams.remove(stream_id) {
            stream.lock().running = false;
            Ok(())
        } else {
            Err(ExtensionError::SessionNotFound(stream_id.to_string()))
        }
    }

    /// Get stream statistics
    pub fn get_stream_stats(&self, stream_id: &str) -> Option<StreamStats> {
        let registry = REGISTRY.lock();
        if let Some(stream) = registry.streams.get(stream_id) {
            let s = stream.lock();
            Some(StreamStats {
                stream_id: stream_id.to_string(),
                frame_count: s.frame_count,
                fps: s.fps,
                total_detections: s.total_detections,
                detected_objects: s.detected_objects.clone(),
            })
        } else {
            None
        }
    }

    /// Get latest frame
    pub fn get_stream_frame(&self, stream_id: &str) -> Option<Vec<u8>> {
        let registry = REGISTRY.lock();
        if let Some(stream) = registry.streams.get(stream_id) {
            let s = stream.lock();
            s.last_frame.clone()
        } else {
            None
        }
    }
}

// ============================================================================
// YOLO Video Processor Extension
// ============================================================================

pub struct YoloVideoProcessor {
    metadata: ExtensionMetadata,
    processor: Arc<StreamProcessor>,
}

impl YoloVideoProcessor {
    pub fn new() -> Self {
        let metadata = ExtensionMetadata::new(
            "yolo-video",
            "YOLO Video Processor",
            Version::new(2, 0, 0),
        )
        .with_description("Real-time video stream processing with YOLOv11 ONNX inference")
        .with_author("NeoMind Team");

        Self {
            metadata,
            processor: Arc::new(StreamProcessor::new()),
        }
    }
}

#[async_trait::async_trait]
impl Extension for YoloVideoProcessor {
    fn metadata(&self) -> &ExtensionMetadata {
        &self.metadata
    }

    fn metrics(&self) -> &[MetricDefinition] {
        &*METRICS
    }

    fn commands(&self) -> &[CommandDefinition] {
        &*COMMANDS
    }

    async fn execute_command(&self, command: &str, args: &Value) -> Result<Value> {
        match command {
            "start_stream" => {
                let config: StreamConfig = serde_json::from_value(args.clone())
                    .unwrap_or_default();
                let info = self.processor.start_stream(config).await?;
                Ok(serde_json::to_value(info).unwrap())
            }
            "stop_stream" => {
                let stream_id = args.get("stream_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ExtensionError::InvalidArguments("Missing stream_id".to_string()))?;
                self.processor.stop_stream(stream_id)?;
                Ok(serde_json::json!({"success": true}))
            }
            "get_stream_stats" => {
                let stream_id = args.get("stream_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ExtensionError::InvalidArguments("Missing stream_id".to_string()))?;
                if let Some(stats) = self.processor.get_stream_stats(stream_id) {
                    Ok(serde_json::to_value(stats).unwrap())
                } else {
                    Err(ExtensionError::SessionNotFound(stream_id.to_string()))
                }
            }
            _ => Err(ExtensionError::CommandNotFound(command.to_string())),
        }
    }

    fn produce_metrics(&self) -> Result<Vec<ExtensionMetricValue>> {
        let registry = REGISTRY.lock();
        Ok(vec![
            ExtensionMetricValue::new("active_streams", ParamMetricValue::Integer(registry.streams.len() as i64)),
            ExtensionMetricValue::new("total_frames_processed", ParamMetricValue::Integer(registry.total_frames as i64)),
            ExtensionMetricValue::new("avg_fps", ParamMetricValue::Float(0.0)),
        ])
    }

    fn stream_capability(&self) -> Option<StreamCapability> {
        Some(StreamCapability {
            direction: StreamDirection::Upload,
            mode: StreamMode::Stateful,
            supported_data_types: vec![
                StreamDataType::Image { format: "jpeg".to_string() },
                StreamDataType::Image { format: "png".to_string() },
            ],
            max_chunk_size: 10 * 1024 * 1024,
            preferred_chunk_size: 1024 * 1024,
            max_concurrent_sessions: 5,
            flow_control: Default::default(),
            config_schema: None,
        })
    }

    async fn init_session(&self, _session: &StreamSession) -> Result<()> {
        Ok(())
    }

    async fn process_session_chunk(&self, _session_id: &str, chunk: neomind_core::extension::DataChunk) -> Result<neomind_core::extension::StreamResult> {
        let start = std::time::Instant::now();

        // Decode image
        let img = image::load_from_memory(&chunk.data)
            .map_err(|e| ExtensionError::ExecutionFailed(format!("Failed to decode image: {}", e)))?;
        let rgb_img = img.to_rgb8();

        // Run inference
        let detector = self.processor.detector.lock();
        let detections = if detector.is_loaded() {
            let raw = detector.detect(&rgb_img, 0.5, 20);
            detections_to_object_detection(raw)
        } else {
            generate_fallback_detections(chunk.sequence, 20)
        };
        drop(detector);

        // Draw boxes on output image
        let mut output_img = rgb_img.clone();
        draw_detections(&mut output_img, &detections);

        // Encode result to JPEG
        let _jpeg_data = encode_jpeg(&output_img, 85);

        // Prepare result
        let result = serde_json::json!({
            "frame_number": chunk.sequence,
            "detections": detections,
            "image_size": { "width": rgb_img.width(), "height": rgb_img.height() },
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
            "processing_ms": (start.elapsed().as_secs_f64() * 1000.0) as f32,
        });

        Ok(neomind_core::extension::StreamResult {
            input_sequence: Some(chunk.sequence),
            output_sequence: chunk.sequence,
            data: serde_json::to_vec(&result).unwrap(),
            data_type: StreamDataType::Json,
            processing_ms: (start.elapsed().as_secs_f64() * 1000.0) as f32,
            metadata: None,
            error: None,
        })
    }

    async fn close_session(&self, session_id: &str) -> Result<SessionStats> {
        self.processor.stop_stream(session_id)?;
        Ok(SessionStats {
            input_chunks: 0,
            output_chunks: 0,
            input_bytes: 0,
            output_bytes: 0,
            errors: 0,
            last_activity: chrono::Utc::now().timestamp_millis(),
        })
    }
}

// ============================================================================
// Public API for HTTP handlers
// ============================================================================

/// Get latest frame for MJPEG streaming
pub fn get_stream_frame(stream_id: &str) -> Option<Vec<u8>> {
    let registry = REGISTRY.lock();
    if let Some(stream) = registry.streams.get(stream_id) {
        let s = stream.lock();
        s.last_frame.clone()
    } else {
        None
    }
}

/// Get stream statistics
pub fn get_stream_stats(stream_id: &str) -> Option<StreamStats> {
    let registry = REGISTRY.lock();
    if let Some(stream) = registry.streams.get(stream_id) {
        let s = stream.lock();
        Some(StreamStats {
            stream_id: stream_id.to_string(),
            frame_count: s.frame_count,
            fps: s.fps,
            total_detections: s.total_detections,
            detected_objects: s.detected_objects.clone(),
        })
    } else {
        None
    }
}

// ============================================================================
// FFI Exports
// ============================================================================

use tokio::sync::RwLock;

#[no_mangle]
pub extern "C" fn neomind_extension_abi_version() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn neomind_extension_metadata() -> CExtensionMetadata {
    use std::ffi::CStr;

    let id = CStr::from_bytes_with_nul(b"yolo-video\0").unwrap();
    let name = CStr::from_bytes_with_nul(b"YOLO Video Processor\0").unwrap();
    let version = CStr::from_bytes_with_nul(b"2.0.0\0").unwrap();
    let description = CStr::from_bytes_with_nul(b"Real-time video detection with YOLOv11 ONNX\0").unwrap();
    let author = CStr::from_bytes_with_nul(b"NeoMind Team\0").unwrap();

    CExtensionMetadata {
        abi_version: ABI_VERSION,
        id: id.as_ptr(),
        name: name.as_ptr(),
        version: version.as_ptr(),
        description: description.as_ptr(),
        author: author.as_ptr(),
        metric_count: 3,
        command_count: 3,
    }
}

#[no_mangle]
pub extern "C" fn neomind_extension_create(
    _config_json: *const u8,
    _config_len: usize,
) -> *mut RwLock<Box<dyn Extension>> {
    let extension = YoloVideoProcessor::new();
    Box::into_raw(Box::new(RwLock::new(Box::new(extension))))
}

#[no_mangle]
pub extern "C" fn neomind_extension_destroy(ptr: *mut RwLock<Box<dyn Extension>>) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_creation() {
        let ext = YoloVideoProcessor::new();
        assert_eq!(ext.metadata().id, "yolo-video");
    }

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert_eq!(config.source_url, "camera://0");
        assert_eq!(config.confidence_threshold, 0.5);
    }

    #[test]
    fn test_fallback_detections() {
        let detections = generate_fallback_detections(0, 5);
        assert!(!detections.is_empty());
        assert_eq!(detections[0].label, "person");
    }

    #[test]
    fn test_encode_jpeg() {
        let img = image::RgbImage::from_pixel(100, 100, image::Rgb([128, 128, 128]));
        let jpeg = encode_jpeg(&img, 80);
        assert!(!jpeg.is_empty());
        // JPEG should start with JPEG signature
        assert_eq!(&jpeg[0..2], &[0xFF, 0xD8]);
    }
}
