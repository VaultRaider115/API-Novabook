//! Inventario mensual del piso de venta y solicitudes de editorial
//! (diagrama de procesos, paso 7).

use serde::{Deserialize, Serialize};

/// Un libro está "añejado" si no se ha vendido en estos días y su última
/// compra fue hace más de estos días (o nunca se compró).
pub const DIAS_ANEJAMIENTO: i32 = 90;

pub const MOTIVO_ANEJAMIENTO: &str = "ANEJAMIENTO";
pub const MOTIVO_SOLICITUD: &str = "SOLICITUD_EDITORIAL";

#[derive(Debug, Clone, Serialize)]
pub struct InventarioResumen {
    pub id_inventario: i64,
    pub contado_por: String,
    pub fecha_inicio: String,
    pub fecha_cierre: Option<String>,
    /// ABIERTO | CERRADO | APROBADO | RECHAZADO
    pub estado: String,
    pub dias_anejamiento: i32,
    pub revisado_por: Option<String>,
    pub fecha_revision: Option<String>,
    pub observaciones: Option<String>,
    pub total_productos: i64,
    pub piezas_contadas: i64,
    pub con_diferencia: i64,
    pub a_devolucion: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetalleInventarioItem {
    pub codigo_ean: i64,
    pub titulo: String,
    pub tipo_producto: String,
    pub proveedor: String,
    pub cantidad_contada: i32,
    /// Stock de piso al cerrar (None mientras está ABIERTO).
    pub stock_sistema: Option<i32>,
    /// contada - sistema (None mientras está ABIERTO).
    pub diferencia: Option<i32>,
    pub stock_apartado: i32,
    /// ANEJAMIENTO | SOLICITUD_EDITORIAL | None
    pub motivo_devolucion: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventarioDetalle {
    pub resumen: InventarioResumen,
    pub items: Vec<DetalleInventarioItem>,
}

/// Fija lo contado de un producto (el cliente suma los escaneos).
#[derive(Debug, Clone, Deserialize)]
pub struct ConteoRequest {
    pub codigo_ean: i64,
    pub cantidad: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RevisionInventarioRequest {
    pub aprobar: bool,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrearSolicitudEditorialRequest {
    pub id_proveedor: i32,
    /// Referencia del correo (asunto, fecha…).
    pub referencia: String,
    pub eans: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SolicitudEditorialItem {
    pub codigo_ean: i64,
    pub titulo: String,
    /// Piezas que hay hoy en la tienda (piso, bodega y apartados): lo que habría que regresar.
    pub piezas: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SolicitudEditorial {
    pub id_solicitud: i64,
    pub id_proveedor: i32,
    pub proveedor: String,
    pub fecha: String,
    pub referencia: String,
    /// ABIERTA | ATENDIDA
    pub estado: String,
    pub registrada_por: String,
    pub items: Vec<SolicitudEditorialItem>,
}
