use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{CompraResponse, RegistrarCompraRequest, validar_patron_ean13},
        repo::CompraRepo,
    },
};

pub struct CompraService;

impl CompraService {
    pub async fn registrar_compra_proveedor(
        pool: &DbPool,
        actor_id: i64,
        req: RegistrarCompraRequest,
    ) -> Result<CompraResponse, AppError> {
        if req.items.is_empty() {
            return Err(AppError::BadRequest(
                "La compra debe incluir al menos un producto".to_string(),
            ));
        }

        if req.factura.as_deref().is_some_and(|f| f.chars().count() > 30) {
            return Err(AppError::BadRequest(
                "El número de factura no puede exceder 30 caracteres".to_string(),
            ));
        }

        // Validar cada artículo y su EAN
        for item in &req.items {
            let tipo_detectado =
                validar_patron_ean13(item.codigo_ean).map_err(AppError::BadRequest)?;

            if tipo_detectado != item.tipo_producto {
                return Err(AppError::BadRequest(format!(
                    "El producto EAN {} fue clasificado como {:?} pero su código EAN corresponde a {:?}",
                    item.codigo_ean, item.tipo_producto, tipo_detectado
                )));
            }
        }

        let compra = CompraRepo::registrar_compra_transaccional(pool, actor_id, &req).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "COMPRA A PROVEEDOR",
            // Estilo del diseño 7: "Recepción de Factura #PRH-582"
            &format!(
                "Recepción de {} · {} · {} pza(s) · ${:.2}",
                match req.factura.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
                    Some(f) => format!("Factura #{}", f),
                    None => format!("compra #{}", compra.id_compra),
                },
                compra.nombre_proveedor,
                compra.total_articulos,
                compra.total
            ),
        )
        .await;

        Ok(compra)
    }

    pub async fn listar_compras_proveedor(pool: &DbPool) -> Result<Vec<CompraResponse>, AppError> {
        CompraRepo::listar_compras(pool).await
    }
}
