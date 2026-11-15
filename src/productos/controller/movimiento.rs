use axum::{
    Extension, Json,
    extract::State,
};

use crate::{
    db::AppState,
    error::{ApiResponse, AppError},
    productos::{
        model::{RequisicionRequest, RequisicionResponse, TipoMovimiento, TrasladoRequest, TrasladoResponse},
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
