use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};

use crate::{
    db::AppState,
    error::{ApiResponse, AppError},
    productos::{
        model::{
            FiltroRequisiciones, RecepcionRequest, RequisicionDetalle, RequisicionRequest,
            RequisicionResponse, RequisicionResumen, TipoMovimiento, TrasladoRequest, TrasladoResponse,
        },
        service::MovimientoService,
    },
    usuarios::model::Claims,
};

/// Bodega: requisición de salida Bodega → Piso de venta (con folio).
pub async fn requisicion_salida_bodega(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<RequisicionRequest>,
) -> Result<Json<ApiResponse<RequisicionResponse>>, AppError> {
    let res = MovimientoService::registrar_requisicion(
        &state.pool, claims.id_usuario, TipoMovimiento::BodegaATienda, payload,
    )
    .await?;
    Ok(Json(ApiResponse::success_msg(res, "Requisición de salida registrada")))
}

/// Vendedor: requisición de entrada Piso de venta → Bodega (con folio).
pub async fn requisicion_entrada_vendedor(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<RequisicionRequest>,
) -> Result<Json<ApiResponse<RequisicionResponse>>, AppError> {
    let res = MovimientoService::registrar_requisicion(
        &state.pool, claims.id_usuario, TipoMovimiento::TiendaABodega, payload,
    )
    .await?;
    Ok(Json(ApiResponse::success_msg(res, "Requisición de entrada registrada")))
}

pub async fn movimiento_bodega_tienda_libros(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<TrasladoRequest>,
) -> Result<Json<ApiResponse<TrasladoResponse>>, AppError> {
    let res =
        MovimientoService::traslado_bodega_a_tienda_libros(&state.pool, claims.id_usuario, payload)
            .await?;
    Ok(Json(ApiResponse::success(res)))
}

pub async fn movimiento_bodega_tienda_revistas(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<TrasladoRequest>,
) -> Result<Json<ApiResponse<TrasladoResponse>>, AppError> {
    let res =
        MovimientoService::traslado_bodega_a_tienda_revistas(&state.pool, claims.id_usuario, payload)
            .await?;
    Ok(Json(ApiResponse::success(res)))
}

pub async fn movimiento_tienda_bodega_libros(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<TrasladoRequest>,
) -> Result<Json<ApiResponse<TrasladoResponse>>, AppError> {
    let res =
        MovimientoService::traslado_tienda_a_bodega_libros(&state.pool, claims.id_usuario, payload)
            .await?;
    Ok(Json(ApiResponse::success(res)))
}

pub async fn movimiento_tienda_bodega_revistas(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<TrasladoRequest>,
) -> Result<Json<ApiResponse<TrasladoResponse>>, AppError> {
    let res =
        MovimientoService::traslado_tienda_a_bodega_revistas(&state.pool, claims.id_usuario, payload)
            .await?;
    Ok(Json(ApiResponse::success(res)))
}

/// Bodega y Vendedor: requisiciones con su estado (filtros `tipo` y `estado`).
pub async fn listar_requisiciones(
    State(state): State<AppState>,
    Query(filtro): Query<FiltroRequisiciones>,
) -> Result<Json<ApiResponse<Vec<RequisicionResumen>>>, AppError> {
    let res = MovimientoService::listar_requisiciones(&state.pool, filtro).await?;
    Ok(Json(ApiResponse::success(res)))
}

/// Bodega y Vendedor: una requisición con sus partidas (para revisarla o reimprimirla).
pub async fn obtener_requisicion(
    State(state): State<AppState>,
    Path(folio): Path<String>,
) -> Result<Json<ApiResponse<RequisicionDetalle>>, AppError> {
    let res = MovimientoService::obtener_requisicion(&state.pool, &folio).await?;
    Ok(Json(ApiResponse::success(res)))
}

/// Vendedor: recibe en piso una requisición de salida (confirma o corrige y
/// elige el mueble de cada libro).
pub async fn recibir_requisicion_vendedor(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(folio): Path<String>,
    Json(payload): Json<RecepcionRequest>,
) -> Result<Json<ApiResponse<RequisicionDetalle>>, AppError> {
    let res = MovimientoService::recibir_requisicion(&state.pool, claims.id_usuario, &folio, payload).await?;
    let msg = if res.resumen.estado == "CORREGIDA" {
        "Requisición corregida con lo recibido; reimprímela"
    } else {
        "Requisición recibida sin diferencias"
    };
    Ok(Json(ApiResponse::success_msg(res, msg)))
}
