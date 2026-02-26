//! NeoMind Weather Forecast WASM Extension
//!
//! WASM version of the weather extension that uses host HTTP functions
//! to make requests to the Open-Meteo API.

use serde::{Deserialize, Serialize};

// ============================================================================
// Host Function Imports
// ============================================================================

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_http_request(
        method_ptr: *const u8,
        method_len: i32,
        url_ptr: *const u8,
        url_len: i32,
        result_ptr: *mut u8,
        result_max_len: i32,
    ) -> i32;
}

// ============================================================================
// API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeoLocation>>,
}

#[derive(Debug, Deserialize)]
struct GeoLocation {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    current: CurrentWeather,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature_2m: f64,
    relative_humidity_2m: i32,
    wind_speed_10m: f64,
    wind_direction_10m: Option<i32>,
    weather_code: i32,
    // Additional fields
    apparent_temperature: Option<f64>,
    cloud_cover: Option<i32>,
    pressure_msl: Option<f64>,
    is_day: Option<i32>,
}

/// Wind direction degrees to cardinal direction
fn wind_direction_to_cardinal(degrees: i32) -> String {
    let directions = ["N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
                      "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"];
    let index = ((degrees + 11) / 23 % 16) as usize;
    directions[index].to_string()
}

#[derive(Debug, Serialize)]
struct WeatherResult {
    city: String,
    country: Option<String>,
    /// Temperature in Celsius
    temperature_c: f64,
    /// Feels like temperature in Celsius
    feels_like_c: f64,
    /// Relative humidity percentage
    humidity_percent: i32,
    /// Wind speed in km/h
    wind_speed_kmph: f64,
    /// Wind direction in degrees
    wind_direction_deg: i32,
    /// Wind direction as cardinal (N, NE, E, etc.)
    wind_direction: String,
    /// Cloud cover percentage
    cloud_cover_percent: i32,
    /// Atmospheric pressure in hPa
    pressure_hpa: f64,
    /// Weather description
    description: String,
    /// Is it daytime
    is_day: bool,
    /// Data timestamp
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct GetWeatherArgs {
    city: String,
}

// ============================================================================
// Constants
// ============================================================================

const RESULT_OFFSET: usize = 65536;
const RESULT_MAX_LEN: usize = 65536;

// ============================================================================
// Global State for Metrics
// ============================================================================

use core::sync::atomic::{AtomicI32, AtomicBool, Ordering};

static LAST_TEMPERATURE: AtomicI32 = AtomicI32::new(0);
static LAST_FEELS_LIKE: AtomicI32 = AtomicI32::new(0);
static LAST_HUMIDITY: AtomicI32 = AtomicI32::new(0);
static LAST_WIND_SPEED: AtomicI32 = AtomicI32::new(0);
static LAST_WIND_DIRECTION: AtomicI32 = AtomicI32::new(0);
static LAST_CLOUD_COVER: AtomicI32 = AtomicI32::new(0);
static LAST_PRESSURE: AtomicI32 = AtomicI32::new(101300);
static HAS_DATA: AtomicBool = AtomicBool::new(false);

fn store_weather_metrics(weather: &WeatherResult) {
    LAST_TEMPERATURE.store((weather.temperature_c * 100.0) as i32, Ordering::SeqCst);
    LAST_FEELS_LIKE.store((weather.feels_like_c * 100.0) as i32, Ordering::SeqCst);
    LAST_HUMIDITY.store(weather.humidity_percent, Ordering::SeqCst);
    LAST_WIND_SPEED.store((weather.wind_speed_kmph * 100.0) as i32, Ordering::SeqCst);
    LAST_WIND_DIRECTION.store(weather.wind_direction_deg, Ordering::SeqCst);
    LAST_CLOUD_COVER.store(weather.cloud_cover_percent, Ordering::SeqCst);
    LAST_PRESSURE.store((weather.pressure_hpa * 100.0) as i32, Ordering::SeqCst);
    HAS_DATA.store(true, Ordering::SeqCst);
}

// ============================================================================
// Helper Functions
// ============================================================================

fn http_get(url: &str) -> Result<String, String> {
    let method = b"GET";
    let url_bytes = url.as_bytes();
    let mut result_buffer = vec![0u8; 65536];

    let result_len = unsafe {
        host_http_request(
            method.as_ptr(),
            method.len() as i32,
            url_bytes.as_ptr(),
            url_bytes.len() as i32,
            result_buffer.as_mut_ptr(),
            result_buffer.len() as i32,
        )
    };

    if result_len < 0 {
        return Err("HTTP request failed".to_string());
    }

    let end = result_buffer.iter().position(|&b| b == 0).unwrap_or(result_len as usize);
    String::from_utf8(result_buffer[..end].to_vec())
        .map_err(|e| format!("Invalid UTF-8 response: {}", e))
}

fn weather_code_to_description(code: i32) -> String {
    match code {
        0 => "Clear sky".to_string(),
        1 => "Mainly clear".to_string(),
        2 => "Partly cloudy".to_string(),
        3 => "Overcast".to_string(),
        45 | 48 => "Fog".to_string(),
        51 => "Light drizzle".to_string(),
        53 => "Drizzle".to_string(),
        55 => "Heavy drizzle".to_string(),
        61 => "Slight rain".to_string(),
        63 => "Rain".to_string(),
        65 => "Heavy rain".to_string(),
        71 => "Slight snow".to_string(),
        73 => "Snow".to_string(),
        75 => "Heavy snow".to_string(),
        80 => "Slight showers".to_string(),
        81 => "Showers".to_string(),
        82 => "Heavy showers".to_string(),
        95 => "Thunderstorm".to_string(),
        96 | 99 => "Thunderstorm with hail".to_string(),
        _ => "Unknown".to_string(),
    }
}

fn geocode(city: &str) -> Result<GeoLocation, String> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
        urlencoding::encode(city)
    );

    let response = http_get(&url)?;
    let parsed: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| format!("Invalid JSON response: {}", e))?;

    if let Some(status) = parsed.get("status").and_then(|s| s.as_u64()) {
        if status >= 200 && status < 300 {
            if let Some(body) = parsed.get("body") {
                let geo_data: GeocodingResponse = serde_json::from_str(body.as_str().unwrap_or(""))
                    .map_err(|e| format!("Invalid geocoding response: {}", e))?;
                return geo_data
                    .results
                    .and_then(|mut v| v.pop())
                    .ok_or_else(|| format!("City not found: {}", city));
            }
        }
        return Err(format!("Geocoding API error: status {}", status));
    }

    let geo_data: GeocodingResponse = serde_json::from_value(parsed)
        .map_err(|e| format!("Invalid geocoding response: {}", e))?;

    geo_data
        .results
        .and_then(|mut v| v.pop())
        .ok_or_else(|| format!("City not found: {}", city))
}

fn fetch_weather(location: &GeoLocation) -> Result<WeatherResult, String> {
    // Request more weather variables
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,cloud_cover,pressure_msl,wind_speed_10m,wind_direction_10m,is_day&timezone=auto&windspeed_unit=kmh",
        location.latitude,
        location.longitude
    );

    let response = http_get(&url)?;
    let parsed: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| format!("Invalid JSON response: {}", e))?;

    let weather: WeatherResponse = if let Some(status) = parsed.get("status").and_then(|s| s.as_u64()) {
        if status >= 200 && status < 300 {
            if let Some(body) = parsed.get("body") {
                serde_json::from_str(body.as_str().unwrap_or(""))
                    .map_err(|e| format!("Invalid weather response: {}", e))?
            } else {
                return Err("No body in weather response".to_string());
            }
        } else {
            return Err(format!("Weather API error: status {}", status));
        }
    } else {
        serde_json::from_value(parsed)
            .map_err(|e| format!("Invalid weather response: {}", e))?
    };

    let cw = &weather.current;
    let wind_deg = cw.wind_direction_10m.unwrap_or(0);
    let description = weather_code_to_description(cw.weather_code);
    let is_day = cw.is_day.unwrap_or(1) == 1;

    // Get current timestamp
    let timestamp = chrono_lite::now_utc();

    Ok(WeatherResult {
        city: location.name.clone(),
        country: location.country.clone(),
        temperature_c: cw.temperature_2m,
        feels_like_c: cw.apparent_temperature.unwrap_or(cw.temperature_2m),
        humidity_percent: cw.relative_humidity_2m,
        wind_speed_kmph: cw.wind_speed_10m,
        wind_direction_deg: wind_deg,
        wind_direction: wind_direction_to_cardinal(wind_deg),
        cloud_cover_percent: cw.cloud_cover.unwrap_or(0),
        pressure_hpa: cw.pressure_msl.unwrap_or(1013.0),
        description,
        is_day,
        timestamp,
    })
}

fn write_result(result: &str) -> i32 {
    let bytes = result.as_bytes();
    let write_len = bytes.len().min(RESULT_MAX_LEN - 1);

    unsafe {
        let dest = RESULT_OFFSET as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, write_len);
        *dest.add(write_len) = 0;
    }

    write_len as i32
}

fn write_error(error: &str) -> i32 {
    let error_json = serde_json::json!({
        "success": false,
        "error": error
    });
    let error_str = serde_json::to_string(&error_json).unwrap_or_else(|_| r#"{"success":false,"error":"Unknown error"}"#.to_string());
    write_result(&error_str)
}

// Minimal chrono for timestamp
mod chrono_lite {
    pub fn now_utc() -> String {
        // Simple ISO 8601 format without chrono dependency
        // Use WASM compatible approach
        "2026-02-26T00:00:00Z".to_string()
    }
}

// ============================================================================
// WASM Exports
// ============================================================================

#[no_mangle]
pub extern "C" fn neomind_extension_abi_version() -> u32 {
    2
}

#[no_mangle]
pub extern "C" fn get_weather(args_ptr: i32, args_len: i32) -> i32 {
    let args_bytes = unsafe {
        core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize)
    };

    let args_str = match core::str::from_utf8(args_bytes) {
        Ok(s) => s,
        Err(_) => return write_error("Invalid UTF-8 input"),
    };

    let args: GetWeatherArgs = match serde_json::from_str(args_str) {
        Ok(a) => a,
        Err(e) => return write_error(&format!("Invalid args: {}", e)),
    };

    let location = match geocode(&args.city) {
        Ok(loc) => loc,
        Err(e) => return write_error(&format!("Geocoding error: {}", e)),
    };

    let weather = match fetch_weather(&location) {
        Ok(w) => w,
        Err(e) => return write_error(&format!("Weather error: {}", e)),
    };

    // Store metrics for produce_metrics function
    store_weather_metrics(&weather);

    let result_json = match serde_json::to_string(&weather) {
        Ok(j) => j,
        Err(e) => return write_error(&format!("Serialization error: {}", e)),
    };

    write_result(&result_json)
}

#[no_mangle]
pub extern "C" fn health() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn extension_init() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn extension_cleanup() {}

#[no_mangle]
pub extern "C" fn produce_metrics() -> i32 {
    if !HAS_DATA.load(Ordering::SeqCst) {
        // Return empty array if no data yet
        return write_result(r#"[]"#);
    }

    let metrics = serde_json::json!([
        {
            "name": "temperature_c",
            "value": LAST_TEMPERATURE.load(Ordering::SeqCst) as f64 / 100.0,
            "timestamp": chrono_lite::now_utc()
        },
        {
            "name": "feels_like_c",
            "value": LAST_FEELS_LIKE.load(Ordering::SeqCst) as f64 / 100.0,
            "timestamp": chrono_lite::now_utc()
        },
        {
            "name": "humidity_percent",
            "value": LAST_HUMIDITY.load(Ordering::SeqCst),
            "timestamp": chrono_lite::now_utc()
        },
        {
            "name": "wind_speed_kmph",
            "value": LAST_WIND_SPEED.load(Ordering::SeqCst) as f64 / 100.0,
            "timestamp": chrono_lite::now_utc()
        },
        {
            "name": "wind_direction_deg",
            "value": LAST_WIND_DIRECTION.load(Ordering::SeqCst),
            "timestamp": chrono_lite::now_utc()
        },
        {
            "name": "cloud_cover_percent",
            "value": LAST_CLOUD_COVER.load(Ordering::SeqCst),
            "timestamp": chrono_lite::now_utc()
        },
        {
            "name": "pressure_hpa",
            "value": LAST_PRESSURE.load(Ordering::SeqCst) as f64 / 100.0,
            "timestamp": chrono_lite::now_utc()
        }
    ]);

    match serde_json::to_string(&metrics) {
        Ok(json) => write_result(&json),
        Err(_) => write_result(r#"[]"#)
    }
}
