use crate::{
    db::DbPool,
    error::AppError,
    limites::{BUSQUEDA_MAX, validar_largo},
    productos::{
        model::{ExistenciaInventario, FiltroExistencias},
        repo::InventarioRepo,
    },
};

pub struct InventarioService;

impl InventarioService {
    pub async fn consultar_existencias(
        pool: &DbPool,
        params: FiltroExistencias,
    ) -> Result<Vec<ExistenciaInventario>, AppError> {
        if let Some(q) = params.q.as_deref() {
            validar_largo("La búsqueda", q.trim(), BUSQUEDA_MAX)?;
        }
        InventarioRepo::consultar_existencias(pool, &params).await
    }
}
