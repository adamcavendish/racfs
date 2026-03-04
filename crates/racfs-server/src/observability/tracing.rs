//! OpenTelemetry tracing for RACFS

use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;

/// Trace exporter configuration
#[derive(Debug, Clone, Default)]
pub enum TraceExporter {
    /// No exporter (disabled)
    #[default]
    None,
}

/// Global tracer provider
static TRACER_PROVIDER: std::sync::OnceLock<opentelemetry_sdk::trace::SdkTracerProvider> =
    std::sync::OnceLock::new();

/// Initialize OpenTelemetry tracing with given exporter configuration
#[allow(unused_variables)]
#[allow(unreachable_code)]
pub fn init_tracing(exporter: TraceExporter) -> Option<opentelemetry_sdk::trace::Tracer> {
    let tracer_provider: opentelemetry_sdk::trace::SdkTracerProvider = match exporter {
        TraceExporter::None => {
            tracing::info!("Tracing disabled");
            return None;
        }
    };

    // Set global tracer provider
    let _ = TRACER_PROVIDER.set(tracer_provider);

    // Set global propagator for context propagation
    let propagator = TraceContextPropagator::new();
    global::set_text_map_propagator(propagator);

    tracing::info!("OpenTelemetry tracing initialized");

    // Build tracer first
    let tracer = tracer_provider.tracer("racfs");
    Some(tracer)
}

/// Get a tracer for current service
#[allow(dead_code)]
pub fn get_tracer(name: String) -> impl opentelemetry::trace::Tracer + 'static {
    opentelemetry::global::tracer(name)
}
