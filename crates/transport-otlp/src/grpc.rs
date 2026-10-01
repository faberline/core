use async_trait::async_trait;
use tonic::{Request, Response, Status};

use crate::consumer::OtlpConsumer;
use crate::error::TransportError;
use crate::payload::{DecodedPayload, PartialSuccess};
use crate::proto;

#[async_trait]
pub trait GrpcProjectAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        metadata: &tonic::metadata::MetadataMap,
    ) -> std::result::Result<String, Status>;
}

struct GrpcService<C, A> {
    consumer: std::sync::Arc<C>,
    authorizer: std::sync::Arc<A>,
}

impl<C, A> Clone for GrpcService<C, A> {
    fn clone(&self) -> Self {
        Self {
            consumer: self.consumer.clone(),
            authorizer: self.authorizer.clone(),
        }
    }
}

impl<C, A> GrpcService<C, A>
where
    C: OtlpConsumer + 'static,
    A: GrpcProjectAuthorizer + 'static,
{
    async fn consume(
        &self,
        metadata: &tonic::metadata::MetadataMap,
        payload: DecodedPayload,
    ) -> std::result::Result<PartialSuccess, Status> {
        let project = self.authorizer.authorize(metadata).await?;
        self.consumer
            .consume(&project, payload)
            .await
            .map_err(status_from_error)
    }
}

#[tonic::async_trait]
impl<C, A> opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::LogsService
    for GrpcService<C, A>
where
    C: OtlpConsumer + 'static,
    A: GrpcProjectAuthorizer + 'static,
{
    async fn export(
        &self,
        request: Request<proto::ExportLogsServiceRequest>,
    ) -> std::result::Result<Response<proto::ExportLogsServiceResponse>, Status> {
        let partial = self
            .consume(
                request.metadata(),
                DecodedPayload::Logs(request.get_ref().clone()),
            )
            .await?;
        Ok(Response::new(proto::ExportLogsServiceResponse {
            partial_success: (!partial.is_empty()).then_some(proto::ExportLogsPartialSuccess {
                rejected_log_records: partial.rejected_items as i64,
                error_message: partial.error_message,
            }),
        }))
    }
}

#[tonic::async_trait]
impl<C, A>
    opentelemetry_proto::tonic::collector::metrics::v1::metrics_service_server::MetricsService
    for GrpcService<C, A>
where
    C: OtlpConsumer + 'static,
    A: GrpcProjectAuthorizer + 'static,
{
    async fn export(
        &self,
        request: Request<proto::ExportMetricsServiceRequest>,
    ) -> std::result::Result<Response<proto::ExportMetricsServiceResponse>, Status> {
        let partial = self
            .consume(
                request.metadata(),
                DecodedPayload::Metrics(request.get_ref().clone()),
            )
            .await?;
        Ok(Response::new(proto::ExportMetricsServiceResponse {
            partial_success: (!partial.is_empty()).then_some(proto::ExportMetricsPartialSuccess {
                rejected_data_points: partial.rejected_items as i64,
                error_message: partial.error_message,
            }),
        }))
    }
}

#[tonic::async_trait]
impl<C, A> opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceService
    for GrpcService<C, A>
where
    C: OtlpConsumer + 'static,
    A: GrpcProjectAuthorizer + 'static,
{
    async fn export(
        &self,
        request: Request<proto::ExportTraceServiceRequest>,
    ) -> std::result::Result<Response<proto::ExportTraceServiceResponse>, Status> {
        let partial = self
            .consume(
                request.metadata(),
                DecodedPayload::Traces(request.get_ref().clone()),
            )
            .await?;
        Ok(Response::new(proto::ExportTraceServiceResponse {
            partial_success: (!partial.is_empty()).then_some(proto::ExportTracePartialSuccess {
                rejected_spans: partial.rejected_items as i64,
                error_message: partial.error_message,
            }),
        }))
    }
}

pub async fn serve_grpc<C, A, Shutdown>(
    listener: tokio::net::TcpListener,
    consumer: std::sync::Arc<C>,
    authorizer: std::sync::Arc<A>,
    maximum_message_bytes: usize,
    shutdown: Shutdown,
) -> anyhow::Result<()>
where
    C: OtlpConsumer + 'static,
    A: GrpcProjectAuthorizer + 'static,
    Shutdown: std::future::Future<Output = ()> + Send + 'static,
{
    use opentelemetry_proto::tonic::collector::{
        logs::v1::logs_service_server::LogsServiceServer,
        metrics::v1::metrics_service_server::MetricsServiceServer,
        trace::v1::trace_service_server::TraceServiceServer,
    };
    use tonic::codec::CompressionEncoding;

    let service = GrpcService {
        consumer,
        authorizer,
    };
    tonic::transport::Server::builder()
        .add_service(
            LogsServiceServer::new(service.clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .send_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(maximum_message_bytes),
        )
        .add_service(
            MetricsServiceServer::new(service.clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .send_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(maximum_message_bytes),
        )
        .add_service(
            TraceServiceServer::new(service)
                .accept_compressed(CompressionEncoding::Gzip)
                .send_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(maximum_message_bytes),
        )
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            shutdown,
        )
        .await?;
    Ok(())
}

fn status_from_error(error: TransportError) -> Status {
    match error {
        TransportError::UnsupportedMediaType { .. }
        | TransportError::UnsupportedContentEncoding { .. }
        | TransportError::InvalidJson { .. }
        | TransportError::InvalidProtobuf { .. }
        | TransportError::InvalidGzip { .. } => Status::invalid_argument(error.to_string()),
        TransportError::DecodedBodyTooLarge { .. } => Status::resource_exhausted(error.to_string()),
        TransportError::Encode { .. } => Status::internal(error.to_string()),
        TransportError::Consumer { .. } => Status::unavailable(error.to_string()),
    }
}
