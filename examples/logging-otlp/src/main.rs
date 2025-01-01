use anyhow::Result;
use opentelemetry::KeyValue;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, WithExportConfig};
use opentelemetry_sdk::logs::LoggerProvider;
use opentelemetry_sdk::{runtime, Resource};
use salvo::logging::Logger;
use salvo::prelude::*;
use tracing::{instrument, level_filters::LevelFilter};
use tracing_subscriber::fmt::{self, format::FmtSpan};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[instrument(fields(http.uri = req.uri().path(), http.method = req.method().as_str()))]
#[handler]
async fn hello(req: &mut Request) -> &'static str {
    "Hello World"
}

#[tokio::main]
async fn main() -> Result<()> {
    // console layer for tracing-subscriber
    let console = fmt::Layer::new()
        .with_span_events(FmtSpan::CLOSE)
        .pretty()
        .with_filter(LevelFilter::DEBUG);

    // file appender layer for tracing-subscriber
    let file_appender = tracing_appender::rolling::daily("./logs", "salvo.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let file = fmt::Layer::new()
        .with_writer(non_blocking)
        .pretty()
        .with_filter(LevelFilter::INFO);

    // opentelemetry tracing layer for tracing-subscriber
    let provider = init_tracer_provider()?;

    tracing_subscriber::registry()
        .with(console)
        .with(file)
        .with(OpenTelemetryTracingBridge::new(&provider))
        .init();

    let router = Router::new().get(hello);
    let service = Service::new(router).hoop(Logger::new());

    let acceptor = TcpListener::new("0.0.0.0:5800").bind().await;
    Server::new(acceptor).serve(service).await;
    Ok(())
}

fn init_tracer_provider() -> anyhow::Result<LoggerProvider> {
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .build()?;
    let provider = LoggerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(Resource::new(vec![KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            "salvo-tracing",
        )]))
        .build();
    Ok(provider)
}