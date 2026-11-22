use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    limites::{METODOS_PAGO, NOMBRE_CLIENTE_MAX, validar_largo},
    productos::{
        model::{FiltroVentas, RegistrarVentaRequest, TicketResumen, VentaResponse},
        repo::{VentaRepo, fecha_valida},
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

    /// Tickets de venta para el Jefe. Sin fechas, los de hoy; `hasta` por omisión
    /// es `desde`.
    pub async fn listar_tickets(pool: &DbPool, filtro: FiltroVentas) -> Result<Vec<TicketResumen>, AppError> {
        let hoy: String = sqlx::query_scalar("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')")
            .fetch_one(pool)
            .await
            .map_err(AppError::Database)?;
        let desde = filtro.desde.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or(hoy);
        let hasta = filtro.hasta.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| desde.clone());
        if !fecha_valida(&desde) || !fecha_valida(&hasta) {
            return Err(AppError::BadRequest("Las fechas deben tener formato YYYY-MM-DD".to_string()));
        }
        if hasta < desde {
            return Err(AppError::BadRequest("La fecha final no puede ser anterior a la inicial".to_string()));
        }
        // Lo más largo que se busca es un nombre de cliente (Qt usa el mismo tope).
        if let Some(q) = filtro.q.as_deref() {
            validar_largo("La búsqueda", q.trim(), NOMBRE_CLIENTE_MAX)?;
        }
        VentaRepo::listar_tickets(pool, &desde, &hasta, filtro.q.as_deref()).await
    }
}
