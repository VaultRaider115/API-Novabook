use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    limites::{METODOS_PAGO, NOMBRE_CLIENTE_MAX, validar_largo},
    productos::{
        model::{RegistrarVentaRequest, VentaResponse},
        repo::VentaRepo,
    },
};

pub struct VentaService;

impl VentaService {
    pub async fn registrar_venta(
        pool: &DbPool,
        actor_id: i64,
        req: RegistrarVentaRequest,
    ) -> Result<VentaResponse, AppError> {
        if req.items.is_empty() {
            return Err(AppError::BadRequest(
                "El carrito de venta no puede estar vacío".to_string(),
            ));
        }

        let cliente_limpio = req.cliente.trim();
        if cliente_limpio.is_empty() {
            return Err(AppError::BadRequest(
                "El nombre del cliente es obligatorio para registrar la venta".to_string(),
            ));
        }
        validar_largo("El nombre del cliente", cliente_limpio, NOMBRE_CLIENTE_MAX)?;
        // "TRANSFERENCIA" se retiró: las ventas viejas la conservan en la base.
        let metodo = req.metodo_pago.trim().to_uppercase();
        if !METODOS_PAGO.contains(&metodo.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Método de pago inválido: {} (usa EFECTIVO o TARJETA)",
                req.metodo_pago.trim()
            )));
        }

        let venta = VentaRepo::registrar_venta_transaccional(pool, actor_id, &req).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "BAJA POR VENTA",
            &format!(
                // Estilo del diseño 7: "Venta completada Ticket #1084"
                "Venta completada Ticket #{} · {} · {} pza(s) · ${:.2}",
                venta.id_venta, cliente_limpio, venta.total_articulos, venta.total
            ),
        )
        .await;

        Ok(venta)
    }
}
