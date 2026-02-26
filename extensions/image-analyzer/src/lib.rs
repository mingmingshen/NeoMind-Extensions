//! Image Analyzer Extension
//!
//! A stateless extension that analyzes images using YOLOv8 object detection.
//! Demonstrates the Stateless streaming mode for single-chunk processing.
//!
//! # Streaming Mode
//!
//! - **Mode**: Stateless (stateless)
//! - **Direction**: Upload (client → extension)
//! - **Supported Types**: JPEG, PNG, WebP images
//! - **Max Chunk Size**: 10MB
//!
//! # Configuration
//!
//! Set environment variable `YOLOV8_MODEL_PATH` to specify the ONNX model path.
//! Default: `./models/yolov8n.onnx`
//!
//! Download YOLOv8 ONNX model:
//! ```bash
//! # Using Python to export
//! pip install ultralytics
//! yolo export model=yolov8n.pt format=onnx
//!
//! # Or download pre-exported model
//! wget https://github.com/ultralytics/assets/releases/download/v8.1.0/yolov8n.onnx
//! ```
//!
//! # Usage
//!
//! Build the extension:
//! ```bash
//! cd /Users/shenmingming/NeoMind-Extension
//! cargo build --release -p neomind-image-analyzer
//! ```

use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

// Import from neomind-core
use neomind_core::extension::system::{
    ParameterDefinition, ParameterGroup, MetricDataType,
    Extension, ExtensionMetadata, ExtensionError, MetricDefinition,
    ExtensionMetricValue, ParamMetricValue, CommandDefinition,
    CExtensionMetadata, ABI_VERSION, Result,
};
use neomind_core::extension::{
    StreamCapability, StreamMode, StreamDirection, StreamDataType, DataChunk, StreamResult,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use semver::Version;
use std::collections::HashMap;

// Import ort types (only on non-WASM targets)
#[cfg(not(target_arch = "wasm32"))]
use ort::session::Session;

// ============================================================================
// Types
// ============================================================================

/// Image detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Detection {
    label: String,
    confidence: f32,
    bbox: Option<BoundingBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoundingBox {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalysisResult {
    objects: Vec<Detection>,
    description: String,
    processing_time_ms: u64,
}

/// Extension statistics
#[derive(Debug, Default)]
struct ImageAnalyzerStats {
    images_processed: u64,
    total_processing_time_ms: u64,
    detections_found: u64,
}

/// COCO class labels for YOLOv8
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

// ============================================================================
// Static Metrics and Commands
// ============================================================================

static METRICS: Lazy<[MetricDefinition; 3]> = Lazy::new(|| [
    MetricDefinition {
        name: "images_processed".to_string(),
        display_name: "Images Processed".to_string(),
        data_type: MetricDataType::Integer,
        unit: "count".to_string(),
        min: Some(0.0),
        max: None,
        required: false,
    },
    MetricDefinition {
        name: "avg_processing_time_ms".to_string(),
        display_name: "Average Processing Time".to_string(),
        data_type: MetricDataType::Float,
        unit: "ms".to_string(),
        min: Some(0.0),
        max: None,
        required: false,
    },
    MetricDefinition {
        name: "total_detections".to_string(),
        display_name: "Total Detections".to_string(),
        data_type: MetricDataType::Integer,
        unit: "count".to_string(),
        min: Some(0.0),
        max: None,
        required: false,
    },
]);

static COMMANDS: Lazy<[CommandDefinition; 2]> = Lazy::new(|| [
    CommandDefinition {
        name: "reset_stats".to_string(),
        display_name: "Reset Statistics".to_string(),
        payload_template: "{}".to_string(),
        parameters: vec![],
        fixed_values: HashMap::new(),
        samples: vec![],
        llm_hints: "Resets all processing statistics to zero".to_string(),
        parameter_groups: vec![],
    },
    CommandDefinition {
        name: "analyze_image".to_string(),
        display_name: "Analyze Image".to_string(),
        payload_template: "{\"image\": \"<base64_image>\"}".to_string(),
        parameters: vec![
            ParameterDefinition {
                name: "image".to_string(),
                display_name: "Image".to_string(),
                description: "Base64 encoded image data".to_string(),
                param_type: MetricDataType::String,
                required: true,
                default_value: None,
                min: None,
                max: None,
                options: vec![],
            },
        ],
        fixed_values: HashMap::new(),
        samples: vec![],
        llm_hints: "Analyze an image for object detection using YOLOv8".to_string(),
        parameter_groups: vec![],
    },
]);

// ============================================================================
// YOLOv8 Detector
// ============================================================================

/// YOLOv8 ONNX detector
#[cfg(not(target_arch = "wasm32"))]
pub struct YOLOv8Detector {
    session: Option<std::sync::Mutex<Session>>,
    model_path: String,
    input_size: u32, // YOLOv8 default input size (640x640)
}

#[cfg(not(target_arch = "wasm32"))]
impl YOLOv8Detector {
    pub fn new() -> Self {
        let model_path = std::env::var("YOLOV8_MODEL_PATH")
            .unwrap_or_else(|_| "./models/yolov8n.onnx".to_string());

        let session = Self::load_model(&model_path).map(std::sync::Mutex::new);

        Self {
            session,
            model_path,
            input_size: 640,
        }
    }

    fn load_model(path: &str) -> Option<Session> {
        match Session::builder() {
            Ok(builder) => {
                match builder.commit_from_file(path) {
                    Ok(session) => {
                        eprintln!("[YOLOv8] Model loaded from: {}", path);
                        Some(session)
                    }
                    Err(e) => {
                        eprintln!("[YOLOv8] Failed to load model from {}: {}", path, e);
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("[YOLOv8] Failed to create session builder: {}", e);
                None
            }
        }
    }

    /// Detect objects in image using YOLOv8
    pub fn detect(&self, image_data: &[u8], confidence_threshold: f32) -> Result<Vec<Detection>> {
        let session_mutex = match &self.session {
            Some(s) => s,
            None => {
                return Err(ExtensionError::ExecutionFailed(
                    "YOLOv8 model not loaded. Please ensure model file exists.".to_string()
                ));
            }
        };

        // Decode image
        let img = image::load_from_memory(image_data)
            .map_err(|e| ExtensionError::ExecutionFailed(format!("Failed to decode image: {}", e)))?;

        let (orig_width, orig_height) = (img.width(), img.height());

        // Preprocess image for YOLOv8
        let input_tensor = self.preprocess_image(&img)?;

        // Run inference using ort 2.0 API
        let input_value = ort::value::TensorRef::from_array_view(input_tensor.view())
            .map_err(|e| ExtensionError::ExecutionFailed(format!("Failed to create input tensor: {}", e)))?;

        // Lock the session for inference
        let mut session = session_mutex.lock().unwrap();
        let outputs = session.run(ort::inputs![input_value])
            .map_err(|e| ExtensionError::ExecutionFailed(format!("YOLOv8 inference failed: {}", e)))?;

        // Parse output - use try_extract_array for ndarray extraction
        let output = outputs[0].try_extract_array::<f32>()
            .map_err(|e| ExtensionError::ExecutionFailed(format!("Failed to extract output: {}", e)))?;

        // Get output shape
        let shape = output.shape();
        eprintln!("[YOLOv8] Output shape: {:?}", shape);

        // YOLOv8 output is typically [1, 84, 8400] or [1, 84, 8400, 1]
        // Post-process detections using raw slice access
        let detections = self.postprocess(output.as_slice().unwrap_or(&[]), shape, orig_width, orig_height, confidence_threshold);

        Ok(detections)
    }

    /// Preprocess image for YOLOv8 input
    fn preprocess_image(&self, img: &image::DynamicImage) -> Result<ndarray::Array4<f32>> {
        // Resize to input size with letterboxing
        let resized = img.resize_exact(
            self.input_size,
            self.input_size,
            image::imageops::FilterType::Triangle,
        );

        // Convert to RGB and normalize
        let rgb = resized.to_rgb8();
        let mut input = ndarray::Array4::<f32>::zeros((1, 3, self.input_size as usize, self.input_size as usize));

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            input[[0, 0, y, x]] = pixel[0] as f32 / 255.0;
            input[[0, 1, y, x]] = pixel[1] as f32 / 255.0;
            input[[0, 2, y, x]] = pixel[2] as f32 / 255.0;
        }

        Ok(input)
    }

    /// Post-process YOLOv8 output to get detections using raw slice
    fn postprocess(
        &self,
        output: &[f32],
        shape: &[usize],
        orig_width: u32,
        orig_height: u32,
        confidence_threshold: f32,
    ) -> Vec<Detection> {
        let mut detections = Vec::new();
        let nms_threshold = 0.45;

        if shape.len() < 2 {
            return detections;
        }

        // YOLOv8 output shape: [1, 84, 8400] or [1, 84, 8400, 1]
        let num_features = shape[shape.len() - 2]; // 84
        let num_boxes = shape[shape.len() - 1];    // 8400
        let num_classes = num_features.saturating_sub(4);

        if num_classes == 0 || output.len() < num_features * num_boxes {
            return detections;
        }

        // Scale factors
        let scale_x = orig_width as f32 / self.input_size as f32;
        let scale_y = orig_height as f32 / self.input_size as f32;

        // Collect all detections
        let mut all_boxes: Vec<(usize, f32, [f32; 4])> = Vec::new();

        for i in 0..num_boxes {
            // Get class scores
            let mut max_class = 0;
            let mut max_score = 0.0f32;

            for c in 0..num_classes {
                // Index: [0, 4+c, i] in 3D or [4+c, i] in 2D
                let idx = (4 + c) * num_boxes + i;
                if idx < output.len() {
                    let score = output[idx];
                    if score > max_score {
                        max_score = score;
                        max_class = c;
                    }
                }
            }

            if max_score >= confidence_threshold {
                // Get bounding box
                let cx = output.get(0 * num_boxes + i).copied().unwrap_or(0.0) * scale_x;
                let cy = output.get(1 * num_boxes + i).copied().unwrap_or(0.0) * scale_y;
                let w = output.get(2 * num_boxes + i).copied().unwrap_or(0.0) * scale_x;
                let h = output.get(3 * num_boxes + i).copied().unwrap_or(0.0) * scale_y;

                // Convert to x1, y1, x2, y2
                let x1 = cx - w / 2.0;
                let y1 = cy - h / 2.0;
                let x2 = cx + w / 2.0;
                let y2 = cy + h / 2.0;

                all_boxes.push((max_class, max_score, [x1, y1, x2, y2]));
            }
        }

        // Apply NMS (Non-Maximum Suppression) per class
        for class_id in 0..num_classes {
            let mut class_boxes: Vec<(usize, f32, [f32; 4])> = all_boxes.iter()
                .filter(|(c, _, _)| *c == class_id)
                .cloned()
                .collect();

            // Sort by confidence
            class_boxes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // NMS
            while !class_boxes.is_empty() {
                let best = class_boxes.remove(0);

                let x1 = best.2[0].max(0.0) as u32;
                let y1 = best.2[1].max(0.0) as u32;
                let x2 = (best.2[2] as u32).min(orig_width);
                let y2 = (best.2[3] as u32).min(orig_height);

                let label = COCO_CLASSES.get(class_id).unwrap_or(&"unknown");

                detections.push(Detection {
                    label: label.to_string(),
                    confidence: best.1,
                    bbox: Some(BoundingBox {
                        x: x1,
                        y: y1,
                        width: x2.saturating_sub(x1),
                        height: y2.saturating_sub(y1),
                    }),
                });

                // Remove overlapping boxes
                class_boxes.retain(|(_, _, box2)| {
                    let iou = self.compute_iou(&best.2, box2);
                    iou < nms_threshold
                });
            }
        }

        // Sort by confidence and limit
        detections.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        detections.truncate(100); // Limit to 100 detections

        detections
    }

    /// Compute IoU (Intersection over Union)
    fn compute_iou(&self, box1: &[f32; 4], box2: &[f32; 4]) -> f32 {
        let x1 = box1[0].max(box2[0]);
        let y1 = box1[1].max(box2[1]);
        let x2 = box1[2].min(box2[2]);
        let y2 = box1[3].min(box2[3]);

        let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
        let area1 = (box1[2] - box1[0]) * (box1[3] - box1[1]);
        let area2 = (box2[2] - box2[0]) * (box2[3] - box2[1]);
        let union = area1 + area2 - intersection;

        if union > 0.0 { intersection / union } else { 0.0 }
    }

    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.session.is_some()
    }
}

// ============================================================================
// Image Analyzer Extension
// ============================================================================

pub struct ImageAnalyzer {
    metadata: ExtensionMetadata,
    stats: Arc<Mutex<ImageAnalyzerStats>>,
    detector: Arc<YOLOv8Detector>,
}

impl ImageAnalyzer {
    pub fn new() -> Self {
        let metadata = ExtensionMetadata::new(
            "image-analyzer",
            "Image Analyzer",
            Version::new(1, 0, 0),
        )
        .with_description("Image analysis extension using YOLOv8 object detection")
        .with_author("NeoMind Team");

        let detector = Arc::new(YOLOv8Detector::new());

        Self {
            metadata,
            stats: Arc::new(Mutex::new(ImageAnalyzerStats::default())),
            detector,
        }
    }

    /// Analyze image data and return detection results
    fn analyze_image(&self, data: &[u8]) -> Result<AnalysisResult> {
        let start = std::time::Instant::now();

        let (objects, description) = if self.detector.is_loaded() {
            // Use YOLOv8 for detection
            match self.detector.detect(data, 0.25) {
                Ok(detections) => {
                    let desc = format!(
                        "YOLOv8 detected {} objects",
                        detections.len()
                    );
                    (detections, desc)
                }
                Err(e) => {
                    eprintln!("[ImageAnalyzer] YOLOv8 error: {}", e);
                    self.fallback_analysis(data)
                }
            }
        } else {
            self.fallback_analysis(data)
        };

        let processing_time = start.elapsed().as_millis() as u64;

        // Update stats
        let mut stats = self.stats.lock().unwrap();
        stats.images_processed += 1;
        stats.total_processing_time_ms += processing_time;
        stats.detections_found += objects.len() as u64;

        Ok(AnalysisResult {
            objects,
            description,
            processing_time_ms: processing_time,
        })
    }

    /// Fallback analysis when YOLOv8 is not available
    fn fallback_analysis(&self, data: &[u8]) -> (Vec<Detection>, String) {
        let size = data.len();
        let mut objects = Vec::new();

        // Check image format
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            objects.push(Detection {
                label: "jpeg_image".to_string(),
                confidence: 0.95,
                bbox: None,
            });
        } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            objects.push(Detection {
                label: "png_image".to_string(),
                confidence: 0.95,
                bbox: None,
            });
        }

        let description = format!(
            "Fallback analysis (YOLOv8 unavailable). Size: {} bytes.",
            size
        );

        (objects, description)
    }

    fn reset_stats(&self) -> Result<Value> {
        let mut stats = self.stats.lock().unwrap();
        stats.images_processed = 0;
        stats.total_processing_time_ms = 0;
        stats.detections_found = 0;
        Ok(serde_json::json!({"status": "reset"}))
    }
}

#[async_trait::async_trait]
impl Extension for ImageAnalyzer {
    fn metadata(&self) -> &ExtensionMetadata {
        &self.metadata
    }

    fn metrics(&self) -> &[MetricDefinition] {
        &*METRICS
    }

    fn commands(&self) -> &[CommandDefinition] {
        &*COMMANDS
    }

    async fn execute_command(
        &self,
        command: &str,
        _args: &Value,
    ) -> Result<Value> {
        match command {
            "analyze_image" => {
                let image_data = _args.get("image")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ExtensionError::InvalidArguments("Missing image data".to_string()))?;
                
                let image_bytes = base64::decode(image_data)
                    .map_err(|e| ExtensionError::InvalidArguments(format!("Invalid base64: {}", e)))?;
                
                match self.analyze_image(&image_bytes) {
                    Ok(result) => Ok(serde_json::to_value(result).unwrap()),
                    Err(e) => Err(ExtensionError::ExecutionFailed(format!("Analysis failed: {}", e))),
                }
            },
            "reset_stats" => self.reset_stats(),
            _ => Err(ExtensionError::CommandNotFound(command.to_string())),
        }
    }

    fn produce_metrics(&self) -> Result<Vec<ExtensionMetricValue>> {
        let stats = self.stats.lock().unwrap();
        let avg_time = if stats.images_processed > 0 {
            stats.total_processing_time_ms as f64 / stats.images_processed as f64
        } else {
            0.0
        };

        Ok(vec![
            ExtensionMetricValue::new(
                "images_processed",
                ParamMetricValue::Integer(stats.images_processed as i64),
            ),
            ExtensionMetricValue::new(
                "avg_processing_time_ms",
                ParamMetricValue::Float(avg_time),
            ),
            ExtensionMetricValue::new(
                "total_detections",
                ParamMetricValue::Integer(stats.detections_found as i64),
            ),
        ])
    }

    fn stream_capability(&self) -> Option<StreamCapability> {
        Some(StreamCapability {
            direction: StreamDirection::Upload,
            mode: StreamMode::Stateless,
            supported_data_types: vec![
                StreamDataType::Image { format: "jpeg".to_string() },
                StreamDataType::Image { format: "png".to_string() },
                StreamDataType::Image { format: "webp".to_string() },
            ],
            max_chunk_size: 10 * 1024 * 1024,
            preferred_chunk_size: 1024 * 1024,
            max_concurrent_sessions: 10,
            flow_control: Default::default(),
            config_schema: None,
        })
    }

    async fn process_chunk(&self, chunk: DataChunk) -> Result<StreamResult> {
        match &chunk.data_type {
            StreamDataType::Image { .. } => (),
            StreamDataType::Binary => (),
            _ => {
                return Err(ExtensionError::InvalidStreamData(
                    "Expected image data".to_string(),
                ))
            }
        }

        let result = self.analyze_image(&chunk.data)?;

        let output_data = serde_json::to_vec(&result)
            .map_err(|e| ExtensionError::InvalidStreamData(e.to_string()))?;

        Ok(StreamResult {
            input_sequence: Some(chunk.sequence),
            output_sequence: chunk.sequence,
            data: output_data,
            data_type: StreamDataType::Json,
            processing_ms: result.processing_time_ms as f32,
            metadata: Some(serde_json::json!({
                "processing_time_ms": result.processing_time_ms,
                "objects_detected": result.objects.len(),
            })),
            error: None,
        })
    }
}

// ============================================================================
// Global Extension Instance
// ============================================================================

static EXTENSION_INSTANCE: Lazy<ImageAnalyzer> = Lazy::new(|| ImageAnalyzer::new());

// ============================================================================
// FFI Exports for Dynamic Loading
// ============================================================================

use tokio::sync::RwLock;

#[no_mangle]
pub extern "C" fn neomind_extension_abi_version() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn neomind_extension_metadata() -> CExtensionMetadata {
    use std::ffi::CStr;

    let id = CStr::from_bytes_with_nul(b"image-analyzer\0").unwrap();
    let name = CStr::from_bytes_with_nul(b"Image Analyzer\0").unwrap();
    let version = CStr::from_bytes_with_nul(b"1.0.0\0").unwrap();
    let description = CStr::from_bytes_with_nul(b"Image analysis with YOLOv8\0").unwrap();
    let author = CStr::from_bytes_with_nul(b"NeoMind Team\0").unwrap();

    CExtensionMetadata {
        abi_version: ABI_VERSION,
        id: id.as_ptr(),
        name: name.as_ptr(),
        version: version.as_ptr(),
        description: description.as_ptr(),
        author: author.as_ptr(),
        metric_count: 3,
        command_count: 1,
    }
}

#[no_mangle]
pub extern "C" fn neomind_extension_create(
    config_json: *const u8,
    config_len: usize,
) -> *mut RwLock<Box<dyn Extension>> {
    let _config = if config_json.is_null() || config_len == 0 {
        serde_json::json!({})
    } else {
        unsafe {
            let slice = std::slice::from_raw_parts(config_json, config_len);
            let s = std::str::from_utf8_unchecked(slice);
            serde_json::from_str(s).unwrap_or(serde_json::json!({}))
        }
    };

    let extension = ImageAnalyzer::new();
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
        let ext = ImageAnalyzer::new();
        assert_eq!(ext.metadata().id, "image-analyzer");
    }

    #[test]
    fn test_fallback_analysis() {
        let ext = ImageAnalyzer::new();
        let (objects, description) = ext.fallback_analysis(&[0xFF, 0xD8, 0xFF, 0x00]);

        assert!(!objects.is_empty());
        assert!(description.contains("Fallback"));
    }

    #[test]
    fn test_detector_creation() {
        let detector = YOLOv8Detector::new();
        // May not have model loaded in test environment
        assert!(!detector.model_path.is_empty());
    }
}
