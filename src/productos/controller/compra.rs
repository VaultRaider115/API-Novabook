use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};

use crate::{
    db::AppState,
    error::{ApiResponse, AppError},
    productos::{
        model::{CompraResponse, FiltroIncidencias, IncidenciaCompra, RegistrarCompraRequest, ReposicionRequest},
        service::CompraService,
    },
    usuarios::model::Claims,
};

pub async fn registrar_compra_bodega(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<RegistrarCompraRequest>,
) -> Result<Json<ApiResponse<CompraResponse>>, AppError> {
    let res = CompraService::registrar_compra_proveedor(&state.pool, claims.id_usuario, payload)
        .await?;
    let msg = if res.incidencia.is_some() {
        "Compra registrada con lo recibido; se abrió una incidencia por los faltantes"
    } else {
        "Compra a proveedor registrada e inventario actualizado"
    };
    Ok(Json(ApiResponse::success_msg(res, msg)))
}

pub async fn listar_compras_bodega(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<ApiResponse<Vec<CompraResponse>>>, AppError> {
    let res = CompraService::listar_compras_proveedor(&state.pool).await?;
    Ok(Json(ApiResponse::success(res)))
}

/// Bodega: incidencias por paquete incompleto (filtro opcional `estado`).
pub async fn listar_incidencias_bodega(
    State(state): State<AppState>,
    Query(filtro): Query<FiltroIncidencias>,
) -> Result<Json<ApiResponse<Vec<IncidenciaCompra>>>, AppError> {
    let res = CompraService::listar_incidencias(&state.pool, filtro).await?;
    Ok(Json(ApiResponse::success(res)))
}

pub async fn obtener_incidencia_bodega(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<IncidenciaCompra>>, AppError> {
    let res = CompraService::obtener_incidencia(&state.pool, id).await?;
    Ok(Json(ApiResponse::success(res)))
}

/// Bodega: llegaron los artículos restantes de una compra incompleta.
pub async fn reponer_incidencia_bodega(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(payload): Json<ReposicionRequest>,
) -> Result<Json<ApiResponse<IncidenciaCompra>>, AppError> {
    let res = CompraService::reponer_incidencia(&state.pool, claims.id_usuario, id, payload).await?;
    let msg = if res.estado == "CERRADA" {
        "Faltantes recibidos; la incidencia quedó cerrada"
    } else {
        "Faltantes recibidos; la incidencia sigue abierta"
    };
    Ok(Json(ApiResponse::success_msg(res, msg)))
}
