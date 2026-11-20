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

/// Un título que va a devolución mientras se cuenta: cuánto hay que regresar
/// y cuántas piezas de piso ya encontró el vendedor.
#[derive(Debug, Clone, Serialize)]
pub struct DevolucionPendienteItem {
    pub codigo_ean: i64,
    pub titulo: String,
    pub proveedor: String,
    /// ANEJAMIENTO | SOLICITUD_EDITORIAL
    pub motivo_devolucion: String,
    pub en_piso: i32,
    pub en_bodega: i32,
    pub apartadas: i32,
    /// Total a devolver del título: piso + bodega + apartadas.
    pub a_devolver: i32,
    /// Piezas de piso que ya escaneó el vendedor (lo contado).
    pub encontradas: i32,
    /// Piezas de piso que faltan por encontrar: max(0, en_piso - encontradas).
    pub faltantes: i32,
}

/// Resumen para el encabezado del inventario: títulos y piezas por devolver.
#[derive(Debug, Clone, Serialize)]
pub struct DevolucionPendiente {
    pub id_inventario: i64,
    pub titulos: i32,
    pub titulos_con_faltantes: i32,
    pub piezas_a_devolver: i32,
    pub en_piso: i32,
    /// Encontradas en piso (sin contar las que sobran sobre el stock de piso).
    pub encontradas: i32,
    pub faltantes: i32,
    pub items: Vec<DevolucionPendienteItem>,
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
