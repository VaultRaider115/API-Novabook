//! Inventario mensual y solicitudes de editorial (diagrama de procesos, paso 7).

use axum::{
    Extension, Json,
    extract::{Path, State},
};

use crate::{
    db::AppState,
    error::{ApiResponse, AppError, MessageResponse},
    productos::{
        model::{
            ConteoRequest, CrearSolicitudEditorialRequest, InventarioDetalle, InventarioResumen,
            RevisionInventarioRequest, SolicitudEditorial,
        },
        service::InventarioMensualService,
    },
    usuarios::model::Claims,
};

// --- Vendedor ---------------------------------------------------------------

/// Inventario ABIERTO en curso (data: null si no hay ninguno).
pub async fn inventario_actual(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Option<InventarioDetalle>>>, AppError> {
    Ok(Json(ApiResponse::success(InventarioMensualService::actual(&state.pool).await?)))
}

pub async fn abrir_inventario(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ApiResponse<InventarioDetalle>>, AppError> {
    let res = InventarioMensualService::abrir(&state.pool, claims.id_usuario).await?;
    Ok(Json(ApiResponse::success_msg(res, "Inventario abierto")))
}

pub async fn registrar_conteo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<ConteoRequest>,
) -> Result<Json<MessageResponse>, AppError> {
    InventarioMensualService::registrar_conteo(&state.pool, id, payload).await?;
    Ok(Json(MessageResponse::new("Conteo registrado")))
}

pub async fn cerrar_inventario(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<InventarioDetalle>>, AppError> {
    let res = InventarioMensualService::cerrar(&state.pool, claims.id_usuario, id).await?;
    Ok(Json(ApiResponse::success_msg(res, "Conteo cerrado; queda pendiente de revisión del Jefe")))
}

// --- Jefe -------------------------------------------------------------------

pub async fn listar_inventarios(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<InventarioResumen>>>, AppError> {
    Ok(Json(ApiResponse::success(InventarioMensualService::listar(&state.pool).await?)))
}

pub async fn obtener_inventario(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<InventarioDetalle>>, AppError> {
    Ok(Json(ApiResponse::success(InventarioMensualService::obtener(&state.pool, id).await?)))
}

pub async fn revisar_inventario(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(payload): Json<RevisionInventarioRequest>,
) -> Result<Json<ApiResponse<InventarioDetalle>>, AppError> {
    let aprobar = payload.aprobar;
    let res = InventarioMensualService::revisar(&state.pool, claims.id_usuario, id, payload).await?;
    Ok(Json(ApiResponse::success_msg(
        res,
        if aprobar { "Inventario aprobado; stock de piso ajustado" } else { "Inventario rechazado" },
    )))
}

pub async fn listar_solicitudes(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<SolicitudEditorial>>>, AppError> {
    Ok(Json(ApiResponse::success(InventarioMensualService::listar_solicitudes(&state.pool).await?)))
}

pub async fn crear_solicitud(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CrearSolicitudEditorialRequest>,
) -> Result<Json<ApiResponse<Vec<SolicitudEditorial>>>, AppError> {
    let res = InventarioMensualService::crear_solicitud(&state.pool, claims.id_usuario, payload).await?;
    Ok(Json(ApiResponse::success_msg(res, "Solicitud de la editorial registrada")))
}

pub async fn atender_solicitud(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<MessageResponse>, AppError> {
    InventarioMensualService::atender_solicitud(&state.pool, claims.id_usuario, id).await?;
    Ok(Json(MessageResponse::new("Solicitud marcada como atendida")))
}
