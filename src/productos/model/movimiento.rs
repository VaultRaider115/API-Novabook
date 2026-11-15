use super::producto::TipoProducto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoMovimiento {
    BodegaATienda,
    TiendaABodega,
}

impl TipoMovimiento {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BodegaATienda => "BODEGA_A_TIENDA",
            Self::TiendaABodega => "TIENDA_A_BODEGA",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movimiento {
    pub id_movimiento: i64,
    pub tipo_movimiento: TipoMovimiento,
    pub tipo_producto: TipoProducto,
    pub codigo_ean: i64,
    pub cantidad: i32,
    pub origen_ubicacion: i32,
    pub destino_ubicacion: i32,
    pub id_usuario: i64,
    pub fecha_hora: String,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrasladoRequest {
    pub codigo_ean: i64,
    pub cantidad: i32,
    pub observaciones: Option<String>,
}

/// Una partida de una requisición (documento con varios productos).
#[derive(Debug, Clone, Deserialize)]
pub struct PartidaRequisicion {
    pub codigo_ean: i64,
    pub cantidad: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequisicionRequest {
    pub items: Vec<PartidaRequisicion>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartidaRequisicionResp {
    pub codigo_ean: i64,
    pub tipo_producto: String,
    pub titulo: String,
    pub cantidad: i32,
}

/// Requisición registrada: todas sus partidas comparten folio (REQ-AAAA-n).
#[derive(Debug, Clone, Serialize)]
pub struct RequisicionResponse {
    pub folio: String,
    pub fecha_hora: String,
    pub tipo_movimiento: TipoMovimiento,
    pub total_piezas: i32,
    /// Editorial (proveedor) del documento: una sola por requisición.
    pub editorial: String,
    pub items: Vec<PartidaRequisicionResp>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrasladoResponse {
    pub codigo_ean: i64,
    pub tipo_producto: TipoProducto,
    pub tipo_movimiento: TipoMovimiento,
    pub cantidad_trasladada: i32,
    pub stock_origen_restante: i32,
    pub stock_destino_nuevo: i32,
    pub mensaje: String,
}
