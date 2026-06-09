use std::time::Duration;

use anyhow::Result;
use opentelemetry::{
    KeyValue,
    global::{self, BoxedTracer},
    metrics::Meter,
};
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{Resource, metrics::SdkMeterProvider, trace::SdkTracerProvider};
use tracing::{Subscriber, error, info};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    EnvFilter, Layer, layer::SubscriberExt as _, registry::LookupSpan, util::SubscriberInitExt as _,
};
use uuid::Uuid;
use zed::unstable::{
    gpui::{AppContext, Global},
    ui::App,
    util::ResultExt,
};

/// Tracing with Opentelemetry, OTLP exporter
pub fn init(cx: &mut App) {
    let instance = Uuid::new_v4();
    let otel_layer = init_otel_tracing(instance).log_err();
    let loki_layer = init_loki_logging(instance).log_err();
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        // .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
        .with(loki_layer)
        .init();

    if let Err(error) = init_otel_metrics(instance, cx) {
        error!("failed to initialize otel metrics: {}", error);
    }

    cx.meter()
        .u64_counter("startup")
        .build()
        .add(1, &[KeyValue::new("instance", instance.to_string())]);
    info!(?instance, "Observability initialized");
}

fn init_otel_tracing<S>(instance_id: Uuid) -> Result<OpenTelemetryLayer<S, BoxedTracer>>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let endpoint = "http://localhost:4317";
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_protocol(Protocol::Grpc)
        .with_timeout(Duration::from_secs(5))
        .with_endpoint(endpoint)
        .build()?;
    let resource = Resource::builder()
        .with_service_name("galvanized")
        .with_attribute(KeyValue::new("instance", instance_id.to_string()))
        .build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();
    global::set_tracer_provider(provider);
    let tracer = global::tracer("galvanized");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    Ok(otel_layer)
}

fn init_loki_logging(instance_id: Uuid) -> Result<tracing_loki::Layer> {
    let url = "http://localhost:3100".parse().expect("valid URL");
    let (layer, task) = tracing_loki::builder()
        .label("service_name", "galvanized")?
        .extra_field("instance", instance_id.to_string())?
        .build_url(url)?;
    tokio::runtime::Handle::current().spawn(task);
    Ok(layer)
}

struct GlobalMetrics(Meter);
impl Global for GlobalMetrics {}
pub trait MetricsExt {
    type Context: AppContext;
    fn meter(&mut self) -> Meter;
}
impl<C: AppContext> MetricsExt for C {
    type Context = C;
    fn meter(&mut self) -> Meter {
        self.read_global::<GlobalMetrics, _>(|it, _cx| it.0.clone())
    }
}

fn init_otel_metrics(instance_id: Uuid, cx: &mut App) -> Result<()> {
    // Initialize OTLP exporter using HTTP binary protocol
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_protocol(Protocol::Grpc)
        .with_endpoint("http://localhost:4317/api/v1/otlp/v1/metrics")
        .build()?;

    // Create a meter provider with the OTLP Metric exporter
    let resource = Resource::builder()
        .with_service_name("galvanized")
        .with_attribute(KeyValue::new("instance", instance_id.to_string()))
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(resource)
        .build();
    global::set_meter_provider(meter_provider.clone());

    // Get a meter
    let meter = global::meter("galvanized");
    cx.set_global(GlobalMetrics(meter));

    Ok(())
}
