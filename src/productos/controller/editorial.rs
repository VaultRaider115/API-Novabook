use axum::{
    Extension, Json,
    extract::{Path, State},
};

use crate::{
    db::AppState,
    error::{ApiResponse, AppError, MessageResponse},
    productos::{
        model::{CrearEditorialRequest, Editorial},
        service::EditorialService,
    },
    usuarios::model::Claims,
};

pub async fn listar_editoriales_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<ApiResponse<Vec<Editorial>>>, AppError> {
    Ok(Json(ApiResponse::success(EditorialService::listar(&state.pool).await?)))
}

pub async fn crear_editorial_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CrearEditorialRequest>,
) -> Result<Json<ApiResponse<Editorial>>, AppError> {
    let res = EditorialService::crear(&state.pool, claims.id_usuario, payload).await?;
    Ok(Json(ApiResponse::success_msg(res, "Editorial registrada exitosamente")))
}

pub async fn actualizar_editorial_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
    Json(payload): Json<CrearEditorialRequest>,
) -> Result<Json<ApiResponse<Editorial>>, AppError> {
    let res = EditorialService::actualizar(&state.pool, claims.id_usuario, id, payload).await?;
    Ok(Json(ApiResponse::success_msg(res, "Editorial actualizada exitosamente")))
}

pub async fn eliminar_editorial_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
) -> Result<Json<MessageResponse>, AppError> {
    EditorialService::eliminar(&state.pool, claims.id_usuario, id).await?;
    Ok(Json(MessageResponse::new(format!("Editorial con ID {} eliminada exitosamente", id))))
}
