//! Devolución de cliente con ticket (diagrama de procesos, paso 5).

use serde::{Deserialize, Serialize};

/// Días máximos desde la compra para aceptar una devolución.
pub const DIAS_MAX_DEVOLUCION: i64 = 15;

/// Renglón de un ticket con lo que todavía se puede devolver.
#[derive(Debug, Clone, Serialize)]
pub struct TicketItem {
    pub codigo_ean: i64,
    pub titulo: String,
    pub tipo_producto: String,
    pub cantidad_vendida: i32,
    /// Ya devuelto en devoluciones anteriores del mismo ticket.
    pub cantidad_devuelta: i32,
    pub precio_unitario: f64,
}

/// Ticket (venta) consultado para devolverlo.
#[derive(Debug, Clone, Serialize)]
pub struct TicketResponse {
    pub id_venta: i64,
    pub fecha_hora: String,
    pub nombre_cliente: String,
    pub metodo_pago: String,
    pub total: f64,
    pub dias_transcurridos: i64,
    /// `dias_transcurridos <= DIAS_MAX_DEVOLUCION`
    pub vigente: bool,
    pub items: Vec<TicketItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartidaDevolucionCliente {
    pub codigo_ean: i64,
    pub cantidad: i32,
    /// true → queda en apartados (no se vende); false → regresa a piso.
    #[serde(default)]
    pub con_defecto: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartidaCambio {
    pub codigo_ean: i64,
    pub cantidad: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrearDevolucionClienteRequest {
    /// Número de ticket (id de la venta original).
    pub id_venta: i64,
    /// "REEMBOLSO" o "CAMBIO".
    pub resolucion: String,
    pub items: Vec<PartidaDevolucionCliente>,
    /// Productos que se lleva el cliente (solo en CAMBIO).
    #[serde(default)]
    pub items_cambio: Vec<PartidaCambio>,
    /// Con qué paga la diferencia del cambio (EFECTIVO por defecto).
    #[serde(default)]
    pub metodo_pago: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartidaDevueltaResp {
    pub codigo_ean: i64,
    pub titulo: String,
    pub cantidad: i32,
    pub precio_unitario: f64,
    pub con_defecto: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DevolucionClienteResponse {
    pub id_devolucion_cliente: i64,
    pub id_venta: i64,
    pub resolucion: String,
    pub fecha_hora: String,
    /// Lo que valían las piezas devueltas (precio pagado en el ticket).
    pub total_devuelto: f64,
    /// Total de los productos nuevos (solo CAMBIO).
    pub total_nuevo: f64,
    /// Lo que paga el cliente (CAMBIO) — 0 en REEMBOLSO.
    pub diferencia_cobrada: f64,
    /// Lo que se le regresa al cliente (REEMBOLSO) — 0 en CAMBIO.
    pub reembolso: f64,
    /// Ticket de la venta del cambio.
    pub id_venta_cambio: Option<i64>,
    pub items: Vec<PartidaDevueltaResp>,
}
