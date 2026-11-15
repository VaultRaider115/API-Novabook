use super::producto::TipoProducto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ItemCompraRequest {
    pub codigo_ean: i64,
    #[serde(default = "default_tipo_libro")]
    pub tipo_producto: TipoProducto,
    /// Piezas que llegaron (entran a bodega). Puede ser 0 si la factura trae
    /// el título pero no llegó ninguna.
    pub cantidad: i32,
    /// Piezas que dice la factura. Si es mayor que `cantidad`, el paquete llegó
    /// incompleto y se abre una incidencia (diagrama, paso 1). Si no se envía,
    /// se toma igual a `cantidad`.
    #[serde(default)]
    pub cantidad_facturada: Option<i32>,
    /// Opcional: el esquema solo guarda el subtotal; si no se envía, el subtotal queda en 0.
    #[serde(default)]
    pub costo_unitario: f64,
    /// Opcional: mueble de bodega destino. Si no se envía se elige uno automáticamente.
    #[serde(default)]
    pub id_mueble: Option<i32>,
}

fn default_tipo_libro() -> TipoProducto {
    TipoProducto::Libro
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistrarCompraRequest {
    pub id_proveedor: i32,
    pub observaciones: Option<String>,
    /// Folio de factura del proveedor (compras_proveedor.factura)
    #[serde(default)]
    pub factura: Option<String>,
    pub items: Vec<ItemCompraRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCompraDto {
    pub id_detalle_compra: i64,
    pub codigo_ean: i64,
    pub tipo_producto: TipoProducto,
    pub nombre_producto: String,
    pub cantidad: i32,
    pub costo_unitario: f64,
    pub subtotal: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompraResponse {
    pub id_compra: i64,
    pub id_proveedor: i32,
    pub nombre_proveedor: String,
    pub id_usuarios: i64,
    pub comprador_nombre: Option<String>,
    pub fecha_hora: String,
    pub total: f64,
    pub total_articulos: i32,
    pub observaciones: Option<String>,
    pub factura: Option<String>,
    pub items: Vec<ItemCompraDto>,
    /// Incidencia abierta si el paquete llegó incompleto.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incidencia: Option<IncidenciaCompra>,
    pub mensaje: String,
}

// ----------------------------------------------------------------------------
// Incidencia por paquete incompleto (diagrama, paso 1)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaltanteIncidencia {
    pub codigo_ean: i64,
    pub titulo: String,
    pub cantidad_facturada: i32,
    pub cantidad_recibida: i32,
    pub cantidad_repuesta: i32,
    /// facturada − recibida − repuesta
    pub pendiente: i32,
}

/// Borrador del correo al proveedor; el cliente lo abre en el programa de
/// correo del equipo (mailto:) para que Bodega lo envíe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorreoBorrador {
    pub para: Option<String>,
    pub asunto: String,
    pub cuerpo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidenciaCompra {
    pub id_incidencia: i64,
    pub id_compra: i64,
    pub factura: Option<String>,
    /// ABIERTA | CERRADA
    pub estado: String,
    pub fecha: String,
    pub fecha_cierre: Option<String>,
    pub id_proveedor: i32,
    pub proveedor: String,
    pub recibio: String,
    pub piezas_pendientes: i32,
    pub items: Vec<FaltanteIncidencia>,
    pub correo: CorreoBorrador,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltroIncidencias {
    pub estado: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartidaReposicion {
    pub codigo_ean: i64,
    pub cantidad: i32,
    /// Opcional: mueble de bodega; si no se envía se elige uno.
    #[serde(default)]
    pub id_mueble: Option<i32>,
}

/// Llegan los artículos que faltaban de una compra.
#[derive(Debug, Clone, Deserialize)]
pub struct ReposicionRequest {
    pub items: Vec<PartidaReposicion>,
}
