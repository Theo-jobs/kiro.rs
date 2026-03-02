//! Prometheus Metrics 模块
//!
//! 提供系统可观测性指标

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, register_histogram_vec, CounterVec,
    Encoder, Gauge, GaugeVec, HistogramVec, TextEncoder,
};

lazy_static! {
    /// 缓存命中率 (0.0 - 1.0)
    pub static ref CACHE_HIT_RATE: Gauge =
        register_gauge!("kiro_cache_hit_rate", "Cache hit rate (0.0 - 1.0)").unwrap();

    /// 请求延迟直方图（秒）
    pub static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "kiro_request_duration_seconds",
        "Request duration in seconds",
        &["endpoint", "model"],
        vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
    )
    .unwrap();

    /// 凭据状态 (0=disabled, 1=enabled)
    pub static ref CREDENTIAL_STATUS: GaugeVec = register_gauge_vec!(
        "kiro_credential_status",
        "Credential status (0=disabled, 1=enabled)",
        &["index", "subscription_type"]
    )
    .unwrap();

    /// 错误计数
    pub static ref ERROR_TOTAL: CounterVec = register_counter_vec!(
        "kiro_error_total",
        "Total number of errors",
        &["error_type", "endpoint"]
    )
    .unwrap();

    /// 活跃请求数
    pub static ref ACTIVE_REQUESTS: Gauge =
        register_gauge!("kiro_active_requests", "Number of active requests").unwrap();
}

/// 更新缓存命中率指标
pub fn update_cache_hit_rate(hits: u64, misses: u64) {
    let total = hits + misses;
    if total > 0 {
        let rate = hits as f64 / total as f64;
        CACHE_HIT_RATE.set(rate);
    }
}

/// 导出 Prometheus 指标
pub fn export_metrics() -> Result<String, Box<dyn std::error::Error>> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}
