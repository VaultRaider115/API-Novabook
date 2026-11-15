use super::producto::TipoProducto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ItemCompraRequest {
    pub codigo_ean: i64,
    #[serde(default = "default_tipo_libro")]
    pub tipo_producto: TipoProducto,
    pub cantidad: i32,
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
    pub mensaje: String,
}
