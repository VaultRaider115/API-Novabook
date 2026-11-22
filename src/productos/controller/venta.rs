use axum::{
    Extension, Json,
    extract::{Query, State},
};

use crate::{
    db::AppState,
    error::{ApiResponse, AppError},
    productos::{
        model::{FiltroVentas, RegistrarVentaRequest, TicketResumen, VentaResponse},
        service::VentaService,
    },
    usuarios::model::Claims,
};

pub async fn registrar_venta_vendedor(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<RegistrarVentaRequest>,
) -> Result<Json<ApiResponse<VentaResponse>>, AppError> {
    let res = VentaService::registrar_venta(&state.pool, claims.id_usuario, payload).await?;
    Ok(Json(ApiResponse::success_msg(
        res,
        "Venta procesada exitosamente",
    )))
}

/// Tickets de venta (Jefe, hito 2): `?desde&hasta&q`.
pub async fn listar_tickets_jefe(
    State(state): State<AppState>,
    Query(filtro): Query<FiltroVentas>,
) -> Result<Json<ApiResponse<Vec<TicketResumen>>>, AppError> {
    let res = VentaService::listar_tickets(&state.pool, filtro).await?;
    Ok(Json(ApiResponse::success(res)))
}
