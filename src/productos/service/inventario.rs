use crate::{
    db::DbPool,
    error::AppError,
    limites::validar_busqueda_producto,
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
            validar_busqueda_producto(q)?;
        }
        InventarioRepo::consultar_existencias(pool, &params).await
    }
}
